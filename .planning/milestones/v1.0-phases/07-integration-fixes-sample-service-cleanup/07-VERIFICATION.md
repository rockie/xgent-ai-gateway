---
phase: 07-integration-fixes-sample-service-cleanup
verified: 2026-03-22T14:00:00Z
status: passed
score: 10/10 must-haves verified
re_verification: false
---

# Phase 07: Integration Fixes and Sample Service Cleanup — Verification Report

**Phase Goal:** Fix integration issues and tech debt identified in the v1.0 audit: proto field gaps, counter bugs, keepalive config, mTLS identity mapping, reaper test coverage, and a sample service binary for E2E testing.
**Verified:** 2026-03-22
**Status:** PASSED
**Re-verification:** No — initial verification

---

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | `in_flight_tasks` counter is decremented when a node reports task completion | VERIFIED | `poll.rs` line 250–257: `update_in_flight_tasks(&mut self.state.auth_conn.clone(), &decrement_service, &report_node_id, -1)` in `report_result` handler |
| 2 | gRPC `SubmitTask` accepts a `callback_url` and stores it in the task hash | VERIFIED | `gateway.proto` line 22: `string callback_url = 4`; `submit.rs` lines 84–91: validates and `hset`s `callback_url` in task hash |
| 3 | Plain HTTP mode configures HTTP/2 keepalive (30s interval, 10s timeout) | VERIFIED | `main.rs` lines 308–314: `keep_alive_interval(Some(Duration::from_secs(30)))` and `keep_alive_timeout(Duration::from_secs(10))` in the plain `else` branch |
| 4 | Runner agent forwards task metadata as HTTP headers when dispatching to local service | VERIFIED | `agent.rs` lines 300–304: `for (key, value) in &assignment.metadata { let header_name = format!("X-Meta-{}", key); request = request.header(&header_name, value); }` |
| 5 | mTLS client cert fingerprint is mapped to authorized services via gateway.toml config | VERIFIED | `config.rs` lines 40–45: `MtlsIdentityConfig { fingerprints: HashMap<String, Vec<String>> }`; `gateway.toml` lines 9–13: commented-out example section present |
| 6 | gRPC requests from mTLS clients are checked against the fingerprint-to-services mapping | VERIFIED | `auth.rs` lines 229–254: `NodeTokenAuthLayer` checks `mtls_identity.fingerprints` after token validation; `cert_fingerprint` helper at line 21; `check_mtls_identity` at line 29 |
| 7 | A timed-out task is marked failed with error_message containing "timed out" — observable via task status query | VERIFIED | `reaper/mod.rs` line 107: `error_msg = format!("task timed out: node did not report result within {}s", ...)` with `HSET state failed error_message <error_msg>` |
| 8 | `reap_timed_out_tasks` is publicly accessible for direct test invocation | VERIFIED | `reaper/mod.rs` line 37: `pub async fn reap_timed_out_tasks(state: &AppState) -> Result<u64, GatewayError>` |
| 9 | A sample service binary exists that receives tasks from the runner agent and returns results | VERIFIED | `gateway/examples/sample_service.rs` exists: POST /execute handler, echoes payload, uses hyper 1.x, zero gateway imports |
| 10 | The sample service supports simulated delay via metadata header and pairs with agent default dispatch URL | VERIFIED | `sample_service.rs` line 60: reads `X-Meta-simulate_delay_ms`; line 33: `default_value = "8090"` matching agent `--dispatch-url` default |

**Score:** 10/10 truths verified

---

### Required Artifacts

#### Plan 07-01 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `proto/src/gateway.proto` | `callback_url` field 4 on SubmitTaskRequest; `node_id` field 5 and `service_name` field 6 on ReportResultRequest | VERIFIED | Lines 22, 68–69 — all three fields present at correct field numbers |
| `gateway/src/grpc/poll.rs` | `update_in_flight_tasks` with `-1` delta in `report_result` | VERIFIED | Lines 250–257 — decrement call with delta `-1`, guarded by `!is_empty` checks |
| `gateway/src/grpc/submit.rs` | `callback_url` resolution and storage; `validate_callback_url` call | VERIFIED | Lines 76–91 — resolves per-task override over per-key default, validates, `hset`s to Redis |
| `gateway/src/main.rs` | `keep_alive_interval` on plain HTTP path | VERIFIED | Lines 308–314 — both `keep_alive_interval(30s)` and `keep_alive_timeout(10s)` in the `else` branch; two total occurrences (TLS + plain) |
| `gateway/src/bin/agent.rs` | `node_id` and `service_name` in `ReportResultRequest`; metadata forwarded as `X-Meta-` headers | VERIFIED | Lines 247–265: both success and error `ReportResultRequest` include `node_id` and `service_name`; lines 300–304: `X-Meta-{key}` header loop |

