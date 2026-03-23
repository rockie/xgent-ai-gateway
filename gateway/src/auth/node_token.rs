use redis::AsyncCommands;
use sha2::{Digest, Sha256};

/// Metadata associated with a node token, stored in Redis.
#[derive(Debug, Clone, serde::Serialize)]
pub struct NodeTokenMetadata {
    pub token_hash: String,
    pub service_name: String,
    pub node_label: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
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
    expires_at: Option<&str>,
) -> Result<(), redis::RedisError> {
    let redis_key = format!("node_tokens:{service_name}:{token_hash}");
    let now = chrono::Utc::now().to_rfc3339();
    let label = node_label.unwrap_or("");
    let mut pipe = redis::pipe();
    pipe.hset(&redis_key, "node_label", label)
        .hset(&redis_key, "created_at", &now);
    if let Some(exp) = expires_at {
        pipe.hset(&redis_key, "expires_at", exp);
    }
    pipe.query_async(conn).await
}

/// Validate a raw node token against Redis for a given service.
///
/// Hashes the raw token, checks if the key exists, and rejects expired tokens.
pub async fn validate_node_token(
    conn: &mut redis::aio::MultiplexedConnection,
    service_name: &str,
    raw_token: &str,
) -> Result<bool, redis::RedisError> {
    let hash = hash_node_token(raw_token);
    let redis_key = format!("node_tokens:{service_name}:{hash}");
    let exists: bool = conn.exists(&redis_key).await?;
    if !exists {
        return Ok(false);
    }
    // Check expiry
    let expires_at: Option<String> = conn.hget(&redis_key, "expires_at").await?;
    if let Some(exp) = expires_at {
        if !exp.is_empty() {
            if let Ok(exp_time) = chrono::DateTime::parse_from_rfc3339(&exp) {
                if exp_time < chrono::Utc::now() {
                    return Ok(false); // Expired
                }
            }
        }
    }
    Ok(true)
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

/// List all node tokens stored in Redis using SCAN.
///
/// Returns all credentials including expired ones (admin visibility).
/// Expiry is only enforced at auth time, not in listings.
pub async fn list_node_tokens(
    conn: &mut redis::aio::MultiplexedConnection,
) -> Result<Vec<NodeTokenMetadata>, redis::RedisError> {
    let mut cursor: u64 = 0;
    let mut results = Vec::new();
    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("node_tokens:*:*")
            .arg("COUNT")
            .arg(100)
            .query_async(conn)
            .await?;
        for key in &keys {
            let hash: std::collections::HashMap<String, String> =
                conn.hgetall(key).await.unwrap_or_default();
            if hash.is_empty() {
                continue;
            }
            let suffix = key.strip_prefix("node_tokens:").unwrap_or(key);
            let parts: Vec<&str> = suffix.splitn(2, ':').collect();
            if parts.len() != 2 {
                continue;
            }
            let service_name = parts[0].to_string();
            let token_hash = parts[1].to_string();
            let node_label = hash.get("node_label").cloned().filter(|s| !s.is_empty());
            let created_at = hash.get("created_at").cloned().unwrap_or_default();
            let expires_at = hash.get("expires_at").cloned().filter(|s| !s.is_empty());
            results.push(NodeTokenMetadata {
                token_hash,
                service_name,
                node_label,
                created_at,
                expires_at,
            });
        }
        cursor = next_cursor;
        if cursor == 0 {
            break;
        }
    }
    Ok(results)
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
