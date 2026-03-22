use crate::config::GatewayConfig;
use crate::queue::RedisQueue;
use redis::aio::MultiplexedConnection;

/// Shared application state accessible by both gRPC and HTTP handlers.
pub struct AppState {
    pub queue: RedisQueue,
    pub config: GatewayConfig,
    /// Dedicated Redis connection for auth lookups (API keys, node tokens).
    pub auth_conn: MultiplexedConnection,
    /// HTTP client for callback delivery.
    pub http_client: reqwest::Client,
}

impl AppState {
    pub fn new(queue: RedisQueue, config: GatewayConfig, auth_conn: MultiplexedConnection, http_client: reqwest::Client) -> Self {
        Self {
            queue,
            config,
            auth_conn,
            http_client,
        }
    }
}
