---
phase: 16-examples-and-end-to-end-validation
plan: 01
subsystem: examples
tags: [sample-service, hyper, yaml, cli, sync-api, async-api, agent-config]

# Dependency graph
requires:
  - phase: 13-agent-config-and-cli-executor
    provides: AgentConfig with load_config YAML parsing and ExecutionMode enum
  - phase: 14-sync-api-executor
    provides: SyncApiSection config shape
  - phase: 15-async-api-executor
    provides: AsyncApiSection with submit/poll/completion config shapes
provides:
  - Extended sample_service with /sync and /async/* mock endpoints
  - CLI echo.sh script for arg and stdin mode testing
  - Example YAML configs for all three execution modes (cli, sync-api, async-api)
affects: [16-02, 16-03, end-to-end-testing]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "Per-route handler functions with shared state via Arc<StdMutex<HashMap>>"
    - "3-poll completion lifecycle for async job simulation"

key-files:
  created:
    - examples/cli-service/echo.sh
    - examples/cli-service/agent-arg.yaml
    - examples/cli-service/agent-stdin.yaml
    - examples/sync-api-service/agent.yaml
    - examples/async-api-service/agent.yaml
  modified:
    - gateway/examples/sample_service.rs

key-decisions:
  - "Used std::sync::Mutex (not tokio) for shared job state since lock hold time is microseconds"
  - "3-poll threshold for async job completion to exercise polling lifecycle"

patterns-established:
  - "Example configs use localhost:8090 to match sample_service default port"
  - "Example configs use dev-token as placeholder for gateway auth token"

requirements-completed: [EXMP-01, EXMP-02, EXMP-03]

# Metrics
duration: 2min
completed: 2026-03-24
---

# Phase 16 Plan 01: Example Configs and Sample Service Summary

**Extended sample_service with /sync and /async endpoints, created example YAML configs and CLI echo script for all three agent execution modes**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-24T14:13:26Z
- **Completed:** 2026-03-24T14:15:30Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Extended sample_service.rs with POST /sync (JSON echo), POST /async/submit (job creation), and GET /async/status/:id (3-poll completion lifecycle)
- Created executable echo.sh script that handles both arg and stdin input modes
- Created 4 YAML agent configs across 3 example directories covering cli (arg), cli (stdin), sync-api, and async-api modes

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend sample_service with /sync and /async endpoints** - `943152b` (feat)
2. **Task 2: Create example directories with configs and echo script** - `52c7ff9` (feat)

## Files Created/Modified
- `gateway/examples/sample_service.rs` - Extended with /sync, /async/submit, /async/status/:id endpoints and shared job state
- `examples/cli-service/echo.sh` - Zero-dependency bash echo script for CLI mode examples
- `examples/cli-service/agent-arg.yaml` - CLI arg-mode agent config
- `examples/cli-service/agent-stdin.yaml` - CLI stdin-mode agent config
- `examples/sync-api-service/agent.yaml` - Sync-API agent config pointing to localhost:8090/sync
- `examples/async-api-service/agent.yaml` - Async-API agent config with submit/poll/completion conditions

## Decisions Made
- Used std::sync::Mutex (not tokio::sync::Mutex) for shared job state since lock hold time is microseconds
- Set 3-poll threshold for async job completion to provide a realistic polling lifecycle demonstration

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Sample service ready for end-to-end testing in Plan 02
- All example configs ready for validation against load_config parser
- echo.sh ready for CLI executor testing

## Self-Check: PASSED

All 6 files found. Both commit hashes verified.

---
*Phase: 16-examples-and-end-to-end-validation*
*Completed: 2026-03-24*
