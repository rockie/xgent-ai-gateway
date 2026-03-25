---
phase: 18-tech-debt-cleanup
plan: 02
subsystem: api
tags: [rust, refactoring, redis, node-health, deduplication]

requires:
  - phase: 18-tech-debt-cleanup-01
    provides: "ServiceConfig import path (moved to node_health module in plan 01)"
provides:
  - "Deduplicated node health fetching -- single canonical source in registry::node_health::get_nodes_for_service"
  - "~65 lines of duplicated Redis query code removed from admin.rs and metrics.rs"
affects: [admin-api, metrics, node-health]

tech-stack:
  added: []
  patterns: ["Canonical data access via registry module functions instead of inline Redis queries"]

key-files:
  created: []
  modified:
    - gateway/src/http/admin.rs
    - gateway/src/metrics.rs

key-decisions:
  - "Used unwrap_or_else(|_| Vec::new()) in metrics.rs for error tolerance matching previous behavior"

patterns-established:
  - "Node health queries go through get_nodes_for_service -- never manual SMEMBERS/HGETALL"

requirements-completed: [TD-03]

duration: 2min
completed: 2026-03-25
---

# Phase 18 Plan 02: Deduplicate Node Health Queries Summary

**Replaced manual Redis SMEMBERS/HGETALL/derive_health_state calls in admin.rs and metrics.rs with canonical get_nodes_for_service, removing ~65 lines of duplicated code**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-25T01:35:16Z
- **Completed:** 2026-03-25T01:37:29Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- admin.rs get_service_detail now delegates to get_nodes_for_service (~27 lines removed)
- metrics.rs refresh_gauges now delegates to get_nodes_for_service (~19 lines removed)
- Node health derivation logic exists in exactly one place (registry::node_health)
- Removed unused derive_health_state import from admin.rs
- All 149 lib tests pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Replace manual node queries in admin.rs get_service_detail** - `10d4726` (refactor)
2. **Task 2: Replace manual node queries in metrics.rs refresh_gauges** - `3854471` (refactor)

## Files Created/Modified
- `gateway/src/http/admin.rs` - get_service_detail refactored to use get_nodes_for_service; derive_health_state import removed
- `gateway/src/metrics.rs` - refresh_gauges refactored to use get_nodes_for_service; ~19 lines of manual Redis code removed

## Decisions Made
- Used `unwrap_or_else(|_| Vec::new())` in metrics.rs refresh_gauges for error tolerance, matching the previous behavior where individual node Redis call failures were silently ignored

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Plan 18-03 can proceed -- all code compiles and tests pass
- Node health derivation is now fully canonical

---
## Self-Check: PASSED

All files exist, all commits verified.

---
*Phase: 18-tech-debt-cleanup*
*Completed: 2026-03-25*
