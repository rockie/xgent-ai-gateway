# Phase 7: Integration Fixes, Sample Service, and Cleanup - Research

**Researched:** 2026-03-22
**Domain:** Integration bug fixes, sample service binary, mTLS identity mapping, tech debt cleanup
**Confidence:** HIGH

## Summary

Phase 7 is a consolidation phase that closes gaps identified in the v1.0 milestone audit. The work breaks into four domains: (1) fixing the `in_flight_tasks` counter decrement bug, (2) adding `callback_url` to the gRPC proto, (3) adding HTTP keepalive to the plain HTTP path, (4) building a sample service binary for end-to-end testing, (5) implementing mTLS client cert identity mapping via `gateway.toml`, (6) improving the reaper integration test, and (7) resolving remaining tech debt. Most items are surgical edits to existing code. The mTLS identity mapping is the only architecturally meaningful addition.

The codebase is well-structured with clear patterns established over 6 phases. All fixes have precise insertion points identified in the CONTEXT.md canonical references. The `update_in_flight_tasks` function already exists and just needs to be called with `-1` in `report_result`. The proto change is a single field addition. The plain HTTP keepalive requires switching from `axum::serve` to the `hyper_util` manual builder pattern (already used in the TLS path).

**Primary recommendation:** Execute in 3 plans -- (1) proto + counter + keepalive fixes, (2) mTLS identity mapping + reaper test, (3) sample service + tech debt verification.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Sample service echo by default -- returns payload as-is. If metadata contains `simulate_delay_ms`, sleeps that duration before responding (combo mode)
- **D-02:** Lives in `examples/` as a standalone single-file example, not a workspace crate or binary target
- **D-03:** Uses hyper directly for the HTTP server -- minimal, no framework overhead
- **D-04:** Plain HTTP only, no TLS support needed -- this is a demo/testing tool
- **D-05:** No changes to revoke routes -- `POST /v1/admin/api-keys/revoke` and `POST /v1/admin/node-tokens/revoke` are accepted as-is
- **D-06:** Keep `200 OK` with JSON response body `{"status": "revoked"}` -- no change to response format
- **D-07:** Verify each audit item during execution -- skip items confirmed already fixed or stale
- **D-08:** `cleanup_redis_keys` stub -- keep as-is, it's harmless and tests pass
- **D-09:** Reaper integration test -- add a full-loop integration test that spawns the reaper, waits, and checks that a timed-out task's status changes to failed
- **D-10:** mTLS identity gap (audit item 5) -- addressed by D-11/D-12 below instead of deferring
- **D-11:** Add a `[grpc.mtls_identity]` section (or similar) in `gateway.toml` that maps certificate fingerprints or Common Names to authorized service lists
- **D-12:** Gateway extracts CN or fingerprint from the client cert at TLS handshake, looks up the mapping in config, and checks against the requested service -- reject if not authorized
- **D-13:** Static config via `gateway.toml` for v1; move to Redis-backed dynamic mapping in v2 (aligns with EAUTH-01)

### Claude's Discretion
- Exact `gateway.toml` config key structure for mTLS mapping
- Whether to extract CN, SAN, or fingerprint from client certs (whichever is most practical with rustls)
- Integration test structure for the reaper full-loop test
- How the sample service example is documented (inline comments vs README)
- Which stale tech debt items to formally document as "already resolved" vs silently skip

### Deferred Ideas (OUT OF SCOPE)
- Move mTLS identity mapping from gateway.toml to Redis (dynamic) -- v2 EAUTH-01
- mTLS certificate rotation without downtime -- v2 EAUTH-02
- Admin API for managing mTLS identity mappings -- v2 OPS-02
- Redesigning revoke routes to REST-style DELETE -- accepted as-is, no action needed
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| NODE-05 | Gateway tracks node health via heartbeat (last poll time, stale detection) | `in_flight_tasks` counter fix -- decrement in `report_result` restores accurate health tracking |
| OBSV-03 | Node health dashboard data available via admin API | Fix ensures `/v1/admin/health` shows accurate in-flight counts instead of monotonically growing values |
| RSLT-03 | Client can optionally provide a callback URL at submission | Adding `callback_url` field to gRPC `SubmitTaskRequest` proto enables gRPC clients to set per-task callbacks |
| INFR-06 | Gateway configures HTTP/2 keepalive pings | Plain HTTP path needs keepalive config to match TLS path behavior |
</phase_requirements>

