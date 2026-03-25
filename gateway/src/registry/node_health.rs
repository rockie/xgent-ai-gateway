use redis::aio::MultiplexedConnection;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::error::GatewayError;

/// Persisted service configuration stored in Redis.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub task_timeout_secs: u64,
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

/// Register or update a node's last_seen timestamp.
/// Creates the node entry if it doesn't exist (auto-registration on first poll).
/// Updates last_seen on subsequent calls.
pub async fn register_or_update_node(
    conn: &mut MultiplexedConnection,
    service_name: &str,
    node_id: &str,
) -> Result<(), GatewayError> {
    let node_key = format!("node:{service_name}:{node_id}");
    let nodes_key = format!("nodes:{service_name}");
    let now = chrono::Utc::now().to_rfc3339();

    // Pipeline: always update last_seen, node_id, service_name, and clear disconnected;
    // use HSETNX for draining, in_flight_tasks (only set if new — preserve intentional state)
    redis::pipe()
        .cmd("HSET")
        .arg(&node_key)
        .arg("node_id")
        .arg(node_id)
        .arg("service_name")
        .arg(service_name)
        .arg("last_seen")
        .arg(&now)
        .arg("disconnected")
        .arg("false")
        .ignore()
        .cmd("HSETNX")
        .arg(&node_key)
        .arg("draining")
        .arg("false")
        .ignore()
        .cmd("HSETNX")
        .arg(&node_key)
        .arg("in_flight_tasks")
        .arg("0")
        .ignore()
        .cmd("SADD")
        .arg(&nodes_key)
        .arg(node_id)
        .ignore()
        .cmd("EXPIRE")
        .arg(&node_key)
        .arg(86400_u64)
        .ignore()
        .query_async::<()>(conn)
        .await
        .map_err(GatewayError::Redis)?;

    Ok(())
}

/// Set a node's draining flag to true.
/// Returns drain_timeout_secs from the service config.
pub async fn set_node_draining(
    conn: &mut MultiplexedConnection,
    service_name: &str,
    node_id: &str,
) -> Result<u64, GatewayError> {
    let node_key = format!("node:{service_name}:{node_id}");

    let _: () = conn
        .hset(&node_key, "draining", "true")
        .await
        .map_err(GatewayError::Redis)?;

    // Get drain_timeout_secs from the service config
    let svc = crate::registry::service::get_service(conn, service_name).await?;
    Ok(svc.drain_timeout_secs)
}

/// Check if a node is in draining state.
pub async fn is_node_draining(
    conn: &mut MultiplexedConnection,
    service_name: &str,
    node_id: &str,
) -> Result<bool, GatewayError> {
    let node_key = format!("node:{service_name}:{node_id}");

    let val: Option<String> = conn
        .hget(&node_key, "draining")
        .await
        .map_err(GatewayError::Redis)?;

    Ok(val.as_deref() == Some("true"))
}

/// Mark a node as disconnected (stream closed).
pub async fn mark_node_disconnected(
    conn: &mut MultiplexedConnection,
    service_name: &str,
    node_id: &str,
) -> Result<(), GatewayError> {
    let node_key = format!("node:{service_name}:{node_id}");

    let _: () = conn
        .hset(&node_key, "disconnected", "true")
        .await
        .map_err(GatewayError::Redis)?;

    Ok(())
}

/// Increment or decrement in_flight_tasks counter for a node.
pub async fn update_in_flight_tasks(
    conn: &mut MultiplexedConnection,
    service_name: &str,
    node_id: &str,
    delta: i64,
) -> Result<(), GatewayError> {
    let node_key = format!("node:{service_name}:{node_id}");

    let _: i64 = redis::cmd("HINCRBY")
        .arg(&node_key)
        .arg("in_flight_tasks")
        .arg(delta)
        .query_async(conn)
        .await
        .map_err(GatewayError::Redis)?;

    Ok(())
}

/// Get all nodes for a service with their current health (derived on-demand per D-15).
pub async fn get_nodes_for_service(
    conn: &mut MultiplexedConnection,
    service_name: &str,
    stale_after_secs: u64,
) -> Result<Vec<NodeStatus>, GatewayError> {
    let nodes_key = format!("nodes:{service_name}");

    let node_ids: Vec<String> = conn
        .smembers(&nodes_key)
        .await
        .map_err(GatewayError::Redis)?;

    let mut nodes = Vec::with_capacity(node_ids.len());
    for nid in &node_ids {
        let node_key = format!("node:{service_name}:{nid}");

        let fields: std::collections::HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&node_key)
            .query_async(conn)
            .await
            .map_err(GatewayError::Redis)?;

        if fields.is_empty() {
            // Node key expired or removed, skip
            continue;
        }

        let last_seen = fields.get("last_seen").cloned().unwrap_or_default();
        let draining = fields.get("draining").map(|v| v == "true").unwrap_or(false);
        let in_flight_tasks = fields
            .get("in_flight_tasks")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let is_disconnected = fields
            .get("disconnected")
            .map(|v| v == "true")
            .unwrap_or(false);

        let health = derive_health_state(&last_seen, stale_after_secs, is_disconnected);

        nodes.push(NodeStatus {
            node_id: nid.clone(),
            service_name: service_name.to_string(),
            last_seen,
            health,
            in_flight_tasks,
            draining,
        });
    }

    Ok(nodes)
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
