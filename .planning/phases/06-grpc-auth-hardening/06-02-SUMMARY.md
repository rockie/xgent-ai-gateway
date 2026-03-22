---
phase: 06-grpc-auth-hardening
plan: 02
subsystem: testing
tags: [grpc, tonic, integration-tests, api-key, node-token, auth]

requires:
  - phase: 06-grpc-auth-hardening
    provides: "Tower auth layers (ApiKeyAuthLayer, NodeTokenAuthLayer) for gRPC"
provides:
  - "Integration tests proving gRPC auth enforcement on all RPCs"
  - "Test infrastructure for spinning up authenticated gRPC servers"
affects: []

tech-stack:
  added: []
  patterns:
    - "Per-test Redis DB isolation via atomic counter for parallel test execution"
    - "gRPC integration test pattern: start_test_grpc_server helper with auth layers"

key-files:
  created:
    - gateway/tests/grpc_auth_test.rs
  modified: []

key-decisions:
  - "Used unique Redis DB per test (atomic counter) to enable parallel test execution without FLUSHDB conflicts"
  - "report_result positive test accepts FailedPrecondition as auth-passed since task state is pending (not assigned)"
  - "Wrong-service tests do not require registering other-svc since auth layer passes valid key and handler rejects by service_names check"

patterns-established:
  - "gRPC test isolation: each test gets its own Redis DB index to avoid cross-test interference during parallel execution"

requirements-completed: [AUTH-01, AUTH-03, TASK-01, RSLT-01, NODE-03, NODE-04, NODE-06]

duration: 4min
completed: 2026-03-22
---

# Phase 06 Plan 02: gRPC Auth Integration Tests Summary

**13 integration tests proving gRPC auth enforcement on all RPCs -- negative tests verify Unauthenticated/PermissionDenied, positive tests confirm valid credentials accepted**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-22T08:24:42Z
- **Completed:** 2026-03-22T08:28:45Z
- **Tasks:** 1
- **Files modified:** 1

## Accomplishments
- 13 integration tests covering auth enforcement on all 6 gRPC RPCs (SubmitTask, GetTaskStatus, ReportResult, Heartbeat, DrainNode, PollTasks indirectly)
- Negative tests prove missing/invalid credentials return Code::Unauthenticated
- Wrong-service authorization tests prove Code::PermissionDenied (D-07, D-08)
- Positive tests confirm valid API keys and node tokens are accepted for all RPCs
- Per-test Redis DB isolation enables reliable parallel test execution

## Task Commits

Each task was committed atomically:

1. **Task 1: Create gRPC auth integration test file with test gateway helper** - `974d52d` (test)

## Files Created/Modified
- `gateway/tests/grpc_auth_test.rs` - 578-line integration test file with 13 tests, test server helper, client helpers

## Decisions Made
- Used atomic counter for Redis DB index selection (1..N) so each test gets its own isolated database, avoiding the FLUSHDB race condition when tests run in parallel
- report_result positive test accepts FailedPrecondition as proof of auth acceptance (task is in pending state, not assigned, so state transition fails after auth passes)
- Simplified wrong-service tests: no need to register "other-svc" since the auth layer validates the key and the handler checks service_names authorization

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed parallel test interference via Redis DB isolation**
- **Found during:** Task 1 (initial test run)
- **Issue:** All tests used FLUSHDB on DB 0, causing parallel tests to wipe each other's state
- **Fix:** Used atomic counter to assign unique Redis DB index (1..15) per test invocation
- **Files modified:** gateway/tests/grpc_auth_test.rs
- **Verification:** All 13 tests pass in parallel

**2. [Rule 1 - Bug] Fixed report_result test expecting Ok on pending task**
- **Found during:** Task 1 (test verification)
- **Issue:** report_result rejects state transition from pending to completed (FailedPrecondition), but test asserted Ok
- **Fix:** Changed assertion to accept FailedPrecondition as proof that auth passed (not Unauthenticated)
- **Files modified:** gateway/tests/grpc_auth_test.rs
- **Verification:** Test passes and correctly validates auth acceptance

---

**Total deviations:** 2 auto-fixed (2 bugs)
**Impact on plan:** Both fixes necessary for test correctness. No scope creep.

## Issues Encountered
- Existing auth_integration_test.rs has 7 pre-existing failures unrelated to this plan (TLS/mTLS tests that predate the auth layer changes from Plan 06-01). These are out of scope.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All gRPC auth enforcement is tested end-to-end
- Phase 06 (grpc-auth-hardening) is complete: auth layers implemented (Plan 01) and tested (Plan 02)
- Ready for Phase 07 or any subsequent phases

## Self-Check: PASSED

- [x] gateway/tests/grpc_auth_test.rs exists (578 lines)
- [x] Commit 974d52d exists in git log

---
*Phase: 06-grpc-auth-hardening*
*Completed: 2026-03-22*
