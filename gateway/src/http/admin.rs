use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::{api_key, node_token};
use crate::state::AppState;

// --- API Key Management ---

#[derive(Debug, Deserialize)]
pub struct CreateApiKeyRequest {
    pub service_names: Vec<String>,
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
) -> Result<(StatusCode, Json<CreateApiKeyResponse>), StatusCode> {
    let (raw_key, key_hash) = api_key::generate_api_key();

    api_key::store_api_key(&mut state.auth_conn.clone(), &key_hash, &req.service_names)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to store API key in Redis");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;

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
