---
phase: 02-authentication-and-tls
verified: 2026-03-21T20:15:00Z
status: passed
score: 7/7 must-haves verified
re_verification: false
---

# Phase 02: Authentication and TLS Verification Report

**Phase Goal:** Secure all endpoints with API-key auth (HTTP), mTLS (gRPC), and node tokens. Enable TLS on both listeners.
**Verified:** 2026-03-21T20:15:00Z
**Status:** passed
**Re-verification:** No -- initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | HTTPS clients without valid API key get 401 | VERIFIED | `api_key_auth_middleware` in `api_key.rs:122-152` returns `StatusCode::UNAUTHORIZED` when key missing or not found in Redis. Applied to `/v1/tasks` routes in `main.rs:95-98`. Integration test `test_http_no_api_key` and `test_http_invalid_api_key` assert 401. |
| 2 | HTTPS clients with valid API key for wrong service get 401 | VERIFIED | `submit.rs:40-48` checks `client_meta.service_names.contains(&req.service_name)` and returns `GatewayError::Unauthorized`. Integration test `test_http_wrong_service_key` asserts 401. |
| 3 | gRPC connections without valid client certificate fail TLS handshake | VERIFIED | `main.rs:59-67` applies `build_grpc_tls_config` which sets `client_ca_root` via `ServerTlsConfig::client_ca_root()` requiring client certs. Integration test `test_grpc_no_client_cert` verifies rejection. |
| 4 | Node poll requests with invalid token are rejected | VERIFIED | `poll.rs:36-67` extracts Bearer token from metadata, calls `validate_node_token` against Redis per-service, returns `Status::unauthenticated` on failure. Integration tests `test_node_invalid_token` and `test_node_wrong_service_token` verify rejection with `Code::Unauthenticated`. |
| 5 | Gateway serves traffic over TLS when TLS is configured | VERIFIED | HTTP: `main.rs:124-161` creates `TlsAcceptor` from rustls config, manual TLS accept loop. gRPC: `main.rs:59-67` applies tonic `ServerTlsConfig`. Integration test `test_https_tls_connection` verifies HTTPS works and plain HTTP to TLS port fails. |
| 6 | HTTP/2 keepalive pings configured on both gRPC and HTTP | VERIFIED | gRPC: `main.rs:56-57` sets `http2_keepalive_interval(30s)` and `http2_keepalive_timeout(10s)`. HTTP: `main.rs:148-150` sets `keep_alive_interval(30s)` and `keep_alive_timeout(10s)` on hyper_util builder. |
| 7 | Admin can create and revoke API keys and node tokens via POST endpoints | VERIFIED | `admin.rs` implements 4 handlers: `create_api_key`, `revoke_api_key`, `create_node_token`, `revoke_node_token`. Wired to routes in `main.rs:101-117`. Integration test `test_admin_create_api_key` creates key via admin endpoint and uses it to submit a task. |

