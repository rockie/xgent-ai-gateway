use crate::config::GatewayConfig;
use crate::queue::RedisQueue;

/// Shared application state accessible by both gRPC and HTTP handlers.
pub struct AppState {
    pub queue: RedisQueue,
    pub config: GatewayConfig,
}

impl AppState {
    pub fn new(queue: RedisQueue, config: GatewayConfig) -> Self {
        Self { queue, config }
    }
}
