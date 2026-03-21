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
}

#[derive(Debug, Deserialize, Clone)]
pub struct GrpcConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_grpc_addr")]
    pub listen_addr: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct HttpConfig {
    #[serde(default = "default_true")]
    pub enabled: bool,
    #[serde(default = "default_http_addr")]
    pub listen_addr: String,
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
    }
}

fn default_http() -> HttpConfig {
    HttpConfig {
        enabled: true,
        listen_addr: default_http_addr(),
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
        .set_default("queue.block_timeout_ms", 5000_i64)?;

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
