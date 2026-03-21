use redis::AsyncCommands;
use sha2::{Digest, Sha256};

/// Metadata associated with a node token, stored in Redis.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeTokenMetadata {
    pub token_hash: String,
    pub service_name: String,
    pub node_label: Option<String>,
    pub created_at: String,
}

/// Generate a new node token pair: (raw_token_hex, sha256_hash_hex).
///
/// Same pattern as API key generation: 32 random bytes -> 64-char hex.
pub fn generate_node_token() -> (String, String) {
    use rand::RngCore;
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    let raw = hex::encode(bytes);
    let hash = hash_node_token(&raw);
    (raw, hash)
}

/// Hash a raw node token string with SHA-256, returning a 64-char hex digest.
pub fn hash_node_token(raw_token: &str) -> String {
    hex::encode(Sha256::digest(raw_token.as_bytes()))
}

/// Store a node token hash in Redis for a given service.
///
/// Redis key: `node_tokens:<service_name>:<token_hash>` (hash type)
pub async fn store_node_token(
    conn: &mut redis::aio::MultiplexedConnection,
    service_name: &str,
    token_hash: &str,
    node_label: Option<&str>,
) -> Result<(), redis::RedisError> {
    let redis_key = format!("node_tokens:{service_name}:{token_hash}");
    let now = chrono::Utc::now().to_rfc3339();
    let label = node_label.unwrap_or("");
    redis::pipe()
        .hset(&redis_key, "node_label", label)
        .hset(&redis_key, "created_at", &now)
        .query_async(conn)
        .await
}

/// Validate a raw node token against Redis for a given service.
///
/// Hashes the raw token, then checks if the key exists in Redis.
pub async fn validate_node_token(
    conn: &mut redis::aio::MultiplexedConnection,
    service_name: &str,
    raw_token: &str,
) -> Result<bool, redis::RedisError> {
    let hash = hash_node_token(raw_token);
    let redis_key = format!("node_tokens:{service_name}:{hash}");
    conn.exists(&redis_key).await
}

/// Revoke (delete) a node token from Redis. Returns true if the token existed.
pub async fn revoke_node_token(
    conn: &mut redis::aio::MultiplexedConnection,
    service_name: &str,
    token_hash: &str,
) -> Result<bool, redis::RedisError> {
    let redis_key = format!("node_tokens:{service_name}:{token_hash}");
    let deleted: i64 = conn.del(&redis_key).await?;
    Ok(deleted > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_node_token_format() {
        let (raw, hash) = generate_node_token();
        assert_eq!(raw.len(), 64, "raw token should be 64 hex chars");
        assert_eq!(hash.len(), 64, "hash should be 64 hex chars");
        assert!(raw.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(hash.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn test_generate_node_token_unique() {
        let (raw1, _) = generate_node_token();
        let (raw2, _) = generate_node_token();
        assert_ne!(raw1, raw2);
    }

    #[test]
    fn test_hash_node_token_deterministic() {
        let input = "test_node_token";
        let hash1 = hash_node_token(input);
        let hash2 = hash_node_token(input);
        assert_eq!(hash1, hash2);
    }

    #[test]
    fn test_hash_matches_generate() {
        let (raw, hash) = generate_node_token();
        assert_eq!(hash_node_token(&raw), hash);
    }
}
