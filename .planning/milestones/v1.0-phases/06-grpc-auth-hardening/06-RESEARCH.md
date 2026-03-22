# Phase 6: gRPC Auth Hardening - Research

**Researched:** 2026-03-22
**Domain:** Tonic gRPC authentication middleware, Tower async service layers, per-service authorization
**Confidence:** HIGH

## Summary

Phase 6 adds authentication enforcement to all gRPC RPCs, bringing them to parity with the HTTP side. The core challenge is that tonic's built-in `Interceptor` trait is **synchronous** (`fn(Request<()>) -> Result<Request<()>, Status>`), but the auth validation requires **async** Redis lookups (hash API key, query Redis, return ClientMetadata/validate node token). The solution is to implement custom Tower `Service` layers that wrap the tonic service servers -- these layers are async-capable and functionally equivalent to interceptors but can perform Redis lookups.

The existing codebase already has all the building blocks: `extract_api_key()` for header parsing, `lookup_api_key()` and `validate_node_token()` for Redis validation, `ClientMetadata` and `NodeTokenMetadata` structs for passing auth context, and `poll_tasks` inline auth as a working proof of concept. The work is primarily integration: wrapping `TaskServiceServer` with an API key auth layer and `NodeServiceServer` with a node token auth layer, then refactoring `poll_tasks` inline auth into the shared layer.

