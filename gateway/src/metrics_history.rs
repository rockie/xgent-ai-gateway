use std::collections::{HashMap, VecDeque};

use serde::Serialize;

use crate::registry::node_health::{NodeHealthState, NodeStatus};

/// Maximum entries in the ring buffer (30 minutes at 10s intervals).
const MAX_ENTRIES: usize = 180;

/// A point-in-time snapshot of gateway metrics.
#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub timestamp: i64,
    pub tasks_submitted: f64,
    pub tasks_completed: f64,
    pub tasks_failed: f64,
    pub queue_depth: HashMap<String, f64>,
    pub nodes_active: HashMap<String, f64>,
}

/// Ring buffer of metrics snapshots for time-series history.
pub struct MetricsHistory {
    entries: VecDeque<MetricsSnapshot>,
}

impl Default for MetricsHistory {
    fn default() -> Self {
        Self::new()
    }
}

impl MetricsHistory {
    pub fn new() -> Self {
        Self {
            entries: VecDeque::with_capacity(MAX_ENTRIES),
        }
    }

    pub fn push_snapshot(&mut self, snapshot: MetricsSnapshot) {
        if self.entries.len() >= MAX_ENTRIES {
            self.entries.pop_front();
        }
        self.entries.push_back(snapshot);
    }

    pub fn get_all(&self) -> Vec<MetricsSnapshot> {
        self.entries.iter().cloned().collect()
    }

    /// Get a snapshot by offset from the back (0 = latest, 1 = one before latest, etc.)
    pub fn get_snapshot_at(&self, entries_ago: usize) -> Option<&MetricsSnapshot> {
        if entries_ago >= self.entries.len() {
            return None;
        }
        let idx = self.entries.len() - 1 - entries_ago;
        self.entries.get(idx)
    }

    /// Compute throughput as (submitted_per_min, completed_per_min) from counter deltas
    /// over the last ~60 seconds (6 snapshots at 10s interval). Returns (0.0, 0.0) if
    /// fewer than 7 snapshots exist.
    pub fn compute_throughput(&self) -> (f64, f64) {
        if self.entries.len() < 7 {
            return (0.0, 0.0);
        }
        let current = &self.entries[self.entries.len() - 1];
        let old = &self.entries[self.entries.len() - 7];
        let elapsed_secs = (current.timestamp - old.timestamp) as f64;
        if elapsed_secs <= 0.0 {
            return (0.0, 0.0);
        }
        let submitted_per_min =
            (current.tasks_submitted - old.tasks_submitted) / elapsed_secs * 60.0;
        let completed_per_min =
            (current.tasks_completed - old.tasks_completed) / elapsed_secs * 60.0;
        (submitted_per_min, completed_per_min)
    }
}

/// Capture a snapshot of current Prometheus metric values.
pub fn capture_snapshot(metrics: &crate::metrics::Metrics) -> MetricsSnapshot {
    let families = metrics.registry.gather();

    let mut tasks_submitted = 0.0_f64;
    let mut tasks_completed = 0.0_f64;
    let mut tasks_failed = 0.0_f64;
    let mut queue_depth = HashMap::new();
    let mut nodes_active = HashMap::new();

    for family in &families {
        match family.name() {
            "gateway_tasks_submitted_total" => {
                for m in family.get_metric() {
                    tasks_submitted += m.get_counter().value();
                }
            }
            "gateway_tasks_completed_total" => {
                for m in family.get_metric() {
                    let labels = m.get_label();
                    let status = labels
                        .iter()
                        .find(|l| l.name() == "status")
                        .map(|l| l.value())
                        .unwrap_or("");
                    let val = m.get_counter().value();
                    if status == "failed" {
                        tasks_failed += val;
                    } else {
                        tasks_completed += val;
                    }
                }
            }
            "gateway_queue_depth" => {
                for m in family.get_metric() {
                    let svc = m
                        .get_label()
                        .iter()
                        .find(|l| l.name() == "service")
                        .map(|l| l.value().to_string())
                        .unwrap_or_default();
                    if !svc.is_empty() {
                        queue_depth.insert(svc, m.get_gauge().value());
                    }
                }
            }
            "gateway_nodes_active" => {
                for m in family.get_metric() {
                    let svc = m
                        .get_label()
                        .iter()
                        .find(|l| l.name() == "service")
                        .map(|l| l.value().to_string())
                        .unwrap_or_default();
                    if !svc.is_empty() {
                        nodes_active.insert(svc, m.get_gauge().value());
                    }
                }
            }
            _ => {}
        }
    }

    MetricsSnapshot {
        timestamp: chrono::Utc::now().timestamp(),
        tasks_submitted,
        tasks_completed,
        tasks_failed,
        queue_depth,
        nodes_active,
    }
}

/// Derive service-level health from node statuses.
/// - No nodes -> "unknown"
/// - All Healthy (non-draining) -> "healthy"
/// - Some Healthy -> "degraded"
/// - Zero Healthy -> "down"
pub fn derive_service_health(nodes: &[NodeStatus]) -> &'static str {
    if nodes.is_empty() {
        return "unknown";
    }
    let healthy_count = nodes
        .iter()
        .filter(|n| n.health == NodeHealthState::Healthy && !n.draining)
        .count();
    if healthy_count == nodes.len() {
        "healthy"
    } else if healthy_count > 0 {
        "degraded"
    } else {
        "down"
    }
}

// --- Response types ---

#[derive(Debug, Serialize)]
pub struct MetricsSummaryResponse {
    pub service_count: u32,
    pub active_nodes: u32,
    pub total_queue_depth: u64,
    pub throughput: ThroughputResponse,
    pub services: Vec<ServiceHealthSummary>,
}

