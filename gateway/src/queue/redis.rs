use crate::config::GatewayConfig;
use crate::error::GatewayError;
use crate::types::{ServiceName, TaskId, TaskState};
use serde::Serialize;
use std::collections::HashMap;
use std::str::FromStr;

/// Full task status retrieved from Redis.
#[derive(Debug, Clone)]
pub struct TaskStatus {
    pub task_id: TaskId,
    pub state: TaskState,
    pub service: String,
    pub payload: String,
    pub result: String,
    pub error_message: String,
    pub metadata: HashMap<String, String>,
    pub created_at: String,
    pub completed_at: String,
    pub stream_id: String,
}

/// Data returned when a node claims a task.
#[derive(Debug, Clone)]
pub struct TaskAssignmentData {
    pub task_id: TaskId,
    pub payload: String,
    pub metadata: HashMap<String, String>,
}

/// Lightweight task summary for list responses (omits payload/result).
#[derive(Debug, Clone, Serialize)]
pub struct TaskSummary {
    pub task_id: String,
    pub state: String,
    pub service: String,
    pub created_at: String,
    pub completed_at: String,
}

/// Redis Streams-backed task queue.
///
/// Uses two connections:
/// - `conn`: shared multiplexed connection for non-blocking commands (HSET, HGETALL, XADD, XACK, etc.)
/// - `blocking_conn`: dedicated connection for blocking operations (XREADGROUP BLOCK)
///
/// This separation prevents blocking XREADGROUP from starving other commands
/// on the multiplexed connection.
#[derive(Clone)]
pub struct RedisQueue {
    conn: ::redis::aio::MultiplexedConnection,
    blocking_conn: ::redis::aio::MultiplexedConnection,
    pub result_ttl_secs: u64,
    pub stream_maxlen: usize,
    pub block_timeout_ms: usize,
}

impl RedisQueue {
    /// Get a reference to the non-blocking connection (for service registration consumer group creation).
    pub fn conn(&self) -> &::redis::aio::MultiplexedConnection {
        &self.conn
    }

    pub async fn new(config: &GatewayConfig) -> Result<Self, GatewayError> {
        let client = ::redis::Client::open(config.redis.url.as_str())
            .map_err(GatewayError::Redis)?;
        let conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(GatewayError::Redis)?;
        let blocking_conn = client
            .get_multiplexed_async_connection()
            .await
            .map_err(GatewayError::Redis)?;

        Ok(Self {
            conn,
            blocking_conn,
            result_ttl_secs: config.redis.result_ttl_secs,
            stream_maxlen: config.queue.stream_maxlen,
            block_timeout_ms: config.queue.block_timeout_ms,
        })
    }

    /// Ensure a consumer group exists for a service stream.
    /// Ignores BUSYGROUP error (group already exists).
    pub async fn ensure_consumer_group(&self, service: &ServiceName) -> Result<(), GatewayError> {
        let stream_key = format!("tasks:{}", service);
        let mut conn = self.conn.clone();

        let result: ::redis::RedisResult<()> =
            ::redis::cmd("XGROUP")
                .arg("CREATE")
                .arg(&stream_key)
                .arg("workers")
                .arg("0")
                .arg("MKSTREAM")
                .query_async(&mut conn)
                .await;

        match result {
            Ok(()) => Ok(()),
            Err(e) if e.to_string().contains("BUSYGROUP") => Ok(()),
            Err(e) => Err(GatewayError::Redis(e)),
        }
    }

