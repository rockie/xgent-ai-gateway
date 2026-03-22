use std::sync::Arc;
use std::time::Duration;

use crate::error::GatewayError;
use crate::registry::node_health::ServiceConfig;
use crate::registry::service::list_services;
use crate::state::AppState;

/// Run the background reaper loop. Cycles through all registered services every 30 seconds,
/// detecting timed-out tasks via Redis XPENDING and marking them as failed.
///
/// This function never returns under normal operation -- it logs errors per cycle and continues.
pub async fn run_reaper(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    // The first tick completes immediately; skip it so we don't reap at startup.
    interval.tick().await;

    loop {
        interval.tick().await;
        match reap_timed_out_tasks(&state).await {
            Ok(count) => {
                if count > 0 {
                    tracing::info!(reaped = count, "reaper cycle completed");
                } else {
                    tracing::debug!("reaper cycle completed, no timed-out tasks");
                }
            }
            Err(e) => {
                tracing::error!(error = %e, "reaper cycle failed");
            }
        }
    }
}

/// Iterate all registered services and reap timed-out tasks from each.
/// Errors on individual services are logged but do not abort the cycle.
async fn reap_timed_out_tasks(state: &AppState) -> Result<u64, GatewayError> {
    let services = list_services(&mut state.auth_conn.clone()).await?;
    let mut total_reaped = 0u64;

    for svc in &services {
        match reap_service(state, svc).await {
            Ok(count) => total_reaped += count,
            Err(e) => {
                tracing::error!(service = %svc.name, error = %e, "reaper failed for service");
            }
        }
    }

    Ok(total_reaped)
}

/// Reap timed-out tasks for a single service.
///
/// Uses XPENDING with IDLE filter to find stream entries that have been pending
/// longer than the service's task_timeout_secs. For each timed-out entry:
/// 1. XRANGE to read the task_id from the stream entry
/// 2. HSET to mark the task hash as failed with error message and timestamp
/// 3. XACK to acknowledge the entry so it is no longer pending
/// 4. INCR failed_count:{service} counter
async fn reap_service(state: &AppState, svc: &ServiceConfig) -> Result<u64, GatewayError> {
    let timeout_ms = svc.task_timeout_secs * 1000;
    let stream_key = format!("tasks:{}", svc.name);
    let mut conn = state.auth_conn.clone();

    // XPENDING <stream> <group> IDLE <min-idle-ms> <start> <end> <count>
    let pending_entries: redis::Value = redis::cmd("XPENDING")
        .arg(&stream_key)
        .arg("workers")
        .arg("IDLE")
        .arg(timeout_ms)
        .arg("-")
        .arg("+")
        .arg(100_u64)
        .query_async(&mut conn)
        .await
        .map_err(GatewayError::Redis)?;

    let entries = parse_xpending_entries(&pending_entries);
    if entries.is_empty() {
        return Ok(0);
    }

    let mut reaped = 0u64;

    for (stream_id, _consumer, idle_ms, _delivery_count) in &entries {
        // XRANGE to get the task_id field from the stream entry
        let range_result: redis::Value = redis::cmd("XRANGE")
            .arg(&stream_key)
            .arg(stream_id)
            .arg(stream_id)
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        let task_id = extract_task_id_from_xrange(&range_result);
        let Some(task_id) = task_id else {
            tracing::warn!(
                stream_id = %stream_id,
                service = %svc.name,
                "could not extract task_id from stream entry, skipping"
            );
            continue;
        };

        let hash_key = format!("task:{}", task_id);
        let error_msg = format!(
            "task timed out: node did not report result within {}s",
            svc.task_timeout_secs
        );
        let now = chrono::Utc::now().to_rfc3339();

        // Mark task as failed and XACK the stream entry
        let _: () = redis::pipe()
            .cmd("HSET")
            .arg(&hash_key)
            .arg("state")
            .arg("failed")
            .arg("error_message")
            .arg(&error_msg)
            .arg("completed_at")
            .arg(&now)
            .ignore()
            .cmd("XACK")
            .arg(&stream_key)
            .arg("workers")
            .arg(stream_id)
            .ignore()
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        // Increment per-service failed task counter
        let _: () = redis::cmd("INCR")
            .arg(format!("failed_count:{}", svc.name))
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        // Check for callback URL on timed-out task and trigger delivery
        let callback_url: Option<String> = redis::cmd("HGET")
            .arg(&hash_key)
            .arg("callback_url")
            .query_async(&mut conn)
            .await
            .unwrap_or(None);

        if let Some(url) = callback_url {
            let client = state.http_client.clone();
            let cfg = &state.config.callback;
            let tid = task_id.clone();
            tokio::spawn(crate::callback::deliver_callback(
                client,
                url,
                tid,
                "failed".to_string(),
                cfg.max_retries,
                cfg.initial_delay_ms,
            ));
        }

        tracing::info!(
            task_id = %task_id,
            service = %svc.name,
            idle_time_ms = idle_ms,
            "reaped timed-out task"
        );

        reaped += 1;
    }

    Ok(reaped)
}

