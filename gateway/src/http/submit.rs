use std::collections::HashMap;
use std::sync::Arc;

use axum::extract::State;
use axum::Extension;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::api_key::ClientMetadata;
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
    /// Optional callback URL for result delivery. Overrides per-key default.
    pub callback_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct SubmitTaskResponse {
    pub task_id: String,
}

/// POST /v1/tasks - Submit a new task.
///
/// Requires API key auth middleware to inject `ClientMetadata` into request extensions.
/// Enforces per-service authorization: the API key must be authorized for the requested service.
pub async fn submit_task(
    State(state): State<Arc<AppState>>,
    Extension(client_meta): Extension<ClientMetadata>,
    Json(req): Json<SubmitTaskRequest>,
) -> Result<Json<SubmitTaskResponse>, GatewayError> {
    let service = ServiceName::new(&req.service_name)?;

    // D-02/D-09: Check service authorization
    if !client_meta.service_names.contains(&req.service_name) {
        tracing::debug!(
            key_hash=%client_meta.key_hash,
            requested_service=%req.service_name,
            authorized_services=?client_meta.service_names,
            "API key not authorized for requested service"
        );
        return Err(GatewayError::Unauthorized);
    }

    // Check service is registered before accepting the task
    let exists =
        crate::registry::service::service_exists(&mut state.auth_conn.clone(), &req.service_name)
            .await?;
    if !exists {
        return Err(GatewayError::ServiceNotFound(req.service_name.clone()));
    }

    // Resolve callback URL: per-task override > per-key default
    let resolved_callback_url = req
        .callback_url
        .as_deref()
        .or(client_meta.callback_url.as_deref());

    // Validate callback URL if present
    if let Some(url) = resolved_callback_url {
        crate::callback::validate_callback_url(url).map_err(GatewayError::InvalidRequest)?;
    }

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

    // Store resolved callback_url in task hash if present
    if let Some(url) = resolved_callback_url {
        let hash_key = format!("task:{}", task_id);
        let mut conn = state.queue.conn().clone();
        let _: () = redis::AsyncCommands::hset(&mut conn, &hash_key, "callback_url", url)
            .await
            .map_err(GatewayError::Redis)?;
    }

    Ok(Json(SubmitTaskResponse {
        task_id: task_id.to_string(),
    }))
}
