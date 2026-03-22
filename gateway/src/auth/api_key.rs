use std::sync::Arc;

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::Next;
use redis::AsyncCommands;
use sha2::{Digest, Sha256};

use crate::state::AppState;

/// Metadata associated with an API key, stored in Redis.
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientMetadata {
    pub key_hash: String,
    pub service_names: Vec<String>,
    pub created_at: String,
    pub callback_url: Option<String>,
}

/// Generate a new API key pair: (raw_key_hex, sha256_hash_hex).
///
/// The raw key is 32 random bytes encoded as 64 hex characters.
/// The hash is SHA-256 of the raw key string.
pub fn generate_api_key() -> (String, String) {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let raw = hex::encode(bytes);
    let hash = hash_api_key(&raw);
    (raw, hash)
}

/// Hash a raw API key string with SHA-256, returning a 64-char hex digest.
pub fn hash_api_key(raw_key: &str) -> String {
    hex::encode(Sha256::digest(raw_key.as_bytes()))
}

/// Store an API key hash in Redis with associated service names and optional callback URL.
///
/// Redis key: `api_keys:<key_hash>` (hash type with fields: service_names, created_at, callback_url)
pub async fn store_api_key(
    conn: &mut redis::aio::MultiplexedConnection,
    key_hash: &str,
    service_names: &[String],
    callback_url: Option<&str>,
) -> Result<(), redis::RedisError> {
    let redis_key = format!("api_keys:{key_hash}");
    let now = chrono::Utc::now().to_rfc3339();
    let services_csv = service_names.join(",");
    let mut pipe = redis::pipe();
    pipe.hset(&redis_key, "service_names", &services_csv)
        .hset(&redis_key, "created_at", &now);
    if let Some(url) = callback_url {
        pipe.hset(&redis_key, "callback_url", url);
    }
    pipe.query_async(conn).await
}

/// Look up an API key by its hash. Returns None if the key does not exist.
pub async fn lookup_api_key(
    conn: &mut redis::aio::MultiplexedConnection,
    key_hash: &str,
) -> Result<Option<ClientMetadata>, redis::RedisError> {
    let redis_key = format!("api_keys:{key_hash}");
    let result: std::collections::HashMap<String, String> =
        conn.hgetall(&redis_key).await?;

    if result.is_empty() {
        return Ok(None);
    }

    let service_names = result
        .get("service_names")
        .map(|s| s.split(',').map(|v| v.trim().to_string()).collect())
        .unwrap_or_default();
    let created_at = result
        .get("created_at")
        .cloned()
        .unwrap_or_default();

    let callback_url = result
        .get("callback_url")
        .cloned()
        .filter(|s| !s.is_empty());

    Ok(Some(ClientMetadata {
        key_hash: key_hash.to_string(),
        service_names,
        created_at,
        callback_url,
    }))
}

/// Revoke (delete) an API key from Redis. Returns true if the key existed.
pub async fn revoke_api_key(
    conn: &mut redis::aio::MultiplexedConnection,
    key_hash: &str,
) -> Result<bool, redis::RedisError> {
    let redis_key = format!("api_keys:{key_hash}");
    let deleted: i64 = conn.del(&redis_key).await?;
    Ok(deleted > 0)
}

/// Update the callback_url on an existing API key.
///
/// If `callback_url` is `Some`, sets the field. If `None`, removes the field.
/// Returns `false` if the key does not exist.
pub async fn update_api_key_callback(
    conn: &mut redis::aio::MultiplexedConnection,
    key_hash: &str,
    callback_url: Option<&str>,
) -> Result<bool, redis::RedisError> {
    let redis_key = format!("api_keys:{key_hash}");
    let exists: bool = redis::cmd("HEXISTS")
        .arg(&redis_key)
        .arg("service_names")
        .query_async(conn)
        .await?;
    if !exists {
        return Ok(false);
    }
    match callback_url {
        Some(url) => {
            let _: () = conn.hset(&redis_key, "callback_url", url).await?;
        }
        None => {
            let _: () = redis::cmd("HDEL")
                .arg(&redis_key)
                .arg("callback_url")
                .query_async(conn)
                .await?;
        }
    }
    Ok(true)
}

