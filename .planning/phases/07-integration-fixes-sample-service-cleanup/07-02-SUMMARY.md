---
phase: 07-integration-fixes-sample-service-cleanup
plan: 02
subsystem: auth, testing
tags: [mtls, sha256, fingerprint, reaper, integration-test, redis]

requires:
  - phase: 02-authentication-tls
    provides: NodeTokenAuthLayer, GrpcTlsConfig, mTLS server config
  - phase: 04-task-reliability-callbacks
    provides: reaper module, reap_timed_out_tasks logic, callback delivery

provides:
  - MtlsIdentityConfig for fingerprint-to-service authorization mapping
  - cert_fingerprint and check_mtls_identity helpers for gRPC auth
  - Public reap_timed_out_tasks function for direct test invocation
  - Full-loop reaper integration test proving task state transitions

affects: [sample-service, deployment-docs]

tech-stack:
  added: []
  patterns: [config-based-cert-identity-mapping, single-pass-reaper-for-testing]

key-files:
  created: []
  modified:
    - gateway/src/config.rs
    - gateway/src/grpc/auth.rs
    - gateway/src/reaper/mod.rs
    - gateway/tests/reaper_callback_integration_test.rs
    - gateway.toml

key-decisions:
  - "MtlsIdentityConfig uses HashMap<String, Vec<String>> for fingerprint-to-services mapping with serde default (empty = disabled)"
  - "mTLS identity check skipped when no peer certs in extensions (allows plaintext dev mode)"
  - "reap_timed_out_tasks made pub as minimal API surface for test access (run_reaper remains production entry point)"

patterns-established:
  - "Config-gated security: empty config disables feature, non-empty enables enforcement"

requirements-completed: [NODE-05]

duration: 4min
completed: 2026-03-22
---

# Phase 07 Plan 02: mTLS Identity Mapping and Reaper Full-Loop Test Summary

**Config-based mTLS cert fingerprint-to-service authorization in NodeTokenAuthLayer, plus full-loop reaper integration test that invokes reap_timed_out_tasks and verifies task state transitions**

## Performance

- **Duration:** 4 min
- **Started:** 2026-03-22T13:18:59Z
- **Completed:** 2026-03-22T13:23:00Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- mTLS identity mapping via gateway.toml [grpc.mtls_identity.fingerprints] -- maps SHA-256 cert fingerprint to authorized service names
- NodeTokenAuthLayer enforces fingerprint check after token validation when mapping is configured
- reap_timed_out_tasks exposed as public single-pass entry point for testing
- Full-loop reaper integration test: submit, claim, timeout, reap, verify state=failed with "timed out" error

## Task Commits

Each task was committed atomically:

1. **Task 1: mTLS identity mapping config + auth enforcement** - `f77ecd0` (feat)
2. **Task 2: Reaper full-loop integration test** - `5dbfbde` (feat)

## Files Created/Modified
- `gateway/src/config.rs` - Added MtlsIdentityConfig struct and mtls_identity field on GrpcConfig
- `gateway/src/grpc/auth.rs` - Added cert_fingerprint, check_mtls_identity, and mTLS check in NodeTokenAuthLayer
- `gateway/src/reaper/mod.rs` - Made reap_timed_out_tasks pub
- `gateway/tests/reaper_callback_integration_test.rs` - Added test_reaper_full_loop_marks_timed_out_task_failed
- `gateway.toml` - Added commented-out mtls_identity example section

## Decisions Made
- MtlsIdentityConfig uses HashMap<fingerprint, Vec<service_name>> -- empty map means disabled (safe default)
- mTLS identity check looks for Arc<Vec<Certificate>> in request extensions (tonic's pattern); skips with debug log if not present (allows plaintext dev mode)
- Made only reap_timed_out_tasks pub (not reap_service) -- minimal API surface expansion for testability

## Deviations from Plan

None - plan executed exactly as written.

## Issues Encountered
- Pre-existing compile error in agent.rs (missing fields in ReportResultRequest) -- out of scope for this plan, does not affect lib or gateway binary targets

## User Setup Required

None - no external service configuration required.

## Next Phase Readiness
- mTLS identity mapping ready for production use (configure fingerprints in gateway.toml)
- Reaper now fully integration-tested with direct invocation
- Ready for plan 07-03

---
*Phase: 07-integration-fixes-sample-service-cleanup*
*Completed: 2026-03-22*

## Self-Check: PASSED

All files exist. All commits verified (f77ecd0, 5dbfbde).
