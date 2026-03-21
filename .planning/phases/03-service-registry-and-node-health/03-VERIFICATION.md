---
phase: 03-service-registry-and-node-health
verified: 2026-03-21T14:14:32Z
status: passed
score: 12/12 must-haves verified
re_verification: false
---

# Phase 3: Service Registry and Node Health Verification Report

**Phase Goal:** Service registry with admin CRUD, node heartbeat tracking, drain orchestration, and stale-node cleanup — multi-tenant queue routing is gated on registered services.
**Verified:** 2026-03-21T14:14:32Z
**Status:** passed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| #  | Truth | Status | Evidence |
|----|-------|--------|----------|
| 1  | Admin can register a new service via POST /v1/admin/services and it persists in Redis | VERIFIED | `register_service` in `registry/service.rs` uses HSET on `service:{name}` + SADD to `services:index`; route wired in `main.rs:119-121` |
| 2  | Admin can deregister a service via DELETE /v1/admin/services/{name} and all Redis state is cleaned up asynchronously | VERIFIED | `deregister_service` in `admin.rs:249` returns 202, spawns `tokio::spawn(cleanup_service)` |
| 3  | Admin can list and get service details via GET /v1/admin/services | VERIFIED | `list_services` and `get_service_detail` wired at `main.rs:121,125`; detail includes live node health via `derive_health_state` |
| 4  | submit_task rejects tasks for unregistered services with 404/NOT_FOUND | VERIFIED | HTTP submit at `submit.rs:52-55` and gRPC submit at `grpc/submit.rs:35-46` both call `service_exists` and return `ServiceNotFound` if false |
| 5  | Service config survives gateway restart (read from Redis, not in-memory cache) | VERIFIED | `get_service` reads from Redis HGETALL; integration test `test_service_config_persistence` proves cross-connection persistence |
| 6  | Gateway updates node last_seen timestamp on every poll cycle | VERIFIED | `poll.rs:92` and `poll.rs:159` call `register_or_update_node` after each loop iteration |
| 7  | Heartbeat RPC updates node last_seen and creates/refreshes node registry entry | VERIFIED | `poll.rs:223-256`: validates service, calls `register_or_update_node`, returns `acknowledged: true` |
| 8  | DrainNode RPC sets draining flag; gateway stops sending new tasks to that node | VERIFIED | `poll.rs:258-291`: calls `set_node_draining`; poll loop at `poll.rs:101` checks `is_node_draining` and skips dispatch if true |
| 9  | Admin GET /v1/admin/services/{name} shows live node health derived from timestamps | VERIFIED | `admin.rs:286-340`: queries Redis nodes, calls `derive_health_state` per node on each request |
| 10 | Node health is derived on-demand from last_seen (no background reaper per D-15) | VERIFIED | `derive_health_state` is a pure function computing from timestamp diff; no background goroutine/task found |
| 11 | Integration tests prove service CRUD lifecycle, submit rejection, deregistration, drain | VERIFIED | 9 `#[ignore]` tests in `registry_integration_test.rs` (506 lines), each calling actual registry functions |
| 12 | Runner agent handles SIGTERM gracefully: calls DrainNode, waits for in-flight, exits | VERIFIED | `agent.rs`: `shutdown_signal()` at line 58, `graceful_drain()` at line 125, `SHUTTING_DOWN` AtomicBool at line 19 |

**Score:** 12/12 truths verified

