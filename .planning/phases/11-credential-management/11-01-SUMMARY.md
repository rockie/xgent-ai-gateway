---
phase: 11-credential-management
plan: 01
subsystem: auth, api
tags: [redis, scan, credential-management, expiry, api-key, node-token]

# Dependency graph
requires:
  - phase: 08-frontend-foundation-backend-auth
    provides: admin auth session middleware, API key and node token CRUD endpoints
provides:
  - GET /v1/admin/api-keys list endpoint
  - GET /v1/admin/node-tokens list endpoint
  - label and expires_at fields on credential creation
  - auth-time expiry enforcement for API keys and node tokens
affects: [11-credential-management, admin-ui]

# Tech tracking
tech-stack:
  added: []
  patterns:
    - "SCAN-based Redis key listing for credential enumeration"
    - "Auth-time expiry check (list shows all, auth rejects expired)"

key-files:
  created: []
  modified:
    - gateway/src/auth/api_key.rs
    - gateway/src/auth/node_token.rs
    - gateway/src/http/admin.rs
    - gateway/src/main.rs
    - gateway/tests/auth_integration_test.rs
    - gateway/tests/grpc_auth_test.rs

key-decisions:
  - "Expired credentials shown in list endpoints but rejected at auth time"
  - "SCAN with COUNT 100 for credential enumeration (consistent with task listing pattern)"

patterns-established:
  - "Credential list pattern: SCAN keys, HGETALL each, map to response struct"
  - "Expiry enforcement: parse RFC 3339 timestamp, compare against Utc::now()"

requirements-completed: [API-03, API-04, CRED-01, CRED-04]

# Metrics
duration: 7min
completed: 2026-03-23
---

# Phase 11 Plan 01: Credential List Endpoints Summary

**Backend list endpoints for API keys and node tokens with label/expiry fields and auth-time expiry enforcement**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-23T07:29:12Z
- **Completed:** 2026-03-23T07:36:05Z
- **Tasks:** 2
- **Files modified:** 6

## Accomplishments
- Added GET /v1/admin/api-keys and GET /v1/admin/node-tokens list endpoints using Redis SCAN
- Extended credential storage with optional label and expires_at fields
- Added expiry enforcement in lookup_api_key (returns None) and validate_node_token (returns false) for expired credentials
- Expired credentials remain visible in list endpoints for admin cleanup

## Task Commits

Each task was committed atomically:

1. **Task 1: Add list functions and extend storage in api_key.rs and node_token.rs** - `59fb718` (feat)
2. **Task 2: Add list handlers in admin.rs, update create handlers, register GET routes** - `88d038f` (feat)

## Files Created/Modified
- `gateway/src/auth/api_key.rs` - Added label/expires_at to ClientMetadata, expiry check in lookup, list_api_keys function
- `gateway/src/auth/node_token.rs` - Added expires_at to NodeTokenMetadata, expiry check in validate, list_node_tokens function
- `gateway/src/http/admin.rs` - Added list handler functions, response structs, extended create request structs
- `gateway/src/main.rs` - Registered GET routes for api-keys and node-tokens
- `gateway/tests/auth_integration_test.rs` - Updated store_api_key and store_node_token calls with new params
- `gateway/tests/grpc_auth_test.rs` - Updated store_api_key and store_node_token calls with new params

## Decisions Made
- Expired credentials shown in list endpoints but rejected at auth time (per RESEARCH.md Pitfall 2)
- Used SCAN with COUNT 100 for credential enumeration, consistent with existing task listing pattern
- Full hash returned in list response (frontend responsible for masking display)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Updated test callers for new function signatures**
- **Found during:** Task 1
- **Issue:** Test files (auth_integration_test.rs, grpc_auth_test.rs) called store_api_key and store_node_token with old signatures
- **Fix:** Added None parameters for label, expires_at in all test call sites
- **Files modified:** gateway/tests/auth_integration_test.rs, gateway/tests/grpc_auth_test.rs
- **Verification:** cargo test passes
- **Committed in:** 59fb718 (Task 1 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Necessary to maintain test compilation. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- List endpoints ready for frontend credential management page (Phase 11 Plan 02)
- Create endpoints now accept label and expires_at for richer credential metadata

---
*Phase: 11-credential-management*
*Completed: 2026-03-23*

## Self-Check: PASSED
