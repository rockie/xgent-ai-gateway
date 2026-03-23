---
phase: 08-frontend-foundation-backend-auth
plan: 01
subsystem: auth
tags: [argon2, session, cookie, cors, redis, axum]

requires:
  - phase: 07-hardening
    provides: "Existing admin middleware and config structure"
provides:
  - "Session-based admin auth endpoints (login/logout/refresh)"
  - "Cookie-based session middleware replacing Bearer token auth"
  - "Extended AdminConfig with username, password_hash, cors_origin, session_ttl_secs, cookie_secure"
  - "CORS configuration with credentials support"
affects: [08-02, 08-03, 09-frontend-app]

tech-stack:
  added: [axum-extra (cookie), argon2, password-hash, time]
  patterns: [HttpOnly cookie session auth, Redis session store with sliding TTL, CookieJar extraction in Axum]

key-files:
  created: [gateway/src/http/auth.rs]
  modified: [gateway/Cargo.toml, gateway/src/config.rs, gateway/src/http/mod.rs, gateway/src/http/admin.rs, gateway/src/main.rs, gateway/src/tls/config.rs, gateway.toml, gateway/tests/auth_integration_test.rs, gateway/tests/grpc_auth_test.rs]

key-decisions:
  - "Argon2id PHC-format for password hashing (industry standard, future-proof)"
  - "Redis HSET for session storage with EXPIRE for sliding TTL"
  - "SameSite=None + Secure cookie for cross-origin SPA compatibility"
  - "Dev mode preserved: no admin.username = endpoints pass through unauthenticated"

patterns-established:
  - "CookieJar pattern: extract session cookie in handlers and middleware via axum-extra CookieJar"
  - "Session key pattern: admin_session:<hex_session_id> in Redis"
  - "CORS at outermost layer: CORS middleware applied outside auth middleware to handle OPTIONS preflight"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03, AUTH-04, API-01, API-02]

duration: 12min
completed: 2026-03-23
---

# Phase 08 Plan 01: Backend Session Auth Summary

**Argon2 password-verified session auth with Redis-backed HttpOnly cookies, CORS, and login/logout/refresh endpoints replacing Bearer token admin middleware**

## Performance

- **Duration:** 12 min
- **Started:** 2026-03-23T00:19:34Z
- **Completed:** 2026-03-23T00:31:28Z
- **Tasks:** 2
- **Files modified:** 10

## Accomplishments
- Replaced Bearer token admin auth with cookie-based session auth backed by Redis
- Three new auth endpoints: POST /v1/admin/auth/login, /logout, /refresh
- Extended AdminConfig with username, password_hash, cors_origin, session_ttl_secs, cookie_secure
- CORS layer with explicit origin support and credentials enabled
- Dev mode preserved: no admin.username configured = endpoints pass through

## Task Commits

Each task was committed atomically:

1. **Task 1: Extend AdminConfig and create session auth module** - `c457a0d` (feat)
2. **Task 2: Wire auth endpoints into main.rs, add CORS, replace admin middleware** - `8a56062` (feat)

## Files Created/Modified
- `gateway/src/http/auth.rs` - Login, logout, refresh handlers and session_auth_middleware
- `gateway/src/config.rs` - Extended AdminConfig with session auth fields
- `gateway/Cargo.toml` - Added axum-extra, argon2, password-hash, time dependencies
- `gateway/src/http/mod.rs` - Added auth module export
- `gateway/src/http/admin.rs` - Removed old admin_auth_middleware
- `gateway/src/main.rs` - Wired auth routes, session middleware, CORS layer
- `gateway/src/tls/config.rs` - Updated tests for new AdminConfig fields
- `gateway.toml` - Updated admin section with session auth configuration
- `gateway/tests/auth_integration_test.rs` - Updated AdminConfig usage
- `gateway/tests/grpc_auth_test.rs` - Updated AdminConfig usage

## Decisions Made
- Used Argon2id via `argon2` + `password-hash` crates for password verification (PHC format)
- Redis HSET with EXPIRE for session storage (sliding window TTL on each authenticated request)
- SameSite=None with Secure flag for cross-origin SPA cookie delivery
- Dev mode behavior preserved: when admin.username is not configured, all admin endpoints pass through

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Moved admin_auth_middleware removal and session_auth_middleware wiring to Task 1**
- **Found during:** Task 1 (compilation)
- **Issue:** Removing AdminConfig.token field caused admin.rs admin_auth_middleware to fail compilation since main.rs referenced it
- **Fix:** Removed admin_auth_middleware and wired session_auth_middleware in Task 1 instead of Task 2
- **Files modified:** gateway/src/http/admin.rs, gateway/src/main.rs
- **Verification:** cargo check passes
- **Committed in:** c457a0d (Task 1 commit)

**2. [Rule 3 - Blocking] Fixed integration tests referencing old AdminConfig.token field**
- **Found during:** Task 2 (cargo test)
- **Issue:** gateway/tests/auth_integration_test.rs and grpc_auth_test.rs used `AdminConfig { token: None }` which no longer exists
- **Fix:** Replaced with `AdminConfig::default()`
- **Files modified:** gateway/tests/auth_integration_test.rs, gateway/tests/grpc_auth_test.rs
- **Verification:** cargo test passes
- **Committed in:** 8a56062 (Task 2 commit)

**3. [Rule 3 - Blocking] Added `time` crate dependency for cookie max_age**
- **Found during:** Task 1 (compilation)
- **Issue:** axum-extra cookie API uses `time::Duration` for max_age, which requires the `time` crate
- **Fix:** Added `time = "0.3"` to Cargo.toml dependencies
- **Files modified:** gateway/Cargo.toml
- **Verification:** cargo check passes
- **Committed in:** c457a0d (Task 1 commit)

---

**Total deviations:** 3 auto-fixed (3 blocking)
**Impact on plan:** All auto-fixes necessary for compilation and test passing. No scope creep.

## Issues Encountered
None beyond the blocking issues documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Backend auth endpoints ready for frontend integration
- Frontend can POST to /v1/admin/auth/login with username/password and receive session cookie
- Admin must generate Argon2 password hash and configure admin.username + admin.password_hash in gateway.toml
- All existing admin routes now protected by session cookie middleware

---
*Phase: 08-frontend-foundation-backend-auth*
*Completed: 2026-03-23*
