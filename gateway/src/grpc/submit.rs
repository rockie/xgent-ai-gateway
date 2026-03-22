use std::sync::Arc;

use redis::AsyncCommands;
use tonic::{Request, Response, Status};

use xgent_proto::{
    task_service_server::TaskService, GetTaskStatusRequest, GetTaskStatusResponse,
    SubmitTaskRequest, SubmitTaskResponse,
};

use crate::state::AppState;
use crate::types::{ServiceName, TaskId};

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
        let client_meta = request
            .extensions()
            .get::<crate::auth::api_key::ClientMetadata>()
            .cloned()
            .ok_or_else(|| Status::internal("auth metadata missing"))?;
        let req = request.into_inner();
        let callback_url_str = req.callback_url.clone();

        let service = ServiceName::new(&req.service_name).map_err(|e| -> Status { e.into() })?;

        // D-07: Per-service authorization check
        if !client_meta.service_names.contains(&req.service_name) {
            tracing::debug!(
                key_hash=%client_meta.key_hash,
                requested_service=%req.service_name,
                authorized_services=?client_meta.service_names,
                "gRPC API key not authorized for requested service"
            );
            return Err(Status::permission_denied("unauthorized"));
        }

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

        let service_name = req.service_name.clone();

        let task_id = self
            .state
            .queue
            .submit_task(&service, req.payload, req.metadata)
            .await
            .map_err(|e| -> Status { e.into() })?;

        // Resolve callback URL: per-task override > per-key default (per D-04/RSLT-03)
        let resolved_callback_url = if !callback_url_str.is_empty() {
            Some(callback_url_str.as_str())
        } else {
            client_meta.callback_url.as_deref()
        };

        // Validate and store callback URL
        if let Some(url) = resolved_callback_url {
            crate::callback::validate_callback_url(url)
                .map_err(|e| Status::invalid_argument(format!("invalid callback_url: {e}")))?;
            let hash_key = format!("task:{}", task_id);
            let mut conn = self.state.queue.conn().clone();
            let _: () = conn
                .hset(&hash_key, "callback_url", url)
                .await
                .map_err(|e| Status::internal(format!("failed to store callback_url: {e}")))?;
        }

        // Record metric: task submitted via gRPC
        self.state.metrics.tasks_submitted_total
            .with_label_values(&[service_name.as_str(), "grpc"])
            .inc();

        tracing::info!(task_id = %task_id, service = %service_name, protocol = "grpc", "task submitted");

        Ok(Response::new(SubmitTaskResponse {
            task_id: task_id.to_string(),
        }))
    }

    async fn get_task_status(
        &self,
        request: Request<GetTaskStatusRequest>,
    ) -> Result<Response<GetTaskStatusResponse>, Status> {
        let client_meta = request
            .extensions()
            .get::<crate::auth::api_key::ClientMetadata>()
            .cloned()
            .ok_or_else(|| Status::internal("auth metadata missing"))?;
        let req = request.into_inner();

        let status = self
            .state
            .queue
            .get_task_status(&TaskId(req.task_id))
            .await
            .map_err(|e| -> Status { e.into() })?;

        // D-08: Verify API key is authorized for the task's service
        if !client_meta.service_names.contains(&status.service) {
            tracing::debug!(
                key_hash=%client_meta.key_hash,
                task_service=%status.service,
                authorized_services=?client_meta.service_names,
                "gRPC API key not authorized for task's service"
            );
            return Err(Status::permission_denied("unauthorized"));
        }

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
