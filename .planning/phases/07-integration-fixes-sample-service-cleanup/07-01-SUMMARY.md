---
phase: 07-integration-fixes-sample-service-cleanup
plan: 01
subsystem: api, grpc, infra
tags: [protobuf, grpc, keepalive, callback, metrics, agent]

requires:
  - phase: 04-task-reliability-and-callbacks
    provides: callback_url validation and delivery, report_result queue method
  - phase: 05-observability-and-packaging
    provides: metrics recording patterns, hyper_util keepalive on TLS path
  - phase: 03-service-registry-and-node-health
    provides: update_in_flight_tasks function, node health tracking
provides:
  - callback_url field on gRPC SubmitTaskRequest (parity with HTTP)
  - node_id and service_name on ReportResultRequest (enables in_flight decrement)
  - in_flight_tasks counter decrement on report_result (accurate node health)
  - Agent metadata forwarding as X-Meta-{key} HTTP headers
  - Plain HTTP keepalive matching TLS path configuration
affects: [07-02, 07-03, sample-service]

tech-stack:
  added: []
  patterns:
    - "Manual accept loop for both TLS and plain HTTP paths (consistent keepalive)"
    - "X-Meta-{key} header convention for task metadata forwarding to local services"

key-files:
  created: []
  modified:
    - proto/src/gateway.proto
    - gateway/src/grpc/poll.rs
    - gateway/src/grpc/submit.rs
    - gateway/src/bin/agent.rs
    - gateway/src/main.rs

key-decisions:
  - "Decrement service falls back to validated auth service_name if ReportResultRequest.service_name is empty"
  - "Plain HTTP path uses identical hyper_util manual accept loop as TLS path for keepalive parity"

patterns-established:
  - "X-Meta-{key} header convention: agent forwards task metadata entries as HTTP headers with X-Meta- prefix"

requirements-completed: [NODE-05, OBSV-03, RSLT-03, INFR-06]

duration: 4min
completed: 2026-03-22
---

# Phase 07 Plan 01: Integration Fixes Summary

**Proto callback_url/node_id fields, in_flight_tasks decrement on report_result, gRPC callback_url storage, agent X-Meta- header forwarding, plain HTTP keepalive**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-22T13:18:54Z
- **Completed:** 2026-03-22T13:22:54Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- Added callback_url to gRPC SubmitTaskRequest and node_id+service_name to ReportResultRequest in proto
- Fixed in_flight_tasks counter never being decremented on report_result (NODE-05)
- Added gRPC callback_url resolution and storage matching HTTP pattern (RSLT-03)
- Agent now forwards task metadata as X-Meta-{key} headers during dispatch
- Plain HTTP mode uses manual accept loop with HTTP/2 keepalive (30s/10s) matching TLS path (INFR-06)

## Task Commits

Each task was committed atomically:

1. **Task 1: Proto changes + in_flight_tasks decrement + gRPC callback_url + agent metadata forwarding** - `549c42b` (feat)
2. **Task 2: Plain HTTP keepalive configuration** - `3e75a86` (feat)

## Files Created/Modified
- `proto/src/gateway.proto` - Added callback_url (field 4) to SubmitTaskRequest, node_id (5) and service_name (6) to ReportResultRequest
- `gateway/src/grpc/poll.rs` - Decrement in_flight_tasks counter on report_result using node_id/service_name from request
- `gateway/src/grpc/submit.rs` - Resolve and store callback_url (per-task > per-key default) with validation
- `gateway/src/bin/agent.rs` - Forward metadata as X-Meta-{key} headers, include node_id and service_name in ReportResultRequest
- `gateway/src/main.rs` - Replace axum::serve with manual accept loop + keepalive for plain HTTP mode

## Decisions Made
- Decrement service name falls back to validated auth service_name if ReportResultRequest.service_name is empty (backward compatibility with older agents)
- Plain HTTP path uses identical hyper_util manual accept loop as TLS path for keepalive parity rather than a simpler approach

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Proto fields available for sample service and integration tests
- Agent metadata forwarding ready for sample service to read simulate_delay_ms via X-Meta- headers
- Plans 07-02 and 07-03 can proceed

---
*Phase: 07-integration-fixes-sample-service-cleanup*
*Completed: 2026-03-22*
