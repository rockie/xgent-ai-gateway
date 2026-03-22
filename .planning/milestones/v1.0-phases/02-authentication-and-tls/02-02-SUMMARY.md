---
phase: 02-authentication-and-tls
plan: 02
subsystem: auth
tags: [tls, rustls, mtls, api-key, node-token, keepalive, admin-api, axum-middleware]

# Dependency graph
requires:
  - phase: 02-authentication-and-tls/01
    provides: "API key module, node token module, TLS config builders, AppState with auth_conn"
provides:
  - "TLS-wrapped server startup for both gRPC and HTTP"
  - "API key auth middleware on HTTP task routes"
  - "Node token validation in gRPC poll handler"
  - "Per-service authorization scoping in submit handler"
  - "Admin CRUD endpoints for API keys and node tokens"
  - "HTTP/2 keepalive configuration on both servers"
affects: [03-service-registry, 02-03-integration-tests]

# Tech tracking
tech-stack:
  added: [hyper-util/TowerToHyperService, tokio-rustls/TlsAcceptor]
  patterns: [manual-tls-accept-loop, layered-auth-middleware, per-service-scoping]

key-files:
  created:
    - gateway/src/http/admin.rs
  modified:
    - gateway/src/main.rs
    - gateway/src/http/mod.rs
    - gateway/src/http/submit.rs
    - gateway/src/grpc/poll.rs

key-decisions:
  - "Admin endpoints unauthenticated in Phase 2 (admin auth deferred to Phase 3)"
  - "report_result not token-validated; task_id serves as implicit auth (deferred)"
  - "Manual TLS accept loop with hyper-util for HTTP/2 keepalive control"

patterns-established:
  - "TLS accept loop: tokio_rustls::TlsAcceptor + hyper_util::server::conn::auto::Builder for per-connection HTTP/2 config"
  - "Auth middleware layering: api_key_auth_middleware on api_routes only, admin_routes separate"
  - "Service-scoping: ClientMetadata.service_names checked in handler after middleware injects it"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03, INFR-05, INFR-06]

# Metrics
duration: 3min
completed: 2026-03-21
---

# Phase 02 Plan 02: Server Integration Summary

**TLS termination, API key auth middleware, node token validation, admin CRUD endpoints, and HTTP/2 keepalive wired into gateway servers**

## Performance

- **Duration:** 3 min
- **Started:** 2026-03-21T11:34:01Z
- **Completed:** 2026-03-21T11:36:56Z
- **Tasks:** 2
- **Files modified:** 5

## Accomplishments
- gRPC server has mTLS (when configured) and HTTP/2 keepalive (30s interval, 10s timeout)
- HTTP server has TLS via manual rustls accept loop (when configured) with keepalive
- API key auth middleware applied to /v1/tasks routes; admin routes unauthenticated
- Node poll_tasks validates Bearer token against Redis per-service before streaming
- HTTP submit_task enforces per-service authorization via ClientMetadata
- Admin endpoints at /v1/admin/* for creating/revoking API keys and node tokens
- Backward compatible: no TLS config = plain HTTP/gRPC (Phase 1 behavior preserved)

## Task Commits

Each task was committed atomically:

1. **Task 1: Wire TLS, auth middleware, and keepalive into main.rs server startup, and add admin endpoints** - `78476b7` (feat)
2. **Task 2: Add node token validation to gRPC poll handler and service-scoping to HTTP submit handler** - `84c27b1` (feat)

## Files Created/Modified
- `gateway/src/http/admin.rs` - Admin API endpoints for API key and node token CRUD
- `gateway/src/main.rs` - TLS server startup, auth middleware wiring, keepalive config, admin routes
- `gateway/src/http/mod.rs` - Added admin module declaration
- `gateway/src/http/submit.rs` - Per-service authorization via ClientMetadata extension
- `gateway/src/grpc/poll.rs` - Node token validation before poll streaming

## Decisions Made
- Admin endpoints are unauthenticated in Phase 2 per CONTEXT.md deferral (admin auth to Phase 3)
- report_result is not token-validated; the unguessable UUID v7 task_id serves as implicit auth
- Used manual TLS accept loop with hyper-util Builder for per-connection HTTP/2 keepalive control (axum::serve does not expose HTTP/2 settings)
- Used `http2()` builder method (not `http2_only()`) to avoid temporary value borrow issues; connections over TLS will negotiate HTTP/2 via ALPN

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed temporary value borrow error in HTTP/2 builder chain**
- **Found during:** Task 1 (main.rs TLS wiring)
- **Issue:** Chaining `.http2_only().http2().keep_alive_interval()` created a temporary dropped while borrowed
- **Fix:** Split into mutable builder with separate `.http2()` configuration call, then `.serve_connection()`
- **Files modified:** gateway/src/main.rs
- **Verification:** cargo build succeeds
- **Committed in:** 78476b7 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 bug)
**Impact on plan:** Minor API adjustment. No scope creep.

## Issues Encountered
None beyond the builder chain fix noted above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Auth integration wired end-to-end; ready for integration tests (Plan 03)
- TLS configuration is optional; tests can run without TLS enabled
- Admin endpoints available for test setup (create keys/tokens before testing auth flows)

## Self-Check: PASSED

All 5 files verified present. Both task commits (78476b7, 84c27b1) verified in git log.

---
*Phase: 02-authentication-and-tls*
*Completed: 2026-03-21*
