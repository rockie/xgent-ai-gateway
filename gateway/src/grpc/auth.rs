use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};

use tonic::body::Body;
use tonic::server::NamedService;
use tonic::Status;
use tower::Service;

use axum::http;

use sha2::{Sha256, Digest};

use crate::auth::api_key::{extract_api_key, hash_api_key, lookup_api_key};
use crate::auth::node_token::validate_node_token;
use crate::config::MtlsIdentityConfig;
use crate::state::AppState;

/// Compute the SHA-256 fingerprint of a DER-encoded certificate, returned as a hex string.
fn cert_fingerprint(cert_der: &[u8]) -> String {
    let hash = Sha256::digest(cert_der);
    hex::encode(hash)
}

/// Check if a peer certificate is authorized for the given service according to the mTLS identity config.
/// Returns Ok(()) if authorized or if identity checking is disabled (empty fingerprints map).
/// Returns Err(Status) if the certificate is unknown or not authorized for the requested service.
fn check_mtls_identity(
    config: &MtlsIdentityConfig,
    peer_certs: &[tonic::transport::Certificate],
    service_name: &str,
) -> Result<(), Status> {
    if config.fingerprints.is_empty() {
        return Ok(());
    }

    let cert = peer_certs.first().ok_or_else(|| {
        Status::permission_denied("no client certificate presented")
    })?;

    let fp = cert_fingerprint(cert.get_ref());
    match config.fingerprints.get(&fp) {
        Some(allowed_services) => {
            if allowed_services.iter().any(|s| s == service_name) {
                Ok(())
            } else {
                Err(Status::permission_denied(
                    "certificate not authorized for this service",
                ))
            }
        }
        None => Err(Status::permission_denied("unknown certificate fingerprint")),
    }
}

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
                    // Check mTLS identity if fingerprint mapping is configured
                    let mtls_config = &state.config.grpc.mtls_identity;
                    if !mtls_config.fingerprints.is_empty() {
                        // Try to extract peer certs from request extensions.
                        // Tonic inserts Arc<Vec<Certificate>> when TLS is enabled.
                        if let Some(certs) = req.extensions().get::<std::sync::Arc<Vec<tonic::transport::Certificate>>>() {
                            if let Err(status) = check_mtls_identity(mtls_config, certs.as_slice(), &service_name) {
                                tracing::warn!(
                                    service = %service_name,
                                    "mTLS identity check failed"
                                );
                                state
                                    .metrics
                                    .errors_total
                                    .with_label_values(&[service_name.as_str(), "auth_mtls_identity"])
                                    .inc();
                                return Ok(status.into_http());
                            }
                        } else {
                            // No peer certs in extensions -- mTLS may not be active (dev/plaintext mode).
                            // Skip identity check to allow plaintext dev mode to work.
                            tracing::debug!(
                                "mTLS identity configured but no peer certs in request extensions; skipping check"
                            );
                        }
                    }

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
