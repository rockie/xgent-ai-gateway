---
phase: 06-grpc-auth-hardening
plan: 01
subsystem: auth
tags: [grpc, tower, tonic, api-key, node-token, middleware]

requires:
  - phase: 02-auth-and-tls
    provides: "API key and node token auth functions (extract, hash, lookup, validate)"
  - phase: 05-observability-and-packaging
    provides: "Prometheus errors_total CounterVec for auth failure metrics"
provides:
  - "Tower Service auth layers for gRPC (ApiKeyAuthLayer, NodeTokenAuthLayer)"
  - "Per-service authorization in gRPC submit and status handlers"
  - "ValidatedNodeAuth extension type for node identity propagation"
  - "All gRPC RPCs auth-gated at parity with HTTP endpoints"
affects: [06-grpc-auth-hardening]

tech-stack:
  added: []
  patterns:
    - "Tower Service wrapper pattern for gRPC auth (clone inner + async call)"
    - "Request extensions for propagating auth metadata to handlers"
    - "NamedService delegation for tonic add_service compatibility"

key-files:
  created:
    - gateway/src/grpc/auth.rs
  modified:
    - gateway/src/grpc/mod.rs
    - gateway/src/grpc/submit.rs
    - gateway/src/grpc/poll.rs
    - gateway/src/main.rs

key-decisions:
  - "Used axum::http re-export rather than adding http crate directly to Cargo.toml"
  - "NodeTokenAuthLayer uses x-service-name metadata header for service scoping (per D-09/D-10)"
  - "ValidatedNodeAuth struct for type-safe node identity propagation through extensions"

patterns-established:
  - "Tower Service wrapper for gRPC auth: implement Service<Request<Body>> + NamedService, clone inner per Tower contract"
  - "Two-phase gRPC auth: Tower layer for authentication, handler for per-service authorization"

requirements-completed: [AUTH-01, AUTH-03, TASK-01, RSLT-01, NODE-03, NODE-04, NODE-06]

duration: 6min
completed: 2026-03-22
---

# Phase 06 Plan 01: gRPC Auth Layers Summary

**Tower Service auth wrappers (ApiKeyAuthLayer, NodeTokenAuthLayer) enforcing API key and node token authentication on all gRPC RPCs with per-service authorization in handlers**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-22T08:14:52Z
- **Completed:** 2026-03-22T08:21:23Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Created two Tower Service auth layers in grpc/auth.rs that wrap tonic service servers
- All gRPC RPCs now auth-gated: SubmitTask/GetTaskStatus via API key, PollTasks/ReportResult/Heartbeat/DrainNode via node token
- Per-service authorization enforced in submit_task (D-07), get_task_status (D-08), poll_tasks/heartbeat/drain_node service scope check (D-10)
- Removed inline auth from poll_tasks and replaced with Tower layer (D-04)
- Auth failures return generic "unauthorized" with no information leakage (D-11) and increment errors_total metrics (D-14)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Tower auth layers in grpc/auth.rs** - `7bffd14` (feat)
2. **Task 2: Wire auth layers in main.rs, add service authz to handlers, refactor poll_tasks** - `9d0e7d8` (feat)

## Files Created/Modified
- `gateway/src/grpc/auth.rs` - Tower Service auth layers (ApiKeyAuthLayer, NodeTokenAuthLayer, ValidatedNodeAuth)
- `gateway/src/grpc/mod.rs` - Added auth module export and re-exports
- `gateway/src/grpc/submit.rs` - Added ClientMetadata extraction and per-service authorization checks
- `gateway/src/grpc/poll.rs` - Replaced inline auth with ValidatedNodeAuth extension, added service scope checks
- `gateway/src/main.rs` - Wrapped TaskServiceServer and NodeServiceServer with auth layers

## Decisions Made
- Used `axum::http` re-export for HTTP types rather than adding the `http` crate as a direct dependency
- NodeTokenAuthLayer extracts service name from `x-service-name` gRPC metadata header for token validation scoping
- Created `ValidatedNodeAuth` struct to propagate validated node identity through request extensions

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Initial compilation failed due to missing `http` crate import -- resolved by using `axum::http` re-export
- Prometheus `with_label_values` type mismatch between `&String` and `&str` -- resolved by using `.as_str()` conversion

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- All gRPC RPCs are now auth-gated at parity with HTTP endpoints
- Ready for Plan 06-02 (integration tests for gRPC auth)

---
*Phase: 06-grpc-auth-hardening*
*Completed: 2026-03-22*
