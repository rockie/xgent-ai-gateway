use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::error::GatewayError;
use crate::state::AppState;
use crate::types::ServiceName;

#[derive(Debug, Deserialize)]
pub struct SubmitTaskRequest {
    pub service_name: String,
    /// Base64-encoded opaque payload. Gateway treats this as opaque per TASK-04.
    pub payload: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitTaskResponse {
    pub task_id: String,
}

/// POST /v1/tasks - Submit a new task.
pub async fn submit_task(
    State(state): State<Arc<AppState>>,
    Json(req): Json<SubmitTaskRequest>,
) -> Result<Json<SubmitTaskResponse>, GatewayError> {
    let service = ServiceName::new(&req.service_name)?;

    // Payload is treated as an opaque base64 string by the gateway.
    // Store it as bytes for consistency with gRPC (which uses bytes natively).
    let payload_bytes = base64::Engine::decode(
        &base64::engine::general_purpose::STANDARD,
        &req.payload,
    )
    .map_err(|e| GatewayError::InvalidRequest(format!("invalid base64 payload: {e}")))?;

    let task_id = state
        .queue
        .submit_task(&service, payload_bytes, req.metadata)
        .await?;

    Ok(Json(SubmitTaskResponse {
        task_id: task_id.to_string(),
    }))
}
