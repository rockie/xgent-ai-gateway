# Phase 7: Integration Fixes, Sample Service, and Cleanup - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Fix remaining integration issues from the v1.0 milestone audit, provide a sample service binary for end-to-end testing, add mTLS identity mapping via config, and clean up tech debt across all phases. No new features beyond what the audit identified.

**Adjusted success criteria** (from ROADMAP.md, with discussion modifications):
1. `in_flight_tasks` counter is decremented when a node reports task completion
2. gRPC `SubmitTaskRequest` proto includes `callback_url` field
3. ~~Revoke routes use REST-style DELETE~~ — **Accepted as-is**: POST method is fine, no redesign
4. Plain HTTP mode configures keepalive
5. A sample service binary exists that can receive tasks from the runner agent and return results
6. NODE-02 marked as deferred (not complete) in REQUIREMENTS.md — **Already done** (`[~]` Deferred)
7. All tech debt items from the audit are resolved (verified, fixed, or confirmed stale)
8. **Added**: mTLS client cert identity mapped to authorized services via `gateway.toml` config

</domain>

<decisions>
## Implementation Decisions

### Sample service binary
- **D-01:** Echo by default — returns payload as-is. If metadata contains `simulate_delay_ms`, sleeps that duration before responding (combo mode)
- **D-02:** Lives in `examples/` as a standalone single-file example, not a workspace crate or binary target
- **D-03:** Uses hyper directly for the HTTP server — minimal, no framework overhead
- **D-04:** Plain HTTP only, no TLS support needed — this is a demo/testing tool

### Revoke routes
- **D-05:** No changes to revoke routes — `POST /v1/admin/api-keys/revoke` and `POST /v1/admin/node-tokens/revoke` are accepted as-is
- **D-06:** Keep `200 OK` with JSON response body `{"status": "revoked"}` — no change to response format

### Tech debt triage
- **D-07:** Verify each audit item during execution — skip items confirmed already fixed or stale (items 1/NODE-02, 3/_opts, 4/AsyncCommands, 6/has_in_flight, 8/get_name, 9/AsyncCommands appeared resolved in codebase scout)
- **D-08:** `cleanup_redis_keys` stub — keep as-is, it's harmless and tests pass
- **D-09:** Reaper integration test — add a full-loop integration test that spawns the reaper, waits, and checks that a timed-out task's status changes to failed (don't just test preconditions)
- **D-10:** mTLS identity gap (audit item 5) — addressed by D-11/D-12 below instead of deferring

### mTLS identity mapping
- **D-11:** Add a `[grpc.mtls_identity]` section (or similar) in `gateway.toml` that maps certificate fingerprints or Common Names to authorized service lists
- **D-12:** Gateway extracts CN or fingerprint from the client cert at TLS handshake, looks up the mapping in config, and checks against the requested service — reject if not authorized
- **D-13:** Static config via `gateway.toml` for v1; move to Redis-backed dynamic mapping in v2 (aligns with EAUTH-01)

### Claude's Discretion
- Exact `gateway.toml` config key structure for mTLS mapping
- Whether to extract CN, SAN, or fingerprint from client certs (whichever is most practical with rustls)
- Integration test structure for the reaper full-loop test
- How the sample service example is documented (inline comments vs README)
- Which stale tech debt items to formally document as "already resolved" vs silently skip

</decisions>

<specifics>
## Specific Ideas

- Sample service should feel like a "hello world" for the gateway — someone cloning the repo should be able to run it alongside the runner agent to see the full task lifecycle
- The `simulate_delay_ms` metadata key makes it useful for load testing and demonstrating async behavior
- mTLS config in gateway.toml should follow existing config patterns (the `[tls]` and `[auth]` sections already exist)

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Audit (source of truth for all work items)
- `.planning/v1.0-MILESTONE-AUDIT.md` — Complete list of integration issues, broken flows, and tech debt items. Phase 7 exists to close these gaps.

### in_flight_tasks counter fix
- `gateway/src/grpc/poll.rs` lines 165-170 — Where `update_in_flight_tasks(1)` is called on dispatch
- `gateway/src/grpc/poll.rs` lines 214-239 — `report_result` handler where decrement must be added
- `gateway/src/queue/redis.rs` lines 217-291 — `report_result` Redis implementation

### Proto callback_url field
- `proto/src/gateway.proto` lines 18-22 — `SubmitTaskRequest` message definition (currently 3 fields, needs `callback_url`)
- `gateway/src/http/handlers.rs` — HTTP submit handler that already handles callback_url (pattern to follow)

### Plain HTTP keepalive
- `gateway/src/main.rs` lines 299-305 — Plain HTTP server startup path (no keepalive configured)
- `gateway/src/main.rs` lines 163-164 — gRPC server keepalive config (pattern to follow)

### Runner agent (sample service connects to this)
- `gateway/src/bin/agent.rs` — Runner agent binary that dispatches tasks via HTTP POST to `--dispatch-url`

### mTLS identity (cert extraction point)
- `gateway/src/main.rs` — TLS accept loop where client certs are available
- `gateway/src/config.rs` — Config loading from gateway.toml

### Reaper test improvement
- `gateway/tests/reaper_callback_integration_test.rs` — Existing reaper tests (precondition-only)
- `gateway/src/queue/redis.rs` — Reaper implementation (`reap_service` is private)

### Tech debt verification
- `gateway/tests/integration_test.rs` lines 141-145 — `cleanup_redis_keys` no-op stub
- `gateway/src/bin/agent.rs` — `has_in_flight` variable usage

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `update_in_flight_tasks(conn, service, node_id, delta)` in `queue/redis.rs` — Already exists, just needs to be called with `-1` in report_result path
- `deliver_callback()` in `queue/redis.rs` — Existing callback delivery function, already wired for HTTP submissions
- gRPC keepalive config pattern in `main.rs` lines 163-164 — `http2_keepalive_interval` and `http2_keepalive_timeout` on tonic server
- Per-test Redis DB isolation via atomic counter — Established in Phase 6 gRPC auth tests, use for new integration tests
- `ServiceConfig` struct in config — Already has fields for service configuration, extend for mTLS mapping

### Established Patterns
- Binary targets in `gateway/src/bin/` — agent.rs is the existing pattern
- Examples in `examples/` — Standard Cargo convention for standalone examples
- Tower middleware pattern — Used for auth layers in Phase 6, could be used for mTLS identity injection
- `gateway.toml` config loading via `config` crate — Layered config with TOML file support

### Integration Points
- `report_result` in `grpc/poll.rs` — Where in_flight decrement must happen (has access to service_name and node_id via `ValidatedNodeAuth`)
- `tonic-build` codegen — Adding `callback_url` to proto triggers codegen, affects `submit_task` gRPC handler
- `axum::serve` in plain HTTP path — Needs keepalive configuration added
- TLS accept loop in `main.rs` — Where client cert is available for CN/fingerprint extraction

</code_context>

<deferred>
## Deferred Ideas

- Move mTLS identity mapping from gateway.toml to Redis (dynamic) — v2 EAUTH-01
- mTLS certificate rotation without downtime — v2 EAUTH-02
- Admin API for managing mTLS identity mappings — v2 OPS-02
- Redesigning revoke routes to REST-style DELETE — accepted as-is, no action needed

</deferred>

---

*Phase: 07-integration-fixes-sample-service-cleanup*
*Context gathered: 2026-03-22*