## Architecture Patterns

### Fix 1: in_flight_tasks Counter Decrement

**What:** Call `update_in_flight_tasks(conn, service, node_id, -1)` in the `report_result` handler after successful result storage.

**Where:** `gateway/src/grpc/poll.rs` in the `report_result` method, after the `self.state.queue.report_result(...)` call succeeds (around line 239).

**Key detail:** The `report_result` handler already has access to `_validated.service_name` from `ValidatedNodeAuth`, but needs the `node_id`. Currently `ReportResultRequest` does not include `node_id` -- the handler only receives `task_id`, `success`, `result`, and `error_message`. Two options:

1. **Retrieve node_id from the task hash** -- the task hash stores the node that claimed it (assigned during XREADGROUP). Need to verify the hash contains an `assigned_to` or `node_id` field.
2. **Add node_id to ReportResultRequest proto** -- simpler but is a proto change.
3. **Use service_name from ValidatedNodeAuth and node_id from the request headers** -- node_id is sent in `x-node-id` header or extracted from the poll context.

**Investigation result:** Looking at the `report_result` Redis implementation (lines 228-232), it fetches the task hash via HGETALL. The hash includes the `service` field. The node that was assigned the task is tracked via Redis Streams consumer name. However, the in-flight counter key is `node:{service_name}:{node_id}`, so we need both service_name and node_id. The `ValidatedNodeAuth` only has `service_name`. The node_id needs to come from somewhere.

**Recommendation:** The cleanest approach is to add `service_name` and `node_id` fields to `ReportResultRequest` in the proto (the agent already knows both values) and use those for the decrement. This is already a proto-change plan (adding callback_url to SubmitTaskRequest), so adding fields to ReportResultRequest is low marginal cost. Alternatively, extract node_id from the `x-node-id` header that the NodeTokenAuthLayer could propagate.

**Confidence:** HIGH -- `update_in_flight_tasks` function exists, the only question is obtaining node_id in the report_result context.

### Fix 2: Proto callback_url Field

**What:** Add `string callback_url = 4;` to `SubmitTaskRequest` in `proto/src/gateway.proto`.

**Where:** Line 22 of `gateway.proto`, after `map<string, string> metadata = 3;`.

**Impact:** Running `cargo build` triggers tonic-build codegen. The gRPC submit handler (`grpc/submit.rs`) then needs to read `req.callback_url` and store it in the task hash, following the HTTP pattern in `http/submit.rs` (lines 61-90).

**Pattern to follow:**
```rust
// From http/submit.rs -- resolve callback URL
let resolved_callback_url = req.callback_url
    .as_deref()
    .or(client_meta.callback_url.as_deref());
```

The gRPC handler should mirror this: resolve per-task callback_url from the proto field, fall back to per-API-key default, validate with `validate_callback_url`, and store in the task hash via HSET.

**Confidence:** HIGH -- straightforward proto addition + handler mirroring.

### Fix 3: Plain HTTP Keepalive

**What:** Replace `axum::serve(listener, app)` with the `hyper_util` manual builder pattern.

**Where:** `gateway/src/main.rs` lines 299-305 (the `else` branch for non-TLS HTTP).

**Pattern:** The TLS path (lines 280-286) already demonstrates this:
```rust
let mut builder = hyper_util::server::conn::auto::Builder::new(
    hyper_util::rt::TokioExecutor::new(),
);
builder
    .http2()
    .keep_alive_interval(Some(Duration::from_secs(30)))
    .keep_alive_timeout(Duration::from_secs(10));
```