    /// Submit a task to a service's queue.
    pub async fn submit_task(
        &self,
        service: &ServiceName,
        payload: String,
        metadata: HashMap<String, String>,
    ) -> Result<TaskId, GatewayError> {
        let task_id = TaskId::new();
        let stream_key = format!("tasks:{}", service);
        let hash_key = format!("task:{}", task_id);
        let metadata_json = serde_json::to_string(&metadata).unwrap_or_default();
        let created_at = chrono::Utc::now().to_rfc3339();

        // Consumer group is created during service registration.
        // No lazy creation here -- service must be registered first.

        let mut conn = self.conn.clone();

        // Store task details in hash
        let _: () = ::redis::pipe()
            .cmd("HSET")
            .arg(&hash_key)
            .arg("state")
            .arg(TaskState::Pending.as_str())
            .arg("service")
            .arg(&service.0)
            .arg("payload")
            .arg(&payload)
            .arg("metadata")
            .arg(&metadata_json)
            .arg("created_at")
            .arg(&created_at)
            .arg("result")
            .arg("")
            .arg("error_message")
            .arg("")
            .arg("completed_at")
            .arg("")
            .arg("stream_id")
            .arg("")
            .ignore()
            // Add to stream with approximate trimming
            .cmd("XADD")
            .arg(&stream_key)
            .arg("MAXLEN")
            .arg("~")
            .arg(self.stream_maxlen)
            .arg("*")
            .arg("task_id")
            .arg(&task_id.0)
            .ignore()
            // Set TTL on the task hash
            .cmd("EXPIRE")
            .arg(&hash_key)
            .arg(self.result_ttl_secs)
            .ignore()
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        Ok(task_id)
    }

    /// Retrieve task status from Redis.
    pub async fn get_task_status(&self, task_id: &TaskId) -> Result<TaskStatus, GatewayError> {
        let hash_key = format!("task:{}", task_id);
        let mut conn = self.conn.clone();

        let fields: HashMap<String, String> = ::redis::cmd("HGETALL")
            .arg(&hash_key)
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        if fields.is_empty() {
            return Err(GatewayError::TaskNotFound(task_id.0.clone()));
        }

        let state = TaskState::from_str(fields.get("state").map(|s| s.as_str()).unwrap_or(""))?;
        let payload = fields.get("payload").cloned().unwrap_or_default();
        let result = fields.get("result").cloned().unwrap_or_default();
        let metadata_json = fields.get("metadata").cloned().unwrap_or_default();
        let metadata: HashMap<String, String> =
            serde_json::from_str(&metadata_json).unwrap_or_default();

        Ok(TaskStatus {
            task_id: task_id.clone(),
            state,
            service: fields.get("service").cloned().unwrap_or_default(),
            payload,
            result,
            error_message: fields.get("error_message").cloned().unwrap_or_default(),
            metadata,
            created_at: fields.get("created_at").cloned().unwrap_or_default(),
            completed_at: fields.get("completed_at").cloned().unwrap_or_default(),
            stream_id: fields.get("stream_id").cloned().unwrap_or_default(),
        })
    }

    /// Report a task result (success or failure). XACKs the stream entry.
    ///
    /// Returns the callback_url from the task hash (if set), so the caller can
    /// spawn callback delivery.
    pub async fn report_result(
        &self,
        task_id: &TaskId,
        success: bool,
        result: String,
        error_message: String,
    ) -> Result<Option<String>, GatewayError> {
        let hash_key = format!("task:{}", task_id);
        let mut conn = self.conn.clone();

        // Retrieve the task to verify it exists and get service/stream_id
        let fields: HashMap<String, String> = ::redis::cmd("HGETALL")
            .arg(&hash_key)
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        if fields.is_empty() {
            return Err(GatewayError::TaskNotFound(task_id.0.clone()));
        }

        let current_state =
            TaskState::from_str(fields.get("state").map(|s| s.as_str()).unwrap_or(""))?;

        // Verify state allows completion
        let new_state = if success {
            TaskState::Completed
        } else {
            TaskState::Failed
        };

        // Allow transition from Assigned or Running
        if current_state != TaskState::Assigned && current_state != TaskState::Running {
            return Err(GatewayError::InvalidStateTransition {
                from: current_state.as_str().to_string(),
                to: new_state.as_str().to_string(),
            });
        }

        let completed_at = chrono::Utc::now().to_rfc3339();
        let service = fields.get("service").cloned().unwrap_or_default();
        let stream_id = fields.get("stream_id").cloned().unwrap_or_default();
        let stream_key = format!("tasks:{}", service);

        // Update hash and XACK
        let mut pipe = ::redis::pipe();
        pipe.cmd("HSET")
            .arg(&hash_key)
            .arg("state")
            .arg(new_state.as_str())
            .arg("result")
            .arg(&result)
            .arg("error_message")
            .arg(&error_message)
            .arg("completed_at")
            .arg(&completed_at)
            .ignore();

        if !stream_id.is_empty() {
            pipe.cmd("XACK")
                .arg(&stream_key)
                .arg("workers")
                .arg(&stream_id)
                .ignore();
        }

        let _: () = pipe
            .query_async(&mut conn)
            .await
            .map_err(|e| {
                tracing::error!(task_id=%task_id, stream_key=%stream_key, stream_id=%stream_id, "report_result pipeline failed: {e}");
                GatewayError::Redis(e)
            })?;

        // Return callback_url if present in the task hash
        let callback_url = fields
            .get("callback_url")
            .cloned()
            .filter(|s| !s.is_empty());

        Ok(callback_url)
    }

