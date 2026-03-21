---
phase: 02-authentication-and-tls
plan: 01
subsystem: auth
tags: [sha256, api-key, node-token, rustls, mtls, tls, redis]

# Dependency graph
requires:
  - phase: 01-core-queue-loop
    provides: GatewayConfig, AppState, GatewayError, RedisQueue, gateway binary
provides:
  - Auth module with API key lifecycle (generate, hash, store, lookup, revoke, extract, middleware)
  - Auth module with node token lifecycle (generate, hash, store, validate, revoke)
  - TLS config builders for HTTP (rustls) and gRPC (mTLS via tonic)
  - Extended GatewayConfig with optional TLS and AdminConfig
  - Extended GatewayError with Unauthorized variant
  - Dedicated auth Redis connection on AppState
affects: [02-02-PLAN, 02-03-PLAN, admin-endpoints, middleware-wiring]

# Tech tracking
tech-stack:
  added: [rustls 0.23, tokio-rustls 0.26, rustls-pemfile 2.2, sha2 0.10, rand 0.9, hex 0.4, hyper 1, hyper-util 0.1, tonic tls-ring feature]
  patterns: [SHA-256 key hashing, Redis hash-per-key storage, Bearer/X-API-Key header extraction, explicit CryptoProvider for rustls]

key-files:
  created:
    - gateway/src/auth/mod.rs
    - gateway/src/auth/api_key.rs
    - gateway/src/auth/node_token.rs
    - gateway/src/tls/mod.rs
    - gateway/src/tls/config.rs
  modified:
    - gateway/Cargo.toml
    - gateway/src/lib.rs
    - gateway/src/config.rs
    - gateway/src/state.rs
    - gateway/src/error.rs
    - gateway/src/main.rs

key-decisions:
  - "Used rand 0.9 instead of plan's 0.10 (0.10 does not exist; 0.9 is latest)"
  - "Used tonic tls-ring feature instead of non-existent tls feature for ServerTlsConfig/Identity/Certificate"
  - "Used explicit CryptoProvider (ring) via builder_with_provider to avoid runtime panic"
  - "Used rcgen 0.13 instead of 0.14 for test cert generation (compatible with rustls 0.23)"

patterns-established:
  - "SHA-256 hashing pattern: hash_api_key/hash_node_token for constant-time-safe key lookup"
  - "Redis key schema: api_keys:<hash> and node_tokens:<service>:<hash> as Redis hashes"
  - "Header extraction priority: Authorization: Bearer > X-API-Key"
  - "Dedicated auth Redis connection separate from queue connection"

requirements-completed: [AUTH-01, AUTH-02, AUTH-03, INFR-05]

# Metrics
duration: 8min
completed: 2026-03-21
---

# Phase 02 Plan 01: Auth Foundation Summary

**SHA-256 API key and node token auth modules with rustls TLS config builders for HTTP and gRPC mTLS**

## Performance

- **Duration:** 8 min
- **Started:** 2026-03-21T11:22:46Z
- **Completed:** 2026-03-21T11:30:46Z
- **Tasks:** 2
- **Files modified:** 11

## Accomplishments
- Auth module with complete API key lifecycle: generate (32-byte random), hash (SHA-256), store/lookup/revoke (Redis), extract (Bearer/X-API-Key headers), Axum middleware
- Node token module with complete lifecycle: generate, hash, store, validate, revoke -- scoped per service in Redis
- TLS config builders: HTTP (rustls ServerConfig with h2+http/1.1 ALPN) and gRPC (tonic ServerTlsConfig with client CA for mTLS)
- GatewayConfig extended with optional TLS fields (backward compatible with Phase 1), AdminConfig with optional bootstrap token
- 17 new unit tests (12 auth + 5 TLS/config), all existing 22 tests still pass

## Task Commits

Each task was committed atomically:

1. **Task 1: Add Phase 2 dependencies and create auth module** - `8160a61` (feat)
2. **Task 2: Extend config/state/error types and TLS config builders** - `2fa072d` (feat)

