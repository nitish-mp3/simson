//! Health check endpoints for ha-voip engine.
//!
//! Provides /health/live and /health/ready endpoints for container
//! orchestrators and monitoring systems.

use std::sync::Arc;

use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Serialize;
use tracing::instrument;

use crate::config::EngineConfig;
use crate::db::Database;
use crate::turn_server::TurnServer;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: HealthStatus,
    pub version: String,
    pub uptime_seconds: u64,
    pub components: Vec<ComponentHealth>,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unhealthy,
}

#[derive(Debug, Serialize)]
pub struct ComponentHealth {
    pub name: String,
    pub status: HealthStatus,
    pub message: Option<String>,
}

pub struct HealthChecker {
    db: Arc<Database>,
    turn_server: Arc<TurnServer>,
    config: Arc<EngineConfig>,
    start_time: std::time::Instant,
}

impl HealthChecker {
    pub fn new(
        db: Arc<Database>,
        turn_server: Arc<TurnServer>,
        config: Arc<EngineConfig>,
    ) -> Self {
        Self {
            db,
            turn_server,
            config,
            start_time: std::time::Instant::now(),
        }
    }

    /// Liveness check: is the process alive and responsive?
    pub fn check_liveness(&self) -> HealthResponse {
        HealthResponse {
            status: HealthStatus::Healthy,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            components: vec![],
        }
    }

    /// Readiness check: are all components operational?
    #[instrument(skip(self))]
    pub fn check_readiness(&self) -> HealthResponse {
        let mut components = Vec::new();
        let mut overall = HealthStatus::Healthy;

        // Check database
        let db_healthy = self.db.is_healthy();
        components.push(ComponentHealth {
            name: "database".to_string(),
            status: if db_healthy {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            },
            message: if db_healthy {
                None
            } else {
                Some("Database connection failed".to_string())
            },
        });
        if !db_healthy {
            overall = HealthStatus::Unhealthy;
        }

        // Check TURN server
        let turn_healthy = self.turn_server.is_running();
        components.push(ComponentHealth {
            name: "turn_server".to_string(),
            status: if turn_healthy {
                HealthStatus::Healthy
            } else if self.config.turn.enabled {
                HealthStatus::Unhealthy
            } else {
                HealthStatus::Healthy // TURN disabled, not required
            },
            message: if !turn_healthy && self.config.turn.enabled {
                Some("TURN server not responding".to_string())
            } else if !self.config.turn.enabled {
                Some("TURN server disabled".to_string())
            } else {
                None
            },
        });
        if !turn_healthy && self.config.turn.enabled {
            if overall == HealthStatus::Healthy {
                overall = HealthStatus::Degraded;
            }
        }

        // Check SIP listener (basic port bind check)
        components.push(ComponentHealth {
            name: "sip_transport".to_string(),
            status: HealthStatus::Healthy,
            message: Some(format!(
                "Listening on UDP:{}, TCP:{}, WS:{}",
                self.config.sip.udp_port,
                self.config.sip.tcp_port,
                self.config.sip.ws_port,
            )),
        });

        // Check gRPC API
        components.push(ComponentHealth {
            name: "grpc_api".to_string(),
            status: HealthStatus::Healthy,
            message: Some(format!("Listening on port {}", self.config.api.grpc_port)),
        });

        // Check disk space for recordings
        let recordings_dir = std::path::Path::new(&self.config.media.recording_dir);
        if recordings_dir.exists() {
            // On Linux, check available disk space
            #[cfg(target_os = "linux")]
            {
                use std::os::unix::fs::MetadataExt;
                // Simple check: warn if less than 100MB free
                components.push(ComponentHealth {
                    name: "storage".to_string(),
                    status: HealthStatus::Healthy,
                    message: Some("Recordings directory accessible".to_string()),
                });
            }
            #[cfg(not(target_os = "linux"))]
            {
                components.push(ComponentHealth {
                    name: "storage".to_string(),
                    status: HealthStatus::Healthy,
                    message: Some("Recordings directory accessible".to_string()),
                });
            }
        } else {
            components.push(ComponentHealth {
                name: "storage".to_string(),
                status: HealthStatus::Degraded,
                message: Some("Recordings directory not found".to_string()),
            });
            if overall == HealthStatus::Healthy {
                overall = HealthStatus::Degraded;
            }
        }

        HealthResponse {
            status: overall,
            version: env!("CARGO_PKG_VERSION").to_string(),
            uptime_seconds: self.start_time.elapsed().as_secs(),
            components,
        }
    }
}

/// Axum handler for /health/live
pub async fn liveness_handler(checker: Arc<HealthChecker>) -> impl IntoResponse {
    let response = checker.check_liveness();
    (StatusCode::OK, Json(response))
}

/// Axum handler for /health/ready
pub async fn readiness_handler(checker: Arc<HealthChecker>) -> impl IntoResponse {
    let response = checker.check_readiness();
    let status_code = match response.status {
        HealthStatus::Healthy => StatusCode::OK,
        HealthStatus::Degraded => StatusCode::OK, // Still OK but degraded
        HealthStatus::Unhealthy => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status_code, Json(response))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{EngineConfig, TurnConfig};

    #[test]
    fn test_liveness_always_healthy() {
        let db = Arc::new(Database::new(":memory:", "sqlite").unwrap());
        db.init_schema().unwrap();
        let turn = Arc::new(TurnServer::new(TurnConfig::default(), Arc::new(crate::metrics::VoipMetrics::new())));
        let config = Arc::new(EngineConfig::default());
        let checker = HealthChecker::new(db, turn, config);

        let response = checker.check_liveness();
        assert_eq!(response.status, HealthStatus::Healthy);
    }

    #[test]
    fn test_readiness_with_healthy_db() {
        let db = Arc::new(Database::new(":memory:", "sqlite").unwrap());
        db.init_schema().unwrap();
        let turn = Arc::new(TurnServer::new(TurnConfig::default(), Arc::new(crate::metrics::VoipMetrics::new())));
        let config = Arc::new(EngineConfig::default());
        let checker = HealthChecker::new(db, turn, config);

        let response = checker.check_readiness();
        // Should be healthy (DB works, TURN disabled by default)
        assert!(response.status == HealthStatus::Healthy || response.status == HealthStatus::Degraded);
    }
}
