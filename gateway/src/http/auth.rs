use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use axum_extra::extract::CookieJar;
use axum_extra::extract::cookie::{Cookie, SameSite};
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};

use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub username: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

/// Generate a cryptographically random session ID (64 hex chars = 32 bytes).
fn generate_session_id() -> String {
    use rand::Rng;
    let bytes: [u8; 32] = rand::rng().random();
    hex::encode(bytes)
}

/// Verify a plaintext password against an Argon2 PHC-format hash.
fn verify_password(password: &str, stored_hash: &str) -> bool {
    use argon2::Argon2;
    use password_hash::{PasswordHash, PasswordVerifier};

    let parsed = match PasswordHash::new(stored_hash) {
        Ok(h) => h,
        Err(e) => {
            tracing::error!(error = %e, "failed to parse stored password hash");
            return false;
        }
    };

    Argon2::default()
        .verify_password(password.as_bytes(), &parsed)
        .is_ok()
}

/// Build a session cookie with the given value and TTL.
fn build_session_cookie(session_id: &str, ttl_secs: u64, secure: bool) -> Cookie<'static> {
    // SameSite::None requires Secure flag; use Lax for HTTP (localhost dev)
    let same_site = if secure { SameSite::None } else { SameSite::Lax };
    let mut cookie = Cookie::build(("session", session_id.to_string()))
        .path("/")
        .http_only(true)
        .secure(secure)
        .same_site(same_site)
        .build();
    cookie.set_max_age(Some(time::Duration::seconds(ttl_secs as i64)));
    cookie
}

/// POST /v1/admin/auth/login
///
/// Authenticates admin credentials, creates a Redis-backed session, and returns
/// an HttpOnly session cookie.
pub async fn login(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>), (StatusCode, Json<ErrorResponse>)> {
    let admin = &state.config.admin;

    // Both username and password_hash must be configured for production auth.
    // In dev mode (neither set), accept any credentials.
    let dev_mode = admin.username.is_none() && admin.password_hash.is_none();
    if !dev_mode {
        let (expected_username, expected_hash) = match (&admin.username, &admin.password_hash) {
            (Some(u), Some(h)) => (u.as_str(), h.as_str()),
            _ => {
                tracing::warn!("login attempt but admin credentials partially configured");
                return Err((
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "Invalid username or password".to_string(),
                    }),
                ));
            }
        };

        // Verify username
        if req.username != expected_username {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid username or password".to_string(),
                }),
            ));
        }

        // Verify password
        if !verify_password(&req.password, expected_hash) {
            return Err((
                StatusCode::UNAUTHORIZED,
                Json(ErrorResponse {
                    error: "Invalid username or password".to_string(),
                }),
            ));
        }
    } else {
        tracing::info!("dev mode: accepting login for '{}'", req.username);
    }

    // Create session in Redis
    let session_id = generate_session_id();
    let session_key = format!("admin_session:{}", session_id);
    let created_at = chrono::Utc::now().to_rfc3339();

    let mut conn = state.auth_conn.clone();
    let _: () = redis::cmd("HSET")
        .arg(&session_key)
        .arg("username")
        .arg(&req.username)
        .arg("created_at")
        .arg(&created_at)
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to create session in Redis");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                }),
            )
        })?;

    let _: () = conn
        .expire(&session_key, admin.session_ttl_secs as i64)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to set session TTL");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: "Internal server error".to_string(),
                }),
            )
        })?;

    tracing::info!(username = %req.username, "admin login successful");

    let cookie = build_session_cookie(&session_id, admin.session_ttl_secs, admin.cookie_secure);
    let jar = jar.add(cookie);

    Ok((jar, Json(LoginResponse { username: req.username })))
}

/// POST /v1/admin/auth/logout
///
/// Deletes the session from Redis and clears the session cookie.
pub async fn logout(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> (CookieJar, StatusCode) {
    if let Some(cookie) = jar.get("session") {
        let session_key = format!("admin_session:{}", cookie.value());
        let mut conn = state.auth_conn.clone();
        let _: Result<(), _> = conn.del(&session_key).await;
        tracing::info!("admin session logged out");
    }

    // Clear cookie by setting max_age to 0
    let removal = Cookie::build(("session", ""))
        .path("/")
        .http_only(true)
        .same_site(SameSite::None)
        .max_age(time::Duration::ZERO)
        .build();

    (jar.add(removal), StatusCode::OK)
}

/// POST /v1/admin/auth/refresh
///
/// Resets the session TTL in Redis (sliding window).
pub async fn refresh(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
) -> Result<Json<LoginResponse>, (StatusCode, Json<ErrorResponse>)> {
    let session_cookie = jar.get("session").ok_or((
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse { error: "No session".to_string() }),
    ))?;
    let session_key = format!("admin_session:{}", session_cookie.value());

    let mut conn = state.auth_conn.clone();
    let username: Option<String> = redis::cmd("HGET")
        .arg(&session_key)
        .arg("username")
        .query_async(&mut conn)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to check session in Redis");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Internal error".to_string() }))
        })?;

    let username = username.ok_or((
        StatusCode::UNAUTHORIZED,
        Json(ErrorResponse { error: "Session expired".to_string() }),
    ))?;

    let _: () = conn
        .expire(&session_key, state.config.admin.session_ttl_secs as i64)
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "failed to refresh session TTL");
            (StatusCode::INTERNAL_SERVER_ERROR, Json(ErrorResponse { error: "Internal error".to_string() }))
        })?;

    Ok(Json(LoginResponse { username }))
}

/// Session-based auth middleware for admin endpoints.
///
/// If `admin.username` is not configured, all requests pass through (dev mode).
/// Otherwise, requires a valid `session` cookie corresponding to an active Redis session.
/// Implements sliding window TTL by refreshing the session expiry on each request.
pub async fn session_auth_middleware(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // Dev mode: no admin credentials configured, pass through
    if state.config.admin.username.is_none() {
        return Ok(next.run(req).await);
    }

    // Extract session cookie
    let session_cookie = jar.get("session").ok_or(StatusCode::UNAUTHORIZED)?;
    let session_key = format!("admin_session:{}", session_cookie.value());

    let mut conn = state.auth_conn.clone();
    let exists: bool = conn.exists(&session_key).await.map_err(|e| {
        tracing::error!(error = %e, "session auth: Redis lookup failed");
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !exists {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Sliding window: refresh TTL on each authenticated request
    let _: Result<(), _> = conn
        .expire(&session_key, state.config.admin.session_ttl_secs as i64)
        .await;

    Ok(next.run(req).await)
}