#### Plan 07-02 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `gateway/src/config.rs` | `MtlsIdentityConfig` struct with `fingerprints: HashMap<String, Vec<String>>`; `mtls_identity` on `GrpcConfig` | VERIFIED | Lines 40–45: struct defined with `#[derive(Default)]`; line 35: `pub mtls_identity: MtlsIdentityConfig` on `GrpcConfig`; `default_grpc()` at line 219 includes it |
| `gateway/src/grpc/auth.rs` | `cert_fingerprint` fn; `Sha256::digest`; mTLS identity check in `NodeTokenAuthLayer` | VERIFIED | Line 21: `fn cert_fingerprint`; line 13: `use sha2::{Sha256, Digest}`; lines 229–254: identity check block |
| `gateway/src/reaper/mod.rs` | `pub async fn reap_timed_out_tasks` | VERIFIED | Line 37: function is `pub` |
| `gateway/tests/reaper_callback_integration_test.rs` | `test_reaper_full_loop_marks_timed_out_task_failed` that calls `reap_timed_out_tasks` and asserts task state | VERIFIED | Lines 251–320: test submits task, claims it, waits 2s, calls `xgent_gateway::reaper::reap_timed_out_tasks(&state)`, asserts `reaped == 1`, asserts `state == "failed"`, asserts `error_message.contains("timed out")` |

#### Plan 07-03 Artifacts

| Artifact | Expected | Status | Details |
|----------|----------|--------|---------|
| `gateway/examples/sample_service.rs` | Standalone echo service; `X-Meta-simulate_delay_ms` header; port 8090; no `use xgent_gateway` | VERIFIED | All four criteria met: POST /execute at line 42, delay header at line 60, port default 8090 at line 33, no gateway imports (grep confirmed empty) |
| `gateway/Cargo.toml` | `http-body-util` and `bytes` dependencies present | VERIFIED | Lines 42–43: `http-body-util = "0.1"` and `bytes = "1"` |

---

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `gateway/src/grpc/poll.rs` | `gateway/src/registry/node_health.rs` | `update_in_flight_tasks` call with `delta=-1` | WIRED | Lines 250–257: call present with `-1` argument; guarded by non-empty node_id and service_name |
| `gateway/src/grpc/submit.rs` | `gateway/src/callback.rs` | `validate_callback_url` call | WIRED | Line 84: `crate::callback::validate_callback_url(url)` called before storing |
| `gateway/src/bin/agent.rs` | dispatch_task HTTP POST | metadata forwarded as `X-Meta-{key}` headers | WIRED | Lines 300–304: loop over `assignment.metadata`, header name formed as `format!("X-Meta-{}", key)` |
| `gateway/src/grpc/auth.rs` | `gateway/src/config.rs` | `MtlsIdentityConfig` lookup from `GatewayConfig` | WIRED | Line 17: `use crate::config::MtlsIdentityConfig`; line 230: `&state.config.grpc.mtls_identity` |
| `gateway/src/grpc/auth.rs` | tonic peer_certs | `request.extensions().get::<Arc<Vec<Certificate>>>()` | WIRED | Lines 234–235: extension lookup for `Arc<Vec<tonic::transport::Certificate>>`; skips with `debug!` log if absent (allows plaintext dev mode) |
| `gateway/examples/sample_service.rs` | `gateway/src/bin/agent.rs` | HTTP POST to `/execute` endpoint | WIRED | `sample_service.rs` line 42: matches `POST /execute`; `agent.rs` default dispatch URL `http://localhost:8090/execute` at line 37 |

