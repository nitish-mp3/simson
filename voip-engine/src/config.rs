use serde::Deserialize;
use std::path::PathBuf;

/// Root configuration loaded from TOML/YAML/JSON via the `config` crate.
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    #[serde(default)]
    pub sip: SipConfig,
    #[serde(default)]
    pub media: MediaConfig,
    #[serde(default)]
    pub turn: TurnConfig,
    #[serde(default)]
    pub tls: TlsConfig,
    #[serde(default)]
    pub database: DatabaseConfig,
    #[serde(default)]
    pub api: ApiConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
    #[serde(default)]
    pub recording: RecordingConfig,
}

/// Type alias used by `main.rs` and other subsystems.
pub type EngineConfig = AppConfig;

// ───────────────────────── SIP ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct SipConfig {
    /// UDP listen port (default 5060)
    #[serde(default = "default_sip_udp_port")]
    pub udp_port: u16,
    /// TCP listen port (default 5060)
    #[serde(default = "default_sip_tcp_port")]
    pub tcp_port: u16,
    /// TLS listen port (default 5061)
    #[serde(default = "default_sip_tls_port")]
    pub tls_port: u16,
    /// WebSocket listen port (default 8088)
    #[serde(default = "default_sip_ws_port")]
    pub ws_port: u16,
    /// WSS listen port (default 8089)
    #[serde(default = "default_sip_wss_port")]
    pub wss_port: u16,
    /// SIP domain / realm
    #[serde(default = "default_sip_domain")]
    pub domain: String,
    /// Enable UDP transport
    #[serde(default = "default_true")]
    pub enable_udp: bool,
    /// Enable TCP transport
    #[serde(default = "default_true")]
    pub enable_tcp: bool,
    /// Enable TLS transport
    #[serde(default)]
    pub enable_tls: bool,
    /// Enable WebSocket transport
    #[serde(default = "default_true")]
    pub enable_ws: bool,
    /// Enable WSS transport
    #[serde(default)]
    pub enable_wss: bool,
    /// Maximum SIP message size in bytes
    #[serde(default = "default_max_sip_message_size")]
    pub max_message_size: usize,
    /// Bind address
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    /// T1 timer in ms (RFC 3261 default 500)
    #[serde(default = "default_t1")]
    pub timer_t1_ms: u64,
    /// T2 timer in ms (RFC 3261 default 4000)
    #[serde(default = "default_t2")]
    pub timer_t2_ms: u64,
    /// T4 timer in ms (RFC 3261 default 5000)
    #[serde(default = "default_t4")]
    pub timer_t4_ms: u64,
}

impl Default for SipConfig {
    fn default() -> Self {
        Self {
            udp_port: 5060,
            tcp_port: 5060,
            tls_port: 5061,
            ws_port: 8088,
            wss_port: 8089,
            domain: "homeassistant.local".into(),
            enable_udp: true,
            enable_tcp: true,
            enable_tls: false,
            enable_ws: true,
            enable_wss: false,
            max_message_size: 65535,
            bind_addr: "0.0.0.0".into(),
            timer_t1_ms: 500,
            timer_t2_ms: 4000,
            timer_t4_ms: 5000,
        }
    }
}

// ───────────────────────── Media ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct MediaConfig {
    /// Start of RTP port range
    #[serde(default = "default_rtp_port_start")]
    pub rtp_port_start: u16,
    /// End of RTP port range
    #[serde(default = "default_rtp_port_end")]
    pub rtp_port_end: u16,
    /// Preferred codecs in priority order
    #[serde(default = "default_codecs")]
    pub codecs: Vec<String>,
    /// Jitter buffer minimum depth (ms)
    #[serde(default = "default_jitter_min")]
    pub jitter_buffer_min_ms: u32,
    /// Jitter buffer maximum depth (ms)
    #[serde(default = "default_jitter_max")]
    pub jitter_buffer_max_ms: u32,
    /// Enable SRTP
    #[serde(default = "default_true")]
    pub enable_srtp: bool,
    /// DTMF mode: rfc2833 | inband | info
    #[serde(default = "default_dtmf_mode")]
    pub dtmf_mode: String,
    /// Enable silence detection
    #[serde(default)]
    pub silence_detection: bool,
    /// Comfort noise level (-dBov)
    #[serde(default = "default_comfort_noise")]
    pub comfort_noise_level: i32,
    /// Recording directory path (used by health checks)
    #[serde(default = "default_recording_dir_str")]
    pub recording_dir: String,
}

impl Default for MediaConfig {
    fn default() -> Self {
        Self {
            rtp_port_start: 10000,
            rtp_port_end: 20000,
            codecs: vec!["opus".into(), "pcma".into(), "pcmu".into()],
            jitter_buffer_min_ms: 20,
            jitter_buffer_max_ms: 200,
            enable_srtp: true,
            dtmf_mode: "rfc2833".into(),
            silence_detection: false,
            comfort_noise_level: 30,
            recording_dir: "recordings".into(),
        }
    }
}

