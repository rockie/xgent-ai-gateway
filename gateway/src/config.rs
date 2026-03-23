use std::collections::HashMap;

use serde::Deserialize;

#[derive(Debug, Deserialize, Clone)]
pub struct GatewayConfig {
    #[serde(default = "default_grpc")]
    pub grpc: GrpcConfig,
    #[serde(default = "default_http")]
    pub http: HttpConfig,
    #[serde(default)]
    pub redis: RedisConfig,
    #[serde(default)]
    pub queue: QueueConfig,
    #[serde(default)]
    pub admin: AdminConfig,
    #[serde(default)]
    pub service_defaults: ServiceDefaultsConfig,
    #[serde(default)]
    pub callback: CallbackConfig,
    #[serde(default)]
    pub logging: LoggingConfig,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GrpcConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_grpc_addr")]
    pub listen_addr: String,
    /// Optional TLS config for mTLS on gRPC. None = plaintext (dev mode).
    pub tls: Option<GrpcTlsConfig>,
    /// mTLS client certificate identity mapping.
    #[serde(default)]
    pub mtls_identity: MtlsIdentityConfig,
}

/// Maps SHA-256 certificate fingerprints (hex-encoded) to authorized service names.
/// When fingerprints is empty or absent, mTLS identity checking is disabled (all valid certs accepted).
#[derive(Debug, Deserialize, Clone, Default)]
pub struct MtlsIdentityConfig {
    /// Map of SHA-256 cert fingerprint (hex-encoded DER) to list of authorized service names.
    #[serde(default)]
    pub fingerprints: HashMap<String, Vec<String>>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_http_addr")]
    pub listen_addr: String,
    /// Optional TLS config for HTTPS. None = plaintext (dev mode).
    pub tls: Option<TlsConfig>,
}

/// TLS certificate and key paths for server identity.
#[derive(Debug, Deserialize, Clone)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

/// gRPC TLS config with server identity and client CA for mTLS.
#[derive(Debug, Deserialize, Clone)]
pub struct GrpcTlsConfig {
    #[serde(flatten)]
    pub server: TlsConfig,
    /// Path to client CA certificate for mTLS -- require client certs.
    pub client_ca_path: String,
}

/// Admin endpoint configuration.
#[derive(Debug, Deserialize, Clone)]
pub struct AdminConfig {
    /// Admin username. If not set, admin endpoints are unauthenticated (dev mode).
    pub username: Option<String>,
    /// Argon2 PHC-format password hash for the admin user.
    pub password_hash: Option<String>,
    /// Allowed CORS origin for the admin SPA (e.g., "https://admin.example.com").
    pub cors_origin: Option<String>,
    /// Session TTL in seconds. Default 86400 (24 hours).
    #[serde(default = "default_session_ttl_secs")]
    pub session_ttl_secs: u64,
    /// Whether to set Secure flag on session cookie. Default true.
    #[serde(default = "default_true")]
    pub cookie_secure: bool,
}

impl Default for AdminConfig {
    fn default() -> Self {
        Self {
            username: None,
            password_hash: None,
            cors_origin: None,
            session_ttl_secs: default_session_ttl_secs(),
            cookie_secure: true,
        }
    }
}

fn default_session_ttl_secs() -> u64 {
    86400
}

#[derive(Debug, Deserialize, Clone)]
pub struct ServiceDefaultsConfig {
    #[serde(default = "default_node_stale_after_secs")]
    pub node_stale_after_secs: u64,
    #[serde(default = "default_drain_timeout_secs")]
    pub drain_timeout_secs: u64,
    #[serde(default = "default_task_timeout_secs")]
    pub task_timeout_secs: u64,
    #[serde(default = "default_max_retries")]
    pub max_retries: u32,
}