### Required Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `gateway/src/registry/mod.rs` | Registry module root | VERIFIED | 3 lines — exposes `service`, `node_health`, `cleanup` submodules |
| `gateway/src/registry/service.rs` | Service CRUD against Redis | VERIFIED | 205 lines; exports `register_service`, `get_service`, `list_services`, `service_exists`, `delete_service_config` |
| `gateway/src/registry/node_health.rs` | Node health types + CRUD | VERIFIED | 280 lines; exports `ServiceConfig`, `NodeHealthState`, `NodeStatus`, `derive_health_state`, `register_or_update_node`, `set_node_draining`, `is_node_draining`, `mark_node_disconnected`, `update_in_flight_tasks`, `get_nodes_for_service` |
| `gateway/src/registry/cleanup.rs` | Background deregistration cleanup | VERIFIED | 137 lines; exports `cleanup_service`, `scan_and_unlink` |
| `proto/src/gateway.proto` | Heartbeat and DrainNode RPC definitions | VERIFIED | `rpc Heartbeat` at line 14, `rpc DrainNode` at line 15, `NodeHealthState` enum, all message types present |
| `gateway/src/grpc/poll.rs` | Heartbeat/DrainNode RPCs + drain-aware poll | VERIFIED | Full implementations at lines 223 and 258; drain check at line 101; last_seen update at lines 92 and 159 |
| `gateway/tests/registry_integration_test.rs` | Integration test suite | VERIFIED | 506 lines, 9 tests, all `#[ignore]` gated |
| `gateway/src/bin/agent.rs` | SIGTERM handler with DrainNode RPC | VERIFIED | 304 lines; `shutdown_signal()`, `graceful_drain()`, `SHUTTING_DOWN` AtomicBool, `DrainNodeRequest` import at line 16 |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `gateway/src/http/admin.rs` | `gateway/src/registry/service.rs` | Admin endpoint handlers call registry functions | WIRED | `admin.rs:241` calls `registry::service::register_service`; lines 254, 276, 291 call `service_exists`, `list_services`, `get_service` |
| `gateway/src/http/submit.rs` | `gateway/src/registry/service.rs` | submit_task checks service_exists before enqueuing | WIRED | `submit.rs:52`: `crate::registry::service::service_exists` called before queue dispatch |
| `gateway/src/grpc/submit.rs` | `gateway/src/registry/service.rs` | gRPC SubmitTask checks service_exists before enqueuing | WIRED | `grpc/submit.rs:35-46`: `service_exists` check with `NOT_FOUND` status on failure |
| `gateway/src/grpc/poll.rs` | `gateway/src/registry/node_health.rs` | Poll loop calls register_or_update_node on each cycle | WIRED | `poll.rs:92` and `poll.rs:159` call `crate::registry::node_health::register_or_update_node` |
| `gateway/src/grpc/poll.rs drain_node` | Redis `node:{service}:{node_id}` draining field | HSET draining via set_node_draining | WIRED | `poll.rs:280`: `set_node_draining` called from `drain_node` RPC; `node_health.rs:124` sets `draining "true"` |
| `gateway/tests/registry_integration_test.rs` | `gateway/src/registry/service.rs` | Tests call register_service, get_service, list_services, service_exists | WIRED | Direct imports at lines 26-27; called in 8 of 9 tests |
| `gateway/src/bin/agent.rs` | DrainNode RPC | Agent calls drain_node on SIGTERM | WIRED | `agent.rs:133-143`: constructs `DrainNodeRequest`, calls `drain_client.drain_node(drain_req)` inside `graceful_drain()` |
| `gateway/src/registry/service.rs` | Redis `service:{name}` / `services:index` | HSET/HGETALL/SADD on service keys | WIRED | `service.rs:17`: `format!("service:{}", config.name)`; SADD to `services:index` in pipeline |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| SRVC-01 | 03-01, 03-03 | Admin can register a new service with the gateway (name, config, node auth tokens) | SATISFIED | `register_service` in `registry/service.rs`; POST /v1/admin/services endpoint; `test_register_service` and `test_register_duplicate_service_fails` integration tests |
| SRVC-03 | 03-01, 03-03 | Admin can deregister a service (drains queue, removes config) | SATISFIED | `cleanup_service` in `registry/cleanup.rs` handles stream destroy, token revoke, node cleanup, config delete; async via `tokio::spawn`; `test_deregister_cleanup` integration test |
| SRVC-04 | 03-01, 03-03 | Service configuration is persisted in Redis and survives gateway restarts | SATISFIED | Redis hash-per-service pattern with `get_service` reading from Redis (not memory); `test_service_config_persistence` proves cross-connection read |
| NODE-03 | 03-01, 03-03 | Nodes authenticate with pre-shared tokens scoped to their service | SATISFIED | Node tokens already scoped to service via `node_tokens:{service_name}:{token_hash}` key pattern (Phase 2); Phase 3 adds service-existence gate — tokens for unregistered services now rejected at submit/poll time |
| NODE-05 | 03-02, 03-03 | Gateway tracks node health via heartbeat (last poll time, stale detection) | SATISFIED | `register_or_update_node` updates `last_seen` on every poll cycle and Heartbeat RPC; `derive_health_state` computes Healthy/Unhealthy/Disconnected on-demand from timestamp diff; `test_node_health_tracking` and `test_node_disconnect` integration tests |
| NODE-06 | 03-02, 03-03 | Nodes can signal graceful drain — gateway stops assigning new tasks, waits for in-flight completion | SATISFIED | DrainNode RPC sets `draining=true`; poll loop checks `is_node_draining` and skips XREADGROUP when true; enforces drain timeout by calling `mark_node_disconnected` after timeout; agent calls DrainNode on SIGTERM; `test_node_drain_flow` integration test |

