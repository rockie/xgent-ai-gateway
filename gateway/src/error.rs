use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),

    #[error("task not found: {0}")]
    TaskNotFound(String),

    #[error("service not found: {0}")]
    ServiceNotFound(String),

    #[error("invalid request: {0}")]
    InvalidRequest(String),

    #[error("invalid state transition from {from} to {to}")]
    InvalidStateTransition { from: String, to: String },

    #[error("service already exists: {0}")]
    ServiceAlreadyExists(String),

    #[error("unauthorized")]
    Unauthorized,
}

impl From<GatewayError> for tonic::Status {
    fn from(err: GatewayError) -> Self {
        match &err {
            GatewayError::TaskNotFound(_) => tonic::Status::not_found(err.to_string()),
            GatewayError::ServiceNotFound(_) => tonic::Status::not_found(err.to_string()),
            GatewayError::InvalidRequest(_) => tonic::Status::invalid_argument(err.to_string()),
            GatewayError::InvalidStateTransition { .. } => {
                tonic::Status::failed_precondition(err.to_string())
            }
            GatewayError::ServiceAlreadyExists(_) => {
                tonic::Status::already_exists(err.to_string())
            }
            GatewayError::Redis(ref e) => {
                tonic::Status::internal(format!("internal error: {e}"))
            }
            GatewayError::Unauthorized => tonic::Status::unauthenticated("unauthorized"),
        }
    }
}

impl IntoResponse for GatewayError {
    fn into_response(self) -> Response {
        let (status, message) = match &self {
            GatewayError::TaskNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            GatewayError::ServiceNotFound(_) => (StatusCode::NOT_FOUND, self.to_string()),
            GatewayError::InvalidRequest(_) => (StatusCode::BAD_REQUEST, self.to_string()),
            GatewayError::InvalidStateTransition { .. } => {
                (StatusCode::CONFLICT, self.to_string())
            }
            GatewayError::ServiceAlreadyExists(_) => (StatusCode::CONFLICT, self.to_string()),
            GatewayError::Redis(_) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "internal error".to_string(),
            ),
            GatewayError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized".to_string()),
        };

        let body = serde_json::json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}
