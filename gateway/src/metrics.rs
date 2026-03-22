use prometheus::{CounterVec, GaugeVec, HistogramVec, Opts, Registry};

/// All gateway Prometheus metrics, registered with a dedicated Registry.
pub struct Metrics {
    pub registry: Registry,

    /// Total tasks submitted, labeled by service and protocol (grpc/http).
    pub tasks_submitted_total: CounterVec,
    /// Total tasks completed, labeled by service and status (completed/failed).
    pub tasks_completed_total: CounterVec,
    /// Total errors, labeled by service and error type.
    pub errors_total: CounterVec,
    /// Total callback delivery attempts, labeled by status (success/failure).
    pub callback_delivery_total: CounterVec,
    /// Current queue depth per service.
    pub queue_depth: GaugeVec,
    /// Current active (healthy) nodes per service.
    pub nodes_active: GaugeVec,
    /// Task duration from submission to completion, labeled by service and status.
    pub task_duration_seconds: HistogramVec,
    /// Node poll latency (time between poll request and task assignment), labeled by service.
    pub node_poll_latency_seconds: HistogramVec,
}

impl Metrics {
    pub fn new() -> Self {
        let registry = Registry::new();

        let duration_buckets =
            vec![0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0, 30.0, 60.0, 120.0, 300.0];

        let tasks_submitted_total = CounterVec::new(
            Opts::new(
                "gateway_tasks_submitted_total",
                "Total tasks submitted to the gateway",
            ),
            &["service", "protocol"],
        )
        .unwrap();

        let tasks_completed_total = CounterVec::new(
            Opts::new(
                "gateway_tasks_completed_total",
                "Total tasks completed by the gateway",
            ),
            &["service", "status"],
        )
        .unwrap();

        let errors_total = CounterVec::new(
            Opts::new("gateway_errors_total", "Total gateway errors"),
            &["service", "type"],
        )
        .unwrap();

        let callback_delivery_total = CounterVec::new(
            Opts::new(
                "gateway_callback_delivery_total",
                "Total callback delivery attempts",
            ),
            &["status"],
        )
        .unwrap();

        let queue_depth = GaugeVec::new(
            Opts::new("gateway_queue_depth", "Current queue depth per service"),
            &["service"],
        )
        .unwrap();

        let nodes_active = GaugeVec::new(
            Opts::new(
                "gateway_nodes_active",
                "Current active nodes per service",
            ),
            &["service"],
        )
        .unwrap();

        let task_duration_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "gateway_task_duration_seconds",
                "Task duration from submission to completion in seconds",
            )
            .buckets(duration_buckets.clone()),
            &["service", "status"],
        )
        .unwrap();

        let node_poll_latency_seconds = HistogramVec::new(
            prometheus::HistogramOpts::new(
                "gateway_node_poll_latency_seconds",
                "Node poll latency in seconds",
            )
            .buckets(duration_buckets),
            &["service"],
        )
        .unwrap();

        // Register all metrics with the registry
        registry
            .register(Box::new(tasks_submitted_total.clone()))
            .unwrap();
        registry
            .register(Box::new(tasks_completed_total.clone()))
            .unwrap();
        registry
            .register(Box::new(errors_total.clone()))
            .unwrap();
        registry
            .register(Box::new(callback_delivery_total.clone()))
            .unwrap();
        registry
            .register(Box::new(queue_depth.clone()))
            .unwrap();
        registry
            .register(Box::new(nodes_active.clone()))
            .unwrap();
        registry
            .register(Box::new(task_duration_seconds.clone()))
            .unwrap();
        registry
            .register(Box::new(node_poll_latency_seconds.clone()))
            .unwrap();

        Self {
            registry,
            tasks_submitted_total,
            tasks_completed_total,
            errors_total,
            callback_delivery_total,
            queue_depth,
            nodes_active,
            task_duration_seconds,
            node_poll_latency_seconds,
        }
    }
}

