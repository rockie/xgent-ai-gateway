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

        self.state
            .queue
            .report_result(
                &TaskId(req.task_id),
                req.success,
                req.result,
                req.error_message,
            )
            .await
            .map_err(|e| -> Status { e.into() })?;

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