impl Default for ServiceDefaultsConfig {
    fn default() -> Self {
        Self {
            node_stale_after_secs: default_node_stale_after_secs(),
            drain_timeout_secs: default_drain_timeout_secs(),
            task_timeout_secs: default_task_timeout_secs(),
            max_retries: default_max_retries(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct CallbackConfig {
    #[serde(default = "default_callback_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_callback_initial_delay_ms")]
    pub initial_delay_ms: u64,
    #[serde(default = "default_callback_timeout_secs")]
    pub timeout_secs: u64,
}

impl Default for CallbackConfig {
    fn default() -> Self {
        Self {
            max_retries: default_callback_max_retries(),
            initial_delay_ms: default_callback_initial_delay_ms(),
            timeout_secs: default_callback_timeout_secs(),
        }
    }
}

#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {
    #[serde(default = "default_log_format")]
    pub format: String,
    pub file: Option<String>,
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            format: default_log_format(),
            file: None,
        }
    }
}

fn default_log_format() -> String {
    "text".to_string()
}

fn default_callback_max_retries() -> u32 { 3 }
fn default_callback_initial_delay_ms() -> u64 { 1000 }
fn default_callback_timeout_secs() -> u64 { 10 }

fn default_node_stale_after_secs() -> u64 {
    60
}

fn default_drain_timeout_secs() -> u64 {
    300
}

fn default_task_timeout_secs() -> u64 {
    300
}

fn default_max_retries() -> u32 {
    3
}

#[derive(Debug, Deserialize, Clone)]
pub struct RedisConfig {
    #[serde(default = "default_redis_url")]
    pub url: String,
    #[serde(default = "default_result_ttl")]
    pub result_ttl_secs: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct QueueConfig {
    #[serde(default = "default_stream_maxlen")]
    pub stream_maxlen: usize,
    #[serde(default = "default_block_timeout")]
    pub block_timeout_ms: usize,
}

fn default_true() -> bool {
    true
}

fn default_grpc_addr() -> String {
    "0.0.0.0:50051".to_string()
}

fn default_http_addr() -> String {
    "0.0.0.0:8080".to_string()
}

fn default_redis_url() -> String {
    "redis://127.0.0.1:6379".to_string()
}

fn default_result_ttl() -> u64 {
    86400
}

fn default_stream_maxlen() -> usize {
    10000
}

fn default_block_timeout() -> usize {
    5000
}

fn default_grpc() -> GrpcConfig {
    GrpcConfig {
        enabled: true,
        listen_addr: default_grpc_addr(),
        tls: None,
        mtls_identity: MtlsIdentityConfig::default(),
    }
}

fn default_http() -> HttpConfig {
    HttpConfig {
        enabled: true,
        listen_addr: default_http_addr(),
        tls: None,
    }
}

impl Default for RedisConfig {
    fn default() -> Self {
        Self {
            url: default_redis_url(),
            result_ttl_secs: default_result_ttl(),
        }
    }
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            stream_maxlen: default_stream_maxlen(),
            block_timeout_ms: default_block_timeout(),
        }
    }
}