    /// Poll for the next task for a service. Returns None on timeout.
    ///
    /// Uses the dedicated `blocking_conn` for the XREADGROUP BLOCK call
    /// to avoid starving other commands on the shared connection.
    pub async fn poll_task(
        &self,
        service: &ServiceName,
        node_id: &str,
    ) -> Result<Option<TaskAssignmentData>, GatewayError> {
        let stream_key = format!("tasks:{}", service);
        let mut conn = self.blocking_conn.clone();

        // Ensure consumer group exists
        self.ensure_consumer_group(service).await?;

        let result: ::redis::streams::StreamReadReply = ::redis::cmd("XREADGROUP")
            .arg("GROUP")
            .arg("workers")
            .arg(node_id)
            .arg("COUNT")
            .arg(1)
            .arg("BLOCK")
            .arg(self.block_timeout_ms)
            .arg("STREAMS")
            .arg(&stream_key)
            .arg(">")
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        // Parse the result
        for stream in &result.keys {
            for entry in &stream.ids {
                let entry_id = &entry.id;
                if let Some(redis::Value::BulkString(task_id_bytes)) = entry.map.get("task_id") {
                    let task_id_str = String::from_utf8_lossy(task_id_bytes).to_string();
                    let task_id = TaskId::from(task_id_str);

                    // Update task state to assigned and record stream_id
                    // Use the main (non-blocking) connection for this write
                    let hash_key = format!("task:{}", task_id);
                    let mut write_conn = self.conn.clone();
                    let _: () = ::redis::pipe()
                        .cmd("HSET")
                        .arg(&hash_key)
                        .arg("state")
                        .arg(TaskState::Assigned.as_str())
                        .arg("stream_id")
                        .arg(entry_id.as_str())
                        .ignore()
                        .query_async(&mut write_conn)
                        .await
                        .map_err(GatewayError::Redis)?;

                    // Retrieve task details
                    let status = self.get_task_status(&task_id).await?;

                    return Ok(Some(TaskAssignmentData {
                        task_id,
                        payload: status.payload,
                        metadata: status.metadata,
                    }));
                }
            }
        }

        Ok(None)
    }

