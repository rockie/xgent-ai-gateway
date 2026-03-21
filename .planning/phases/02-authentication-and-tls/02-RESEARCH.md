# Phase 2: Authentication and TLS - Research

**Researched:** 2026-03-21
**Domain:** TLS termination, API key auth, mTLS, node token auth, HTTP/2 keepalive
**Confidence:** HIGH

## Summary

Phase 2 layers authentication and encryption onto the existing dual-port gateway from Phase 1. The three auth models (API key for HTTPS, mTLS for gRPC, pre-shared tokens for nodes) each use different mechanisms but share a common pattern: middleware intercepts requests before handlers, validates credentials against Redis or TLS state, and rejects unauthorized requests with minimal information leakage.

The TLS stack is entirely rustls-based (no OpenSSL). Tonic has built-in `ServerTlsConfig` with mTLS support via `client_ca_root()`. Axum's HTTP side needs either the `axum-server` crate (0.8.0, compatible with axum 0.8) or a low-level `tokio-rustls` TlsAcceptor loop -- the low-level approach gives more control and avoids an extra dependency, matching the existing manual `TcpListener::bind` pattern in `main.rs`. HTTP/2 keepalive is built into tonic's `Server::builder()` and can be configured on the hyper side for the HTTP listener.

**Primary recommendation:** Use tonic's built-in `ServerTlsConfig` for gRPC mTLS, low-level `tokio-rustls::TlsAcceptor` for HTTP TLS (consistent with existing pattern), Redis HGET on SHA-256 hashes for API key lookup, and Axum middleware layers for API key and node token validation.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- D-01: API keys stored in a Redis hash, keyed by SHA-256 hash of the key
- D-02: Each key maps to client metadata including authorized service names (per-service scoping)
- D-03: Gateway hashes incoming key with SHA-256 and looks up the hash in Redis -- raw keys never stored
- D-04: Consistent with Phase 1's Redis-for-everything pattern; keys survive restarts
- D-05: Accept API keys via both `Authorization: Bearer <key>` header and `X-API-Key: <key>` header
- D-06: If both headers are present, prefer `Authorization: Bearer`
- D-07: Admin API endpoint (POST /v1/admin/api-keys) to create and revoke keys
- D-08: Gateway generates the key, returns it once on creation -- cannot be retrieved again
- D-09: Key creation requires specifying which services the key is authorized for
- D-10: Always return generic `401 Unauthorized` with no detail about whether key was missing, invalid, expired, or unauthorized for the service
- D-11: Log the specific failure reason server-side for debugging (at debug/trace level)