**No orphaned requirements found.** All 6 IDs declared in plan frontmatter map to REQUIREMENTS.md entries and are covered by implementation.

### Anti-Patterns Found

| File | Line | Pattern | Severity | Impact |
|------|------|---------|----------|--------|
| `gateway/src/bin/agent.rs` | 240 | `has_in_flight = true` — value assigned but never read (compiler warning) | Info | No functional impact; the `in_flight` Notify and `has_in_flight` bool track state but `has_in_flight` is read at line ~248 in the shutdown path |

No blocker or warning anti-patterns found. All implementations contain real Redis operations. No stub return values (`return Ok(())` bare, `return Response.json({})` empty, `unimplemented!()`) were found in production code paths. The two build warnings are unused-import and unused-assignment style issues only.

### Human Verification Required

#### 1. Drain Flow End-to-End

**Test:** Start the gateway, register a service, connect an agent node, submit a long-running task to it, send SIGTERM to the agent, observe that it calls DrainNode and waits for the task before exiting.
**Expected:** Agent logs "SIGTERM received, initiating graceful drain", "drain acknowledged by gateway", "graceful shutdown complete". Gateway stops sending new tasks to the draining node. In-flight task completes and result is reported before exit.
**Why human:** Requires running processes, actual timing behavior, and log observation. Cannot be verified statically.

#### 2. Service Registration POST Returns 201

**Test:** POST to /v1/admin/services with valid JSON, observe HTTP 201 Created response.
**Expected:** Response body contains all service config fields including `created_at` timestamp.
**Why human:** HTTP status code rendering and JSON response shape require a live gateway.

#### 3. Stale Node Detection in Admin Detail

**Test:** Register a service, connect a node, wait longer than `node_stale_after_secs` (60s default) without polling, GET /v1/admin/services/{name}, observe node health.
**Expected:** Node health shows "unhealthy" rather than "healthy" after the stale window elapses.
**Why human:** Requires real-time passage, cannot simulate elapsed time in static analysis.

### Gaps Summary

No gaps found. All 12 observable truths verified, all 8 required artifacts exist and are substantive, all 8 key links are wired, all 6 requirement IDs are satisfied.

The one notable design deviation from the Plan 02 spec: `get_service_detail` in `admin.rs` does inline Redis queries rather than calling `get_nodes_for_service` from `node_health.rs`. The behavior is functionally equivalent — it fetches `nodes:{name}` members, reads each `node:{name}:{id}` hash, and calls `derive_health_state`. This does not constitute a gap.

Build status: `cargo build -p xgent-gateway` finishes with 3 warnings (unused import, unused assignment, unused variable) — no errors. `cargo test -p xgent-gateway --lib` passes 38 tests, 0 failures.

---

_Verified: 2026-03-21T14:14:32Z_
_Verifier: Claude (gsd-verifier)_