    /// List tasks using Redis SCAN with optional service/status filters.
    /// Returns (tasks, next_cursor). next_cursor is None when keyspace exhausted.
    pub async fn list_tasks(
        &self,
        cursor: Option<&str>,
        page_size: usize,
        service_filter: Option<&str>,
        status_filter: &[TaskState],
        task_id_search: Option<&str>,
    ) -> Result<(Vec<TaskSummary>, Option<String>), GatewayError> {
        // Direct lookup mode (D-09)
        if let Some(search_id) = task_id_search {
            let task_id = TaskId::from(search_id.to_string());
            return match self.get_task_status(&task_id).await {
                Ok(status) => Ok((
                    vec![TaskSummary {
                        task_id: status.task_id.0,
                        state: status.state.as_str().to_string(),
                        service: status.service,
                        created_at: status.created_at,
                        completed_at: status.completed_at,
                    }],
                    None,
                )),
                Err(GatewayError::TaskNotFound(_)) => Ok((vec![], None)),
                Err(e) => Err(e),
            };
        }

        let mut conn = self.conn.clone();
        let mut redis_cursor: u64 = cursor.and_then(|c| c.parse().ok()).unwrap_or(0);
        let mut results = Vec::new();
        let mut iterations = 0;
        let max_iterations: usize = 200;

        while results.len() < page_size && iterations < max_iterations {
            iterations += 1;
            let (next_cursor, keys): (u64, Vec<String>) = ::redis::cmd("SCAN")
                .arg(redis_cursor)
                .arg("MATCH")
                .arg("task:*")
                .arg("COUNT")
                .arg(page_size * 2)
                .query_async(&mut conn)
                .await
                .map_err(GatewayError::Redis)?;

            for key in &keys {
                if results.len() >= page_size {
                    break;
                }
                let task_id_str = match key.strip_prefix("task:") {
                    Some(id) => id,
                    None => continue,
                };
                let task_id = TaskId::from(task_id_str.to_string());
                match self.get_task_status(&task_id).await {
                    Ok(status) => {
                        // Apply service filter
                        if let Some(svc) = service_filter {
                            if status.service != svc {
                                continue;
                            }
                        }
                        // Apply status filter
                        if !status_filter.is_empty() && !status_filter.contains(&status.state) {
                            continue;
                        }
                        results.push(TaskSummary {
                            task_id: status.task_id.0,
                            state: status.state.as_str().to_string(),
                            service: status.service,
                            created_at: status.created_at,
                            completed_at: status.completed_at,
                        });
                    }
                    Err(_) => continue, // key expired between SCAN and HGETALL
                }
            }

            redis_cursor = next_cursor;
            if redis_cursor == 0 {
                break;
            }
        }

        // Sort by task_id descending (UUID v7 = newest first)
        results.sort_by(|a, b| b.task_id.cmp(&a.task_id));

        let next_cursor = if redis_cursor == 0 {
            None
        } else {
            Some(redis_cursor.to_string())
        };

        Ok((results, next_cursor))
    }

