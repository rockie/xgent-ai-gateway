use crate::error::GatewayError;

/// Full deregistration cleanup for a service.
///
/// Runs as a background task (spawned via tokio::spawn). Steps:
/// 1. Mark pending/assigned tasks as failed
/// 2. Delete node tokens
/// 3. Destroy consumer group
/// 4. Delete task stream
/// 5. Delete node health entries
/// 6. Delete service config + remove from index
pub async fn cleanup_service(
    conn: &mut redis::aio::MultiplexedConnection,
    service_name: &str,
) -> Result<(), GatewayError> {
    tracing::info!(service=%service_name, "starting service deregistration cleanup");

    // 1. Mark all pending tasks as failed
    scan_and_fail_service_tasks(conn, service_name).await?;

    // 2. Delete node tokens
    scan_and_unlink(conn, &format!("node_tokens:{}:*", service_name)).await?;

    // 3. Destroy consumer group (ignore error if doesn't exist)
    let stream_key = format!("tasks:{}", service_name);
    let result: redis::RedisResult<()> = redis::cmd("XGROUP")
        .arg("DESTROY")
        .arg(&stream_key)
        .arg("workers")
        .query_async(conn)
        .await;
    if let Err(e) = result {
        tracing::debug!(service=%service_name, error=%e, "XGROUP DESTROY (may not exist)");
    }

    // 4. Delete task stream
    let _: redis::RedisResult<()> = redis::cmd("DEL")
        .arg(&stream_key)
        .query_async(conn)
        .await;

    // 5. Delete node health entries
    scan_and_unlink(conn, &format!("node:{}:*", service_name)).await?;
    let _: redis::RedisResult<()> = redis::cmd("DEL")
        .arg(format!("nodes:{}", service_name))
        .query_async(conn)
        .await;

    // 6. Delete service config + remove from index
    crate::registry::service::delete_service_config(conn, service_name).await?;

    tracing::info!(service=%service_name, "service deregistration cleanup complete");
    Ok(())
}

/// SCAN for keys matching a pattern and UNLINK them in batches.
pub async fn scan_and_unlink(
    conn: &mut redis::aio::MultiplexedConnection,
    pattern: &str,
) -> Result<(), GatewayError> {
    let mut cursor: u64 = 0;
    loop {
        let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(100)
            .query_async(conn)
            .await
            .map_err(GatewayError::Redis)?;

        if !keys.is_empty() {
            let mut cmd = redis::cmd("UNLINK");
            for key in &keys {
                cmd.arg(key);
            }
            let _: redis::RedisResult<()> = cmd.query_async(conn).await;
        }

        cursor = new_cursor;
        if cursor == 0 {
            break;
        }
    }
    Ok(())
}

/// SCAN for task:* keys belonging to a service and mark them failed, then delete.
async fn scan_and_fail_service_tasks(
    conn: &mut redis::aio::MultiplexedConnection,
    service_name: &str,
) -> Result<(), GatewayError> {
    let mut cursor: u64 = 0;
    loop {
        let (new_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("task:*")
            .arg("COUNT")
            .arg(100)
            .query_async(conn)
            .await
            .map_err(GatewayError::Redis)?;

        for key in &keys {
            // Check if this task belongs to the service being deregistered
            let svc: redis::RedisResult<Option<String>> =
                redis::cmd("HGET").arg(key).arg("service").query_async(conn).await;

            if let Ok(Some(svc_name)) = svc {
                if svc_name == service_name {
                    // Mark as failed and then delete
                    let _: redis::RedisResult<()> = redis::pipe()
                        .cmd("HSET")
                        .arg(key)
                        .arg("state")
                        .arg("failed")
                        .arg("error_message")
                        .arg("service deregistered")
                        .ignore()
                        .cmd("DEL")
                        .arg(key)
                        .ignore()
                        .query_async(conn)
                        .await;
                }
            }
        }

        cursor = new_cursor;
        if cursor == 0 {
            break;
        }
    }
    Ok(())
}