/// Extract an API key from request headers.
///
/// Checks `Authorization: Bearer <key>` first, then falls back to `X-API-Key: <key>`.
/// Returns None if neither header is present.
pub fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    // Prefer Authorization: Bearer
    if let Some(auth) = headers.get("authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }

    // Fall back to X-API-Key
    if let Some(key) = headers.get("x-api-key") {
        if let Ok(val) = key.to_str() {
            return Some(val.to_string());
        }
    }

    None
}

/// Axum middleware that validates API keys against Redis.
///
/// On success, inserts `ClientMetadata` into request extensions.
/// On failure, returns 401 Unauthorized.
pub async fn api_key_auth_middleware(
    State(state): State<Arc<AppState>>,
    mut request: axum::extract::Request,
    next: Next,
) -> Result<axum::response::Response, StatusCode> {
    let raw_key = match extract_api_key(request.headers()) {
        Some(key) => key,
        None => {
            tracing::debug!("API key missing from request");
            state.metrics.errors_total
                .with_label_values(&["unknown", "auth_api_key"])
                .inc();
            return Err(StatusCode::UNAUTHORIZED);
        }
    };

    let key_hash = hash_api_key(&raw_key);
    let mut conn = state.auth_conn.clone();

    match lookup_api_key(&mut conn, &key_hash).await {
        Ok(Some(meta)) => {
            request.extensions_mut().insert(meta);
            Ok(next.run(request).await)
        }
        Ok(None) => {
            tracing::debug!("API key not found in store");
            state.metrics.errors_total
                .with_label_values(&["unknown", "auth_api_key"])
                .inc();
            Err(StatusCode::UNAUTHORIZED)
        }
        Err(e) => {
            tracing::error!(error = %e, "Redis error during API key lookup");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_api_key_format() {
        let (raw, hash) = generate_api_key();
        assert_eq!(raw.len(), 64, "raw key should be 64 hex chars");
        assert_eq!(hash.len(), 64, "hash should be 64 hex chars (SHA-256)");
        assert!(
            raw.chars().all(|c| c.is_ascii_hexdigit()),
            "raw key should be hex"
        );
        assert!(
            hash.chars().all(|c| c.is_ascii_hexdigit()),
            "hash should be hex"
        );
    }

    #[test]
    fn test_hash_api_key_deterministic() {
        let input = "test_key_value";
        let hash1 = hash_api_key(input);
        let hash2 = hash_api_key(input);
        assert_eq!(hash1, hash2, "hashing same input should yield same output");
    }

    #[test]
    fn test_hash_matches_generate() {
        let (raw, hash) = generate_api_key();
        assert_eq!(
            hash_api_key(&raw),
            hash,
            "hash of raw key should match generated hash"
        );
    }

    #[test]
    fn test_generate_api_key_unique() {
        let (raw1, _) = generate_api_key();
        let (raw2, _) = generate_api_key();
        assert_ne!(raw1, raw2, "two generated keys should differ");
    }

    #[test]
    fn test_extract_api_key_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer my_test_key".parse().unwrap());
        assert_eq!(extract_api_key(&headers), Some("my_test_key".to_string()));
    }

    #[test]
    fn test_extract_api_key_x_api_key() {
        let mut headers = HeaderMap::new();
        headers.insert("x-api-key", "my_test_key".parse().unwrap());
        assert_eq!(extract_api_key(&headers), Some("my_test_key".to_string()));
    }

    #[test]
    fn test_extract_api_key_prefers_bearer() {
        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer bearer_key".parse().unwrap());
        headers.insert("x-api-key", "x_api_key".parse().unwrap());
        assert_eq!(extract_api_key(&headers), Some("bearer_key".to_string()));
    }

    #[test]
    fn test_extract_api_key_none() {
        let headers = HeaderMap::new();
        assert_eq!(extract_api_key(&headers), None);
    }
}