// ───────────────────────── TURN ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct TurnConfig {
    /// Enable embedded TURN/STUN server
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// STUN/TURN UDP port
    #[serde(default = "default_turn_port")]
    pub port: u16,
    /// STUN/TURN UDP port (alias of `port` for callers using `udp_port`).
    #[serde(default = "default_turn_port")]
    pub udp_port: u16,
    /// TURN TLS port
    #[serde(default = "default_turn_tls_port")]
    pub tls_port: u16,
    /// TCP fallback port (usually 443)
    #[serde(default = "default_turn_alt_port")]
    pub alt_port: u16,
    /// TURN realm
    #[serde(default = "default_sip_domain")]
    pub realm: String,
    /// Static credentials (for development)
    #[serde(default)]
    pub users: Vec<TurnUser>,
    /// Shared secret for ephemeral credentials
    #[serde(default)]
    pub shared_secret: Option<String>,
    /// Maximum allocations per IP
    #[serde(default = "default_max_allocs_per_ip")]
    pub max_allocations_per_ip: usize,
    /// Allocation lifetime (seconds)
    #[serde(default = "default_allocation_lifetime")]
    pub allocation_lifetime_sec: u64,
    /// Relay port range start
    #[serde(default = "default_relay_port_start")]
    pub relay_port_start: u16,
    /// Relay port range end
    #[serde(default = "default_relay_port_end")]
    pub relay_port_end: u16,
    /// Rate limit: max requests per second per IP
    #[serde(default = "default_turn_rate_limit")]
    pub rate_limit_per_sec: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TurnUser {
    pub username: String,
    pub password: String,
}

impl Default for TurnConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            port: 3478,
            udp_port: 3478,
            tls_port: 5349,
            alt_port: 443,
            realm: "homeassistant.local".into(),
            users: Vec::new(),
            shared_secret: None,
            max_allocations_per_ip: 10,
            allocation_lifetime_sec: 600,
            relay_port_start: 49152,
            relay_port_end: 65535,
            rate_limit_per_sec: 50,
        }
    }
}

// ───────────────────────── TLS ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct TlsConfig {
    /// Path to PEM certificate
    #[serde(default)]
    pub cert_path: Option<PathBuf>,
    /// Path to PEM private key
    #[serde(default)]
    pub key_path: Option<PathBuf>,
    /// Path to CA bundle for client cert verification
    #[serde(default)]
    pub ca_path: Option<PathBuf>,
    /// Require client certificates (mTLS)
    #[serde(default)]
    pub require_client_cert: bool,
}

impl Default for TlsConfig {
    fn default() -> Self {
        Self {
            cert_path: None,
            key_path: None,
            ca_path: None,
            require_client_cert: false,
        }
    }
}

// ───────────────────────── Database ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    /// "sqlite" or "postgres"
    #[serde(default = "default_db_backend")]
    pub backend: String,
    /// Alias for `backend` used by main.rs.
    #[serde(default = "default_db_backend")]
    pub db_type: String,
    /// SQLite file path (when backend = sqlite)
    #[serde(default = "default_sqlite_path")]
    pub sqlite_path: PathBuf,
    /// Alias for `sqlite_path` used by main.rs.
    #[serde(default = "default_sqlite_path_str")]
    pub path: String,
    /// PostgreSQL connection URL
    #[serde(default)]
    pub postgres_url: Option<String>,
    /// Max open connections
    #[serde(default = "default_db_max_conns")]
    pub max_connections: u32,
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            backend: "sqlite".into(),
            db_type: "sqlite".into(),
            sqlite_path: PathBuf::from("voip-engine.db"),
            path: "voip-engine.db".into(),
            postgres_url: None,
            max_connections: 10,
        }
    }
}

// ───────────────────────── API / gRPC ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct ApiConfig {
    /// gRPC listen port
    #[serde(default = "default_grpc_port")]
    pub grpc_port: u16,
    /// HTTP metrics/health port
    #[serde(default = "default_http_port")]
    pub http_port: u16,
    /// Bind address
    #[serde(default = "default_bind_addr")]
    pub bind_addr: String,
    /// API keys (bearer tokens) that are allowed
    #[serde(default)]
    pub api_keys: Vec<String>,
    /// Enable mTLS for gRPC
    #[serde(default)]
    pub enable_mtls: bool,
    /// Request rate limit per key (requests/sec)
    #[serde(default = "default_api_rate_limit")]
    pub rate_limit_per_sec: u32,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            grpc_port: 50051,
            http_port: 8080,
            bind_addr: "0.0.0.0".into(),
            api_keys: Vec::new(),
            enable_mtls: false,
            rate_limit_per_sec: 100,
        }
    }
}

// ───────────────────────── Logging ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    /// Minimum log level: trace, debug, info, warn, error
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Output format: "text" or "json"
    #[serde(default = "default_log_format")]
    pub format: String,
    /// Log file path (stdout if None)
    #[serde(default)]
    pub file: Option<PathBuf>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info".into(),
            format: "text".into(),
            file: None,
        }
    }
}

// ───────────────────────── Recording ─────────────────────────

