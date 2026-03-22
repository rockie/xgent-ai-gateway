# Phase 6: gRPC Auth Hardening - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

All gRPC RPCs enforce the same authentication as their HTTP counterparts. API key auth on client-facing RPCs (SubmitTask, GetTaskStatus) and node token auth on node-facing RPCs (ReportResult, Heartbeat, DrainNode). No new auth schemes — reuse existing API key and node token validation from `auth/` module.

</domain>

<decisions>
## Implementation Decisions

### gRPC auth mechanism
- **D-01:** Use tonic interceptors, not inline auth or Tower middleware
- **D-02:** Two interceptors: one for TaskService (API key auth), one for NodeService (node token auth)
- **D-03:** Interceptors take `Arc<AppState>` at construction time, clone `MultiplexedConnection` for Redis access
- **D-04:** Refactor existing inline auth in `poll_tasks` into the NodeService interceptor — all 4 node RPCs use the same code path
- **D-05:** API keys in gRPC metadata: check `authorization: Bearer <key>` first, fall back to `x-api-key: <key>` — mirrors `extract_api_key` logic from HTTP

### Service authorization on gRPC
- **D-06:** Two-phase auth: interceptor validates key/token and injects metadata into request extensions; handler checks per-service authorization
- **D-07:** gRPC SubmitTask enforces per-service authorization — API key must be authorized for the requested service_name (parity with HTTP)
- **D-08:** gRPC GetTaskStatus enforces service scoping — API key must be authorized for the service that owns the task (requires Redis lookup of task's service)
- **D-09:** Node token auth on report_result: nodes send `x-service-name: <service>` metadata alongside Bearer token; interceptor uses both to validate
- **D-10:** Heartbeat and DrainNode already have `service_name` in proto message — interceptor extracts from `x-service-name` metadata for consistency

### Error response consistency
- **D-11:** Generic error messages: return `Status::unauthenticated("unauthorized")` with no hints about what failed; log specific reason at debug level
- **D-12:** Service authorization failures use `Status::permission_denied("unauthorized")` (PERMISSION_DENIED, not UNAUTHENTICATED) — distinguishes auth vs authz
- **D-13:** No protocol label on `errors_total` metric — existing `error_type` labels (`auth_api_key`, `auth_node_token`) already distinguish client vs node auth. No breaking changes to dashboards.
- **D-14:** gRPC auth failures increment `errors_total` with same labels as HTTP: `[service, "auth_api_key"]` or `[service, "auth_node_token"]`

### Claude's Discretion
- Exact interceptor function signatures and how to pass metadata through tonic extensions
- Whether to create a shared extraction helper or keep API key and node token extraction separate
- Integration test structure and helper utilities

</decisions>

<specifics>
## Specific Ideas

- poll_tasks already proves the pattern: extract Bearer from `authorization` metadata, hash, validate against Redis. The interceptor should generalize this.
- HTTP `extract_api_key` checks Bearer then x-api-key — gRPC interceptor should mirror this exactly for consistency.
- `ClientMetadata` and `NodeTokenMetadata` structs already exist — inject these into tonic request extensions the same way HTTP injects `ClientMetadata` via Axum extensions.

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Auth module (source of truth for validation logic)
- `gateway/src/auth/api_key.rs` — API key extraction, hashing, lookup, ClientMetadata struct, Axum middleware pattern to replicate
- `gateway/src/auth/node_token.rs` — Node token hashing, validation, NodeTokenMetadata struct

### gRPC services (files to modify)
- `gateway/src/grpc/submit.rs` — TaskService impl: submit_task and get_task_status need auth added
- `gateway/src/grpc/poll.rs` — NodeService impl: poll_tasks has inline auth to refactor; report_result/heartbeat/drain_node need auth added
- `gateway/src/main.rs` lines 176-182 — Where TaskServiceServer and NodeServiceServer are constructed and added to tonic Router

### Existing auth integration tests
- `gateway/tests/auth_integration_test.rs` — Existing HTTP auth tests to use as pattern for gRPC auth tests

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `auth::api_key::extract_api_key(headers)` — Extracts API key from HeaderMap; gRPC metadata is also a HeaderMap-like structure, may be adaptable
- `auth::api_key::lookup_api_key(conn, hash)` — Returns `Option<ClientMetadata>` with service_names; reuse directly in interceptor
- `auth::node_token::validate_node_token(conn, service, raw_token)` — Boolean validation; reuse directly in interceptor
- `auth::api_key::hash_api_key(raw)` and `auth::node_token::hash_node_token(raw)` — SHA-256 hashing functions

### Established Patterns
- Axum auth middleware injects `ClientMetadata` into request extensions; tonic interceptors can inject into `tonic::Extensions` similarly
- `poll_tasks` extracts `authorization` metadata via `request.metadata().get("authorization")` — proven pattern for gRPC metadata access
- `errors_total.with_label_values(&[service, error_type]).inc()` — metric recording pattern for auth failures

### Integration Points
- `main.rs` line 177-181: `TaskServiceServer::new()` and `NodeServiceServer::new()` — interceptors wrap these via `TaskServiceServer::with_interceptor()` / `NodeServiceServer::with_interceptor()`
- `AppState.auth_conn` — Dedicated Redis connection for auth lookups, clone into interceptors

</code_context>

<deferred>
## Deferred Ideas

- Adding `service_name` field directly to `ReportResultRequest` proto message — would be cleaner than metadata but is a proto breaking change. Consider for v2.
- Admin API auth hardening (currently unauthenticated per Phase 2 decision D-02) — separate concern, not in scope.

</deferred>

---

*Phase: 06-grpc-auth-hardening*
*Context gathered: 2026-03-22*
