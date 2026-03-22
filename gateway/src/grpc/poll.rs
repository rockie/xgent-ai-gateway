use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio_stream::wrappers::ReceiverStream;
use tonic::{Request, Response, Status};

use xgent_proto::{
    node_service_server::NodeService, DrainNodeRequest, DrainNodeResponse, HeartbeatRequest,
    HeartbeatResponse, PollTasksRequest, ReportResultRequest, ReportResultResponse,
    TaskAssignment,
};

use crate::state::AppState;
use crate::types::{ServiceName, TaskId};

/// Compute poll latency: time from task creation to when a node claims it.
pub fn compute_poll_latency_secs(created_at: &str) -> Option<f64> {
    let created = chrono::DateTime::parse_from_rfc3339(created_at).ok()?;
    let now = chrono::Utc::now();
    Some(now.signed_duration_since(created).num_milliseconds() as f64 / 1000.0)
}

/// Compute task duration from created_at to completed_at.
pub fn compute_task_duration_secs(created_at: &str, completed_at: &str) -> Option<f64> {
    let created = chrono::DateTime::parse_from_rfc3339(created_at).ok()?;
    let completed = chrono::DateTime::parse_from_rfc3339(completed_at).ok()?;
    Some(completed.signed_duration_since(created).num_milliseconds() as f64 / 1000.0)
}

/// gRPC implementation of the NodeService (node-facing).
pub struct GrpcNodeService {
    state: Arc<AppState>,
}

impl GrpcNodeService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl NodeService for GrpcNodeService {
    type PollTasksStream = ReceiverStream<Result<TaskAssignment, Status>>;

