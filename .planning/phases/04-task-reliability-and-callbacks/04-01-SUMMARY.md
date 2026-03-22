---
phase: 04-task-reliability-and-callbacks
plan: 01
subsystem: queue
tags: [redis-streams, xpending, reaper, callback, reqwest, exponential-backoff]

requires:
  - phase: 03-service-registry-and-node-health
    provides: "Service registry with list_services/get_service and ServiceConfig with task_timeout_secs"
provides:
  - "Background reaper detecting timed-out tasks via XPENDING IDLE scan"
  - "Mark-failed pipeline (HSET state=failed + XACK) for reaped tasks"
  - "Callback delivery function with exponential backoff retry"
  - "URL validation helper for callback URLs"
  - "CallbackConfig in GatewayConfig with defaults"
  - "reqwest::Client in AppState for HTTP callbacks"
  - "Assigned->Failed state transition in task state machine"
affects: [04-02-PLAN]

tech-stack:
  added: [url 2.5]
  patterns: [background-reaper-loop, xpending-idle-scan, exponential-backoff-retry]

key-files:
  created:
    - gateway/src/reaper/mod.rs
    - gateway/src/callback/mod.rs
  modified:
    - gateway/Cargo.toml
    - gateway/src/config.rs
    - gateway/src/types.rs
    - gateway/src/state.rs
    - gateway/src/lib.rs
    - gateway/src/main.rs

key-decisions:
  - "Reaper skips first tick to avoid reaping at startup"
  - "Per-service failed_count counter via Redis INCR for metrics"
  - "Callback delivery is fire-and-forget (log-only on exhausted retries)"

patterns-established:
  - "Background loop pattern: tokio::time::interval with error-per-cycle logging, never exits"
  - "XPENDING IDLE scan for timeout detection across consumer groups"
  - "Exponential backoff: delay = initial_delay_ms * 2^(attempt-1)"

requirements-completed: [LIFE-03]

duration: 4min
completed: 2026-03-22
---

# Phase 04 Plan 01: Task Reliability Infrastructure Summary

**Background reaper with XPENDING IDLE scan for timed-out task detection, callback delivery with exponential backoff, and foundational config/state changes**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-22T02:06:09Z
- **Completed:** 2026-03-22T02:09:47Z
- **Tasks:** 3
- **Files modified:** 8

## Accomplishments
- Background reaper spawned in main.rs scans all services every 30s for timed-out tasks using Redis XPENDING IDLE filter
- Timed-out tasks marked failed with descriptive error message, XACK'd, and counted via per-service Redis counter
- Callback delivery function with exponential backoff retry (configurable retries, delay, timeout)
- CallbackConfig added to GatewayConfig with sensible defaults (3 retries, 1000ms initial delay, 10s timeout)
- Assigned->Failed state transition added to task state machine for reaper use
- reqwest::Client wired into AppState for callback HTTP delivery

## Task Commits

Each task was committed atomically:

1. **Task 1: Add CallbackConfig, state machine fix, url crate, reqwest client in AppState** - `963e89c` (feat)
2. **Task 2: Create reaper module with XPENDING scan and mark-failed pipeline** - `fe49e8c` (feat)
3. **Task 3: Create callback delivery module with exponential backoff retry** - `9ab7ccc` (feat)

## Files Created/Modified
- `gateway/src/reaper/mod.rs` - Background reaper with XPENDING scan, XRANGE task_id extraction, mark-failed pipeline
- `gateway/src/callback/mod.rs` - Callback delivery with exponential backoff, URL validation
- `gateway/Cargo.toml` - Added url 2.5 dependency
- `gateway/src/config.rs` - CallbackConfig struct with defaults wired into GatewayConfig
- `gateway/src/types.rs` - Assigned->Failed state transition and test
- `gateway/src/state.rs` - http_client: reqwest::Client field in AppState
- `gateway/src/lib.rs` - pub mod reaper and pub mod callback declarations
- `gateway/src/main.rs` - reqwest client builder and reaper spawn

## Decisions Made
- Reaper skips first tick to avoid reaping at startup (tasks assigned <30s ago are not yet timed out)
- Per-service failed_count counter via Redis INCR for metrics tracking
- Callback delivery is fire-and-forget with log-only on exhausted retries (per D-19)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Reaper and callback infrastructure ready for Plan 04-02 to wire callback URLs into task submission and result reporting
- All 48 library tests pass (5 ignored integration tests)

---
*Phase: 04-task-reliability-and-callbacks*
*Completed: 2026-03-22*