#[derive(Debug, Serialize)]
pub struct ThroughputResponse {
    pub submitted_per_min: f64,
    pub completed_per_min: f64,
}

#[derive(Debug, Serialize)]
pub struct ServiceHealthSummary {
    pub name: String,
    pub health: String,
    pub active_nodes: u32,
    pub total_nodes: u32,
    pub queue_depth: u64,
}

#[derive(Debug, Serialize)]
pub struct MetricsHistoryResponse {
    pub interval_secs: u32,
    pub points: Vec<MetricsSnapshot>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_snapshot(ts: i64, submitted: f64, completed: f64) -> MetricsSnapshot {
        MetricsSnapshot {
            timestamp: ts,
            tasks_submitted: submitted,
            tasks_completed: completed,
            tasks_failed: 0.0,
            queue_depth: HashMap::new(),
            nodes_active: HashMap::new(),
        }
    }

    fn make_node(health: NodeHealthState, draining: bool) -> NodeStatus {
        NodeStatus {
            node_id: "n1".to_string(),
            service_name: "svc".to_string(),
            last_seen: chrono::Utc::now().to_rfc3339(),
            health,
            in_flight_tasks: 0,
            draining,
        }
    }

    #[test]
    fn metrics_history_new_creates_empty_buffer() {
        let h = MetricsHistory::new();
        assert_eq!(h.get_all().len(), 0);
    }

    #[test]
    fn metrics_history_push_snapshot_appears_in_get_all() {
        let mut h = MetricsHistory::new();
        h.push_snapshot(make_snapshot(100, 10.0, 5.0));
        let all = h.get_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].timestamp, 100);
    }

    #[test]
    fn metrics_history_drops_oldest_beyond_180() {
        let mut h = MetricsHistory::new();
        for i in 0..200 {
            h.push_snapshot(make_snapshot(i as i64, i as f64, 0.0));
        }
        let all = h.get_all();
        assert_eq!(all.len(), MAX_ENTRIES);
        // Oldest should be entry 20 (0..19 were dropped)
        assert_eq!(all[0].timestamp, 20);
        assert_eq!(all[MAX_ENTRIES - 1].timestamp, 199);
    }

    #[test]
    fn metrics_history_get_snapshot_at_valid() {
        let mut h = MetricsHistory::new();
        h.push_snapshot(make_snapshot(1, 1.0, 0.0));
        h.push_snapshot(make_snapshot(2, 2.0, 0.0));
        h.push_snapshot(make_snapshot(3, 3.0, 0.0));
        assert_eq!(h.get_snapshot_at(0).unwrap().timestamp, 3);
        assert_eq!(h.get_snapshot_at(2).unwrap().timestamp, 1);
    }

    #[test]
    fn metrics_history_get_snapshot_at_out_of_range() {
        let mut h = MetricsHistory::new();
        h.push_snapshot(make_snapshot(1, 1.0, 0.0));
        assert!(h.get_snapshot_at(1).is_none());
        assert!(h.get_snapshot_at(100).is_none());
    }

    #[test]
    fn metrics_history_compute_throughput_too_few_snapshots() {
        let mut h = MetricsHistory::new();
        for i in 0..6 {
            h.push_snapshot(make_snapshot(i * 10, i as f64 * 10.0, 0.0));
        }
        let (s, c) = h.compute_throughput();
        assert_eq!(s, 0.0);
        assert_eq!(c, 0.0);
    }

    #[test]
    fn metrics_history_compute_throughput_correct() {
        let mut h = MetricsHistory::new();
        // 7 snapshots, 10s apart: timestamps 0, 10, 20, 30, 40, 50, 60
        // submitted goes from 0 to 60, completed from 0 to 30
        for i in 0..7 {
            h.push_snapshot(make_snapshot(i * 10, i as f64 * 10.0, i as f64 * 5.0));
        }
        let (sub_per_min, comp_per_min) = h.compute_throughput();
        // delta_submitted = 60 - 0 = 60 over 60s = 60/min
        // delta_completed = 30 - 0 = 30 over 60s = 30/min
        assert!((sub_per_min - 60.0).abs() < 0.001);
        assert!((comp_per_min - 30.0).abs() < 0.001);
    }

    #[test]
    fn derive_service_health_all_healthy() {
        let nodes = vec![
            make_node(NodeHealthState::Healthy, false),
            make_node(NodeHealthState::Healthy, false),
        ];
        assert_eq!(derive_service_health(&nodes), "healthy");
    }

    #[test]
    fn derive_service_health_some_healthy() {
        let nodes = vec![
            make_node(NodeHealthState::Healthy, false),
            make_node(NodeHealthState::Unhealthy, false),
        ];
        assert_eq!(derive_service_health(&nodes), "degraded");
    }

    #[test]
    fn derive_service_health_none_healthy() {
        let nodes = vec![
            make_node(NodeHealthState::Unhealthy, false),
            make_node(NodeHealthState::Disconnected, false),
        ];
        assert_eq!(derive_service_health(&nodes), "down");
    }

    #[test]
    fn derive_service_health_no_nodes() {
        let nodes: Vec<NodeStatus> = vec![];
        assert_eq!(derive_service_health(&nodes), "unknown");
    }

    #[test]
    fn derive_service_health_healthy_but_draining_is_degraded() {
        // A node that is healthy but draining should not count as healthy
        let nodes = vec![
            make_node(NodeHealthState::Healthy, true),
            make_node(NodeHealthState::Healthy, false),
        ];
        assert_eq!(derive_service_health(&nodes), "degraded");
    }
}
