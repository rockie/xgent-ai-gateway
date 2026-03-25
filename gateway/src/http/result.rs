use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::{Path, State};
use axum::Json;
use serde::Serialize;

use crate::error::GatewayError;
use crate::state::AppState;
use crate::types::TaskId;

#[derive(Debug, Serialize)]
pub struct GetTaskResponse {
    pub task_id: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<String>,
    pub metadata: HashMap<String, String>,
}

/// GET /v1/tasks/{task_id} - Retrieve task status and result.
pub async fn get_task(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> Result<Json<GetTaskResponse>, GatewayError> {
    let status = state.queue.get_task_status(&TaskId(task_id)).await?;

    let result = if status.result.is_empty() {
        None
    } else {
        Some(serde_json::from_str(&status.result).unwrap_or(serde_json::Value::String(status.result.clone())))
    };

    let error_message = if status.error_message.is_empty() {
        None
    } else {
        Some(status.error_message)
    };

    let completed_at = if status.completed_at.is_empty() {
        None
    } else {
        Some(status.completed_at)
    };

    Ok(Json(GetTaskResponse {
        task_id: status.task_id.to_string(),
        state: status.state.to_string(),
        result,
        error_message,
        created_at: status.created_at,
        completed_at,
        metadata: status.metadata,
    }))
}
