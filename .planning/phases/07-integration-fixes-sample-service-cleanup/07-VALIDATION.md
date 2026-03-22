---
phase: 7
slug: integration-fixes-sample-service-cleanup
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-22
---

# Phase 7 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `Cargo.toml` (workspace root) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test --all-targets` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test --all-targets`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 07-01-01 | 01 | 1 | NODE-05, OBSV-03, RSLT-03 | build | `cargo build -p xgent-gateway 2>&1 \| tail -5` | pending |
| 07-01-02 | 01 | 1 | INFR-06 | build | `cargo build -p xgent-gateway 2>&1 \| tail -5` | pending |
| 07-02-01 | 02 | 1 | NODE-05 | build+unit | `cargo build -p xgent-gateway 2>&1 \| tail -5` | pending |
| 07-02-02 | 02 | 1 | NODE-05 | integration | `cargo test -p xgent-gateway --test reaper_callback_integration_test test_reaper_full_loop -- --ignored 2>&1 \| tail -10` | pending |
| 07-03-01 | 03 | 1 | INFR-06 | build | `cargo build -p xgent-gateway --example sample_service 2>&1 \| tail -5` | pending |
| 07-03-02 | 03 | 1 | (tech debt) | build | `cargo build -p xgent-gateway 2>&1 \| tail -5` | pending |

*Status: pending / green / red / flaky*

### Task-to-Plan Mapping Notes

- **07-01-01** = Plan 01 Task 1: Proto changes + in_flight_tasks decrement + gRPC callback_url + agent metadata forwarding
- **07-01-02** = Plan 01 Task 2: Plain HTTP keepalive configuration
- **07-02-01** = Plan 02 Task 1: mTLS identity mapping config + auth enforcement
- **07-02-02** = Plan 02 Task 2: Reaper full-loop integration test
- **07-03-01** = Plan 03 Task 1: Sample service echo binary
- **07-03-02** = Plan 03 Task 2: Tech debt verification and cleanup

---

## Wave 0 Requirements

Existing infrastructure covers all phase requirements. Each plan task has an `<automated>` verify command that runs `cargo build` or `cargo test` -- no additional test stubs are needed as Wave 0 tasks.

- All Plan 01 tasks verified by `cargo build -p xgent-gateway` (compilation proves proto integration, counter wiring, callback handling)
- Plan 02 Task 1 verified by `cargo build` + `cargo test --lib` (compilation proves config struct and auth integration)
- Plan 02 Task 2 verified by `cargo test --test reaper_callback_integration_test` (dedicated integration test created as part of the task itself)
- Plan 03 Task 1 verified by `cargo build --example sample_service` (compilation proves standalone example)
- Plan 03 Task 2 verified by `cargo build` (clean build confirms no warnings from tech debt items)

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| mTLS identity extraction | NODE-05 | Requires TLS handshake with client cert | Start gateway with mTLS, connect with grpcurl using client cert, verify logs show fingerprint |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify commands
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] Wave 0 covers all MISSING references (none needed -- all tasks self-verify)
- [x] No watch-mode flags
- [x] Feedback latency < 30s
- [x] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
