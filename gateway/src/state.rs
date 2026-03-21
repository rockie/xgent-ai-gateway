use crate::config::GatewayConfig;
use crate::queue::RedisQueue;
use redis::aio::MultiplexedConnection;

/// Shared application state accessible by both gRPC and HTTP handlers.
pub struct AppState {
    pub queue: RedisQueue,
    pub config: GatewayConfig,
    /// Dedicated Redis connection for auth lookups (API keys, node tokens).
    pub auth_conn: MultiplexedConnection,
}

impl AppState {
    pub fn new(queue: RedisQueue, config: GatewayConfig, auth_conn: MultiplexedConnection) -> Self {
        Self {
            queue,
            config,
            auth_conn,
        }
    }
}