pub fn load_config(config_path: Option<&str>) -> Result<GatewayConfig, config::ConfigError> {
    let mut builder = config::Config::builder();

    // Defaults
    builder = builder
        .set_default("grpc.enabled", true)?
        .set_default("grpc.listen_addr", "0.0.0.0:50051")?
        .set_default("http.enabled", true)?
        .set_default("http.listen_addr", "0.0.0.0:8080")?
        .set_default("redis.url", "redis://127.0.0.1:6379")?
        .set_default("redis.result_ttl_secs", 86400_i64)?
        .set_default("queue.stream_maxlen", 10000_i64)?
        .set_default("queue.block_timeout_ms", 5000_i64)?
        .set_default("service_defaults.node_stale_after_secs", 60_i64)?
        .set_default("service_defaults.drain_timeout_secs", 300_i64)?
        .set_default("service_defaults.task_timeout_secs", 300_i64)?
        .set_default("service_defaults.max_retries", 3_i64)?
        .set_default("callback.max_retries", 3_i64)?
        .set_default("callback.initial_delay_ms", 1000_i64)?
        .set_default("callback.timeout_secs", 10_i64)?
        .set_default("admin.session_ttl_secs", 86400_i64)?
        .set_default("admin.cookie_secure", true)?
        .set_default("logging.format", "text")?;

    // TOML file override
    if let Some(path) = config_path {
        builder = builder.add_source(config::File::with_name(path).required(true));
    }

    // Environment variable overrides with GATEWAY__ prefix
    builder = builder.add_source(
        config::Environment::with_prefix("GATEWAY")
            .separator("__")
            .try_parsing(true),
    );

    builder.build()?.try_deserialize()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Serialize config tests that touch env vars to prevent races.
    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn default_config_values() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Clear any GATEWAY__* env vars that might interfere
        for (key, _) in std::env::vars() {
            if key.starts_with("GATEWAY__") {
                std::env::remove_var(&key);
            }
        }
        let cfg = load_config(None).unwrap();
        assert!(cfg.grpc.enabled);
        assert_eq!(cfg.grpc.listen_addr, "0.0.0.0:50051");
        assert!(cfg.http.enabled);
        assert_eq!(cfg.http.listen_addr, "0.0.0.0:8080");
        assert_eq!(cfg.redis.url, "redis://127.0.0.1:6379");
        assert_eq!(cfg.redis.result_ttl_secs, 86400);
        assert_eq!(cfg.queue.stream_maxlen, 10000);
        assert_eq!(cfg.queue.block_timeout_ms, 5000);
    }

    #[test]
    fn config_loads_from_toml() {
        use std::io::Write;
        let _guard = ENV_LOCK.lock().unwrap();
        // Clear any GATEWAY__* env vars that might interfere
        for (key, _) in std::env::vars() {
            if key.starts_with("GATEWAY__") {
                std::env::remove_var(&key);
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"
[grpc]
listen_addr = "127.0.0.1:9090"

[redis]
result_ttl_secs = 3600
"#
        )
        .unwrap();

        let cfg = load_config(Some(path.to_str().unwrap())).unwrap();
        assert_eq!(cfg.grpc.listen_addr, "127.0.0.1:9090");
        assert_eq!(cfg.redis.result_ttl_secs, 3600);
        // Defaults preserved for unset fields
        assert_eq!(cfg.http.listen_addr, "0.0.0.0:8080");
    }

    #[test]
    fn config_env_var_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        // Clear first, then set only the one we want
        for (key, _) in std::env::vars() {
            if key.starts_with("GATEWAY__") {
                std::env::remove_var(&key);
            }
        }
        std::env::set_var("GATEWAY__QUEUE__BLOCK_TIMEOUT_MS", "9999");
        let cfg = load_config(None).unwrap();
        assert_eq!(cfg.queue.block_timeout_ms, 9999);
        // Verify other defaults are untouched
        assert_eq!(cfg.grpc.listen_addr, "0.0.0.0:50051");
        std::env::remove_var("GATEWAY__QUEUE__BLOCK_TIMEOUT_MS");
    }

    #[test]
    fn logging_config_defaults() {
        let _guard = ENV_LOCK.lock().unwrap();
        for (key, _) in std::env::vars() {
            if key.starts_with("GATEWAY__") {
                std::env::remove_var(&key);
            }
        }
        let cfg = load_config(None).unwrap();
        assert_eq!(cfg.logging.format, "text");
        assert!(cfg.logging.file.is_none());
    }

    #[test]
    fn logging_config_toml_override() {
        use std::io::Write;
        let _guard = ENV_LOCK.lock().unwrap();
        for (key, _) in std::env::vars() {
            if key.starts_with("GATEWAY__") {
                std::env::remove_var(&key);
            }
        }
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("logging_test.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        writeln!(
            f,
            r#"
[logging]
format = "json"
file = "/var/log/gateway.log"
"#
        )
        .unwrap();

        let cfg = load_config(Some(path.to_str().unwrap())).unwrap();
        assert_eq!(cfg.logging.format, "json");
        assert_eq!(cfg.logging.file.as_deref(), Some("/var/log/gateway.log"));
    }
}