**Score:** 7/7 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `gateway/src/auth/mod.rs` | Auth module re-exports | VERIFIED | Exports `api_key` and `node_token` submodules (2 lines) |
| `gateway/src/auth/api_key.rs` | API key generation, SHA-256 hashing, Redis CRUD, header extraction, Axum middleware | VERIFIED | 226 lines. `generate_api_key`, `hash_api_key`, `store_api_key`, `lookup_api_key`, `revoke_api_key`, `extract_api_key`, `api_key_auth_middleware`, `ClientMetadata` struct. 8 unit tests. |
| `gateway/src/auth/node_token.rs` | Node token generation, SHA-256 hashing, Redis CRUD, validation | VERIFIED | 107 lines. `generate_node_token`, `hash_node_token`, `store_node_token`, `validate_node_token`, `revoke_node_token`. 4 unit tests. |
| `gateway/src/tls/mod.rs` | TLS module re-exports | VERIFIED | Exports `config` submodule |
| `gateway/src/tls/config.rs` | rustls ServerConfig builders for HTTP and gRPC | VERIFIED | 160 lines. `build_http_tls_config` (h2+http/1.1 ALPN), `build_grpc_tls_config` (mTLS with client CA). 5 tests including cert generation. |
| `gateway/src/config.rs` | Extended config with TLS and admin fields | VERIFIED | `TlsConfig`, `GrpcTlsConfig`, `AdminConfig` structs. Optional TLS on both `GrpcConfig` and `HttpConfig`. Backward-compatible deserialization tested. |
| `gateway/src/http/admin.rs` | Admin API endpoints for key/token management | VERIFIED | 147 lines. Create/revoke for both API keys and node tokens. Proper Redis integration. |
| `gateway/src/main.rs` | TLS-wrapped server startup with keepalive and auth middleware | VERIFIED | 184 lines. TLS accept loops, auth middleware on API routes, admin routes separate, keepalive on both servers. |
| `gateway/src/http/submit.rs` | Service-scoping authorization | VERIFIED | Uses `Extension(ClientMetadata)` to check service authorization before task submission. |
| `gateway/src/grpc/poll.rs` | Node token validation before streaming | VERIFIED | Extracts Bearer token from gRPC metadata, validates against Redis per-service before streaming tasks. |
| `gateway/tests/auth_integration_test.rs` | Integration tests for all auth success criteria | VERIFIED | 733 lines. 12 tests covering AUTH-01 (5 HTTP key tests), AUTH-02 (2 mTLS tests), AUTH-03 (3 node token tests), admin (1 test), INFR-05 (1 TLS test). Uses rcgen for test cert infrastructure. |
| `gateway/src/bin/agent.rs` | Runner agent with auth token support | VERIFIED | 195 lines. `--token` (required), `--ca-cert`, `--tls-skip-verify` flags. Bearer token added to gRPC metadata. TLS auto-detected. |
| `gateway/Cargo.toml` | New dependencies | VERIFIED | rustls 0.23, tokio-rustls 0.26, rustls-pemfile 2.2, sha2 0.10, rand 0.9, hex 0.4, hyper-util 0.1 all present. |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `auth/api_key.rs` | Redis `api_keys:<hash>` | HSET/HGET on SHA-256 hash | WIRED | `store_api_key` uses `redis::pipe().hset("api_keys:{key_hash}", ...)`, `lookup_api_key` uses `conn.hgetall("api_keys:{key_hash}")` |
| `auth/node_token.rs` | Redis `node_tokens:<service>:<hash>` | HSET/EXISTS on SHA-256 hash | WIRED | `store_node_token` uses `"node_tokens:{service_name}:{token_hash}"`, `validate_node_token` uses `conn.exists()` |
| `tls/config.rs` | `rustls::ServerConfig` | rustls-pemfile + builder_with_provider | WIRED | Parses PEM certs, uses `ServerConfig::builder_with_provider(ring)`, sets ALPN |
| `main.rs` | `tls/config.rs` | `build_http_tls_config` and `build_grpc_tls_config` calls | WIRED | Both functions called conditionally when TLS config present |
| `main.rs` | `auth/api_key.rs` | `api_key_auth_middleware` on HTTP router | WIRED | Applied via `axum::middleware::from_fn_with_state` to API routes |
| `http/submit.rs` | `auth/api_key.rs` | `ClientMetadata` from request extensions | WIRED | `Extension(client_meta): Extension<ClientMetadata>` extracted, `service_names` checked |
| `grpc/poll.rs` | `auth/node_token.rs` | `validate_node_token` with token from metadata | WIRED | Bearer token extracted from `request.metadata()`, passed to `validate_node_token` |
| `tests/auth_integration_test.rs` | `auth/api_key.rs` | Creates keys, uses in requests | WIRED | `generate_api_key`, `store_api_key` used in test helpers |
| `tests/auth_integration_test.rs` | `tls/config.rs` | rcgen certs with TLS-enabled gateway | WIRED | `build_grpc_tls_config` and `build_http_tls_config` called in test gateway setup |
| `src/bin/agent.rs` | gRPC metadata | Bearer token added to PollTasks requests | WIRED | `request.metadata_mut().insert("authorization", format!("Bearer {}", cli.token))` |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| AUTH-01 | 02-01, 02-02, 02-03 | HTTPS clients authenticate via API key (bearer token) | SATISFIED | API key middleware (`api_key.rs`), applied to HTTP routes (`main.rs:95-98`), 5 integration tests |
| AUTH-02 | 02-02, 02-03 | gRPC clients authenticate via mTLS (mutual TLS certificates) | SATISFIED | `build_grpc_tls_config` with `client_ca_root` (`tls/config.rs`), applied in `main.rs:59-67`, 2 integration tests |
| AUTH-03 | 02-01, 02-02, 02-03 | Internal nodes authenticate via pre-shared tokens validated on each poll | SATISFIED | `validate_node_token` in `node_token.rs`, called in `poll.rs:53-67`, 3 integration tests |
| INFR-05 | 02-01, 02-02, 02-03 | Gateway supports TLS termination for HTTPS and gRPC | SATISFIED | rustls HTTP TLS (`main.rs:124-161`), tonic mTLS (`main.rs:59-67`), `test_https_tls_connection` |
| INFR-06 | 02-02, 02-03 | Gateway configures HTTP/2 keepalive pings | SATISFIED | gRPC: `main.rs:56-57` (30s/10s), HTTP: `main.rs:148-150` (30s/10s) |

No orphaned requirements found.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| (none) | -- | -- | -- | No TODO/FIXME/placeholder/stub patterns found in any Phase 2 files |

**Compilation:** Project compiles successfully (`cargo check` passes with only 2 unused import warnings).

### Human Verification Required

### 1. TLS Handshake Behavior Under Load

**Test:** Submit multiple concurrent HTTPS requests with valid API keys while TLS is enabled.
**Expected:** All connections complete TLS handshake and receive proper responses.
**Why human:** Cannot verify TLS performance characteristics or connection pooling behavior via static analysis.

### 2. mTLS Certificate Rejection

**Test:** Attempt gRPC connection with a client certificate signed by a different CA.
**Expected:** TLS handshake fails with certificate verification error.
**Why human:** Integration tests only test "no cert" case; wrong-CA cert behavior depends on tonic/rustls runtime behavior.

### 3. Redis Connection Resilience for Auth

**Test:** Restart Redis while gateway is running, then submit authenticated requests.
**Expected:** Gateway recovers auth connection and continues validating keys.
**Why human:** Connection recovery behavior of `MultiplexedConnection` under failure needs runtime testing.

### Gaps Summary

No gaps found. All 7 observable truths are verified with supporting artifacts at all three levels (exists, substantive, wired). All 5 requirements (AUTH-01, AUTH-02, AUTH-03, INFR-05, INFR-06) are satisfied with implementation evidence and integration tests. No anti-patterns or stubs detected. The project compiles cleanly.

---

_Verified: 2026-03-21T20:15:00Z_
_Verifier: Claude (gsd-verifier)_
