use std::sync::Arc;

use tonic::{Request, Response, Status};

use xgent_proto::{
    task_service_server::TaskService, GetTaskStatusRequest, GetTaskStatusResponse,
    SubmitTaskRequest, SubmitTaskResponse,
};

use crate::state::AppState;
use crate::types::{ServiceName, TaskId, TaskState};

/// gRPC implementation of the TaskService (client-facing).
pub struct GrpcTaskService {
    state: Arc<AppState>,
}

impl GrpcTaskService {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl TaskService for GrpcTaskService {
    async fn submit_task(
        &self,
        request: Request<SubmitTaskRequest>,
    ) -> Result<Response<SubmitTaskResponse>, Status> {
        let req = request.into_inner();

        let service = ServiceName::new(&req.service_name).map_err(|e| -> Status { e.into() })?;

        // Check service is registered before accepting the task
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

        let task_id = self
            .state
            .queue
            .submit_task(&service, req.payload, req.metadata)
            .await
            .map_err(|e| -> Status { e.into() })?;

        Ok(Response::new(SubmitTaskResponse {
            task_id: task_id.to_string(),
        }))
    }

    async fn get_task_status(
        &self,
        request: Request<GetTaskStatusRequest>,
    ) -> Result<Response<GetTaskStatusResponse>, Status> {
        let req = request.into_inner();

        let status = self
            .state
            .queue
            .get_task_status(&TaskId(req.task_id))
            .await
            .map_err(|e| -> Status { e.into() })?;

        let state_i32: i32 = status.state.into();

        Ok(Response::new(GetTaskStatusResponse {
            task_id: status.task_id.to_string(),
            state: state_i32,
            result: status.result,
            error_message: status.error_message,
            created_at: status.created_at,
            completed_at: status.completed_at,
            metadata: status.metadata,
        }))
    }
}
