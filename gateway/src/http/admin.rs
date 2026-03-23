use std::sync::Arc;

use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::Json;
use base64::Engine as _;
use serde::{Deserialize, Serialize};

use crate::auth::{api_key, node_token};
use crate::error::GatewayError;
use crate::registry;
use crate::registry::node_health::{derive_health_state, NodeHealthState, ServiceConfig};
use crate::state::AppState;

// --- API Key Management ---

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub service_names: Vec<String>,
    /// Optional default callback URL for all tasks submitted with this API key.
    pub callback_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateApiKeyResponse {
    pub api_key: String,
    pub key_hash: String,
}

/// POST /v1/admin/api-keys - Create a new API key.
///
/// Returns the raw key exactly once; it cannot be retrieved again.
pub async fn create_api_key(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateApiKeyRequest>,
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), GatewayError> {
    // Validate callback URL if present
    if let Some(ref url) = req.callback_url {
        crate::callback::validate_callback_url(url)
            .map_err(GatewayError::InvalidRequest)?;
    }

    let (raw_key, key_hash) = api_key::generate_api_key();

    api_key::store_api_key(
        &mut state.auth_conn.clone(),
        &key_hash,
        &req.service_names,
        req.callback_url.as_deref(),
    )
    .await
    .map_err(GatewayError::Redis)?;

    Ok((
        StatusCode::CREATED,
        Json(CreateApiKeyResponse {
            api_key: raw_key,
            key_hash,
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct RevokeApiKeyRequest {
    pub key_hash: String,
}

/// POST /v1/admin/api-keys/revoke - Revoke an API key by its hash.
pub async fn revoke_api_key(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RevokeApiKeyRequest>,
) -> Result<StatusCode, StatusCode> {
    let deleted = api_key::revoke_api_key(&mut state.auth_conn.clone(), &req.key_hash)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to revoke API key from Redis");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

    if deleted {
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdateApiKeyCallbackRequest {
    /// Set or clear the default callback URL for this API key.
    /// Omit or set to null to remove.
    pub callback_url: Option<String>,
}

/// PATCH /v1/admin/api-keys/{key_hash} - Update callback URL on an API key.
pub async fn update_api_key_callback(
    State(state): State<Arc<AppState>>,
    Path(key_hash): Path<String>,
    Json(req): Json<UpdateApiKeyCallbackRequest>,
) -> Result<StatusCode, GatewayError> {
    // Validate URL if present
    if let Some(ref url) = req.callback_url {
        crate::callback::validate_callback_url(url)
            .map_err(GatewayError::InvalidRequest)?;
    }

    let updated = api_key::update_api_key_callback(
        &mut state.auth_conn.clone(),
        &key_hash,
        req.callback_url.as_deref(),
    )
    .await
    .map_err(GatewayError::Redis)?;

    if updated {
        Ok(StatusCode::OK)
    } else {
        // Key not found -- return 404 via TaskNotFound (reusing existing variant)
        Err(GatewayError::TaskNotFound(format!(
            "API key not found: {}",
            key_hash
        )))
    }
}

// --- Node Token Management ---

#[derive(Debug, Deserialize)]
pub struct CreateNodeTokenRequest {
    pub service_name: String,
    pub node_label: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct CreateNodeTokenResponse {
    pub token: String,
    pub token_hash: String,
    pub service_name: String,
}

/// POST /v1/admin/node-tokens - Create a new node token for a service.
///
/// Returns the raw token exactly once; it cannot be retrieved again.
pub async fn create_node_token(
    State(state): State<Arc<AppState>>,
    Json(req): Json<CreateNodeTokenRequest>,
) -> Result<(StatusCode, Json<CreateNodeTokenResponse>), StatusCode> {
    let (raw_token, token_hash) = node_token::generate_node_token();

    node_token::store_node_token(
        &mut state.auth_conn.clone(),
        &req.service_name,
        &token_hash,
        req.node_label.as_deref(),
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Failed to store node token in Redis");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    Ok((
        StatusCode::CREATED,
        Json(CreateNodeTokenResponse {
            token: raw_token,
            token_hash,
            service_name: req.service_name,
        }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct RevokeNodeTokenRequest {
    pub service_name: String,
    pub token_hash: String,
}

/// POST /v1/admin/node-tokens/revoke - Revoke a node token.
pub async fn revoke_node_token(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RevokeNodeTokenRequest>,
) -> Result<StatusCode, StatusCode> {
    let deleted = node_token::revoke_node_token(
        &mut state.auth_conn.clone(),
        &req.service_name,
        &req.token_hash,
    )
    .await
    .map_err(|e| {
        tracing::error!(error = %e, "Failed to revoke node token from Redis");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if deleted {
        Ok(StatusCode::OK)
    } else {
        Err(StatusCode::NOT_FOUND)
    }
}

// --- Service Management ---

#[derive(Debug, Deserialize)]
pub struct RegisterServiceRequest {
    pub name: String,
    pub description: Option<String>,
    pub task_timeout_secs: Option<u64>,
    pub max_retries: Option<u32>,
    pub max_nodes: Option<u32>,
    pub node_stale_after_secs: Option<u64>,
    pub drain_timeout_secs: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct ServiceResponse {
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub task_timeout_secs: u64,
    pub max_retries: u32,
    pub max_nodes: Option<u32>,
    pub node_stale_after_secs: u64,
    pub drain_timeout_secs: u64,
}

#[derive(Debug, Serialize)]
pub struct ListServicesResponse {
    pub services: Vec<ServiceResponse>,
}

#[derive(Debug, Serialize)]
pub struct ServiceDetailResponse {
    #[serde(flatten)]
    pub service: ServiceResponse,
    pub nodes: Vec<NodeStatusResponse>,
}

#[derive(Debug, Serialize)]
pub struct NodeStatusResponse {
    pub node_id: String,
    pub health: String,
    pub last_seen: String,
    pub in_flight_tasks: u32,
    pub draining: bool,
}

impl From<&ServiceConfig> for ServiceResponse {
    fn from(cfg: &ServiceConfig) -> Self {
        Self {
            name: cfg.name.clone(),
            description: cfg.description.clone(),
            created_at: cfg.created_at.clone(),
            task_timeout_secs: cfg.task_timeout_secs,
            max_retries: cfg.max_retries,
            max_nodes: cfg.max_nodes,
            node_stale_after_secs: cfg.node_stale_after_secs,
            drain_timeout_secs: cfg.drain_timeout_secs,
        }
    }
}

/// POST /v1/admin/services - Register a new service.
pub async fn register_service(
    State(state): State<Arc<AppState>>,
    Json(req): Json<RegisterServiceRequest>,
) -> Result<(StatusCode, Json<ServiceResponse>), GatewayError> {
    if req.name.is_empty() {
        return Err(GatewayError::InvalidRequest(
            "service name must not be empty".to_string(),
        ));
    }

    let defaults = &state.config.service_defaults;
    let config = ServiceConfig {
        name: req.name.clone(),
        description: req.description.unwrap_or_default(),
        created_at: chrono::Utc::now().to_rfc3339(),
        task_timeout_secs: req.task_timeout_secs.unwrap_or(defaults.task_timeout_secs),
        max_retries: req.max_retries.unwrap_or(defaults.max_retries),
        max_nodes: req.max_nodes,
        node_stale_after_secs: req
            .node_stale_after_secs
            .unwrap_or(defaults.node_stale_after_secs),
        drain_timeout_secs: req
            .drain_timeout_secs
            .unwrap_or(defaults.drain_timeout_secs),
    };

    let mut conn = state.auth_conn.clone();
    let mut queue_conn = state.queue.conn().clone();
    registry::service::register_service(&mut conn, &config, &mut queue_conn).await?;

    Ok((StatusCode::CREATED, Json(ServiceResponse::from(&config))))
}

/// DELETE /v1/admin/services/{name} - Deregister a service.
///
/// Returns 202 Accepted immediately and runs cleanup in the background.
pub async fn deregister_service(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<StatusCode, GatewayError> {
    let exists =
        registry::service::service_exists(&mut state.auth_conn.clone(), &name).await?;
    if !exists {
        return Err(GatewayError::ServiceNotFound(name));
    }

    // Spawn background cleanup
    let mut conn = state.auth_conn.clone();
    let service_name = name.clone();
    tokio::spawn(async move {
        if let Err(e) = registry::cleanup::cleanup_service(&mut conn, &service_name).await {
            tracing::error!(service=%service_name, error=%e, "service deregistration cleanup failed");
        }
    });

    Ok(StatusCode::ACCEPTED)
}

/// GET /v1/admin/services - List all registered services.
pub async fn list_services(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ListServicesResponse>, GatewayError> {
    let services =
        registry::service::list_services(&mut state.auth_conn.clone()).await?;

    let response = ListServicesResponse {
        services: services.iter().map(ServiceResponse::from).collect(),
    };

    Ok(Json(response))
}

/// GET /v1/admin/services/{name} - Get service details with live node health.
pub async fn get_service_detail(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<Json<ServiceDetailResponse>, GatewayError> {
    let config =
        registry::service::get_service(&mut state.auth_conn.clone(), &name).await?;

    // Enumerate nodes via SMEMBERS on nodes:{name}
    let mut conn = state.auth_conn.clone();
    let nodes_key = format!("nodes:{}", name);
    let node_ids: Vec<String> = redis::AsyncCommands::smembers(&mut conn, &nodes_key)
        .await
        .unwrap_or_default();

    let mut nodes = Vec::new();
    for node_id in &node_ids {
        let node_key = format!("node:{}:{}", name, node_id);
        let fields: std::collections::HashMap<String, String> = redis::cmd("HGETALL")
            .arg(&node_key)
            .query_async(&mut conn)
            .await
            .unwrap_or_default();

        if fields.is_empty() {
            continue;
        }

        let last_seen = fields.get("last_seen").cloned().unwrap_or_default();
        let is_disconnected = fields
            .get("disconnected")
            .map(|v| v == "true")
            .unwrap_or(false);
        let health =
            derive_health_state(&last_seen, config.node_stale_after_secs, is_disconnected);
        let in_flight_tasks: u32 = fields
            .get("in_flight_tasks")
            .and_then(|v| v.parse().ok())
            .unwrap_or(0);
        let draining = fields
            .get("draining")
            .map(|v| v == "true")
            .unwrap_or(false);

        nodes.push(NodeStatusResponse {
            node_id: node_id.clone(),
            health: format!("{:?}", health).to_lowercase(),
            last_seen,
            in_flight_tasks,
            draining,
        });
    }

    Ok(Json(ServiceDetailResponse {
        service: ServiceResponse::from(&config),
        nodes,
    }))
}

// --- Metrics Endpoint ---

/// GET /metrics - Returns Prometheus exposition format with all registered metrics.
pub async fn metrics_handler(
    State(state): State<Arc<AppState>>,
) -> impl axum::response::IntoResponse {
    let encoder = prometheus::TextEncoder::new();
    let metric_families = state.metrics.registry.gather();
    let mut buffer = Vec::new();
    prometheus::Encoder::encode(&encoder, &metric_families, &mut buffer).unwrap();
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        buffer,
    )
}

// --- Health Endpoint ---

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub services: Vec<ServiceHealthResponse>,
}

#[derive(Debug, Serialize)]
pub struct ServiceHealthResponse {
    pub name: String,
    pub active_nodes: u32,
    pub total_nodes: u32,
    pub nodes: Vec<NodeStatusResponse>,
}

/// GET /v1/admin/health - Returns per-service node health overview.
pub async fn health_handler(
    State(state): State<Arc<AppState>>,
) -> Result<Json<HealthResponse>, GatewayError> {
    let services = registry::service::list_services(&mut state.auth_conn.clone()).await?;

    let mut service_healths = Vec::new();
    for svc in &services {
        let nodes =
            registry::node_health::get_nodes_for_service(
                &mut state.auth_conn.clone(),
                &svc.name,
                svc.node_stale_after_secs,
            )
            .await?;

        let active_nodes = nodes
            .iter()
            .filter(|n| n.health == NodeHealthState::Healthy)
            .count() as u32;
        let total_nodes = nodes.len() as u32;

        let node_responses: Vec<NodeStatusResponse> = nodes
            .iter()
            .map(|n| NodeStatusResponse {
                node_id: n.node_id.clone(),
                health: format!("{:?}", n.health).to_lowercase(),
                last_seen: n.last_seen.clone(),
                in_flight_tasks: n.in_flight_tasks,
                draining: n.draining,
            })
            .collect();

        service_healths.push(ServiceHealthResponse {
            name: svc.name.clone(),
            active_nodes,
            total_nodes,
            nodes: node_responses,
        });
    }

    Ok(Json(HealthResponse {
        services: service_healths,
    }))
}

// --- Task Management ---

#[derive(Debug, Deserialize)]
pub struct ListTasksParams {
    pub cursor: Option<String>,
    pub page_size: Option<usize>,
    pub service: Option<String>,
    pub status: Option<String>,  // comma-separated: "pending,running"
    pub task_id: Option<String>, // direct lookup by ID
}

#[derive(Debug, Serialize)]
pub struct ListTasksResponse {
    pub tasks: Vec<crate::queue::redis::TaskSummary>,
    pub cursor: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct TaskDetailResponse {
    pub task_id: String,
    pub state: String,
    pub service: String,
    pub payload: String,  // base64-encoded
    pub result: String,   // base64-encoded
    pub error_message: String,
    pub metadata: std::collections::HashMap<String, String>,
    pub created_at: String,
    pub completed_at: String,
    pub stream_id: String,
}

/// GET /v1/admin/tasks - List tasks with pagination and filters
pub async fn list_tasks_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<ListTasksParams>,
) -> Result<Json<ListTasksResponse>, GatewayError> {
    let page_size = params.page_size.unwrap_or(25).min(50).max(1);
    let status_filter: Vec<crate::types::TaskState> = params
        .status
        .as_deref()
        .map(|s| {
            s.split(',')
                .filter_map(|v| crate::types::TaskState::from_str(v.trim()).ok())
                .collect()
        })
        .unwrap_or_default();

    let (tasks, cursor) = state
        .queue
        .list_tasks(
            params.cursor.as_deref(),
            page_size,
            params.service.as_deref(),
            &status_filter,
            params.task_id.as_deref(),
        )
        .await?;

    Ok(Json(ListTasksResponse { tasks, cursor }))
}

/// GET /v1/admin/tasks/{task_id} - Get full task detail
pub async fn get_task_detail_handler(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskDetailResponse>, GatewayError> {
    let tid = crate::types::TaskId::from(task_id);
    let status = state.queue.get_task_status(&tid).await?;

    let payload_b64 = base64::engine::general_purpose::STANDARD.encode(&status.payload);
    let result_b64 = if status.result.is_empty() {
        String::new()
    } else {
        base64::engine::general_purpose::STANDARD.encode(&status.result)
    };

    Ok(Json(TaskDetailResponse {
        task_id: status.task_id.0,
        state: status.state.as_str().to_string(),
        service: status.service,
        payload: payload_b64,
        result: result_b64,
        error_message: status.error_message,
        metadata: status.metadata,
        created_at: status.created_at,
        completed_at: status.completed_at,
        stream_id: status.stream_id,
    }))
}

/// POST /v1/admin/tasks/{task_id}/cancel - Cancel a pending or running task
pub async fn cancel_task_handler(
    State(state): State<Arc<AppState>>,
    Path(task_id): Path<String>,
) -> Result<StatusCode, GatewayError> {
    let tid = crate::types::TaskId::from(task_id);
    state.queue.cancel_task(&tid).await?;
    Ok(StatusCode::OK)
}