**Primary recommendation:** Implement two Tower `Service` wrapper structs (one for API key auth, one for node token auth) that intercept requests, perform async Redis validation, inject auth metadata into tonic request extensions, and either pass through or reject with `Status::unauthenticated`. Apply per-service via `Server::builder().add_service(wrapped_service)`. Service-level authorization (D-07/D-08/D-09) stays in the RPC handlers where the request payload is available.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use tonic interceptors, not inline auth or Tower middleware [NOTE: tonic's built-in Interceptor is sync-only; implementation requires Tower Service layers that functionally serve as interceptors -- see Architecture Patterns below]
- **D-02:** Two interceptors: one for TaskService (API key auth), one for NodeService (node token auth)
- **D-03:** Interceptors take `Arc<AppState>` at construction time, clone `MultiplexedConnection` for Redis access
- **D-04:** Refactor existing inline auth in `poll_tasks` into the NodeService interceptor -- all 4 node RPCs use the same code path
- **D-05:** API keys in gRPC metadata: check `authorization: Bearer <key>` first, fall back to `x-api-key: <key>` -- mirrors `extract_api_key` logic from HTTP
- **D-06:** Two-phase auth: interceptor validates key/token and injects metadata into request extensions; handler checks per-service authorization
- **D-07:** gRPC SubmitTask enforces per-service authorization -- API key must be authorized for the requested service_name (parity with HTTP)
- **D-08:** gRPC GetTaskStatus enforces service scoping -- API key must be authorized for the service that owns the task (requires Redis lookup of task's service)
- **D-09:** Node token auth on report_result: nodes send `x-service-name: <service>` metadata alongside Bearer token; interceptor uses both to validate
- **D-10:** Heartbeat and DrainNode already have `service_name` in proto message -- interceptor extracts from `x-service-name` metadata for consistency
- **D-11:** Generic error messages: return `Status::unauthenticated("unauthorized")` with no hints about what failed; log specific reason at debug level
- **D-12:** Service authorization failures use `Status::permission_denied("unauthorized")` (PERMISSION_DENIED, not UNAUTHENTICATED) -- distinguishes auth vs authz
- **D-13:** No protocol label on `errors_total` metric -- existing `error_type` labels (`auth_api_key`, `auth_node_token`) already distinguish client vs node auth. No breaking changes to dashboards.
- **D-14:** gRPC auth failures increment `errors_total` with same labels as HTTP: `[service, "auth_api_key"]` or `[service, "auth_node_token"]`

### Claude's Discretion
- Exact interceptor function signatures and how to pass metadata through tonic extensions
- Whether to create a shared extraction helper or keep API key and node token extraction separate
- Integration test structure and helper utilities

### Deferred Ideas (OUT OF SCOPE)
- Adding `service_name` field directly to `ReportResultRequest` proto message -- would be cleaner than metadata but is a proto breaking change. Consider for v2.
- Admin API auth hardening (currently unauthenticated per Phase 2 decision D-02) -- separate concern, not in scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AUTH-01 | HTTPS clients authenticate via API key (bearer token) | Already complete on HTTP side; gRPC side must mirror `extract_api_key` + `lookup_api_key` pattern via Tower layer on TaskService |
| AUTH-03 | Internal nodes authenticate via pre-shared tokens validated on each poll | `poll_tasks` has inline auth; refactor into Tower layer on NodeService applied to all 4 RPCs |
| TASK-01 | Client can submit a task via gRPC with opaque payload and receive task ID | `submit_task` in `submit.rs` currently has no auth; Tower layer adds API key validation before handler executes |
| RSLT-01 | Client can poll task status and result by task ID via gRPC | `get_task_status` in `submit.rs` currently has no auth; needs API key auth + service scoping (D-08) |
| NODE-03 | Nodes authenticate with pre-shared tokens scoped to their service | Node token auth via `x-service-name` metadata + Bearer token; Tower layer validates before handler |
| NODE-04 | Nodes report task completion with result payload back to gateway | `report_result` in `poll.rs` currently has no auth; needs node token validation via D-09 pattern |
| NODE-06 | Nodes can signal graceful drain | `drain_node` in `poll.rs` currently has no auth; needs node token validation via D-10 pattern |
</phase_requirements>

## Standard Stack

No new dependencies required. All needed libraries are already in the project.

### Core (Already Present)
| Library | Version | Purpose | Role in This Phase |
|---------|---------|---------|-------------------|
| tonic | 0.14.x | gRPC server | `Request::extensions()` for passing auth metadata to handlers |
| tower | 0.5.x | Middleware framework | `Service` trait impl for async auth layers wrapping tonic services |
| redis-rs | 1.0.x | Redis client | Async `MultiplexedConnection` for API key lookup and node token validation |
| sha2 | (via auth module) | SHA-256 hashing | `hash_api_key` and `hash_node_token` already exist |

### Supporting (Already Present)
| Library | Version | Purpose | Role in This Phase |
|---------|---------|---------|-------------------|
| tracing | 0.1.x | Structured logging | Debug-level auth failure logging per D-11 |
| prometheus | (via metrics module) | Metrics | `errors_total` counter for auth failures per D-14 |

### No New Dependencies
This phase reuses existing auth functions, Redis connections, and middleware patterns. No `cargo add` needed.

## Architecture Patterns

### Critical: Tonic Interceptor Sync Limitation

Tonic's built-in `Interceptor` trait is **synchronous**:

```rust
// tonic::service::Interceptor -- CANNOT await inside this
pub trait Interceptor {
    fn call(&mut self, request: Request<()>) -> Result<Request<()>, Status>;
}
```

This means you **cannot** call `lookup_api_key()` or `validate_node_token()` inside a tonic interceptor because they require `.await` for Redis access. The CONTEXT.md decisions reference "interceptors" semantically -- the implementation uses **Tower Service layers** that achieve the same goal (intercept, validate, inject metadata, reject) but support async operations.

Source: [tonic Interceptor docs](https://docs.rs/tonic/0.14.2/tonic/service/trait.Interceptor.html)

### Pattern 1: Tower Service Auth Layer (Async Interceptor)

**What:** A struct that implements `tower::Service<http::Request<B>>`, wrapping a tonic service server. It intercepts the HTTP/2 request, extracts auth credentials from gRPC metadata (which are HTTP headers), performs async Redis validation, and either forwards the request with auth metadata injected into extensions or returns an error response.

**When to use:** When you need async operations (Redis lookup) in a tonic middleware layer.

**Architecture:**

```
Client Request
    |
    v
Tower Auth Layer (async: extract token -> hash -> Redis lookup -> inject extensions)
    |
    v
Tonic ServiceServer (TaskServiceServer or NodeServiceServer)
    |
    v
RPC Handler (reads extensions for ClientMetadata/NodeTokenMetadata, checks per-service authz)
```

**Example pattern:**

```rust
use std::sync::Arc;
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;
use tonic::body::BoxBody;
use tower::Service;
use http::{Request, Response};

/// Tower auth layer for API key validation on TaskService.
#[derive(Clone)]
pub struct ApiKeyAuthLayer<S> {
    inner: S,
    state: Arc<AppState>,
}

impl<S> Service<http::Request<BoxBody>> for ApiKeyAuthLayer<S>
where
    S: Service<http::Request<BoxBody>, Response = http::Response<BoxBody>>
        + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, req: http::Request<BoxBody>) -> Self::Future {
        // Clone inner service per Tower Service contract
        let mut inner = self.inner.clone();
        let state = self.state.clone();

        Box::pin(async move {
            // Extract API key from headers (same logic as HTTP extract_api_key)
            let raw_key = extract_api_key_from_headers(req.headers());
            match raw_key {
                None => {
                    // Return UNAUTHENTICATED gRPC status
                    Ok(tonic::Status::unauthenticated("unauthorized").into_http())
                }
                Some(key) => {
                    let hash = hash_api_key(&key);
                    let mut conn = state.auth_conn.clone();
                    match lookup_api_key(&mut conn, &hash).await {
                        Ok(Some(meta)) => {
                            // Inject ClientMetadata into request extensions
                            let mut req = req;
                            req.extensions_mut().insert(meta);
                            inner.call(req).await
                        }
                        Ok(None) => {
                            state.metrics.errors_total
                                .with_label_values(&["unknown", "auth_api_key"])
                                .inc();
                            Ok(tonic::Status::unauthenticated("unauthorized").into_http())
                        }
                        Err(_) => {
                            Ok(tonic::Status::internal("internal error").into_http())
                        }
                    }
                }
            }
        })
    }
}
```

**NOTE:** The exact body types may differ. Tonic 0.14 uses `http::Request<tonic::body::BoxBody>` internally. The implementor should check the actual `S::Request` type by examining tonic's `NamedService` and `Service` impls for `TaskServiceServer`.

### Pattern 2: Two-Phase Auth (D-06)

**What:** Phase 1 (Tower layer) validates the credential and injects metadata. Phase 2 (RPC handler) checks per-service authorization using the injected metadata + request payload.

**Why split:** The Tower layer intercepts at the HTTP level and does not have access to the decoded protobuf message. It cannot check which `service_name` the client is requesting. The handler has both the decoded message AND the injected auth metadata, so it can check service authorization.

**Example for gRPC submit_task:**

```rust
// In the RPC handler, after Tower layer has validated API key:
async fn submit_task(
    &self,
    request: Request<SubmitTaskRequest>,
) -> Result<Response<SubmitTaskResponse>, Status> {
    // Phase 2: Read ClientMetadata injected by Tower auth layer
    let client_meta = request.extensions()
        .get::<ClientMetadata>()
        .ok_or_else(|| Status::internal("auth metadata missing"))?
        .clone();

    let req = request.into_inner();

    // D-07: Per-service authorization check
    if !client_meta.service_names.contains(&req.service_name) {
        tracing::debug!(
            key_hash=%client_meta.key_hash,
            requested_service=%req.service_name,
            "gRPC API key not authorized for requested service"
        );
        return Err(Status::permission_denied("unauthorized")); // D-12
    }

    // ... proceed with task submission
}
```

### Pattern 3: Node Token Auth with Service Name from Metadata (D-09/D-10)

**What:** Node-facing RPCs require both a Bearer token AND `x-service-name` metadata. The Tower layer extracts both, validates the token for that service via `validate_node_token()`, and injects `NodeTokenMetadata` or a validated service name into extensions.

**Key detail:** `report_result` does not have `service_name` in the proto message, so D-09 requires `x-service-name` in gRPC metadata. `heartbeat` and `drain_node` DO have `service_name` in the proto, but D-10 says the interceptor extracts from metadata for consistency across all 4 RPCs.

```rust
// Node auth Tower layer extracts:
// 1. Bearer token from "authorization" metadata
// 2. Service name from "x-service-name" metadata
// Then calls validate_node_token(conn, service_name, raw_token)
```

### Pattern 4: Applying Per-Service Tower Layers in main.rs

**Current code (main.rs lines 176-182):**

```rust
grpc_builder
    .add_service(TaskServiceServer::new(
        grpc::GrpcTaskService::new(grpc_state.clone()),
    ))
    .add_service(NodeServiceServer::new(
        grpc::GrpcNodeService::new(grpc_state),
    ))
```

**After change -- wrap each service with its auth layer:**

```rust
let task_svc = TaskServiceServer::new(
    grpc::GrpcTaskService::new(grpc_state.clone()),
);
let node_svc = NodeServiceServer::new(
    grpc::GrpcNodeService::new(grpc_state.clone()),
);

grpc_builder
    .add_service(ApiKeyAuthLayer::new(task_svc, grpc_state.clone()))
    .add_service(NodeTokenAuthLayer::new(node_svc, grpc_state))
```

**Important:** The wrapped service must still implement `tonic::server::NamedService` so the tonic router can route by gRPC service name. This means the wrapper struct needs:

```rust
impl<S: tonic::server::NamedService> tonic::server::NamedService for AuthWrapper<S> {
    const NAME: &'static str = S::NAME;
}
```

### Recommended Project Structure

```
gateway/src/
├── auth/
│   ├── api_key.rs           # Existing: extract_api_key, lookup_api_key, ClientMetadata
│   ├── node_token.rs        # Existing: validate_node_token, NodeTokenMetadata
│   └── mod.rs               # Existing
├── grpc/
│   ├── auth.rs              # NEW: Tower auth layers (ApiKeyAuthLayer, NodeTokenAuthLayer)
│   ├── submit.rs            # MODIFY: Add service authz checks in handlers (D-07, D-08)
│   ├── poll.rs              # MODIFY: Remove inline auth from poll_tasks, add authz in handlers
│   └── mod.rs               # MODIFY: Export auth module
└── main.rs                  # MODIFY: Wrap services with auth layers
```

### Anti-Patterns to Avoid
- **Async work in tonic `Interceptor`:** Will not compile. Use Tower `Service` layer instead.
- **Blocking Redis calls via `block_on`:** Never block the async runtime. The Tower layer is already in async context -- just `.await`.
- **Extracting protobuf fields in the Tower layer:** The layer sees raw HTTP/2 bytes, not decoded protobuf. Leave per-service authorization to the handler.
- **Returning detailed error info:** Per D-11, always `Status::unauthenticated("unauthorized")` -- never reveal whether the key was missing, invalid, or expired.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| API key extraction from gRPC metadata | New extraction logic | Reuse `auth::api_key::extract_api_key()` -- gRPC metadata maps to HTTP headers | Maintains exact parity with HTTP extraction logic |
| API key validation | New Redis lookup logic | Reuse `auth::api_key::lookup_api_key()` | Already handles hash lookup, returns ClientMetadata |
| Node token validation | New validation logic | Reuse `auth::node_token::validate_node_token()` | Already handles hash + service-scoped lookup |
| SHA-256 key hashing | Direct sha2 calls | Reuse `hash_api_key()` / `hash_node_token()` | Consistency with stored hashes |
| gRPC error responses | Custom response construction | `tonic::Status::unauthenticated("unauthorized").into_http()` | Tonic handles correct gRPC status encoding |

**Key insight:** Every auth primitive already exists in the codebase. This phase is pure integration -- wrapping existing functions in Tower layers and wiring them into the gRPC service construction.

## Common Pitfalls

### Pitfall 1: Async Operations in Tonic Interceptors
**What goes wrong:** Tonic's `Interceptor` trait is synchronous. Attempting `.await` inside it fails to compile.
**Why it happens:** The CONTEXT.md says "interceptors" but the tonic API constraint requires Tower layers for async work.
**How to avoid:** Implement `tower::Service` for a custom wrapper struct. This is the standard async middleware pattern in the tonic ecosystem.
**Warning signs:** Compiler error "`.await` is only allowed inside `async` functions and blocks."

### Pitfall 2: Forgetting NamedService on Auth Wrapper
**What goes wrong:** `grpc_builder.add_service(wrapped_service)` fails to compile because the wrapper doesn't implement `tonic::server::NamedService`.
**Why it happens:** Tonic routes gRPC requests by service name (from the `NamedService` trait). Wrapping a service in a Tower layer loses this trait.
**How to avoid:** Manually implement `NamedService` on the wrapper, delegating to the inner service's `NAME` constant.
**Warning signs:** Trait bound error on `add_service`.

### Pitfall 3: Tower Service Clone Contract
**What goes wrong:** Tower's `Service::call` takes `&mut self`, but the service may be called concurrently. The standard pattern is to clone `self.inner` at the start of `call()`.
**Why it happens:** Tower services must be `Clone` for concurrent use. The `call` method should clone the inner service before use.
**How to avoid:** Follow the `tower::Service` clone pattern: `let mut inner = self.inner.clone();` at the start of `call()`.
**Warning signs:** Borrow checker errors about `&mut self` lifetimes in `call()`.

### Pitfall 4: gRPC Metadata vs HTTP Headers
**What goes wrong:** Tonic's `Request::metadata()` is available in RPC handlers but NOT in Tower layers (which see raw `http::Request`). In the Tower layer, use `request.headers()` instead.
**Why it happens:** Tower layers operate at the HTTP level, before tonic deserializes the gRPC envelope. `request.headers()` gives access to the same key-value pairs that tonic exposes as metadata.
**How to avoid:** In Tower layers use `req.headers().get("authorization")`. In RPC handlers use `request.metadata().get("authorization")`. The `extract_api_key()` function already takes `&HeaderMap` so it works in Tower layers directly.
**Warning signs:** Trying to call `.metadata()` on an `http::Request` (it doesn't exist).

### Pitfall 5: Extensions Not Available After into_inner()
**What goes wrong:** Calling `request.into_inner()` consumes the tonic `Request`, dropping extensions. If you read extensions after `into_inner()`, they are gone.
**Why it happens:** `into_inner()` returns just the message body, discarding metadata and extensions.
**How to avoid:** Read extensions BEFORE calling `into_inner()`. Clone what you need: `let meta = request.extensions().get::<ClientMetadata>().cloned();`
**Warning signs:** `None` when calling `.get::<ClientMetadata>()` on extensions.

### Pitfall 6: HTTP Extensions vs Tonic Extensions
**What goes wrong:** Tower layers insert into `http::Request::extensions_mut()` (http crate Extensions). Tonic handlers read from `tonic::Request::extensions()` (tonic Extensions). These are different types but tonic bridges them -- extensions inserted at the HTTP layer ARE available in the tonic Request.
**Why it happens:** Tonic converts `http::Request` to `tonic::Request` internally and carries over extensions.
**How to avoid:** Insert `ClientMetadata` into `http::Request::extensions_mut()` in the Tower layer. It will be available via `tonic::Request::extensions().get::<ClientMetadata>()` in the handler.
**Warning signs:** This should "just work" but verify in integration tests.

## Code Examples

### Existing: API Key Extraction (reuse directly)
```rust
// Source: gateway/src/auth/api_key.rs lines 141-159
pub fn extract_api_key(headers: &HeaderMap) -> Option<String> {
    if let Some(auth) = headers.get("authorization") {
        if let Ok(val) = auth.to_str() {
            if let Some(token) = val.strip_prefix("Bearer ") {
                return Some(token.to_string());
            }
        }
    }
    if let Some(key) = headers.get("x-api-key") {
        if let Ok(val) = key.to_str() {
            return Some(val.to_string());
        }
    }
    None
}
```

### Existing: Inline Auth in poll_tasks (to be refactored out)
```rust
// Source: gateway/src/grpc/poll.rs lines 51-85
// This inline auth code should be REMOVED and replaced by NodeService Tower layer
let raw_token = request
    .metadata()
    .get("authorization")
    .and_then(|v| v.to_str().ok())
    .and_then(|v| v.strip_prefix("Bearer "))
    .map(|s| s.to_string())
    .ok_or_else(|| Status::unauthenticated("unauthorized"))?;
// ... validate_node_token call ...
```

### Existing: HTTP Service Authorization (pattern to replicate in gRPC handlers)
```rust
// Source: gateway/src/http/submit.rs lines 41-50
if !client_meta.service_names.contains(&req.service_name) {
    tracing::debug!(
        key_hash=%client_meta.key_hash,
        requested_service=%req.service_name,
        authorized_services=?client_meta.service_names,
        "API key not authorized for requested service"
    );
    return Err(GatewayError::Unauthorized);
}
```

### New: Reading Extensions in gRPC Handler
```rust
// Source: tonic docs - https://docs.rs/tonic/0.14.2/tonic/struct.Request.html
// Extensions inserted by Tower layer are available via request.extensions()
let client_meta = request.extensions()
    .get::<ClientMetadata>()
    .cloned()
    .ok_or_else(|| Status::internal("auth metadata missing"))?;
```

### Existing: Metric Recording for Auth Failures
```rust
// Source: gateway/src/grpc/poll.rs lines 81-83
self.state.metrics.errors_total
    .with_label_values(&[req.service_name.as_str(), "auth_node_token"])
    .inc();
```

### Existing: Integration Test Pattern
```rust
// Source: gateway/tests/auth_integration_test.rs
// Helper: create API key
async fn create_test_api_key(conn: &mut MultiplexedConnection, services: &[String]) -> String {
    let (raw_key, key_hash) = auth::api_key::generate_api_key();
    auth::api_key::store_api_key(conn, &key_hash, services, None).await.unwrap();
    raw_key
}

// Helper: create node token
async fn create_test_node_token(conn: &mut MultiplexedConnection, service: &str) -> String {
    let (raw_token, token_hash) = auth::node_token::generate_node_token();
    auth::node_token::store_node_token(conn, service, &token_hash, Some("test-node")).await.unwrap();
    raw_token
}

// Test: gRPC client sends metadata
let mut request = tonic::Request::new(SubmitTaskRequest { ... });
request.metadata_mut().insert("authorization", format!("Bearer {api_key}").parse().unwrap());
let result = client.submit_task(request).await;
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Inline auth in each handler | Tower Service layers | Tonic 0.5+ (2021) | Centralized auth, DRY across RPCs |
| Tonic sync Interceptor for auth | Tower async Service layer | Always (Interceptor was never async) | Enables async Redis lookups in middleware |
| `tonic-middleware` crate | Custom Tower Service | N/A | Custom is simpler for two auth variants; no new dependency needed |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust built-in) |
| Config file | gateway/Cargo.toml (test targets) |
| Quick run command | `cargo test -p xgent-gateway --lib` |
| Full suite command | `cargo test -p xgent-gateway --test auth_integration_test -- --ignored` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-01 | gRPC SubmitTask rejects without valid API key | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_submit_no_api_key` | Wave 0 |
| AUTH-01 | gRPC SubmitTask accepts valid API key | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_submit_valid_api_key` | Wave 0 |
| AUTH-03 | gRPC ReportResult rejects without valid node token | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_report_no_token` | Wave 0 |
| AUTH-03 | gRPC Heartbeat rejects without valid node token | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_heartbeat_no_token` | Wave 0 |
| AUTH-03 | gRPC DrainNode rejects without valid node token | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_drain_no_token` | Wave 0 |
| TASK-01 | gRPC SubmitTask with valid API key succeeds | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_submit_valid` | Wave 0 |
| RSLT-01 | gRPC GetTaskStatus rejects without API key | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_status_no_key` | Wave 0 |
| RSLT-01 | gRPC GetTaskStatus rejects wrong-service key (D-08) | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_status_wrong_service` | Wave 0 |
| NODE-03 | gRPC PollTasks rejects invalid token | integration | Already exists in `auth_integration_test.rs` | Exists |
| NODE-04 | gRPC ReportResult accepts valid token | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_report_valid` | Wave 0 |
| NODE-06 | gRPC DrainNode accepts valid token | integration | `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored test_grpc_drain_valid` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway --lib`
- **Per wave merge:** `cargo test -p xgent-gateway --test grpc_auth_test -- --ignored`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `gateway/tests/grpc_auth_test.rs` -- new test file for gRPC auth integration tests (covers all phase requirements)
- Test infrastructure reuse: `start_auth_test_gateway`, `create_test_api_key`, `create_test_node_token` helpers exist in `auth_integration_test.rs` and should be extracted to shared module or duplicated

## Open Questions

1. **HTTP body type in Tower layer**
   - What we know: Tonic 0.14 internally uses `http::Request<tonic::body::BoxBody>` or similar. The exact type depends on the `ServiceServer` impl.
   - What's unclear: The precise `Service` trait bounds needed for the wrapper to compile with `add_service()`.
   - Recommendation: Check `TaskServiceServer`'s `Service` impl signature in generated code or tonic source. Start with `http::Request<BoxBody>` and let the compiler guide you.

2. **Extensions bridge between http and tonic**
   - What we know: Tonic documentation shows extensions set in interceptors are available in handlers. Tower layers insert into `http::Extensions`.
   - What's unclear: Whether tonic 0.14 reliably copies `http::Extensions` to `tonic::Extensions` for all request types including streaming RPCs (PollTasks).
   - Recommendation: Verify with an integration test early. If extensions don't bridge for streaming RPCs, fall back to injecting a custom header.

3. **Metrics service label for unknown service**
   - What we know: D-14 says use `[service, "auth_api_key"]` labels. When auth fails, we may not know the target service.
   - What's unclear: What value to use for `service` when the API key is missing/invalid (before we know which service they wanted).
   - Recommendation: Use `"unknown"` as the service label (matches existing HTTP pattern in `api_key_auth_middleware` line 175).

## Sources

### Primary (HIGH confidence)
- [tonic 0.14.2 Interceptor docs](https://docs.rs/tonic/0.14.2/tonic/service/trait.Interceptor.html) -- Confirmed sync-only trait signature
- [tonic 0.14.2 Request::extensions docs](https://docs.rs/tonic/0.14.2/tonic/struct.Request.html) -- Extensions API for injecting/reading auth metadata
- [tonic tower server example](https://github.com/hyperium/tonic/tree/master/examples/src/tower) -- Official pattern for Tower middleware with tonic
- Codebase: `gateway/src/auth/api_key.rs` -- ClientMetadata, extract_api_key, lookup_api_key, hash_api_key
- Codebase: `gateway/src/auth/node_token.rs` -- NodeTokenMetadata, validate_node_token, hash_node_token
- Codebase: `gateway/src/grpc/poll.rs` lines 51-85 -- Existing inline auth pattern in poll_tasks
- Codebase: `gateway/src/http/submit.rs` lines 41-50 -- Per-service authorization pattern to replicate
- Codebase: `gateway/tests/auth_integration_test.rs` -- Test infrastructure and helpers

### Secondary (MEDIUM confidence)
- [tonic-middleware crate](https://crates.io/crates/tonic-middleware) -- Confirms community pattern of async auth middleware for tonic
- [tonic-async-interceptor crate](https://docs.rs/tonic-async-interceptor/latest/tonic_async_interceptor/) -- Validates that async interceptors require Tower layer approach
- Phase 2 Research: `.planning/phases/02-authentication-and-tls/02-RESEARCH.md` lines 292-296 -- Previously identified the sync interceptor pitfall

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- No new dependencies, all libraries already in use
- Architecture: HIGH -- Tower Service layer pattern is well-documented in tonic ecosystem; existing codebase provides all auth primitives
- Pitfalls: HIGH -- Sync interceptor limitation is compiler-enforced and well-documented; extensions bridging is the one medium-confidence area
- Integration tests: HIGH -- Existing test infrastructure in auth_integration_test.rs provides proven helpers and patterns

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable domain, no expected breaking changes)
