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

        tokio::spawn(async move {
            loop {
                tokio::select! {
                    _ = tx.closed() => {
                        tracing::info!(node_id=%node_id, service=%service, "node disconnected");
                        break;
                    }
                    result = queue.poll_task(&service, &node_id) => {
                        match result {
                            Ok(Some(task_data)) => {
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

    /// Heartbeat RPC -- stub for Plan 03-02 implementation.
    async fn heartbeat(
        &self,
        _request: Request<HeartbeatRequest>,
    ) -> Result<Response<HeartbeatResponse>, Status> {
        // TODO: Plan 03-02 will implement node heartbeat tracking
        Ok(Response::new(HeartbeatResponse {
            acknowledged: true,
        }))
    }

    /// DrainNode RPC -- stub for Plan 03-02 implementation.
    async fn drain_node(
        &self,
        _request: Request<DrainNodeRequest>,
    ) -> Result<Response<DrainNodeResponse>, Status> {
        // TODO: Plan 03-02 will implement node drain logic
        Err(Status::unimplemented("drain_node not yet implemented"))
    }
}