---

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|------------|-------------|--------|----------|
| NODE-05 | 07-01, 07-02 | Gateway tracks node health via heartbeat (last poll time, stale detection) — specifically in-flight counter accuracy | SATISFIED | `poll.rs`: `update_in_flight_tasks(..., -1)` called on every `report_result`; mTLS identity mapping also contributes to node auth health tracking |
| OBSV-03 | 07-01 | Node health dashboard data available via admin API (active nodes, last seen, in-flight tasks) | SATISFIED | In-flight counter now correctly decremented (was never decremented before Phase 07); the admin dashboard reads this counter from Redis, so values will now reflect reality |
| RSLT-03 | 07-01 | Client can optionally provide a callback URL at submission for result delivery | SATISFIED | `gateway.proto` adds `callback_url = 4` to `SubmitTaskRequest`; `grpc/submit.rs` resolves, validates, and stores it — matching the existing HTTP path behavior |
| INFR-06 | 07-01, 07-03 | Gateway configures HTTP/2 keepalive pings to prevent silent connection death through NAT/LB | SATISFIED | `main.rs` now has keepalive on BOTH TLS and plain HTTP paths (2 occurrences of `keep_alive_interval`); sample service verifies E2E via agent default port |

All four requirement IDs from plan frontmatter are accounted for. No orphaned requirements found for Phase 07 in REQUIREMENTS.md.

---

### Anti-Patterns Found

No blockers or warnings found. Scan of all 9 phase-modified files produced zero matches for: TODO, FIXME, XXX, HACK, PLACEHOLDER, placeholder text, `return null`, empty handlers, or hardcoded stub returns.

One pre-existing compiler warning noted in 07-03-SUMMARY.md: `has_in_flight` value assigned but never read in `agent.rs`. This is a false positive from `tokio::select!` macro expansion — the value IS passed to `graceful_drain` at line 228. Not a phase-introduced issue and not a stub pattern.

---

### Human Verification Required

#### 1. mTLS Identity Enforcement Under Real TLS

**Test:** Start gateway with mTLS enabled and a non-empty `[grpc.mtls_identity.fingerprints]` config. Present a client certificate whose fingerprint is in the map but not for the requested service.
**Expected:** Gateway returns `PERMISSION_DENIED "certificate not authorized for this service"`.
**Why human:** Requires a running gateway with real TLS certs; cert fingerprint computation from DER and the tonic extension wiring cannot be exercised via grep/static analysis alone.

#### 2. End-to-End Sample Service + Agent + Gateway

**Test:** Run `cargo run -p xgent-gateway --example sample_service`, run the runner agent against a live gateway, submit a task with `metadata: {simulate_delay_ms: "200"}`.
**Expected:** Agent forwards the `X-Meta-simulate_delay_ms: 200` header; sample service delays 200ms; result is echoed back to the gateway; callback (if configured) fires.
**Why human:** Full lifecycle requires live gateway + Redis + agent + sample service concurrently; the metadata-header-to-delay path spans three binaries.

#### 3. in_flight_tasks Counter Accuracy Under Load

**Test:** Submit 10 tasks to a service, have 2 nodes poll and complete them, query `/v1/admin/services/<name>` to inspect per-node `in_flight_tasks`.
**Expected:** Counter reaches 0 after all tasks complete; no negative values; no stale non-zero values.
**Why human:** Requires live Redis, concurrent node agents, and admin API; verifying counter correctness is a runtime property.

---

### Gaps Summary

No gaps. All 10 observable truths verified, all required artifacts exist and are substantive, all key links are wired. Requirements NODE-05, OBSV-03, RSLT-03, and INFR-06 are all satisfied by implementation evidence.

The only notable subtlety is the `test_reaper_marks_timed_out_task_as_failed` test (an older test from Plan 07-02 precursor work) — it calls `reap_timed_out_tasks` **indirectly** through a comment and then checks XPENDING state only (lines 116–144). However the new `test_reaper_full_loop_marks_timed_out_task_failed` (lines 251–320) does the full verification: calls `reap_timed_out_tasks` directly and asserts `state == "failed"` and `error_message.contains("timed out")`. The plan-02 acceptance criterion is met by the new test.

---

_Verified: 2026-03-22_
_Verifier: Claude (gsd-verifier)_
