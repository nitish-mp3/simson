//! ha-voip VoIP Engine - High-performance SIP/WebRTC media engine
//!
//! Entry point for the voip-engine binary. Initializes all subsystems:
//! SIP transport, TURN server, gRPC control API, HTTP metrics/health,
//! and the database layer.

mod api;
mod config;
mod db;
mod health;
mod media;
mod metrics;
mod recording;
mod sip;
mod turn_server;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::{Context, Result};
use clap::Parser;
use tokio::signal;
use tokio::sync::broadcast;
use tracing::{error, info, warn};

use crate::config::EngineConfig;
use crate::db::Database;
use crate::health::HealthChecker;
use crate::metrics::VoipMetrics;
use crate::recording::RecordingManager;
use crate::sip::dialog::DialogManager;
use crate::sip::transport::TransportManager;
use crate::turn_server::{ConfigCredentialProvider, TurnServer};

/// ha-voip VoIP Engine
#[derive(Parser, Debug)]
#[command(name = "voip-engine", version, about = "High-performance VoIP engine for Home Assistant")]
struct Cli {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    /// SIP UDP port override
    #[arg(long)]
    sip_port: Option<u16>,

    /// gRPC API port override
    #[arg(long)]
    grpc_port: Option<u16>,

    /// TURN server UDP port override
    #[arg(long)]
    turn_port: Option<u16>,