/// Parse the XPENDING extended form result into a vec of (stream_id, consumer, idle_ms, delivery_count).
///
/// XPENDING extended form returns an array of arrays, where each inner array is:
/// [stream_id (BulkString), consumer (BulkString), idle_ms (Int), delivery_count (Int)]
fn parse_xpending_entries(value: &redis::Value) -> Vec<(String, String, u64, u64)> {
    let mut results = Vec::new();

    let entries = match value {
        redis::Value::Array(arr) => arr,
        _ => return results,
    };

    for entry in entries {
        let fields = match entry {
            redis::Value::Array(arr) => arr,
            _ => continue,
        };

        if fields.len() < 4 {
            continue;
        }

        let stream_id = match &fields[0] {
            redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
            redis::Value::SimpleString(s) => s.clone(),
            _ => continue,
        };

        let consumer = match &fields[1] {
            redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
            redis::Value::SimpleString(s) => s.clone(),
            _ => continue,
        };

        let idle_ms = match &fields[2] {
            redis::Value::Int(i) => *i as u64,
            _ => continue,
        };

        let delivery_count = match &fields[3] {
            redis::Value::Int(i) => *i as u64,
            _ => continue,
        };

        results.push((stream_id, consumer, idle_ms, delivery_count));
    }

    results
}

/// Extract the task_id field from an XRANGE result.
///
/// XRANGE returns an array of entries, each entry is [stream_id, [field, value, field, value, ...]].
/// We look for the "task_id" field in the first entry.
fn extract_task_id_from_xrange(value: &redis::Value) -> Option<String> {
    let entries = match value {
        redis::Value::Array(arr) => arr,
        _ => return None,
    };

    let first_entry = entries.first()?;
    let entry_fields = match first_entry {
        redis::Value::Array(arr) if arr.len() >= 2 => arr,
        _ => return None,
    };

    // entry_fields[1] is the field-value array
    let field_values = match &entry_fields[1] {
        redis::Value::Array(arr) => arr,
        _ => return None,
    };

    // Iterate pairs: [field, value, field, value, ...]
    let mut i = 0;
    while i + 1 < field_values.len() {
        let key = match &field_values[i] {
            redis::Value::BulkString(b) => String::from_utf8_lossy(b).to_string(),
            redis::Value::SimpleString(s) => s.clone(),
            _ => {
                i += 2;
                continue;
            }
        };

        if key == "task_id" {
            return match &field_values[i + 1] {
                redis::Value::BulkString(b) => Some(String::from_utf8_lossy(b).to_string()),
                redis::Value::SimpleString(s) => Some(s.clone()),
                _ => None,
            };
        }

        i += 2;
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_xpending_value() {
        // Simulate XPENDING extended form output:
        // Each entry: [stream_id, consumer, idle_time_ms, delivery_count]
        let mock_value = redis::Value::Array(vec![
            redis::Value::Array(vec![
                redis::Value::BulkString(b"1234567890-0".to_vec()),
                redis::Value::BulkString(b"node-abc".to_vec()),
                redis::Value::Int(350000), // 350s idle
                redis::Value::Int(1),      // delivered once
            ]),
            redis::Value::Array(vec![
                redis::Value::BulkString(b"1234567891-0".to_vec()),
                redis::Value::BulkString(b"node-def".to_vec()),
                redis::Value::Int(600000), // 600s idle
                redis::Value::Int(2),      // delivered twice
            ]),
        ]);

        let entries = parse_xpending_entries(&mock_value);
        assert_eq!(entries.len(), 2);

        assert_eq!(entries[0].0, "1234567890-0");
        assert_eq!(entries[0].1, "node-abc");
        assert_eq!(entries[0].2, 350000);
        assert_eq!(entries[0].3, 1);

        assert_eq!(entries[1].0, "1234567891-0");
        assert_eq!(entries[1].1, "node-def");
        assert_eq!(entries[1].2, 600000);
        assert_eq!(entries[1].3, 2);
    }

    #[test]
    fn test_parse_xpending_empty() {
        let mock_value = redis::Value::Array(vec![]);
        let entries = parse_xpending_entries(&mock_value);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_parse_xpending_non_array() {
        let mock_value = redis::Value::Nil;
        let entries = parse_xpending_entries(&mock_value);
        assert!(entries.is_empty());
    }

    #[test]
    fn test_extract_task_id_from_xrange() {
        // XRANGE returns: [[stream_id, [field, value, field, value, ...]]]
        let mock_value = redis::Value::Array(vec![redis::Value::Array(vec![
            redis::Value::BulkString(b"1234567890-0".to_vec()),
            redis::Value::Array(vec![
                redis::Value::BulkString(b"task_id".to_vec()),
                redis::Value::BulkString(b"abc-123-def".to_vec()),
                redis::Value::BulkString(b"service".to_vec()),
                redis::Value::BulkString(b"my-service".to_vec()),
            ]),
        ])]);

        let task_id = extract_task_id_from_xrange(&mock_value);
        assert_eq!(task_id, Some("abc-123-def".to_string()));
    }

    #[test]
    fn test_extract_task_id_from_xrange_empty() {
        let mock_value = redis::Value::Array(vec![]);
        let task_id = extract_task_id_from_xrange(&mock_value);
        assert_eq!(task_id, None);
    }
}
