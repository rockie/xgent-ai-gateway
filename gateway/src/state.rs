use std::sync::{Arc, Mutex};

use crate::config::GatewayConfig;
use crate::metrics::Metrics;
use crate::metrics_history::MetricsHistory;
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
    /// Prometheus metrics registry and metric handles.
    pub metrics: Metrics,
    /// In-memory ring buffer of metrics snapshots for dashboard history.
    pub metrics_history: Arc<Mutex<MetricsHistory>>,
}

impl AppState {
    pub fn new(
        queue: RedisQueue,
        config: GatewayConfig,
        auth_conn: MultiplexedConnection,
        http_client: reqwest::Client,
        metrics: Metrics,
        metrics_history: Arc<Mutex<MetricsHistory>>,
    ) -> Self {
        Self {
            queue,
            config,
            auth_conn,
            http_client,
            metrics,
            metrics_history,
        }
    }
}