    /// HTTP metrics/health port override
    #[arg(long)]
    http_port: Option<u16>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(short, long, default_value = "info")]
    log_level: String,

    /// Log format (json, pretty)
    #[arg(long, default_value = "json")]
    log_format: String,

    /// Data directory for recordings and database
    #[arg(long, default_value = "/data/voip")]
    data_dir: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Initialize tracing/logging
    init_logging(&cli.log_level, &cli.log_format)?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting ha-voip engine"
    );

    // Load configuration
    let mut engine_config = EngineConfig::load(&cli.config)
        .unwrap_or_else(|e| {
            warn!(?e, "Failed to load config file, using defaults");
            EngineConfig::default()
        });

    // Apply CLI overrides
    if let Some(port) = cli.sip_port {
        engine_config.sip.udp_port = port;
    }
    if let Some(port) = cli.grpc_port {
        engine_config.api.grpc_port = port;
    }
    if let Some(port) = cli.turn_port {
        engine_config.turn.udp_port = port;
    }
    if let Some(port) = cli.http_port {
        engine_config.api.http_port = port;
    }

    // Ensure data directory exists
    let data_dir = std::path::PathBuf::from(&cli.data_dir);
    tokio::fs::create_dir_all(&data_dir)
        .await
        .context("Failed to create data directory")?;
    tokio::fs::create_dir_all(data_dir.join("recordings"))
        .await
        .context("Failed to create recordings directory")?;

    // Initialize shared state
    let config = Arc::new(engine_config.clone());
    let voip_metrics = Arc::new(VoipMetrics::new());

    // Initialize database
    let db_path = match engine_config.database.db_type.as_str() {
        "sqlite" => {
            let path = data_dir.join("voip.db");
            path.to_string_lossy().to_string()
        }
        _ => engine_config.database.path.clone(),
    };
    let database = Arc::new(
        Database::new(&db_path, &engine_config.database.db_type)
            .context("Failed to initialize database")?,
    );
    database
        .init_schema()
        .context("Failed to initialize database schema")?;
    info!("Database initialized at {}", db_path);

    // Initialize subsystems (used by SIP processing pipeline)
    let _dialog_manager = Arc::new(DialogManager::new(engine_config.sip.domain.clone()));
    let _recording_manager = Arc::new(RecordingManager::new(engine_config.recording.clone()));

    // Start SIP transports (start() requires &mut self, so call before Arc wrapping)
    let (mut transport_manager, _transport_shutdown_rx) = TransportManager::new();
    {
        let bind: std::net::IpAddr = engine_config
            .sip
            .bind_addr
            .parse()
            .unwrap_or(std::net::IpAddr::V4(std::net::Ipv4Addr::UNSPECIFIED));
        let udp_addr = if engine_config.sip.enable_udp {
            Some(SocketAddr::new(bind, engine_config.sip.udp_port))
        } else {
            None
        };
        let tcp_addr = if engine_config.sip.enable_tcp {
            Some(SocketAddr::new(bind, engine_config.sip.tcp_port))
        } else {
            None
        };
        let ws_addr = if engine_config.sip.enable_ws {
            Some(SocketAddr::new(bind, engine_config.sip.ws_port))
        } else {
            None
        };

        info!(
            udp_port = engine_config.sip.udp_port,
            tcp_port = engine_config.sip.tcp_port,
            ws_port = engine_config.sip.ws_port,
            "Starting SIP transport listeners"
        );
        transport_manager
            .start(udp_addr, tcp_addr, ws_addr, engine_config.sip.max_message_size)
            .await
            .context("Failed to start SIP transports")?;
        info!("SIP transports started");
    }
    let transport_manager = Arc::new(transport_manager);

    // Shutdown signal channel
    let (shutdown_tx, _) = broadcast::channel::<()>(1);

    // Start TURN server
    let credentials: Arc<dyn turn_server::CredentialProvider> =
        Arc::new(ConfigCredentialProvider::from_config(&engine_config.turn));
    let turn_server = TurnServer::new(engine_config.turn.clone(), credentials);
    let turn_handle = {
        let ts = turn_server.clone();
        let bind_addr = engine_config.sip.bind_addr.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            info!(
                udp_port = ts.config.udp_port,
                tls_port = ts.config.tls_port,
                "Starting embedded TURN server"
            );
            let recv_handle = match ts.start(&bind_addr).await {
                Ok(h) => h,
                Err(e) => {
                    error!(?e, "Failed to start TURN server");
                    return;
                }
            };
            info!("TURN server started");
            let _ = shutdown_rx.recv().await;
            info!("Shutting down TURN server");
            ts.shutdown.notify_waiters();
            recv_handle.abort();
        })
    };

    // Start gRPC API server
    let grpc_handle = {
        let addr: SocketAddr = format!("0.0.0.0:{}", config.api.grpc_port)
            .parse()
            .context("Invalid gRPC address")?;
        let cfg = config.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!(%addr, "Starting gRPC API server");
            let state = api::grpc::ServiceState::new(
                cfg.api.api_keys.clone(),
                cfg.api.rate_limit_per_sec,
            );
            let service = api::grpc::VoipGrpcService::new(state);

            // When generated proto is available, use the tonic service wrapper.
            // Otherwise, just log that gRPC is in stub mode and wait for shutdown.
            #[cfg(feature = "_generated_proto")]
            {
                use crate::api::generated::VoipEngineServer;
                let server = tonic::transport::Server::builder()
                    .add_service(VoipEngineServer::new(service))
                    .serve_with_shutdown(addr, async {
                        let _ = shutdown_rx.recv().await;
                    });

                if let Err(e) = server.await {
                    error!(?e, "gRPC server error");
                }
            }

            #[cfg(not(feature = "_generated_proto"))]
            {
                let _ = service;
                warn!("gRPC server running in stub mode (proto not generated)");
                let _ = shutdown_rx.recv().await;
            }

            info!("gRPC server stopped");
        })
    };

    // Start HTTP metrics + health server
    let http_handle = {
        let addr: SocketAddr = format!("0.0.0.0:{}", config.api.http_port)
            .parse()
            .context("Invalid HTTP address")?;
        let m = voip_metrics.clone();
        let db = database.clone();
        let ts = turn_server.clone();
        let cfg = config.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();

        tokio::spawn(async move {
            info!(%addr, "Starting HTTP metrics/health server");

            let health_checker = Arc::new(HealthChecker::new(db, ts, cfg));

            let app = axum::Router::new()
                .route("/metrics", axum::routing::get({
                    let m = m.clone();
                    move || metrics::metrics_handler(m.clone())
                }))
                .route("/health/live", axum::routing::get({
                    let hc = health_checker.clone();
                    move || health::liveness_handler(hc.clone())
                }))
                .route("/health/ready", axum::routing::get({
                    let hc = health_checker.clone();
                    move || health::readiness_handler(hc.clone())
                }));

            let listener = match tokio::net::TcpListener::bind(addr).await {
                Ok(l) => l,
                Err(e) => {
                    error!(?e, "Failed to bind HTTP server");
                    return;
                }
            };

            axum::serve(listener, app)
                .with_graceful_shutdown(async {
                    let _ = shutdown_rx.recv().await;
                })
                .await
                .unwrap_or_else(|e| error!(?e, "HTTP server error"));

            info!("HTTP server stopped");
        })
    };

    // Start periodic tasks
    let cleanup_handle = {
        let ts = turn_server.clone();
        let m = voip_metrics.clone();
        let mut shutdown_rx = shutdown_tx.subscribe();
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(30));
            loop {
                tokio::select! {
                    _ = interval.tick() => {
                        ts.cleanup_expired_allocations();
                        m.update_system_metrics();
                    }
                    _ = shutdown_rx.recv() => break,
                }
            }
        })
    };

    info!(
        grpc_port = config.api.grpc_port,
        http_port = config.api.http_port,
        sip_udp = config.sip.udp_port,
        turn_udp = config.turn.udp_port,
        "ha-voip engine fully started"
    );

    // Wait for shutdown signal
    shutdown_signal().await;
    info!("Shutdown signal received, stopping services...");

    // Signal all tasks to stop
    let _ = shutdown_tx.send(());

    // Shut down SIP transports
    transport_manager.shutdown();

    // Wait for all tasks to complete with timeout
    let shutdown_timeout = tokio::time::Duration::from_secs(10);
    let _ = tokio::time::timeout(shutdown_timeout, async {
        let _ = tokio::join!(
            turn_handle,
            grpc_handle,
            http_handle,
            cleanup_handle,
        );
    })
    .await;

    info!("ha-voip engine stopped gracefully");
    Ok(())
}

fn init_logging(level: &str, format: &str) -> Result<()> {
    let env_filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(level));

    let subscriber = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(true)
        .with_thread_ids(true)
        .with_file(true)
        .with_line_number(true);

    match format {
        "json" => {
            subscriber.json().init();
        }
        _ => {
            subscriber.init();
        }
    }

    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install SIGTERM handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => info!("Received Ctrl+C"),
        _ = terminate => info!("Received SIGTERM"),
    }
}
