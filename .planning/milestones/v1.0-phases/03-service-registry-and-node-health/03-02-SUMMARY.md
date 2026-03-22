---
phase: 03-service-registry-and-node-health
plan: 02
subsystem: registry
tags: [redis, grpc, heartbeat, drain, node-health]

requires:
  - phase: 03-service-registry-and-node-health plan 01
    provides: ServiceConfig, NodeStatus, NodeHealthState types, derive_health_state, service CRUD, proto definitions for Heartbeat/DrainNode

provides:
  - Node registry CRUD functions (register_or_update_node, set_node_draining, is_node_draining, mark_node_disconnected, update_in_flight_tasks, get_nodes_for_service)
  - Working Heartbeat RPC with service validation and last_seen tracking
  - Working DrainNode RPC with drain flag and timeout from service config
  - Drain-aware poll loop with passive health tracking and timeout enforcement

affects: [03-service-registry-and-node-health plan 03, admin-endpoints, node-lifecycle]

tech-stack:
  added: []
  patterns: [redis-pipeline-crud, hsetnx-for-conditional-init, passive-health-tracking, drain-timeout-enforcement]

key-files:
  created: []
  modified:
    - gateway/src/registry/node_health.rs
    - gateway/src/grpc/poll.rs

key-decisions:
  - "HSETNX for draining/in_flight_tasks/disconnected fields -- only set defaults on first registration, preserving existing state"
  - "24h TTL on node hash keys as auto-cleanup safety net for abandoned nodes"
  - "Drain timeout tracked via tokio::time::Instant in poll loop -- no additional Redis state needed"

patterns-established:
  - "Node auto-registration: nodes register on first poll cycle, no separate registration RPC needed"
  - "Passive health tracking: last_seen updated on every poll cycle and heartbeat, health derived on-demand"
  - "Drain flow: DrainNode sets flag, poll loop stops dispatching, timeout marks disconnected"

requirements-completed: [NODE-05, NODE-06]

duration: 2min
completed: 2026-03-21
---

# Phase 03 Plan 02: Node Health Tracking Summary

**Node registry CRUD with Heartbeat/DrainNode RPCs and drain-aware poll loop using Redis hash-based node tracking**

## Performance

- **Duration:** 2 min
- **Started:** 2026-03-21T13:59:32Z
- **Completed:** 2026-03-21T14:01:59Z
- **Tasks:** 2
- **Files modified:** 2

## Accomplishments
- Six node registry CRUD functions for Redis-backed node state management
- Heartbeat RPC validates service existence and updates node last_seen timestamp
- DrainNode RPC sets draining flag and returns drain_timeout_secs from service config
- Poll loop tracks last_seen on every cycle, checks drain before dispatching, enforces drain timeout, marks disconnected on stream close, and tracks in_flight_tasks

## Task Commits

Each task was committed atomically:

1. **Task 1: Add node registry CRUD functions to node_health.rs** - `86c37b1` (feat)
2. **Task 2: Implement Heartbeat/DrainNode RPCs and drain-aware poll loop** - `56e2dbd` (feat)

## Files Created/Modified
- `gateway/src/registry/node_health.rs` - Added register_or_update_node, set_node_draining, is_node_draining, mark_node_disconnected, update_in_flight_tasks, get_nodes_for_service
- `gateway/src/grpc/poll.rs` - Replaced Heartbeat/DrainNode stubs with full implementations, added drain-aware poll loop with passive health tracking

## Decisions Made
- Used HSETNX for draining, in_flight_tasks, disconnected fields so existing state is preserved on re-registration
- 24h TTL on node hash keys as safety net for abandoned nodes
- Drain timeout tracked in-memory via tokio::time::Instant rather than storing drain_started_at in Redis -- simpler and sufficient since timeout is per-connection

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Node health functions ready for admin GET /v1/admin/services/{name} endpoint (Plan 03-03)
- Drain flow complete: DrainNode -> poll loop stops dispatching -> timeout marks disconnected
- get_nodes_for_service ready for service detail admin endpoint

---
*Phase: 03-service-registry-and-node-health*
*Completed: 2026-03-21*

## Self-Check: PASSED
