---
phase: 01-core-queue-loop
plan: 01
subsystem: infra
tags: [rust, cargo-workspace, tonic, prost, redis-streams, axum, grpc, protobuf]

requires:
  - phase: none
    provides: greenfield project

provides:
  - 2-crate Cargo workspace (proto + gateway) with proto codegen
  - TaskState state machine with enforced transitions
  - TaskId (UUID v7) and ServiceName newtypes
  - GatewayError with tonic::Status and axum IntoResponse conversions
  - RedisQueue with submit, poll, report_result, get_status operations
  - GatewayConfig with layered TOML + env var loading
  - Per-service Redis stream isolation pattern

affects: [01-02-PLAN, 01-03-PLAN, all subsequent plans]

tech-stack:
  added: [tonic 0.14, tonic-prost 0.14, tonic-prost-build 0.14, prost 0.14, axum 0.8, redis 1.0, tokio 1.50, tower 0.5, tower-http 0.6, clap 4.6, config 0.15, tracing 0.1, thiserror 2, uuid 1.22, chrono 0.4, base64 0.22, serde 1.0, serde_json 1.0, tokio-stream 0.1, futures 0.3]
  patterns: [cargo workspace with proto + gateway crates, tonic-prost-build codegen, layered config with env var override, Redis Streams consumer group queue, task state machine with valid transition enforcement, mutex-serialized env var config tests]

key-files:
  created: [Cargo.toml, proto/Cargo.toml, proto/build.rs, proto/src/lib.rs, proto/src/gateway.proto, gateway/Cargo.toml, gateway/src/lib.rs, gateway/src/main.rs, gateway/src/config.rs, gateway/src/types.rs, gateway/src/error.rs, gateway/src/queue/mod.rs, gateway/src/queue/redis.rs]
  modified: []

key-decisions:
  - "Used tonic-prost-build 0.14 instead of tonic-build::configure() -- API moved in tonic 0.14"
  - "Added lib.rs to gateway crate for testable library target alongside binary"
  - "Used Mutex-based serialization for config env var tests to prevent parallel test races"
  - "Added base64 crate for payload encoding in Redis hash storage"

patterns-established:
  - "Proto codegen: tonic-prost-build in proto/build.rs, include_proto! in lib.rs"
  - "Redis key pattern: tasks:{service} for streams, task:{id} for hashes"
  - "State machine: TaskState::try_transition() returns Result for valid/invalid transitions"
  - "Config loading: defaults -> TOML file -> GATEWAY__ env vars"
  - "Error handling: GatewayError with From impls for tonic::Status and axum IntoResponse"

requirements-completed: [TASK-03, TASK-04, RSLT-05, LIFE-01, LIFE-02, SRVC-02, INFR-01, INFR-02]

duration: 11min
completed: 2026-03-21
---

# Phase 01 Plan 01: Foundation Summary

**Cargo workspace with proto codegen, Redis Streams queue layer, task state machine, and layered config**

## Performance

- **Duration:** 11 min
- **Started:** 2026-03-21T07:53:54Z
- **Completed:** 2026-03-21T08:04:56Z
- **Tasks:** 2
- **Files modified:** 14

## Accomplishments
- 2-crate Cargo workspace compiles cleanly with proto codegen generating TaskService, NodeService, and all message types
- Redis Streams queue layer with submit, poll, report_result, get_status -- per-service stream isolation and BUSYGROUP handling
- TaskState state machine enforces valid transitions (pending->assigned->running->completed/failed) with 17 unit tests passing
- GatewayConfig loads from TOML files with env var overrides (GATEWAY__ prefix)

## Task Commits

Each task was committed atomically:

1. **Task 1: Create Cargo workspace, proto crate with codegen, and gateway crate skeleton** - `51ee9c7` (feat)
2. **Task 2: Implement config, types, error modules, and Redis Streams queue layer with tests** - `c9af5f6` (feat)

## Files Created/Modified
- `Cargo.toml` - Workspace root with proto and gateway members
- `proto/Cargo.toml` - Proto crate with tonic, prost, tonic-prost-build dependencies
- `proto/build.rs` - tonic-prost-build codegen configuration
- `proto/src/gateway.proto` - gRPC service and message definitions (TaskService, NodeService)
- `proto/src/lib.rs` - Proto re-exports via include_proto!
- `gateway/Cargo.toml` - Gateway crate with all dependencies
- `gateway/src/lib.rs` - Library target exposing all modules
- `gateway/src/main.rs` - Binary entry point with CLI args, config loading, tracing
- `gateway/src/config.rs` - GatewayConfig with layered loading and tests
- `gateway/src/types.rs` - TaskId, ServiceName, TaskState with state machine and tests
- `gateway/src/error.rs` - GatewayError with tonic::Status and IntoResponse conversions
- `gateway/src/queue/mod.rs` - Queue module re-export
- `gateway/src/queue/redis.rs` - RedisQueue with all CRUD operations and integration tests

## Decisions Made
- Used tonic-prost-build 0.14 for codegen -- the tonic-build API changed in 0.14, moving compile_protos to tonic-prost-build
- Added gateway/src/lib.rs to enable `cargo test --lib` alongside the binary target
- Used Mutex-based test serialization for config tests that manipulate env vars
- Added base64 crate for encoding binary payloads stored in Redis hashes

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] tonic-build API change in 0.14**
- **Found during:** Task 1 (workspace setup)
- **Issue:** `tonic_build::configure()` no longer exists in tonic-build 0.14; API moved to `tonic_prost_build::configure()`
- **Fix:** Added `tonic-prost-build = "0.14"` to build-dependencies and `tonic-prost = "0.14"` to runtime dependencies; updated build.rs
- **Files modified:** proto/Cargo.toml, proto/build.rs
- **Verification:** `cargo build --workspace` succeeds
- **Committed in:** 51ee9c7 (Task 1 commit)

**2. [Rule 3 - Blocking] Gateway needed lib.rs for test target**
- **Found during:** Task 2 (testing)
- **Issue:** `cargo test -p xgent-gateway --lib` requires a library target; gateway was binary-only
- **Fix:** Created gateway/src/lib.rs re-exporting all modules; updated main.rs to import from library
- **Files modified:** gateway/src/lib.rs, gateway/src/main.rs
- **Verification:** `cargo test -p xgent-gateway --lib` runs all 17 unit tests
- **Committed in:** c9af5f6 (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (2 blocking)
**Impact on plan:** Both fixes necessary for compilation and testing. No scope creep.

## Issues Encountered
- Parallel test races with env var config tests -- resolved with Mutex-based serialization

## User Setup Required
None - no external service configuration required.

## Known Stubs
None - all implementation is functional. Redis integration tests are gated with `#[ignore]` (require running Redis) but this is intentional test design, not a stub.

## Next Phase Readiness
- Proto codegen complete, ready for gRPC service implementations (Plan 02)
- Queue layer ready for gRPC handlers to call submit/poll/report
- Config ready for dual-port listener setup
- Error types ready for gRPC and HTTP error responses

## Self-Check: PASSED

All 13 created files verified present. Both commit hashes (51ee9c7, c9af5f6) verified in git log.

---
*Phase: 01-core-queue-loop*
*Completed: 2026-03-21*
