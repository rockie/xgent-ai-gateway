use redis::AsyncCommands;

use crate::error::GatewayError;
use crate::registry::node_health::ServiceConfig;

/// Register a new service in Redis.
///
/// Creates the service hash at `service:{name}`, adds to `services:index`,
/// and creates the consumer group on the task stream.
///
/// Returns `ServiceAlreadyExists` if the service is already registered.
pub async fn register_service(
    conn: &mut redis::aio::MultiplexedConnection,
    config: &ServiceConfig,
    queue_conn: &mut redis::aio::MultiplexedConnection,
) -> Result<(), GatewayError> {
    let service_key = format!("service:{}", config.name);

    // Check if service already exists
    let exists: bool = conn.exists(&service_key).await.map_err(GatewayError::Redis)?;
    if exists {
        return Err(GatewayError::ServiceAlreadyExists(config.name.clone()));
    }

    // Store service config as a hash
    let max_nodes_str = config
        .max_nodes
        .map(|n| n.to_string())
        .unwrap_or_default();

    redis::pipe()
        .cmd("HSET")
        .arg(&service_key)
        .arg("name")
        .arg(&config.name)
        .arg("description")
        .arg(&config.description)
        .arg("created_at")
        .arg(&config.created_at)
        .arg("task_timeout_secs")
        .arg(config.task_timeout_secs)
        .arg("max_nodes")
        .arg(&max_nodes_str)
        .arg("node_stale_after_secs")
        .arg(config.node_stale_after_secs)
        .arg("drain_timeout_secs")
        .arg(config.drain_timeout_secs)
        .ignore()
        .cmd("SADD")
        .arg("services:index")
        .arg(&config.name)
        .ignore()
        .query_async::<()>(conn)
        .await
        .map_err(GatewayError::Redis)?;

    // Create the consumer group on the task stream
    let stream_key = format!("tasks:{}", config.name);
    let result: redis::RedisResult<()> = redis::cmd("XGROUP")
        .arg("CREATE")
        .arg(&stream_key)
        .arg("workers")
        .arg("0")
        .arg("MKSTREAM")
        .query_async(queue_conn)
        .await;

    match result {
        Ok(()) => {}
        Err(e) if e.to_string().contains("BUSYGROUP") => {}
        Err(e) => return Err(GatewayError::Redis(e)),
    }

    tracing::info!(service=%config.name, "service registered");
    Ok(())
}

/// Get a service configuration from Redis.
pub async fn get_service(
    conn: &mut redis::aio::MultiplexedConnection,
    name: &str,
) -> Result<ServiceConfig, GatewayError> {
    let service_key = format!("service:{}", name);

    let fields: std::collections::HashMap<String, String> = redis::cmd("HGETALL")
        .arg(&service_key)
        .query_async(conn)
        .await
        .map_err(GatewayError::Redis)?;

    if fields.is_empty() {
        return Err(GatewayError::ServiceNotFound(name.to_string()));
    }

    parse_service_config(&fields)
}

/// List all registered services.
pub async fn list_services(
    conn: &mut redis::aio::MultiplexedConnection,
) -> Result<Vec<ServiceConfig>, GatewayError> {
    let names: Vec<String> = conn
        .smembers("services:index")
        .await
        .map_err(GatewayError::Redis)?;

    let mut services = Vec::with_capacity(names.len());
    for name in &names {
        match get_service(conn, name).await {
            Ok(svc) => services.push(svc),
            Err(GatewayError::ServiceNotFound(_)) => {
                // Index entry without config -- stale, skip it
                tracing::warn!(service=%name, "stale entry in services:index, skipping");
            }
            Err(e) => return Err(e),
        }
    }

    Ok(services)
}

/// Check if a service is registered.
pub async fn service_exists(
    conn: &mut redis::aio::MultiplexedConnection,
    name: &str,
) -> Result<bool, GatewayError> {
    let exists: bool = conn
        .sismember("services:index", name)
        .await
        .map_err(GatewayError::Redis)?;
    Ok(exists)
}

/// Delete a service's config and remove it from the index.
/// This is the final step of cleanup -- use `cleanup::cleanup_service` for full deregistration.
pub async fn delete_service_config(
    conn: &mut redis::aio::MultiplexedConnection,
    name: &str,
) -> Result<(), GatewayError> {
    let service_key = format!("service:{}", name);

    redis::pipe()
        .cmd("DEL")
        .arg(&service_key)
        .ignore()
        .cmd("SREM")
        .arg("services:index")
        .arg(name)
        .ignore()
        .query_async::<()>(conn)
        .await
        .map_err(GatewayError::Redis)?;

    Ok(())
}

/// Parse a HashMap from HGETALL into a ServiceConfig.
fn parse_service_config(
    fields: &std::collections::HashMap<String, String>,
) -> Result<ServiceConfig, GatewayError> {
    let name = fields
        .get("name")
        .cloned()
        .unwrap_or_default();
    let description = fields
        .get("description")
        .cloned()
        .unwrap_or_default();
    let created_at = fields
        .get("created_at")
        .cloned()
        .unwrap_or_default();
    let task_timeout_secs = fields
        .get("task_timeout_secs")
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);
    let max_nodes = fields
        .get("max_nodes")
        .and_then(|v| if v.is_empty() { None } else { v.parse().ok() });
    let node_stale_after_secs = fields
        .get("node_stale_after_secs")
        .and_then(|v| v.parse().ok())
        .unwrap_or(60);
    let drain_timeout_secs = fields
        .get("drain_timeout_secs")
        .and_then(|v| v.parse().ok())
        .unwrap_or(300);

    Ok(ServiceConfig {
        name,
        description,
        created_at,
        task_timeout_secs,
        max_nodes,
        node_stale_after_secs,
        drain_timeout_secs,
    })
}