`axum::serve()` does not expose keepalive configuration -- confirmed by the [Axum GitHub Discussion #2939](https://github.com/tokio-rs/axum/discussions/2939). The fix requires a manual TCP accept loop with `hyper_util::server::conn::auto::Builder`, identical to what the TLS path does, just without the TLS acceptor layer.

**Confidence:** HIGH -- pattern already exists in the same file.

### Fix 4: mTLS Identity Mapping

**What:** Map client certificate identity (CN or fingerprint) to authorized services via `gateway.toml` config.

**Approach -- use tonic's `Request::peer_certs()`:**
When tonic's TLS is enabled (via `ServerTlsConfig`), each incoming gRPC request exposes peer certificates through `request.peer_certs()`. This returns `Option<Arc<Vec<Certificate>>>`. The CN can be extracted by parsing the DER-encoded certificate using the `x509-parser` or `rustls` certificate parsing utilities.

**Recommended approach -- SHA-256 fingerprint of the DER-encoded client cert:**
- Fingerprint is unambiguous, simple to compute (`sha2::Sha256`), and stable
- CN extraction requires DER parsing which adds a dependency (`x509-parser`)
- Fingerprint matching is a HashMap lookup on the hex-encoded SHA-256 hash
- The `sha2` crate is already a dependency (used for API key hashing)

**Config structure:**
```toml
[grpc.mtls_identity]
# Map certificate SHA-256 fingerprint to authorized services
[grpc.mtls_identity.fingerprints]
"ab:cd:ef:12:34:56:..." = ["service-a", "service-b"]
"fe:dc:ba:98:76:54:..." = ["service-c"]
```

**Implementation in auth layer:**
- In the `NodeTokenAuthLayer` (or a new mTLS identity layer), extract peer certs from the request
- Compute SHA-256 fingerprint of the first client cert
- Look up fingerprint in the config mapping
- Verify the requested service_name is in the authorized list
- This check is in addition to (not replacing) the node token check

**Alternative -- CN extraction:**
If CN is preferred over fingerprint, use `x509-parser` crate to parse the DER certificate and extract the CN from the subject. This is a ~200-line addition but more human-readable in config. The tradeoff: CN can be duplicated across certs (less secure), while fingerprint is unique per cert.

**Recommendation:** Use fingerprint (SHA-256). No new dependencies needed (`sha2` + `hex` already in Cargo.toml). Config is precise and unambiguous.

**Where to add the check:**
- Option A: Add to existing `NodeTokenAuthLayer::call()` -- check mTLS identity after token validation
- Option B: Separate Tower layer that runs before NodeTokenAuthLayer
- **Recommend Option A** -- keeps auth logic centralized, and the check is simple (config lookup)

**Getting peer certs in tonic:** `request.peer_certs()` is available on `tonic::Request` when the `tls` feature is enabled. The current setup uses `tonic = { features = ["tls-ring"] }` which includes TLS support. However, when using `tonic::transport::Server::builder().tls_config()`, tonic manages TLS internally and makes peer certs available via the request extensions.

**Confidence:** MEDIUM -- `peer_certs()` availability confirmed in tonic docs, but the exact DER format and cert chain ordering needs testing. Fallback: if `peer_certs()` does not work with tonic 0.14's ServerTlsConfig, the alternative is a manual TLS accept loop (like the HTTP side) where rustls exposes `peer_certificates()` directly on the `ServerConnection`.

### Fix 5: Sample Service Binary

**What:** Standalone Rust example in `examples/sample_service.rs` that listens for HTTP POST requests, echoes the payload, and optionally simulates delay.

**Structure (per D-01 through D-04):**
```rust
// examples/sample_service.rs
// Uses hyper 1.x directly (already a dependency)
// Listens on --port (default 8090, matching agent's --dispatch-url default)
// Reads X-Task-Id header for logging
// Reads body as payload, returns it as-is
// If X-Simulate-Delay-Ms header present, sleeps that duration
```

**Runner agent integration:** The agent dispatches tasks via `POST` to `--dispatch-url` (default `http://localhost:8090/execute`) with `X-Task-Id` header and the task payload as the body. The sample service must listen on this path and return 200 with the result body.

**Cargo convention:** Files in `examples/` are built with `cargo run --example sample_service`. No changes to `Cargo.toml` needed (Cargo auto-discovers examples). However, since the example uses crates from the workspace (hyper, tokio, clap), the `[dev-dependencies]` or `[[example]]` section may need updating -- or the example can use its own inline dependencies via the `examples/` convention.

**Important:** Since the gateway project is a workspace and the example lives at `examples/` (workspace root, not `gateway/examples/`), it needs a `Cargo.toml` at `examples/` or be declared as an example in the gateway crate. Per D-02, it should be `examples/sample_service.rs` at the workspace root level or `gateway/examples/sample_service.rs` in the gateway crate.

**Recommendation:** Place at `gateway/examples/sample_service.rs`. This automatically gets access to the gateway crate's dependencies. Use hyper 1.x + tokio + clap (all already in gateway deps). Run with `cargo run -p xgent-gateway --example sample_service`.

**Confidence:** HIGH -- standard Cargo example convention.

### Fix 6: Reaper Integration Test

**What:** Replace the precondition-only test with one that actually invokes the reaper and verifies task state changes to failed.

**Problem:** `reap_service` is private (`async fn reap_service`). The public API is `run_reaper` which is an infinite loop.

**Solution:** Make `reap_timed_out_tasks` pub (or add a `pub async fn reap_once` that calls it). This is a minimal API surface expansion specifically for testability. Then the integration test:
1. Registers a service with 1s timeout
2. Submits a task
3. Claims the task via XREADGROUP
4. Waits 2s for timeout
5. Calls `reap_once(state)` or `reap_timed_out_tasks(state)`
6. Asserts task state is "failed" via HGET

**Confidence:** HIGH -- the reaper logic is correct, just needs a public entry point for testing.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Certificate fingerprinting | Custom DER parsing | `sha2::Sha256::digest(cert_der)` + `hex::encode` | sha2 and hex already in deps, DER is the raw cert bytes |
| HTTP server for sample service | Custom TCP handler | `hyper 1.x` with `hyper_util` | Already a project dependency, minimal overhead |
| Config section parsing | Manual TOML parsing | `serde::Deserialize` with `config` crate | Established pattern in config.rs |
| Keepalive configuration | Custom ping logic | `hyper_util::server::conn::auto::Builder::http2()` | Built-in HTTP/2 PING frame support |

## Common Pitfalls

### Pitfall 1: Missing node_id in report_result for in_flight decrement
**What goes wrong:** The `report_result` handler needs `node_id` to call `update_in_flight_tasks` but `ReportResultRequest` only has `task_id`.
**Why it happens:** Original proto design assumed report_result only needs task_id.
**How to avoid:** Either add `node_id` + `service_name` fields to the `ReportResultRequest` proto, or look up the assigned node from the task hash in Redis. Proto change is cleaner and avoids an extra Redis round-trip.
**Warning signs:** Compilation succeeds but the counter silently never decrements because the node_id is empty/wrong.

### Pitfall 2: Proto field number conflicts
**What goes wrong:** Adding fields to existing proto messages with wrong field numbers causes wire-format incompatibility.
**Why it happens:** Reusing a previously deleted field number, or not incrementing properly.
**How to avoid:** Use the next available field number. `SubmitTaskRequest` currently uses 1-3, so `callback_url` is field 4. If adding to `ReportResultRequest` (currently 1-4), use field 5+.
**Warning signs:** Deserialization errors or silent data corruption.

### Pitfall 3: axum::serve replacement breaking graceful shutdown
**What goes wrong:** The current `axum::serve(listener, app).await` handles shutdown cleanly. Switching to a manual accept loop in the plain HTTP path could lose graceful shutdown behavior.
**Why it happens:** Manual accept loops need explicit shutdown signal handling.
**How to avoid:** Follow the TLS path pattern exactly -- it already handles this via the tokio::spawn per-connection pattern. The outer task completes when the gateway shuts down.
**Warning signs:** Connections not draining on SIGTERM in plain HTTP mode.

### Pitfall 4: tonic peer_certs() returning None
**What goes wrong:** `request.peer_certs()` returns `None` even with mTLS enabled.
**Why it happens:** Peer cert extraction depends on tonic's internal TLS handling. If the TLS config doesn't propagate certs to the request extensions, the method returns None.
**How to avoid:** Test early with a simple debug log. If it returns None, the fallback is to switch to a manual TLS accept loop for the gRPC server (like the HTTP side does), which gives direct access to `rustls::ServerConnection::peer_certificates()`.
**Warning signs:** All mTLS identity checks fail with "no peer certificate" even when the client presents a valid cert.

### Pitfall 5: Example binary depending on gateway internals
**What goes wrong:** The sample service example imports gateway types, creating a coupling that makes the example fragile.
**Why it happens:** Using `use xgent_gateway::*` in the example.
**How to avoid:** The sample service should be fully standalone -- it only needs hyper, tokio, and clap. No imports from the gateway crate. It receives HTTP POSTs and returns HTTP responses. It does not need to know about Redis, protos, or gateway types.
**Warning signs:** Example fails to compile when gateway internals change.

## Code Examples

### in_flight_tasks Decrement (insertion point in poll.rs report_result)
```rust
// After successful report_result, decrement in_flight counter
// Needs service_name and node_id -- get from ValidatedNodeAuth + request
let _ = crate::registry::node_health::update_in_flight_tasks(
    &mut health_conn,
    &validated.service_name,
    &node_id,  // source TBD: proto field or task hash lookup
    -1,
).await;
```

### Proto callback_url Addition
```protobuf
message SubmitTaskRequest {
  string service_name = 1;
  bytes payload = 2;
  map<string, string> metadata = 3;
  string callback_url = 4;  // Optional per-task callback URL
}
```

### Plain HTTP Keepalive (replacing axum::serve)
```rust
// Plain HTTP mode with keepalive (replacing axum::serve)
let listener = tokio::net::TcpListener::bind(&http_addr).await?;
tracing::info!(%http_addr, "HTTP server starting (plain, with keepalive)");
loop {
    let (tcp_stream, addr) = listener.accept().await?;
    let app = app.clone();
    tokio::spawn(async move {
        let io = hyper_util::rt::TokioIo::new(tcp_stream);
        let service = hyper_util::service::TowerToHyperService::new(app);
        let mut builder = hyper_util::server::conn::auto::Builder::new(
            hyper_util::rt::TokioExecutor::new(),
        );
        builder
            .http2()
            .keep_alive_interval(Some(Duration::from_secs(30)))
            .keep_alive_timeout(Duration::from_secs(10));
        if let Err(e) = builder.serve_connection(io, service).await {
            tracing::debug!(%addr, error=%e, "HTTP connection error");
        }
    });
}
```

### mTLS Identity Config Structure
```toml
[grpc.mtls_identity]
# Map SHA-256 fingerprint of client cert DER to authorized services
[grpc.mtls_identity.fingerprints]
"a1b2c3d4e5f6..." = ["my-service", "other-service"]
"f6e5d4c3b2a1..." = ["another-service"]
```

### mTLS Fingerprint Extraction
```rust
use sha2::{Sha256, Digest};

fn cert_fingerprint(cert_der: &[u8]) -> String {
    let hash = Sha256::digest(cert_der);
    hex::encode(hash)
}
```

### Sample Service (hyper 1.x echo server)
```rust
// gateway/examples/sample_service.rs
use hyper::{body::Incoming, Request, Response};
use hyper::body::Bytes;
use http_body_util::Full;
use std::time::Duration;

async fn handle(req: Request<Incoming>) -> Result<Response<Full<Bytes>>, hyper::Error> {
    let delay_ms = req.headers()
        .get("X-Simulate-Delay-Ms")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.parse::<u64>().ok());

    if let Some(ms) = delay_ms {
        tokio::time::sleep(Duration::from_millis(ms)).await;
    }

    let body = http_body_util::BodyExt::collect(req.into_body())
        .await?
        .to_bytes();
    Ok(Response::new(Full::new(body)))
}
```

## Tech Debt Triage (from Audit)

| # | Item | Status | Action |
|---|------|--------|--------|
| 1 | NODE-02 marked `[x]` in REQUIREMENTS.md but deferred | **Already fixed** | REQUIREMENTS.md already shows `[~]` Deferred (D-13). Verify and skip. |
| 2 | `cleanup_redis_keys` no-op stub | **Keep as-is (D-08)** | Harmless, tests pass. No action. |
| 3 | Unused `_opts` variable in queue/redis.rs | **Already fixed** | Grep finds no `_opts` in queue/redis.rs. Verify and skip. |
| 4 | Unused import `redis::AsyncCommands` in report_result | **Already fixed** | Grep finds no unused AsyncCommands import. Verify and skip. |
| 5 | mTLS no per-client identity mapping | **Fix in this phase** | D-11/D-12 address this with config-based fingerprint mapping. |
| 6 | `has_in_flight` assigned but never read in agent.rs | **Still present** | `has_in_flight` is read -- it is passed to `graceful_drain()` and used to decide whether to wait for task completion. The audit may be wrong about this. Verify compiler output. |
| 7 | Reaper test only validates precondition | **Fix in this phase (D-09)** | Add full-loop test that invokes reaper and checks state change. |
| 8 | `f.get_name()` deprecated in metrics.rs | **Already fixed** | Grep shows `f.name()` is used (not `get_name()`). Verify and skip. |
| 9 | Unused `redis::AsyncCommands` import in test | **Verify** | Check if still present in any test file. May be in reaper_callback_integration_test.rs (line 14 has `use redis::AsyncCommands;` which IS used for `.hset`, `.hget`, `.srem` calls). Likely not actually unused. |

**Summary:** Of 9 tech debt items, 4-5 are already resolved, 2 are fixed in this phase (mTLS identity + reaper test), 1 is kept as-is (cleanup_redis_keys), and 1-2 need verification during execution.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test (`#[tokio::test]`) + `#[ignore]` for integration tests |
| Config file | None (standard Cargo test) |
| Quick run command | `cargo test -p xgent-gateway --lib` |
| Full suite command | `cargo test -p xgent-gateway --test '*' -- --ignored` (requires Redis) |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| NODE-05 | in_flight_tasks decremented on report_result | integration | `cargo test -p xgent-gateway --test integration_test test_in_flight_decrement -- --ignored` | No -- Wave 0 |
| OBSV-03 | Health endpoint shows accurate in_flight | integration | Covered by NODE-05 test above | No -- Wave 0 |
| RSLT-03 | gRPC SubmitTask accepts callback_url | integration | `cargo test -p xgent-gateway --test grpc_auth_test test_submit_callback_url -- --ignored` | No -- Wave 0 |
| INFR-06 | Plain HTTP keepalive configured | unit | Manual verification (code review) -- keepalive is a transport concern | N/A -- manual |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway --lib`
- **Per wave merge:** `cargo test -p xgent-gateway --test '*' -- --ignored` (requires Redis)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Reaper full-loop test (replaces precondition-only test) -- covers D-09
- [ ] `reap_timed_out_tasks` or `reap_once` made pub for test access
- [ ] in_flight decrement integration test
- [ ] gRPC callback_url submission test (can extend existing grpc_auth_test.rs)

## Sources

### Primary (HIGH confidence)
- Codebase inspection: `gateway/src/grpc/poll.rs`, `gateway/src/queue/redis.rs`, `gateway/src/reaper/mod.rs`, `gateway/src/main.rs`, `gateway/src/config.rs`, `gateway/src/grpc/auth.rs`
- `.planning/v1.0-MILESTONE-AUDIT.md` -- authoritative list of all issues and tech debt
- `proto/src/gateway.proto` -- current proto definition
- `gateway/src/bin/agent.rs` -- runner agent dispatch pattern

### Secondary (MEDIUM confidence)
- [Axum Discussion #2939](https://github.com/tokio-rs/axum/discussions/2939) -- confirms axum::serve has no keepalive config
- [Tonic TlsConnectInfo](https://docs.rs/tonic/latest/tonic/transport/server/struct.TlsConnectInfo.html) -- peer cert extraction with tonic TLS
- [Axum serve docs](https://docs.rs/axum/latest/axum/fn.serve.html) -- serve() limitations confirmed

### Tertiary (LOW confidence)
- tonic 0.14 `peer_certs()` behavior with `ServerTlsConfig` -- needs runtime verification

## Metadata

**Confidence breakdown:**
- Integration fixes (counter, proto, keepalive): HIGH -- insertion points identified, patterns exist in codebase
- mTLS identity mapping: MEDIUM -- config structure clear, but `peer_certs()` availability needs runtime testing
- Sample service: HIGH -- standard Cargo example, no complex dependencies
- Tech debt triage: HIGH -- most items verified as already resolved via grep

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable codebase, no external dependency concerns)
