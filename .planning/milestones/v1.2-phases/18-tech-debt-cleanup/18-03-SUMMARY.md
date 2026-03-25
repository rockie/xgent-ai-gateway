---
phase: 18-tech-debt-cleanup
plan: 03
subsystem: gateway
tags: [tracing, error-handling, refactoring]

requires:
  - phase: 18-tech-debt-cleanup
    provides: "Prior plans cleaned up metrics.rs and admin.rs patterns"
provides:
  - "Deduplicated init_tracing with composable layer construction"
  - "Consistent GatewayError returns across all admin handlers"
affects: []

tech-stack:
  added: []
  patterns:
    - "tracing_subscriber Option<Layer> trick for conditional file logging"
    - "All admin handlers return GatewayError for consistent JSON error bodies"

key-files:
  created: []
  modified:
    - gateway/src/main.rs
    - gateway/src/http/admin.rs

key-decisions:
  - "Used Box-free Option<Layer> by adding file_layer before stdout_layer to share subscriber type"
  - "Reused TaskNotFound variant for not-found errors in credential handlers (maps to 404)"

patterns-established:
  - "init_tracing: file layer added to registry before stdout layer for type compatibility"

requirements-completed: [TD-04, TD-05]

duration: 7min
completed: 2026-03-25
---

# Phase 18 Plan 03: Tracing and Admin Error Cleanup Summary

**Deduplicated init_tracing from 4-arm match to 2 branches with shared file layer; standardized all admin handlers to return GatewayError**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-25T01:39:44Z
- **Completed:** 2026-03-25T01:46:16Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Refactored init_tracing from 4-arm match (67 lines) to composable layers (39 lines) -- 42% reduction
- Log file opening code now appears exactly once instead of twice
- All 3 inconsistent admin handlers (revoke_api_key, create_node_token, revoke_node_token) now return GatewayError
- Error responses from all admin endpoints now consistently produce JSON bodies

## Task Commits

Each task was committed atomically:

1. **Task 1: Refactor init_tracing to eliminate duplicated match arms** - `3919a4e` (refactor)
2. **Task 2: Standardize admin handler error types to GatewayError** - `a193ef2` (refactor)

## Files Created/Modified
- `gateway/src/main.rs` - Refactored init_tracing with composable layer construction
- `gateway/src/http/admin.rs` - Standardized error types from StatusCode to GatewayError

## Decisions Made
- Used Option<Layer> trick by ordering file_layer before stdout_layer in registry composition, avoiding the need for Box<dyn Layer> type erasure
- Reused existing TaskNotFound variant for not-found errors in revoke_api_key and revoke_node_token (it maps to 404, matching previous behavior)

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Initial attempt to box the file layer as `Box<dyn Layer<Registry>>` failed because each `.with()` call changes the subscriber type. Solved by adding the file layer to the registry before the stdout layer so both branches share the same inner subscriber type.

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Phase 18 (tech-debt-cleanup) is now complete (all 3 plans done)
- Gateway builds cleanly and all 149 tests pass

---
*Phase: 18-tech-debt-cleanup*
*Completed: 2026-03-25*