    /// Cancel a task (admin action). Sets state to Failed with cancellation message.
    /// XACKs the stream entry if present.
    pub async fn cancel_task(&self, task_id: &TaskId) -> Result<(), GatewayError> {
        let hash_key = format!("task:{}", task_id);
        let mut conn = self.conn.clone();

        let fields: HashMap<String, String> = ::redis::cmd("HGETALL")
            .arg(&hash_key)
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        if fields.is_empty() {
            return Err(GatewayError::TaskNotFound(task_id.0.clone()));
        }

        let current_state =
            TaskState::from_str(fields.get("state").map(|s| s.as_str()).unwrap_or(""))?;

        // Validate: only Pending or Running can be cancelled
        current_state.try_transition(TaskState::Failed)?;

        let completed_at = chrono::Utc::now().to_rfc3339();
        let service = fields.get("service").cloned().unwrap_or_default();
        let stream_id = fields.get("stream_id").cloned().unwrap_or_default();

        let mut pipe = ::redis::pipe();
        pipe.cmd("HSET")
            .arg(&hash_key)
            .arg("state")
            .arg(TaskState::Failed.as_str())
            .arg("error_message")
            .arg("Cancelled by administrator")
            .arg("completed_at")
            .arg(&completed_at)
            .ignore();

        if !stream_id.is_empty() {
            let stream_key = format!("tasks:{}", service);
            pipe.cmd("XACK")
                .arg(&stream_key)
                .arg("workers")
                .arg(&stream_id)
                .ignore();
        }

        let _: () = pipe
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::load_config;

    /// Helper to create a RedisQueue connected to local Redis for testing.
    /// Tests using this are gated with #[ignore] -- run with `cargo test -- --ignored`.
    async fn test_queue() -> RedisQueue {
        let config = load_config(None).unwrap();
        RedisQueue::new(&config).await.expect("Redis must be running for integration tests")
    }

    /// Helper to clean up test keys.
    async fn cleanup(queue: &RedisQueue, keys: &[&str]) {
        let mut conn = queue.conn.clone();
        for key in keys {
            let _: Result<(), _> = ::redis::cmd("DEL").arg(*key).query_async(&mut conn).await;
        }
    }

    #[tokio::test]
    #[ignore]
    async fn submit_task_stores_hash_and_adds_to_stream() {
        let queue = test_queue().await;
        let service = ServiceName::new("test-submit").unwrap();
        let payload = "\"hello world\"".to_string();
        let mut metadata = HashMap::new();
        metadata.insert("key".to_string(), "value".to_string());

        let task_id = queue.submit_task(&service, payload.clone(), metadata.clone()).await.unwrap();

        // Verify task hash exists
        let status = queue.get_task_status(&task_id).await.unwrap();
        assert_eq!(status.state, TaskState::Pending);
        assert_eq!(status.payload, payload);
        assert_eq!(status.metadata.get("key").unwrap(), "value");
        assert_eq!(status.service, "test-submit");
        assert!(!status.created_at.is_empty());

        cleanup(&queue, &[&format!("task:{}", task_id), "tasks:test-submit"]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn get_task_status_returns_not_found_for_missing() {
        let queue = test_queue().await;
        let fake_id = TaskId::from("nonexistent-id".to_string());
        let result = queue.get_task_status(&fake_id).await;
        assert!(matches!(result, Err(GatewayError::TaskNotFound(_))));
    }

    #[tokio::test]
    #[ignore]
    async fn consumer_group_creation_handles_busygroup() {
        let queue = test_queue().await;
        let service = ServiceName::new("test-busygroup").unwrap();

        // Create group twice -- second call should not error
        queue.ensure_consumer_group(&service).await.unwrap();
        queue.ensure_consumer_group(&service).await.unwrap();

        cleanup(&queue, &["tasks:test-busygroup"]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn different_services_use_different_streams() {
        let queue = test_queue().await;
        let svc_a = ServiceName::new("test-svc-a").unwrap();
        let svc_b = ServiceName::new("test-svc-b").unwrap();

        let id_a = queue.submit_task(&svc_a, "\"a\"".to_string(), HashMap::new()).await.unwrap();
        let id_b = queue.submit_task(&svc_b, "\"b\"".to_string(), HashMap::new()).await.unwrap();

        // Both tasks should be retrievable
        let status_a = queue.get_task_status(&id_a).await.unwrap();
        let status_b = queue.get_task_status(&id_b).await.unwrap();
        assert_eq!(status_a.service, "test-svc-a");
        assert_eq!(status_b.service, "test-svc-b");

        cleanup(&queue, &[
            &format!("task:{}", id_a),
            &format!("task:{}", id_b),
            "tasks:test-svc-a",
            "tasks:test-svc-b",
        ]).await;
    }

    #[tokio::test]
    #[ignore]
    async fn result_ttl_is_set_on_task_hash() {
        let queue = test_queue().await;
        let service = ServiceName::new("test-ttl").unwrap();
        let task_id = queue.submit_task(&service, "\"data\"".to_string(), HashMap::new()).await.unwrap();

        // Check that TTL is set
        let hash_key = format!("task:{}", task_id);
        let mut conn = queue.conn.clone();
        let ttl: i64 = ::redis::cmd("TTL").arg(&hash_key).query_async(&mut conn).await.unwrap();
        assert!(ttl > 0, "TTL should be positive, got {}", ttl);
        assert!(ttl <= 86400, "TTL should be <= 86400, got {}", ttl);

        cleanup(&queue, &[&hash_key, "tasks:test-ttl"]).await;
    }
}