/// Refresh gauge metrics by querying Redis for current queue depths and active node counts.
///
/// Called periodically from a background task in main.rs. Iterates all registered services,
/// reads XLEN for queue depth and counts healthy nodes per service.
pub async fn refresh_gauges(state: &crate::state::AppState) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let mut conn = state.auth_conn.clone();

    // Get all registered services
    let services = crate::registry::service::list_services(&mut conn).await?;

    for svc in &services {
        // Queue depth: XLEN on the service's task stream
        let stream_key = format!("tasks:{}", svc.name);
        let xlen: i64 = redis::cmd("XLEN")
            .arg(&stream_key)
            .query_async(&mut conn)
            .await
            .unwrap_or(0);
        state.metrics.queue_depth
            .with_label_values(&[&svc.name])
            .set(xlen as f64);

        // Active nodes: count healthy nodes via SMEMBERS + health check
        let nodes_key = format!("nodes:{}", svc.name);
        let node_ids: Vec<String> = redis::AsyncCommands::smembers(&mut conn, &nodes_key)
            .await
            .unwrap_or_default();

        let mut active_count = 0i64;
        for node_id in &node_ids {
            let node_key = format!("node:{}:{}", svc.name, node_id);
            let fields: std::collections::HashMap<String, String> = redis::cmd("HGETALL")
                .arg(&node_key)
                .query_async(&mut conn)
                .await
                .unwrap_or_default();

            if !fields.is_empty() {
                let last_seen = fields.get("last_seen").cloned().unwrap_or_default();
                let is_disconnected = fields.get("disconnected").map(|v| v == "true").unwrap_or(false);
                let health = crate::registry::node_health::derive_health_state(
                    &last_seen, svc.node_stale_after_secs, is_disconnected
                );
                if matches!(health, crate::registry::node_health::NodeHealthState::Healthy) {
                    active_count += 1;
                }
            }
        }
        state.metrics.nodes_active
            .with_label_values(&[&svc.name])
            .set(active_count as f64);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metrics_new_registers_all_eight_families() {
        let metrics = Metrics::new();

        // Initialize all metrics with at least one label set so gather() returns them
        metrics.tasks_submitted_total.with_label_values(&["test-svc", "http"]).inc();
        metrics.tasks_completed_total.with_label_values(&["test-svc", "completed"]).inc();
        metrics.errors_total.with_label_values(&["test-svc", "timeout"]).inc();
        metrics.callback_delivery_total.with_label_values(&["success"]).inc();
        metrics.queue_depth.with_label_values(&["test-svc"]).set(1.0);
        metrics.nodes_active.with_label_values(&["test-svc"]).set(1.0);
        metrics.task_duration_seconds.with_label_values(&["test-svc", "completed"]).observe(1.0);
        metrics.node_poll_latency_seconds.with_label_values(&["test-svc"]).observe(0.1);

        let families = metrics.registry.gather();
        let names: Vec<&str> = families.iter().map(|f| f.name()).collect();

        assert!(
            names.contains(&"gateway_tasks_submitted_total"),
            "missing gateway_tasks_submitted_total"
        );
        assert!(
            names.contains(&"gateway_tasks_completed_total"),
            "missing gateway_tasks_completed_total"
        );
        assert!(
            names.contains(&"gateway_errors_total"),
            "missing gateway_errors_total"
        );
        assert!(
            names.contains(&"gateway_callback_delivery_total"),
            "missing gateway_callback_delivery_total"
        );
        assert!(
            names.contains(&"gateway_queue_depth"),
            "missing gateway_queue_depth"
        );
        assert!(
            names.contains(&"gateway_nodes_active"),
            "missing gateway_nodes_active"
        );
        assert!(
            names.contains(&"gateway_task_duration_seconds"),
            "missing gateway_task_duration_seconds"
        );
        assert!(
            names.contains(&"gateway_node_poll_latency_seconds"),
            "missing gateway_node_poll_latency_seconds"
        );

        // Exactly 8 metric families
        assert_eq!(families.len(), 8, "expected 8 metric families, got {}", families.len());
    }
}
