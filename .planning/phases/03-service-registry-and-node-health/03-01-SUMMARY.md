---
phase: 03-service-registry-and-node-health
plan: 01
subsystem: api
tags: [redis, grpc, axum, protobuf, service-registry]

requires:
  - phase: 02-authentication-and-tls
    provides: Admin endpoints pattern, auth Redis connection, AppState with auth_conn
provides:
  - Service CRUD operations (register, deregister, get, list, exists) against Redis
  - Service admin HTTP endpoints (POST/GET/DELETE /v1/admin/services)
  - submit_task gating on service registration (HTTP and gRPC)
  - NodeHealthState types and derive_health_state function
  - Background cleanup logic for service deregistration
  - Proto definitions for Heartbeat and DrainNode RPCs (stub implementations)
affects: [03-02-PLAN, 03-03-PLAN]

tech-stack:
  added: []
  patterns: [on-demand health derivation, background cleanup via tokio::spawn, service registry gating]

key-files:
  created:
    - gateway/src/registry/mod.rs
    - gateway/src/registry/service.rs
    - gateway/src/registry/node_health.rs
    - gateway/src/registry/cleanup.rs
  modified:
    - proto/src/gateway.proto
    - gateway/src/error.rs
    - gateway/src/config.rs
    - gateway/src/lib.rs
    - gateway/src/http/admin.rs
    - gateway/src/queue/redis.rs
    - gateway/src/main.rs
    - gateway/src/http/submit.rs
    - gateway/src/grpc/submit.rs
    - gateway/src/grpc/poll.rs

key-decisions:
  - "Service registry check added to both HTTP and gRPC submit paths rather than inside RedisQueue"
  - "Consumer group creation moved from submit_task to service registration for explicit lifecycle"
  - "Heartbeat and DrainNode RPCs stubbed in poll.rs for proto compatibility, full implementation in Plan 03-02"

patterns-established:
  - "Registry pattern: Redis hash per service (service:{name}) + index set (services:index)"
  - "On-demand health derivation: derive_health_state computes from last_seen, no background reaper"
  - "Async deregistration: DELETE returns 202 immediately, cleanup runs via tokio::spawn"

requirements-completed: [SRVC-01, SRVC-03, SRVC-04, NODE-03]

duration: 8min
completed: 2026-03-21
---

# Phase 03 Plan 01: Service Registry Foundation Summary

**Service registry with Redis-backed CRUD, admin HTTP endpoints, proto Heartbeat/DrainNode RPCs, and submit_task gating on registered services**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-21T13:47:27Z
- **Completed:** 2026-03-21T13:56:17Z
- **Tasks:** 2
- **Files modified:** 14

## Accomplishments
- Service CRUD operations (register, get, list, exists, delete) implemented against Redis with hash-per-service pattern
- Admin HTTP endpoints wired: POST /v1/admin/services (201), DELETE (202 async), GET list, GET detail with live node health
- submit_task gated on service registration in both HTTP and gRPC paths -- unregistered services rejected with 404/NOT_FOUND
- NodeHealthState types with on-demand derive_health_state function and 4 unit tests
- Proto extended with Heartbeat, DrainNode RPCs, and NodeHealthState enum
- Background cleanup for service deregistration (tasks, tokens, streams, nodes, config)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create registry module with service CRUD, cleanup, node health types, and proto extensions** - `10949d2` (feat)
2. **Task 2: Wire service admin HTTP endpoints and gate submit_task on service registry** - `9d2a848` (feat)

## Files Created/Modified
- `gateway/src/registry/mod.rs` - Registry module root exposing service, node_health, cleanup submodules
- `gateway/src/registry/service.rs` - Service CRUD against Redis (register, get, list, exists, delete)
- `gateway/src/registry/node_health.rs` - ServiceConfig, NodeHealthState, NodeStatus types and derive_health_state
- `gateway/src/registry/cleanup.rs` - Full deregistration cleanup: fail tasks, delete tokens, destroy stream, clean nodes
- `proto/src/gateway.proto` - Heartbeat, DrainNode RPCs and NodeHealthState enum added to NodeService
- `gateway/src/error.rs` - ServiceAlreadyExists variant mapped to CONFLICT/already_exists
- `gateway/src/config.rs` - ServiceDefaultsConfig with node_stale_after_secs, drain_timeout_secs, task_timeout_secs, max_retries
- `gateway/src/lib.rs` - Added pub mod registry
- `gateway/src/http/admin.rs` - Service admin endpoints: register, deregister, list, get_detail
- `gateway/src/queue/redis.rs` - Exposed conn(), removed lazy consumer group creation from submit_task
- `gateway/src/main.rs` - Registered service admin routes
- `gateway/src/http/submit.rs` - Added service_exists check before submit
- `gateway/src/grpc/submit.rs` - Added service_exists check before submit
- `gateway/src/grpc/poll.rs` - Stub implementations for heartbeat and drain_node RPCs

## Decisions Made
- Service registry check placed in HTTP/gRPC handlers rather than inside RedisQueue to avoid coupling queue logic to registry
- Consumer group creation moved from submit_task to service registration for explicit lifecycle management
- Heartbeat and DrainNode RPCs stubbed (not implemented) in this plan -- full implementation deferred to Plan 03-02

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Added stub implementations for Heartbeat and DrainNode RPCs**
- **Found during:** Task 1 (proto extension)
- **Issue:** Adding RPCs to proto generated trait methods that the existing GrpcNodeService must implement, causing compilation failure
- **Fix:** Added stub heartbeat (returns acknowledged=true) and drain_node (returns unimplemented) to poll.rs
- **Files modified:** gateway/src/grpc/poll.rs
- **Verification:** cargo build succeeds
- **Committed in:** 10949d2 (Task 1 commit)

**2. [Rule 3 - Blocking] Exposed RedisQueue.conn() method**
- **Found during:** Task 2 (register_service needs queue connection for consumer group)
- **Issue:** register_service needs a queue connection to create consumer groups, but RedisQueue.conn was private
- **Fix:** Added pub fn conn() returning reference to the non-blocking connection
- **Files modified:** gateway/src/queue/redis.rs
- **Verification:** cargo build succeeds
- **Committed in:** 9d2a848 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both auto-fixes necessary for compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Registry module complete, ready for Plan 03-02 (node heartbeat tracking and drain implementation)
- Proto definitions for Heartbeat/DrainNode are in place with stub RPCs
- ServiceConfig and NodeHealthState types ready for node health tracking

## Self-Check: PASSED

All 4 created files verified. Both commit hashes (10949d2, 9d2a848) confirmed in git log.

---
*Phase: 03-service-registry-and-node-health*
*Completed: 2026-03-21*