### Claude's Discretion
- mTLS certificate handling: CA trust chain setup, client cert validation with rustls, rcgen for dev/test certs
- Node token design: token format, per-service scoping mechanism, storage in Redis, validation on each poll
- TLS configuration: cert/key file paths in config, separate TLS configs per port, rustls ServerConfig setup
- HTTP/2 keepalive: ping interval, timeout values, tonic and hyper keepalive configuration
- Admin API authentication (how to secure the admin endpoints themselves)
- Tower middleware vs Axum extractors for auth layer placement
- Whether to add TLS to Redis connection (rediss://) in this phase or defer

### Deferred Ideas (OUT OF SCOPE)
- Admin API authentication -- Phase 2 creates the endpoints; securing them deferred to Phase 3
- API key rotation without downtime -- v2 requirement (EAUTH-02)
- Node authentication via mTLS certificates -- v2 requirement (EAUTH-01)
- Redis TLS (rediss://) -- may add in Phase 5
- Single-port co-hosting -- still deferred
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AUTH-01 | HTTPS clients authenticate via API key (bearer token) | SHA-256 hash lookup in Redis via HGET; Axum middleware extracts from Authorization/X-API-Key headers; per-service scoping validates against client metadata |
| AUTH-02 | gRPC clients authenticate via mTLS (mutual TLS certificates) | Tonic `ServerTlsConfig::new().identity().client_ca_root()` enforces client cert at TLS handshake level; rejected connections never reach handlers |
| AUTH-03 | Internal nodes authenticate via pre-shared tokens validated on each poll | Token stored as SHA-256 hash in Redis per-service; tonic interceptor extracts from metadata; validated before streaming begins |
| INFR-05 | Gateway supports TLS termination for HTTPS and gRPC | rustls 0.23.x + tokio-rustls 0.26.x for HTTP; tonic built-in TLS for gRPC; separate TLS configs per port in GatewayConfig |
| INFR-06 | Gateway configures HTTP/2 keepalive pings to prevent silent connection death | tonic `http2_keepalive_interval` + `http2_keepalive_timeout`; hyper-util `auto::Builder` keepalive settings for HTTP side |
</phase_requirements>

## Standard Stack

### Core (new dependencies for Phase 2)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| rustls | 0.23.37 | TLS implementation | Pure Rust, no OpenSSL. Project constraint. Already in CLAUDE.md stack. |
| tokio-rustls | 0.26.4 | Async TLS acceptor | Bridges rustls into tokio async I/O. Required for HTTP TLS listener. |
| rustls-pemfile | 2.2.0 | PEM file parsing | Parse cert/key PEM files into rustls types. Standard companion to rustls. |
| rcgen | 0.14.7 | Dev cert generation | Generate self-signed CA + certs for testing mTLS. Dev/test only. |
| sha2 | 0.10.9 | SHA-256 hashing | Hash API keys before storage/lookup. RustCrypto standard. |
| rand | 0.10.0 | Secure random gen | Generate cryptographically random API keys and node tokens. |
| hyper-util | 0.1.x | HTTP server builder | Low-level HTTP/2 connection builder with keepalive support. Already a transitive dep of axum. |

### Already Present (from Phase 1)
| Library | Version | Purpose |
|---------|---------|---------|
| tonic | 0.14.5 | gRPC server -- has built-in TLS/mTLS via `tls_config()` |
| axum | 0.8.8 | HTTP server -- needs TLS acceptor wrapping |
| redis | 1.0.x | API key and node token storage |
| tower | 0.5.x | Middleware layers for auth |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Low-level tokio-rustls for HTTP | axum-server 0.8.0 | axum-server is simpler but hides control. Low-level approach matches existing `TcpListener::bind` pattern in main.rs and allows custom TLS error handling. |
| sha2 for key hashing | ring, blake3 | sha2 (RustCrypto) is lighter, no C dependencies, SHA-256 is the standard for key hashing. ring is overkill. |
| rand for key generation | uuid | rand gives 32-byte cryptographic randomness encoded as hex/base64. uuid v4 is only 122 random bits -- too short for API keys. |

**Installation (additions to gateway/Cargo.toml):**
```toml
# TLS
rustls = { version = "0.23", default-features = false, features = ["ring", "logging", "std", "tls12"] }
tokio-rustls = "0.26"
rustls-pemfile = "2.2"

# Auth
sha2 = "0.10"
rand = "0.10"

# gRPC TLS (enable tonic's tls feature)
tonic = { version = "0.14", features = ["tls"] }

# Dev/test only
[dev-dependencies]
rcgen = "0.14"
```

## Architecture Patterns

### Recommended Project Structure (new files)
```
gateway/src/
├── auth/
│   ├── mod.rs           # Re-exports
│   ├── api_key.rs       # API key middleware, Redis lookup, key generation
│   ├── node_token.rs    # Node token validation, tonic interceptor
│   └── mtls.rs          # mTLS config builder, client cert extraction (if needed)
├── tls/
│   ├── mod.rs           # Re-exports
│   └── config.rs        # TLS config loading, rustls ServerConfig builders
├── http/
│   ├── admin.rs         # POST /v1/admin/api-keys (create/revoke)
│   └── ...existing...
├── config.rs            # Extended with TlsConfig fields
├── state.rs             # Extended with auth state (Redis connection for key lookups)
└── error.rs             # Extended with Unauthorized variant
```

### Pattern 1: API Key Authentication Middleware (Axum)
**What:** Axum middleware layer that extracts API key from headers, hashes it, looks up in Redis, and validates service authorization.
**When to use:** All HTTP routes except admin endpoints (which are unsecured in Phase 2 per deferred decision).
**Example:**
```rust
// Source: Axum middleware patterns + CONTEXT.md decisions D-01..D-11
use axum::{extract::Request, middleware::Next, response::Response};
use sha2::{Sha256, Digest};

pub async fn api_key_auth(
    State(state): State<Arc<AppState>>,
    request: Request,
    next: Next,
) -> Result<Response, StatusCode> {
    // D-05/D-06: Extract key from Authorization: Bearer or X-API-Key
    let api_key = extract_api_key(request.headers())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    // D-03: Hash the key with SHA-256
    let key_hash = hex::encode(Sha256::digest(api_key.as_bytes()));

    // D-01: Lookup hash in Redis HGET
    let client_meta: Option<ClientMetadata> = state
        .auth_store
        .get_api_key_metadata(&key_hash)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let meta = client_meta.ok_or_else(|| {
        // D-11: Log specific reason at debug level
        tracing::debug!("API key not found");
        // D-10: Return generic 401
        StatusCode::UNAUTHORIZED
    })?;

    // D-02/D-09: Inject client metadata for service-scoping check in handler
    request.extensions_mut().insert(meta);
    Ok(next.run(request).await)
}
```

### Pattern 2: mTLS for gRPC (Tonic ServerTlsConfig)
**What:** Tonic's built-in TLS with client certificate validation. Rejects connections at TLS handshake if no valid client cert.
**When to use:** gRPC listener setup in main.rs.
**Example:**
```rust
// Source: docs.rs/tonic ServerTlsConfig API
use tonic::transport::{Identity, Certificate, ServerTlsConfig};

let cert = std::fs::read_to_string(&config.grpc.tls.cert_path)?;
let key = std::fs::read_to_string(&config.grpc.tls.key_path)?;
let client_ca = std::fs::read_to_string(&config.grpc.tls.client_ca_path)?;

let tls_config = ServerTlsConfig::new()
    .identity(Identity::from_pem(&cert, &key))
    .client_ca_root(Certificate::from_pem(&client_ca))
    .client_auth_optional(false);  // Require client cert

tonic::transport::Server::builder()
    .tls_config(tls_config)?
    .http2_keepalive_interval(Some(Duration::from_secs(30)))
    .http2_keepalive_timeout(Some(Duration::from_secs(10)))
    .add_service(TaskServiceServer::new(...))
    .add_service(NodeServiceServer::new(...))
    .serve(grpc_addr)
    .await?;
```

### Pattern 3: Node Token Validation (Tonic Interceptor)
**What:** Tonic interceptor that validates node auth tokens from gRPC metadata before the handler runs.
**When to use:** Applied to NodeServiceServer only (not TaskServiceServer -- clients use mTLS).
**Example:**
```rust
// Source: tonic interceptor pattern
use tonic::{Request, Status};

fn node_auth_interceptor(
    state: Arc<AppState>,
) -> impl Fn(Request<()>) -> Result<Request<()>, Status> + Clone {
    move |req: Request<()>| {
        let token = req.metadata()
            .get("authorization")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
            .ok_or_else(|| Status::unauthenticated("missing auth token"))?;

        // Hash and validate against Redis (blocking here is an issue --
        // use InterceptedService with async or validate in handler)
        // See Pitfall 1 below for async interceptor pattern
        Ok(req)
    }
}
```

### Pattern 4: HTTP TLS with tokio-rustls (Low-Level)
**What:** Wrap the existing TCP listener with a TLS acceptor for HTTPS.
**When to use:** HTTP listener in main.rs, replacing plain `axum::serve`.
**Example:**
```rust
// Source: axum low-level-rustls example (github.com/tokio-rs/axum)
use tokio_rustls::TlsAcceptor;
use rustls::ServerConfig;

let tls_config = build_http_tls_config(&config)?;  // No client auth
let tls_acceptor = TlsAcceptor::from(Arc::new(tls_config));
let tcp_listener = TcpListener::bind(&http_addr).await?;

loop {
    let (tcp_stream, addr) = tcp_listener.accept().await?;
    let acceptor = tls_acceptor.clone();
    let app = app.clone();

    tokio::spawn(async move {
        let Ok(tls_stream) = acceptor.accept(tcp_stream).await else {
            tracing::debug!(%addr, "TLS handshake failed");
            return;
        };
        let io = hyper_util::rt::TokioIo::new(tls_stream);
        let service = hyper::service::service_fn(move |req| {
            app.clone().call(req)
        });
        let _ = hyper_util::server::conn::auto::Builder::new(
            hyper_util::rt::TokioExecutor::new()
        )
        .serve_connection_with_upgrades(io, service)
        .await;
    });
}
```

### Pattern 5: Redis Key Schema for Auth
**What:** Redis hash structure for API keys and node tokens.
**When to use:** All auth lookups.
```
# API keys: single hash, keyed by SHA-256(raw_key)
HSET api_keys:<sha256_hex> service_names "svc1,svc2" created_at "2026-03-21T..."
HGET api_keys:<sha256_hex> service_names

# Node tokens: per-service, keyed by SHA-256(raw_token)
HSET node_tokens:<service_name>:<sha256_hex> node_label "worker-1" created_at "..."
HEXISTS node_tokens:<service_name>:<sha256_hex>
```

### Anti-Patterns to Avoid
- **Storing raw API keys:** Never store unhashed keys in Redis. Always SHA-256 before storage.
- **Detailed auth error messages:** Never return "key expired" or "wrong service" to clients. Always generic 401.
- **Synchronous interceptors for async operations:** Tonic interceptors are synchronous -- cannot do async Redis lookups. Use `InterceptedService` wrapper or validate in the handler method instead.
- **Shared TLS config for both ports:** gRPC needs mTLS (client cert required), HTTP does not. Separate `ServerConfig` instances are required.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| PEM file parsing | Custom PEM parser | rustls-pemfile 2.2 | PEM format has edge cases (multiple certs in chain, different key formats). Library handles all variants. |
| TLS configuration | Manual rustls config from scratch | tonic's `ServerTlsConfig` for gRPC | Tonic wraps rustls correctly with proper ALPN, cipher suites, and protocol settings. |
| Cryptographic key generation | `uuid::Uuid::new_v4()` for API keys | `rand::rngs::OsRng` + 32 bytes | UUID v4 has only 122 bits of randomness. API keys need 256 bits minimum. |
| SHA-256 hashing | Hand-rolled or via ring | sha2 crate from RustCrypto | Pure Rust, well-audited, no C dependencies, compatible with musl static builds. |
| Certificate generation for tests | OpenSSL CLI commands | rcgen 0.14 | Generates CA + client/server certs programmatically in Rust tests. No external tool dependency. |
| HTTP/2 keepalive | Custom ping frames | tonic's built-in `http2_keepalive_interval` | Already implemented correctly in tonic/hyper. Just configure the interval. |

**Key insight:** TLS and cryptography are domains where custom code introduces vulnerabilities. Every component here has a well-tested Rust crate.

## Common Pitfalls

### Pitfall 1: Async Operations in Tonic Interceptors
**What goes wrong:** Tonic's `Interceptor` trait is synchronous (`fn(Request<()>) -> Result<Request<()>, Status>`). Attempting async Redis lookups inside it will fail to compile.
**Why it happens:** Tonic interceptors run in the request path before async context is fully established.
**How to avoid:** Two options: (a) Validate tokens in the `poll_tasks` handler itself (simplest), or (b) use Tower service layers with async `Service` impl that wraps the tonic service. Option (a) is recommended for this project -- the handler already has access to `AppState` with Redis.
**Warning signs:** Trying to `.await` inside an interceptor function.

### Pitfall 2: rustls Certificate Chain Order
**What goes wrong:** rustls requires certificates in the correct order: leaf cert first, then intermediates, then (optionally) root. Wrong order causes `TLS handshake failed` with no useful error.
**Why it happens:** Some tools (OpenSSL) are lenient about order; rustls is strict.
**How to avoid:** Document cert file format requirements. Use `rustls_pemfile::certs()` which returns certs in file order -- ensure PEM files are correctly ordered. Test with rcgen-generated certs first.
**Warning signs:** TLS handshake failures with valid-looking certificates.

### Pitfall 3: Missing ALPN Protocol Negotiation
**What goes wrong:** HTTP/2 requires ALPN negotiation during TLS handshake. Without `h2` in ALPN protocols, clients fall back to HTTP/1.1 or fail entirely. gRPC requires HTTP/2.
**Why it happens:** Default rustls `ServerConfig` does not set ALPN protocols.
**How to avoid:** Always set `config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()]` for HTTP, and `vec![b"h2".to_vec()]` for gRPC. Tonic's `ServerTlsConfig` handles this automatically for gRPC.
**Warning signs:** `connection closed` errors from gRPC clients; HTTP/2 clients falling back to HTTP/1.1.

### Pitfall 4: Timing Attacks on Key Comparison
**What goes wrong:** Comparing API key hashes with `==` leaks timing information about how many bytes matched.
**Why it happens:** String equality short-circuits on first mismatch.
**How to avoid:** Not a real concern here because we use the hash as a Redis key for HGET lookup, not a comparison. The key either exists or doesn't. No timing leak from Redis `HGET`. This is a non-issue with the hash-as-key design (D-01).
**Warning signs:** None -- the design avoids this by construction.

### Pitfall 5: TLS Config Not Optional for Development
**What goes wrong:** Requiring TLS configuration makes local development painful. Developers need cert files just to run the gateway.
**Why it happens:** Making TLS mandatory without a dev escape hatch.
**How to avoid:** Make TLS optional in config (default: disabled). When TLS fields are absent, serve plain HTTP/gRPC as Phase 1 does. Only enforce TLS when cert paths are provided. This preserves backward compatibility with existing tests.
**Warning signs:** Integration tests breaking because they don't have TLS certs.

### Pitfall 6: Service Scoping Race Between Auth and Handler
**What goes wrong:** API key auth validates the key exists but the service_name check happens in the handler. If not coordinated, a valid key could submit to an unauthorized service.
**Why it happens:** Auth middleware and handler run in separate layers with different responsibilities.
**How to avoid:** Auth middleware injects `ClientMetadata` (including authorized services) into request extensions. Handler extracts it and checks `service_name` against the authorized list before processing. This is a two-step pattern: authenticate (middleware) then authorize (handler).
**Warning signs:** Tests that use a valid key but submit to a wrong service -- should get 401, not success.

## Code Examples

### Generating API Keys
```rust
// Source: rand + sha2 crate patterns
use rand::RngCore;
use sha2::{Sha256, Digest};

fn generate_api_key() -> (String, String) {
    let mut key_bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut key_bytes);
    let raw_key = hex::encode(key_bytes);  // 64-char hex string
    let key_hash = hex::encode(Sha256::digest(raw_key.as_bytes()));
    (raw_key, key_hash)  // Return raw (once) and hash (for storage)
}
```

### Redis API Key Storage
```rust
// Source: redis-rs 1.0 patterns
use redis::AsyncCommands;

async fn store_api_key(
    conn: &mut redis::aio::MultiplexedConnection,
    key_hash: &str,
    services: &[String],
) -> Result<(), redis::RedisError> {
    let redis_key = format!("api_keys:{}", key_hash);
    conn.hset(&redis_key, "service_names", services.join(",")).await?;
    conn.hset(&redis_key, "created_at", chrono::Utc::now().to_rfc3339()).await?;
    Ok(())
}

async fn lookup_api_key(
    conn: &mut redis::aio::MultiplexedConnection,
    key_hash: &str,
) -> Result<Option<Vec<String>>, redis::RedisError> {
    let redis_key = format!("api_keys:{}", key_hash);
    let services: Option<String> = conn.hget(&redis_key, "service_names").await?;
    Ok(services.map(|s| s.split(',').map(String::from).collect()))
}
```

### TLS Config Extension
```rust
// Extends existing GatewayConfig in config.rs
#[derive(Debug, Deserialize, Clone)]
pub struct TlsConfig {
    pub cert_path: String,
    pub key_path: String,
}

#[derive(Debug, Deserialize, Clone)]
pub struct GrpcTlsConfig {
    #[serde(flatten)]
    pub server: TlsConfig,
    pub client_ca_path: String,  // For mTLS
}

// Updated GrpcConfig
#[derive(Debug, Deserialize, Clone)]
pub struct GrpcConfig {
    pub enabled: bool,
    pub listen_addr: String,
    pub tls: Option<GrpcTlsConfig>,  // None = no TLS (dev mode)
}

// Updated HttpConfig
#[derive(Debug, Deserialize, Clone)]
pub struct HttpConfig {
    pub enabled: bool,
    pub listen_addr: String,
    pub tls: Option<TlsConfig>,  // None = no TLS (dev mode)
}
```

### Building rustls ServerConfig for HTTP
```rust
// Source: axum low-level-rustls example + rustls docs
use rustls::ServerConfig;
use rustls_pemfile::{certs, private_key};
use tokio_rustls::rustls::pki_types::{CertificateDer, PrivateKeyDer};

fn build_http_tls_config(tls: &TlsConfig) -> Result<ServerConfig, Box<dyn std::error::Error>> {
    let cert_file = std::fs::File::open(&tls.cert_path)?;
    let key_file = std::fs::File::open(&tls.key_path)?;

    let certs: Vec<CertificateDer<'static>> = certs(&mut std::io::BufReader::new(cert_file))
        .collect::<Result<Vec<_>, _>>()?;
    let key: PrivateKeyDer<'static> = private_key(&mut std::io::BufReader::new(key_file))?
        .ok_or("no private key found in file")?;

    let mut config = ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(certs, key)?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(config)
}
```

### rcgen Test Certificate Generation
```rust
// Source: rcgen crate patterns
#[cfg(test)]
fn generate_test_ca_and_certs() -> (String, String, String, String, String) {
    use rcgen::{CertificateParams, KeyPair};

    // Generate CA
    let ca_key = KeyPair::generate().unwrap();
    let mut ca_params = CertificateParams::new(vec!["Test CA".to_string()]).unwrap();
    ca_params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Unconstrained);
    let ca_cert = ca_params.self_signed(&ca_key).unwrap();

    // Generate server cert signed by CA
    let server_key = KeyPair::generate().unwrap();
    let server_params = CertificateParams::new(vec!["localhost".to_string()]).unwrap();
    let server_cert = server_params.signed_by(&server_key, &ca_cert, &ca_key).unwrap();

    // Generate client cert signed by CA (for mTLS)
    let client_key = KeyPair::generate().unwrap();
    let client_params = CertificateParams::new(vec!["test-client".to_string()]).unwrap();
    let client_cert = client_params.signed_by(&client_key, &ca_cert, &ca_key).unwrap();

    (
        ca_cert.pem(),
        server_cert.pem(), server_key.serialize_pem(),
        client_cert.pem(), client_key.serialize_pem(),
    )
}
```

### HTTP/2 Keepalive Configuration
```rust
// gRPC side (tonic): built-in
tonic::transport::Server::builder()
    .http2_keepalive_interval(Some(Duration::from_secs(30)))
    .http2_keepalive_timeout(Some(Duration::from_secs(10)))
    // ...

// HTTP side (hyper-util): on the connection builder
hyper_util::server::conn::auto::Builder::new(TokioExecutor::new())
    .http2()
    .keep_alive_interval(Some(Duration::from_secs(30)))
    .keep_alive_timeout(Duration::from_secs(10))
    .serve_connection_with_upgrades(io, service)
    .await;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `rustls_pemfile::read_one()` loop | `certs()` / `private_key()` iterators | rustls-pemfile 2.0 | Simpler API, returns typed iterators |
| `rustls::Certificate` / `rustls::PrivateKey` | `pki_types::CertificateDer` / `PrivateKeyDer` | rustls 0.22+ | Types moved to `rustls-pki-types` crate, shared across ecosystem |
| `rcgen::Certificate::from_params()` | `CertificateParams::self_signed(&key)` / `.signed_by()` | rcgen 0.12+ | Separated key generation from cert signing |
| tonic custom TLS setup | `ServerTlsConfig` with `client_ca_root` | tonic 0.8+ | Built-in mTLS support, no manual rustls config needed for gRPC |
| `rand::thread_rng()` | `rand::rng()` | rand 0.9+ | Simplified API, `rng()` is the new entry point |
| `sha2 0.9` with `Digest::new()` | `sha2 0.10` with same API | 2022 | Minor version bump, API stable |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + integration tests |
| Config file | gateway/Cargo.toml `[dev-dependencies]` |
| Quick run command | `cargo test -p xgent-gateway --lib` |
| Full suite command | `cargo test -p xgent-gateway --test integration_test -- --ignored` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-01 | HTTPS request without valid API key returns 401 | integration | `cargo test -p xgent-gateway --test integration_test test_http_no_api_key -- --ignored` | Wave 0 |
| AUTH-01 | HTTPS request with valid key for wrong service returns 401 | integration | `cargo test -p xgent-gateway --test integration_test test_http_wrong_service_key -- --ignored` | Wave 0 |
| AUTH-01 | HTTPS request with valid key succeeds | integration | `cargo test -p xgent-gateway --test integration_test test_http_valid_api_key -- --ignored` | Wave 0 |
| AUTH-02 | gRPC connection without client cert rejected at TLS handshake | integration | `cargo test -p xgent-gateway --test integration_test test_grpc_no_client_cert -- --ignored` | Wave 0 |
| AUTH-02 | gRPC connection with valid client cert succeeds | integration | `cargo test -p xgent-gateway --test integration_test test_grpc_valid_mtls -- --ignored` | Wave 0 |
| AUTH-03 | Node poll with invalid token rejected | integration | `cargo test -p xgent-gateway --test integration_test test_node_invalid_token -- --ignored` | Wave 0 |
| AUTH-03 | Node poll with wrong-service token rejected | integration | `cargo test -p xgent-gateway --test integration_test test_node_wrong_service_token -- --ignored` | Wave 0 |
| INFR-05 | Gateway serves HTTPS with valid TLS | integration | `cargo test -p xgent-gateway --test integration_test test_https_tls_connection -- --ignored` | Wave 0 |
| INFR-06 | HTTP/2 keepalive pings configured | unit | `cargo test -p xgent-gateway --lib test_keepalive_config` | Wave 0 |
| AUTH-01 | API key SHA-256 hash matches Redis lookup | unit | `cargo test -p xgent-gateway --lib test_api_key_hash_lookup` | Wave 0 |
| AUTH-01 | Key extraction from Authorization and X-API-Key headers | unit | `cargo test -p xgent-gateway --lib test_extract_api_key` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway --lib`
- **Per wave merge:** `cargo test -p xgent-gateway` (lib + integration with Redis)
- **Phase gate:** Full suite green + manual curl verification of TLS endpoints

### Wave 0 Gaps
- [ ] `gateway/tests/integration_test.rs` -- extend with TLS and auth test cases (requires rcgen for test certs)
- [ ] `gateway/src/auth/` unit tests for key extraction, hashing, service scoping
- [ ] rcgen added to `[dev-dependencies]` for programmatic cert generation in tests
- [ ] hex crate needed for SHA-256 hex encoding (or use `format!("{:x}", ...)`)

## Open Questions

1. **Admin endpoint auth in Phase 2**
   - What we know: D-07 says admin API creates/revokes keys. Deferred decision says "securing them deferred to Phase 3."
   - What's unclear: Should admin endpoints be completely unauthenticated in Phase 2, or require a bootstrap admin token from config?
   - Recommendation: Use a config-file-based admin token (`admin.token` in TOML) as a simple bootstrap. This prevents accidental exposure while keeping it simple. Phase 3 can replace with proper admin auth.

2. **Node token provisioning**
   - What we know: Tokens are pre-shared and per-service. They need to exist in Redis before nodes can poll.
   - What's unclear: How are tokens initially created? Via admin API? Seeded from config?
   - Recommendation: Add a node token create endpoint to the admin API alongside API key management. Same pattern: generate, hash, store, return once.

3. **hex encoding dependency**
   - What we know: SHA-256 produces bytes. Need hex string for Redis keys.
   - What's unclear: Use `hex` crate or inline formatting.
   - Recommendation: Use the `hex` crate (0.4.x) -- it's tiny and avoids manual formatting bugs. Alternatively, `format!("{:02x}", byte)` loop works but is less readable.

## Sources

### Primary (HIGH confidence)
- [tonic ServerTlsConfig docs](https://docs.rs/tonic/latest/tonic/transport/server/struct.ServerTlsConfig.html) - mTLS API: `identity()`, `client_ca_root()`, `client_auth_optional()`
- [tonic Server docs](https://docs.rs/tonic/latest/tonic/transport/server/struct.Server.html) - HTTP/2 keepalive: `http2_keepalive_interval()`, `http2_keepalive_timeout()`
- [axum low-level-rustls example](https://github.com/tokio-rs/axum/blob/main/examples/low-level-rustls/src/main.rs) - TLS acceptor pattern for HTTP
- [crates.io](https://crates.io) - Version verification: rustls 0.23.37, tokio-rustls 0.26.4, rcgen 0.14.7, sha2 0.10.9, rand 0.10.0, rustls-pemfile 2.2.0, axum-server 0.8.0

### Secondary (MEDIUM confidence)
- [axum-server crate](https://docs.rs/axum-server/0.8.0) - Alternative to low-level TLS approach
- [tonic issue #1615](https://github.com/hyperium/tonic/issues/1615) - Custom rustls config with tonic

### Tertiary (LOW confidence)
- hyper-util HTTP/2 keepalive settings - API verified via search but not tested. tonic side is well-documented; HTTP side needs validation during implementation.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All crates verified on crates.io with current versions. rustls + tonic TLS is well-documented.
- Architecture: HIGH - Patterns verified against official examples (axum low-level-rustls, tonic ServerTlsConfig docs). Existing codebase structure understood.
- Pitfalls: HIGH - Async interceptor limitation is well-known. ALPN and cert chain issues are documented rustls behaviors.
- Validation: MEDIUM - Test patterns are standard cargo test. Integration tests need Redis + TLS certs which adds setup complexity.

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable ecosystem, 30 days)