    async fn poll_tasks(
        &self,
        request: Request<PollTasksRequest>,
    ) -> Result<Response<Self::PollTasksStream>, Status> {
        // Extract auth token from metadata BEFORE consuming the request
        let raw_token = request
            .metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .map(|s| s.to_string())
            .ok_or_else(|| {
                tracing::debug!("Node poll rejected: missing auth token");
                Status::unauthenticated("unauthorized")
            })?;

        let req = request.into_inner();

        let service =
            ServiceName::new(&req.service_name).map_err(|e| -> Status { e.into() })?;

        // Validate token against Redis for this service
        let token_valid = crate::auth::node_token::validate_node_token(
            &mut self.state.auth_conn.clone(),
            &req.service_name,
            &raw_token,
        )
        .await
        .map_err(|e| {
            tracing::error!("Redis error during node auth: {}", e);
            Status::internal("internal error")
        })?;

        if !token_valid {
            tracing::debug!(service=%req.service_name, "Node poll rejected: invalid token");
            self.state.metrics.errors_total
                .with_label_values(&[req.service_name.as_str(), "auth_node_token"])
                .inc();
            return Err(Status::unauthenticated("unauthorized"));
        }

        if req.node_id.is_empty() {
            return Err(Status::invalid_argument("node_id must not be empty"));
        }

        let node_id = req.node_id;

        // Buffer of 1: one task at a time per D-12
        let (tx, rx) = mpsc::channel::<Result<TaskAssignment, Status>>(1);

        let queue = self.state.queue.clone();
        let state_clone = self.state.clone();

        tokio::spawn(async move {
            let mut health_conn = state_clone.auth_conn.clone();
            let service_name_str = service.0.clone();
            let node_id_clone = node_id.clone();

            // Drain timeout tracking (per D-20)
            let mut drain_started_at: Option<tokio::time::Instant> = None;
            let mut drain_timeout: Option<Duration> = None;

            // Register node on first poll
            let _ = crate::registry::node_health::register_or_update_node(
                &mut health_conn,
                &service_name_str,
                &node_id_clone,
            )
            .await;

            loop {
                // Check drain state BEFORE polling (per D-18)
                let draining = crate::registry::node_health::is_node_draining(
                    &mut health_conn,
                    &service_name_str,
                    &node_id_clone,
                )
                .await
                .unwrap_or(false);

                if draining {
                    // Track drain start time for timeout enforcement (per D-20)
                    if drain_started_at.is_none() {
                        let svc = crate::registry::service::get_service(
                            &mut health_conn,
                            &service_name_str,
                        )
                        .await
                        .ok();
                        let timeout_secs = svc.map(|s| s.drain_timeout_secs).unwrap_or(300);
                        drain_started_at = Some(tokio::time::Instant::now());
                        drain_timeout = Some(Duration::from_secs(timeout_secs));
                        tracing::info!(
                            node_id=%node_id_clone, service=%service_name_str,
                            timeout_secs, "node draining, stopping task dispatch"
                        );
                    }
                    // Check if drain timeout has elapsed
                    if let (Some(started), Some(timeout)) = (drain_started_at, drain_timeout) {
                        if started.elapsed() >= timeout {
                            tracing::warn!(
                                node_id=%node_id_clone, service=%service_name_str,
                                "drain timeout expired, marking node disconnected"
                            );
                            let _ = crate::registry::node_health::mark_node_disconnected(
                                &mut health_conn,
                                &service_name_str,
                                &node_id_clone,
                            )
                            .await;
                            break;
                        }
                    }
                    // Don't poll for new tasks, just keep stream alive as liveness signal
                    tokio::time::sleep(Duration::from_secs(1)).await;
                    continue;
                }

                tokio::select! {
                    _ = tx.closed() => {
                        tracing::info!(node_id=%node_id_clone, service=%service_name_str, "node disconnected");
                        let _ = crate::registry::node_health::mark_node_disconnected(
                            &mut health_conn,
                            &service_name_str,
                            &node_id_clone,
                        ).await;
                        break;
                    }
                    result = queue.poll_task(&service, &node_id) => {
                        // Update last_seen on each poll cycle (passive tracking per D-12)
                        let _ = crate::registry::node_health::register_or_update_node(
                            &mut health_conn,
                            &service_name_str,
                            &node_id_clone,
                        ).await;

                        match result {
                            Ok(Some(task_data)) => {
                                // Increment in_flight_tasks on dispatch
                                let _ = crate::registry::node_health::update_in_flight_tasks(
                                    &mut health_conn,
                                    &service_name_str,
                                    &node_id_clone,
                                    1,
                                ).await;

                                // Record poll latency metric (time from task creation to claim)
                                let task_hash_key = format!("task:{}", task_data.task_id);
                                let created_at: Option<String> = redis::cmd("HGET")
                                    .arg(&task_hash_key)
                                    .arg("created_at")
                                    .query_async(&mut health_conn)
                                    .await
                                    .unwrap_or(None);
                                if let Some(ref created) = created_at {
                                    if let Some(latency) = compute_poll_latency_secs(created) {
                                        state_clone.metrics.node_poll_latency_seconds
                                            .with_label_values(&[&service_name_str])
                                            .observe(latency);
                                    }
                                }

                                let assignment = TaskAssignment {
                                    task_id: task_data.task_id.to_string(),
                                    payload: task_data.payload,
                                    metadata: task_data.metadata,
                                };
                                if tx.send(Ok(assignment)).await.is_err() {
                                    break;
                                }
                            }
                            Ok(None) => {
                                // Timeout from XREADGROUP, loop and retry
                                continue;
                            }
                            Err(e) => {
                                tracing::error!(?e, "redis read error during poll");
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                        }
                    }
                }
            }
        });

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn report_result(
        &self,
        request: Request<ReportResultRequest>,
    ) -> Result<Response<ReportResultResponse>, Status> {
        let req = request.into_inner();
        let task_id_str = req.task_id.clone();
        let was_success = req.success;

        let callback_url = self
            .state
            .queue
            .report_result(
                &TaskId(req.task_id),
                req.success,
                req.result,
                req.error_message,
            )
            .await
            .map_err(|e| -> Status { e.into() })?;

        // Record task completion metrics
        let status_str = if was_success { "completed" } else { "failed" };
        // Fetch task details for service name and duration computation
        let task_status = self
            .state
            .queue
            .get_task_status(&TaskId(task_id_str.clone()))
            .await
            .ok();
        if let Some(ref ts) = task_status {
            self.state.metrics.tasks_completed_total
                .with_label_values(&[ts.service.as_str(), status_str])
                .inc();
            if let Some(duration) = compute_task_duration_secs(&ts.created_at, &ts.completed_at) {
                self.state.metrics.task_duration_seconds
                    .with_label_values(&[ts.service.as_str(), status_str])
                    .observe(duration);
            }
        }

        // Spawn callback delivery if task has a callback URL
        if let Some(url) = callback_url {
            let client = self.state.http_client.clone();
            let cfg = &self.state.config.callback;
            let max_retries = cfg.max_retries;
            let initial_delay_ms = cfg.initial_delay_ms;
            let metrics_ref = &self.state.metrics.callback_delivery_total;
            let cb_metrics = metrics_ref.clone();
            tokio::spawn(crate::callback::deliver_callback(
                client,
                url,
                task_id_str,
                status_str.to_string(),
                max_retries,
                initial_delay_ms,
                Some(cb_metrics),
            ));
        }

        Ok(Response::new(ReportResultResponse {
            acknowledged: true,
        }))
    }

    async fn heartbeat(
        &self,
        request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        let req = request.into_inner();

        // Validate service exists
        let exists = crate::registry::service::service_exists(
            &mut self.state.auth_conn.clone(),
            &req.service_name,
        )
        .await
        .map_err(|e| -> Status { e.into() })?;

        if !exists {
            return Err(Status::not_found(format!(
                "service not found: {}",
                req.service_name
            )));
        }

        // Update node last_seen
        crate::registry::node_health::register_or_update_node(
            &mut self.state.auth_conn.clone(),
            &req.service_name,
            &req.node_id,
        )
        .await
        .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(HeartbeatResponse {
            acknowledged: true,
        }))
    }

    async fn drain_node(
        &self,
        request: Request<DrainNodeRequest>,
    ) -> Result<Response<DrainNodeResponse>, Status> {
        let req = request.into_inner();

        // Validate service exists
        let exists = crate::registry::service::service_exists(
            &mut self.state.auth_conn.clone(),
            &req.service_name,
        )
        .await
        .map_err(|e| -> Status { e.into() })?;

        if !exists {
            return Err(Status::not_found(format!(
                "service not found: {}",
                req.service_name
            )));
        }

        // Set drain flag, get timeout from service config
        let drain_timeout = crate::registry::node_health::set_node_draining(
            &mut self.state.auth_conn.clone(),
            &req.service_name,
            &req.node_id,
        )
        .await
        .map_err(|e| -> Status { e.into() })?;

        tracing::info!(service=%req.service_name, node_id=%req.node_id, "node drain initiated");

        Ok(Response::new(DrainNodeResponse {
            acknowledged: true,
            drain_timeout_secs: drain_timeout,
        }))
    }
}