## Files Created/Modified
- `gateway/src/auth/mod.rs` - Auth module re-exports
- `gateway/src/auth/api_key.rs` - API key generation, hashing, Redis CRUD, header extraction, Axum middleware
- `gateway/src/auth/node_token.rs` - Node token generation, hashing, Redis CRUD, validation
- `gateway/src/tls/mod.rs` - TLS module re-exports
- `gateway/src/tls/config.rs` - rustls ServerConfig and tonic ServerTlsConfig builders
- `gateway/Cargo.toml` - Added rustls, sha2, rand, hex, hyper, tls-ring dependencies
- `gateway/src/lib.rs` - Added auth and tls module declarations
- `gateway/src/config.rs` - Extended with TlsConfig, GrpcTlsConfig, AdminConfig
- `gateway/src/state.rs` - Added auth_conn (MultiplexedConnection)
- `gateway/src/error.rs` - Added Unauthorized variant
- `gateway/src/main.rs` - Opens dedicated auth Redis connection

## Decisions Made
- Used `rand 0.9` (plan specified 0.10 which does not exist on crates.io)
- Used `tonic tls-ring` feature instead of `tls` (tonic 0.14 does not have a `tls` feature; TLS types are behind `_tls-any` which `tls-ring` enables)
- Used explicit `rustls::crypto::ring::default_provider()` via `builder_with_provider` to avoid runtime CryptoProvider panic (rustls 0.23 requires explicit provider selection)
- Used `rcgen 0.13` instead of `0.14` for test cert generation (compatible with rustls 0.23 dependency chain)
- Pinned `time` crate to 0.3.41 for rustc compatibility (0.3.47 requires Rust 1.88.0)

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Fixed rand version from 0.10 to 0.9**
- **Found during:** Task 1
- **Issue:** Plan specified `rand = "0.10"` but the latest version on crates.io is 0.9.x
- **Fix:** Changed to `rand = "0.9"` and used `rand::rng()` API
- **Files modified:** gateway/Cargo.toml
- **Committed in:** 8160a61

**2. [Rule 3 - Blocking] Fixed tonic TLS feature flag**
- **Found during:** Task 2
- **Issue:** Plan specified `features = ["tls"]` but tonic 0.14 does not have a `tls` feature. The TLS types (ServerTlsConfig, Identity, Certificate) require `_tls-any` internal feature.
- **Fix:** Used `features = ["tls-ring"]` which enables `_tls-any` and is compatible with our rustls ring provider
- **Files modified:** gateway/Cargo.toml
- **Committed in:** 2fa072d

**3. [Rule 3 - Blocking] Added explicit CryptoProvider for rustls**
- **Found during:** Task 2
- **Issue:** rustls 0.23 requires explicit CryptoProvider selection; `ServerConfig::builder()` panics at runtime
- **Fix:** Used `ServerConfig::builder_with_provider(Arc::new(rustls::crypto::ring::default_provider()))` instead
- **Files modified:** gateway/src/tls/config.rs
- **Committed in:** 2fa072d

**4. [Rule 3 - Blocking] Pinned time crate for rustc compatibility**
- **Found during:** Task 1
- **Issue:** `time 0.3.47` requires Rust 1.88.0, but current rustc is 1.87.0-nightly
- **Fix:** Pinned `time` to 0.3.41 via `cargo update --precise`
- **Files modified:** Cargo.lock
- **Committed in:** 8160a61

---

**Total deviations:** 4 auto-fixed (4 blocking issues)
**Impact on plan:** All fixes were necessary to resolve compilation/dependency issues. No scope creep.

## Issues Encountered
None beyond the deviations documented above.

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Auth foundation complete: API key and node token modules ready for middleware wiring in Plan 02
- TLS config builders ready for server startup integration in Plan 02
- Config backward compatible: existing Phase 1 configs work unchanged
- All 39 tests pass (34 non-ignored)

---
*Phase: 02-authentication-and-tls*
*Completed: 2026-03-21*
