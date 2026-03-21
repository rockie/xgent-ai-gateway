use serde::{Deserialize, Serialize};

/// Persisted service configuration stored in Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub task_timeout_secs: u64,
    pub max_retries: u32,
    pub max_nodes: Option<u32>,
    pub node_stale_after_secs: u64,
    pub drain_timeout_secs: u64,
}

/// Computed node health state -- derived on-demand, never stored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum NodeHealthState {
    Healthy,
    Unhealthy,
    Disconnected,
}

/// Snapshot of a node's current status within a service.
#[derive(Debug, Clone, Serialize)]
pub struct NodeStatus {
    pub node_id: String,
    pub service_name: String,
    pub last_seen: String,
    pub health: NodeHealthState,
    pub in_flight_tasks: u32,
    pub draining: bool,
}

/// Derive node health from last_seen timestamp and service config.
/// No background reaper -- health is computed on-demand.
pub fn derive_health_state(
    last_seen: &str,
    stale_after_secs: u64,
    is_disconnected: bool,
) -> NodeHealthState {
    if is_disconnected {
        return NodeHealthState::Disconnected;
    }
    match chrono::DateTime::parse_from_rfc3339(last_seen) {
        Ok(ts) => {
            let elapsed = chrono::Utc::now().signed_duration_since(ts);
            if elapsed.num_seconds() <= stale_after_secs as i64 {
                NodeHealthState::Healthy
            } else {
                NodeHealthState::Unhealthy
            }
        }
        Err(_) => NodeHealthState::Unhealthy,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recent_timestamp_not_disconnected_is_healthy() {
        let now = chrono::Utc::now().to_rfc3339();
        assert_eq!(
            derive_health_state(&now, 60, false),
            NodeHealthState::Healthy
        );
    }

    #[test]
    fn old_timestamp_not_disconnected_is_unhealthy() {
        let old = (chrono::Utc::now() - chrono::Duration::seconds(120)).to_rfc3339();
        assert_eq!(
            derive_health_state(&old, 60, false),
            NodeHealthState::Unhealthy
        );
    }

    #[test]
    fn any_timestamp_disconnected_is_disconnected() {
        let now = chrono::Utc::now().to_rfc3339();
        assert_eq!(
            derive_health_state(&now, 60, true),
            NodeHealthState::Disconnected
        );
    }

    #[test]
    fn invalid_timestamp_is_unhealthy() {
        assert_eq!(
            derive_health_state("not-a-date", 60, false),
            NodeHealthState::Unhealthy
        );
    }
}
