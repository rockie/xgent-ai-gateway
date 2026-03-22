---
phase: 07-integration-fixes-sample-service-cleanup
plan: 03
subsystem: infra
tags: [hyper, sample-service, echo, tech-debt, end-to-end-testing]

# Dependency graph
requires:
  - phase: 01-core-queue-loop
    provides: "Runner agent dispatch pattern (POST /execute with X-Task-Id header)"
provides:
  - "Standalone sample service binary for end-to-end testing"
  - "Tech debt audit closure (all 9 items verified)"
affects: []

# Tech tracking
tech-stack:
  added: [http-body-util, bytes]
  patterns: [hyper-1.x-standalone-example]

key-files:
  created:
    - gateway/examples/sample_service.rs
  modified:
    - gateway/Cargo.toml

key-decisions:
  - "Used hyper 1.x directly (no axum) for standalone example with zero gateway imports"
  - "All 9 tech debt items verified: 5 already resolved, 2 handled by Plan 02, 1 kept-as-is (D-08), 1 false positive"

patterns-established:
  - "Example binary pattern: gateway/examples/ with standalone hyper 1.x service_fn"

requirements-completed: [INFR-06]

# Metrics
duration: 3min
completed: 2026-03-22
---

# Phase 07 Plan 03: Sample Service and Tech Debt Summary

**Standalone echo service (hyper 1.x) on POST /execute with optional X-Meta-simulate_delay_ms delay, plus full tech debt audit closure across all 9 items**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-22T13:19:03Z
- **Completed:** 2026-03-22T13:21:39Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Created sample_service.rs: standalone HTTP echo server on port 8090 (matches agent default dispatch URL)
- Supports simulated processing delay via X-Meta-simulate_delay_ms header for end-to-end testing
- Verified all 9 tech debt items from v1.0 audit -- all resolved, handled, or documented

## Task Commits

Each task was committed atomically:

1. **Task 1: Sample service echo binary** - `8d9866b` (feat)
2. **Task 2: Tech debt verification and cleanup** - No code changes needed (verification only)

## Files Created/Modified
- `gateway/examples/sample_service.rs` - Standalone echo service for end-to-end testing with runner agent
- `gateway/Cargo.toml` - Added http-body-util and bytes dependencies

## Decisions Made
- Used hyper 1.x directly with service_fn for the simplest possible standalone example
- No gateway crate imports -- example is fully standalone per D-02
- Kept has_in_flight warning as-is: it IS used (passed to graceful_drain), warning is a false positive from tokio::select! macro expansion

## Tech Debt Verification Results

| # | Item | Status | Evidence |
|---|------|--------|----------|
| 1 | NODE-02 marked [x] in REQUIREMENTS.md | Resolved | REQUIREMENTS.md shows `[~]` Deferred (D-13) |
| 2 | cleanup_redis_keys no-op stub | Kept as-is (D-08) | Stub exists in integration_test.rs line 141 |
| 3 | Unused _opts in queue/redis.rs | Resolved | grep finds no _opts in file |
| 4 | Unused AsyncCommands import in report_result | Resolved | grep finds no AsyncCommands in queue/redis.rs |
| 5 | mTLS no per-client identity | Handled by Plan 02 (D-11/D-12) | N/A |
| 6 | has_in_flight never read in agent.rs | False positive | Used on line 228 (passed to graceful_drain); compiler warning is from select! macro |
| 7 | Reaper test only validates precondition | Handled by Plan 02 (D-09) | N/A |
| 8 | f.get_name() deprecated in metrics.rs | Resolved | grep finds no get_name in metrics.rs |
| 9 | Unused AsyncCommands import in test | Resolved | Import IS used for .hset/.hget calls in reaper test |

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added http-body-util and bytes dependencies**
- **Found during:** Task 1 (Sample service echo binary)
- **Issue:** http-body-util and bytes crates not in gateway/Cargo.toml dependencies
- **Fix:** Added `http-body-util = "0.1"` and `bytes = "1"` to [dependencies]
- **Files modified:** gateway/Cargo.toml
- **Verification:** cargo build succeeds
- **Committed in:** 8d9866b (Task 1 commit)

**2. [Rule 1 - Bug] Fixed borrow-after-move in request handler**
- **Found during:** Task 1 (Sample service echo binary)
- **Issue:** task_id borrowed from request headers, then request consumed by collect()
- **Fix:** Added .to_string() to clone task_id before request is consumed
- **Files modified:** gateway/examples/sample_service.rs
- **Verification:** cargo build succeeds
- **Committed in:** 8d9866b (Task 1 commit)

---

**Total deviations:** 2 auto-fixed (1 blocking, 1 bug)
**Impact on plan:** Both fixes necessary for compilation. No scope creep.

## Issues Encountered
- One remaining compiler warning: `has_in_flight` value assigned but never read in agent.rs. This is a false positive from `tokio::select!` macro expansion -- the value IS read when passed to `graceful_drain`. Not addressed as it would reduce code clarity.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Sample service ready for pairing with runner agent in end-to-end demos
- All tech debt items from v1.0 audit verified and documented
- Phase 07 complete

---
*Phase: 07-integration-fixes-sample-service-cleanup*
*Completed: 2026-03-22*

## Self-Check: PASSED
- gateway/examples/sample_service.rs: FOUND
- 07-03-SUMMARY.md: FOUND
- Commit 8d9866b: FOUND
