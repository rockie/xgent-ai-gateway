use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tonic::body::Body;
use tonic::server::NamedService;
use tonic::Status;
use tower::Service;

use axum::http;

use crate::auth::api_key::{extract_api_key, hash_api_key, lookup_api_key};
use crate::auth::node_token::validate_node_token;
use crate::state::AppState;

/// Validated node auth data inserted into request extensions by NodeTokenAuthLayer.
#[derive(Debug, Clone)]
pub struct ValidatedNodeAuth {
    pub service_name: String,
}

// ---------------------------------------------------------------------------
// ApiKeyAuthLayer: wraps a tonic service, enforces API key authentication
// ---------------------------------------------------------------------------

/// Tower Service wrapper that authenticates gRPC requests via API key.
///
/// Extracts the API key from `Authorization: Bearer <key>` or `X-API-Key` header,
/// validates it against Redis, and inserts `ClientMetadata` into request extensions
/// before forwarding to the inner service.
#[derive(Clone)]
pub struct ApiKeyAuthLayer<S> {
    inner: S,
    state: Arc<AppState>,
}

impl<S> ApiKeyAuthLayer<S> {
    pub fn new(inner: S, state: Arc<AppState>) -> Self {
        Self { inner, state }
    }
}

impl<S> Service<http::Request<Body>> for ApiKeyAuthLayer<S>
where
    S: Service<http::Request<Body>, Response = http::Response<Body>, Error = std::convert::Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = http::Response<Body>;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<Body>) -> Self::Future {
        // Clone per Tower clone contract (poll_ready / call pairing)
        let mut inner = self.inner.clone();
        let state = self.state.clone();

        Box::pin(async move {
            // Extract API key from headers
            let raw_key = match extract_api_key(req.headers()) {
                Some(key) => key,
                None => {
                    tracing::debug!("gRPC API key missing");
                    state
                        .metrics
                        .errors_total
                        .with_label_values(&["unknown", "auth_api_key"])
                        .inc();
                    return Ok(Status::unauthenticated("unauthorized").into_http());
                }
            };

            let key_hash = hash_api_key(&raw_key);
            let mut conn = state.auth_conn.clone();

            match lookup_api_key(&mut conn, &key_hash).await {
                Ok(Some(meta)) => {
                    req.extensions_mut().insert(meta);
                    inner.call(req).await
                }
                Ok(None) => {
                    tracing::debug!("gRPC API key not found");
                    state
                        .metrics
                        .errors_total
                        .with_label_values(&["unknown", "auth_api_key"])
                        .inc();
                    Ok(Status::unauthenticated("unauthorized").into_http())
                }
                Err(e) => {
                    tracing::error!("Redis error during gRPC API key lookup: {e}");
                    Ok(Status::internal("internal error").into_http())
                }
            }
        })
    }
}

impl<S: NamedService> NamedService for ApiKeyAuthLayer<S> {
    const NAME: &'static str = S::NAME;
}

// ---------------------------------------------------------------------------
// NodeTokenAuthLayer: wraps a tonic service, enforces node token authentication
// ---------------------------------------------------------------------------

/// Tower Service wrapper that authenticates gRPC requests via node token.
///
/// Extracts the Bearer token from `Authorization` header and service name from
/// `x-service-name` header, validates the token against Redis for that service,
/// and inserts `ValidatedNodeAuth` into request extensions.
#[derive(Clone)]
pub struct NodeTokenAuthLayer<S> {
    inner: S,
    state: Arc<AppState>,
}

impl<S> NodeTokenAuthLayer<S> {
    pub fn new(inner: S, state: Arc<AppState>) -> Self {
        Self { inner, state }
    }
}

impl<S> Service<http::Request<Body>> for NodeTokenAuthLayer<S>
where
    S: Service<http::Request<Body>, Response = http::Response<Body>, Error = std::convert::Infallible>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
{
    type Response = http::Response<Body>;
    type Error = std::convert::Infallible;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut req: http::Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();
        let state = self.state.clone();

        Box::pin(async move {
            // Extract Bearer token from Authorization header
            let raw_token = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|s| s.to_string());

            // Extract service name from x-service-name header
            let service_name = req
                .headers()
                .get("x-service-name")
                .and_then(|v| v.to_str().ok())
                .map(|s| s.to_string());

            let (raw_token, service_name) = match (raw_token, service_name) {
                (Some(t), Some(s)) => (t, s),
                _ => {
                    let svc_label = req
                        .headers()
                        .get("x-service-name")
                        .and_then(|v| v.to_str().ok())
                        .unwrap_or("unknown");
                    tracing::debug!("gRPC node auth missing token or service name");
                    state
                        .metrics
                        .errors_total
                        .with_label_values(&[svc_label, "auth_node_token"])
                        .inc();
                    return Ok(Status::unauthenticated("unauthorized").into_http());
                }
            };

            let mut conn = state.auth_conn.clone();

            match validate_node_token(&mut conn, &service_name, &raw_token).await {
                Ok(true) => {
                    req.extensions_mut().insert(ValidatedNodeAuth {
                        service_name,
                    });
                    inner.call(req).await
                }
                Ok(false) => {
                    tracing::debug!(service=%service_name, "gRPC node token invalid");
                    state
                        .metrics
                        .errors_total
                        .with_label_values(&[service_name.as_str(), "auth_node_token"])
                        .inc();
                    Ok(Status::unauthenticated("unauthorized").into_http())
                }
                Err(e) => {
                    tracing::error!("Redis error during gRPC node token validation: {e}");
                    Ok(Status::internal("internal error").into_http())
                }
            }
        })
    }
}

impl<S: NamedService> NamedService for NodeTokenAuthLayer<S> {
    const NAME: &'static str = S::NAME;
}
