---
phase: 04-task-reliability-and-callbacks
plan: 02
subsystem: queue
tags: [callback-url, api-key, task-submission, reaper, reqwest, integration-tests]

requires:
  - phase: 04-task-reliability-and-callbacks
    plan: 01
    provides: "Callback delivery function, validate_callback_url, CallbackConfig, reqwest Client in AppState, reaper module"
provides:
  - "callback_url field on ClientMetadata with per-key default storage"
  - "callback_url on SubmitTaskRequest with per-task override and URL validation"
  - "PATCH /v1/admin/api-keys/{key_hash} endpoint for callback URL management"
  - "report_result returns callback_url for caller-driven delivery trigger"
  - "Reaper triggers callback delivery on timed-out tasks via HGET callback_url"
  - "Integration tests for reaper timeout detection and callback_url storage"
affects: [05-deployment-and-hardening]

tech-stack:
  added: []
  patterns: [per-key-default-callback, per-task-override-callback, caller-returns-trigger-data]

key-files:
  created:
    - gateway/tests/reaper_callback_integration_test.rs
  modified:
    - gateway/src/auth/api_key.rs
    - gateway/src/http/submit.rs
    - gateway/src/http/admin.rs
    - gateway/src/main.rs
    - gateway/src/queue/redis.rs
    - gateway/src/grpc/poll.rs
    - gateway/src/reaper/mod.rs

key-decisions:
  - "report_result returns Option<String> callback_url rather than taking AppState -- keeps queue layer decoupled"
  - "Callback URL resolved at submission time (per-task > per-key default) and stored in task hash"
  - "PATCH endpoint reuses TaskNotFound error variant for 404 on missing API key"

patterns-established:
  - "Per-task override of per-key defaults: resolve at submission, store resolved value in task hash"
  - "Return trigger data from queue methods, let callers spawn side effects (callback delivery)"

requirements-completed: [RSLT-03, RSLT-04]

duration: 5min
completed: 2026-03-22
---

# Phase 04 Plan 02: Callback URL Wiring and Delivery Triggers Summary

**Callback URL support wired through API key creation, task submission with per-task override, admin PATCH endpoint, and delivery triggers from both report_result and reaper**

## Performance

- **Duration:** 5 min
- **Started:** 2026-03-22T02:12:12Z
- **Completed:** 2026-03-22T02:17:12Z
- **Tasks:** 2
- **Files modified:** 8

## Accomplishments
- callback_url field added to ClientMetadata, SubmitTaskRequest, and CreateApiKeyRequest with full Redis persistence
- PATCH /v1/admin/api-keys/{key_hash} endpoint for updating/clearing callback URL on existing API keys
- Callback delivery triggered from gRPC report_result handler and reaper on terminal task states
- report_result return type changed to Option<String> to pass callback_url to callers without coupling queue to AppState
- 4 integration tests proving reaper timeout detection, skip behavior, counter increment, and callback_url storage

## Task Commits

Each task was committed atomically:

1. **Task 1: Add callback_url to ClientMetadata, SubmitTaskRequest, admin endpoints, and task hash storage** - `91e296e` (feat)
2. **Task 2: Wire callback delivery triggers in report_result and reaper, add integration tests** - `dc3f9cc` (feat)

## Files Created/Modified
- `gateway/src/auth/api_key.rs` - callback_url in ClientMetadata, store_api_key with callback_url param, update_api_key_callback function
- `gateway/src/http/submit.rs` - callback_url in SubmitTaskRequest, URL resolution (per-task > per-key), validation, HSET to task hash
- `gateway/src/http/admin.rs` - callback_url in CreateApiKeyRequest, UpdateApiKeyCallbackRequest, PATCH handler with validation
- `gateway/src/main.rs` - PATCH route for /v1/admin/api-keys/{key_hash}
- `gateway/src/queue/redis.rs` - report_result returns Option<String> callback_url from task hash fields
- `gateway/src/grpc/poll.rs` - Spawn callback delivery after report_result returns Some(url)
- `gateway/src/reaper/mod.rs` - HGET callback_url after marking task failed, spawn callback delivery
- `gateway/tests/reaper_callback_integration_test.rs` - 4 integration tests for reaper and callback_url storage

## Decisions Made
- report_result returns Option<String> callback_url rather than taking AppState -- keeps the queue layer decoupled from application concerns; callers decide how to use the trigger data
- Callback URL resolved at submission time and stored in task hash -- no need to re-lookup API key metadata at delivery time
- Reused TaskNotFound error variant for 404 on missing API key in PATCH endpoint to avoid adding a new error variant

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered

None

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- Full callback URL flow complete: API key creation -> task submission -> task hash -> delivery trigger on completion/failure
- All 48 library tests pass, 4 new integration tests added (9 total ignored/integration)
- Phase 04 fully complete -- task reliability infrastructure ready for Phase 05

---
*Phase: 04-task-reliability-and-callbacks*
*Completed: 2026-03-22*