#[derive(Debug, Clone, Deserialize)]
pub struct RecordingConfig {
    /// Enable call recording globally
    #[serde(default)]
    pub enabled: bool,
    /// Directory to store recordings
    #[serde(default = "default_recording_dir")]
    pub directory: PathBuf,
    /// Format: "wav" or "opus"
    #[serde(default = "default_recording_format")]
    pub format: String,
    /// Encrypt recordings at rest
    #[serde(default)]
    pub encrypt: bool,
    /// AES encryption key (base64), must be 32 bytes decoded
    #[serde(default)]
    pub encryption_key: Option<String>,
    /// Maximum disk usage in MB (0 = unlimited)
    #[serde(default)]
    pub max_disk_mb: u64,
    /// Retention days (0 = unlimited)
    #[serde(default)]
    pub retention_days: u32,
}

impl Default for RecordingConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            directory: PathBuf::from("recordings"),
            format: "opus".into(),
            encrypt: false,
            encryption_key: None,
            max_disk_mb: 0,
            retention_days: 0,
        }
    }
}

// ───────────────────────── Helpers ─────────────────────────

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            sip: SipConfig::default(),
            media: MediaConfig::default(),
            turn: TurnConfig::default(),
            tls: TlsConfig::default(),
            database: DatabaseConfig::default(),
            api: ApiConfig::default(),
            logging: LoggingConfig::default(),
            recording: RecordingConfig::default(),
        }
    }
}

impl AppConfig {
    /// Load config from a file path, layering environment variables on top.
    pub fn load(path: &str) -> anyhow::Result<Self> {
        let settings = config::Config::builder()
            .add_source(config::File::with_name(path).required(false))
            .add_source(
                config::Environment::with_prefix("VOIP")
                    .separator("__")
                    .try_parsing(true),
            )
            .build()?;

        let cfg: AppConfig = settings.try_deserialize()?;
        cfg.validate()?;
        Ok(cfg)
    }

    /// Validate configuration values.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.media.rtp_port_start >= self.media.rtp_port_end {
            anyhow::bail!("rtp_port_start must be less than rtp_port_end");
        }
        if self.media.jitter_buffer_min_ms >= self.media.jitter_buffer_max_ms {
            anyhow::bail!("jitter_buffer_min_ms must be less than jitter_buffer_max_ms");
        }
        if self.sip.enable_tls && self.tls.cert_path.is_none() {
            anyhow::bail!("TLS enabled for SIP but no cert_path specified");
        }
        if self.sip.enable_wss && self.tls.cert_path.is_none() {
            anyhow::bail!("WSS enabled but no cert_path specified");
        }
        if self.recording.encrypt && self.recording.encryption_key.is_none() {
            anyhow::bail!("Recording encryption enabled but no encryption_key specified");
        }
        Ok(())
    }
}

// default helpers
fn default_sip_udp_port() -> u16 { 5060 }
fn default_sip_tcp_port() -> u16 { 5060 }
fn default_sip_tls_port() -> u16 { 5061 }
fn default_sip_ws_port() -> u16 { 8088 }
fn default_sip_wss_port() -> u16 { 8089 }
fn default_sip_domain() -> String { "homeassistant.local".into() }
fn default_true() -> bool { true }
fn default_max_sip_message_size() -> usize { 65535 }
fn default_bind_addr() -> String { "0.0.0.0".into() }
fn default_t1() -> u64 { 500 }
fn default_t2() -> u64 { 4000 }
fn default_t4() -> u64 { 5000 }
fn default_rtp_port_start() -> u16 { 10000 }
fn default_rtp_port_end() -> u16 { 20000 }
fn default_codecs() -> Vec<String> { vec!["opus".into(), "pcma".into(), "pcmu".into()] }
fn default_jitter_min() -> u32 { 20 }
fn default_jitter_max() -> u32 { 200 }
fn default_dtmf_mode() -> String { "rfc2833".into() }
fn default_comfort_noise() -> i32 { 30 }
fn default_turn_port() -> u16 { 3478 }
fn default_turn_tls_port() -> u16 { 5349 }
fn default_turn_alt_port() -> u16 { 443 }
fn default_max_allocs_per_ip() -> usize { 10 }
fn default_allocation_lifetime() -> u64 { 600 }
fn default_relay_port_start() -> u16 { 49152 }
fn default_relay_port_end() -> u16 { 65535 }
fn default_turn_rate_limit() -> u32 { 50 }
fn default_db_backend() -> String { "sqlite".into() }
fn default_sqlite_path() -> PathBuf { PathBuf::from("voip-engine.db") }
fn default_db_max_conns() -> u32 { 10 }
fn default_grpc_port() -> u16 { 50051 }
fn default_http_port() -> u16 { 8080 }
fn default_api_rate_limit() -> u32 { 100 }
fn default_log_level() -> String { "info".into() }
fn default_log_format() -> String { "text".into() }
fn default_recording_dir() -> PathBuf { PathBuf::from("recordings") }
fn default_recording_format() -> String { "opus".into() }
fn default_sqlite_path_str() -> String { "voip-engine.db".into() }
fn default_recording_dir_str() -> String { "recordings".into() }
