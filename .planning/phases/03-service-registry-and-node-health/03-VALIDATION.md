---
phase: 3
slug: service-registry-and-node-health
status: draft
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-21
---

# Phase 3 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `gateway/Cargo.toml` `[dev-dependencies]` |
| **Quick run command** | `cargo test -p xgent-gateway --lib` |
| **Full suite command** | `cargo test -p xgent-gateway` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p xgent-gateway --lib`
- **After every plan wave:** Run `cargo test -p xgent-gateway`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | Status |
|---------|------|------|-------------|-----------|-------------------|--------|
| 03-01-T1 | 01 | 1 | SRVC-01, SRVC-03, SRVC-04 | unit + build | `cargo build -p xgent-gateway && cargo test -p xgent-gateway --lib` | ⬜ pending |
| 03-01-T2 | 01 | 1 | NODE-03 | build | `cargo build -p xgent-gateway` | ⬜ pending |
| 03-02-T1 | 02 | 2 | NODE-05 | unit | `cargo test -p xgent-gateway --lib` | ⬜ pending |
| 03-02-T2 | 02 | 2 | NODE-05, NODE-06 | build | `cargo build -p xgent-gateway` | ⬜ pending |
| 03-03-T1 | 03 | 3 | SRVC-01, SRVC-03, SRVC-04, NODE-03, NODE-05, NODE-06 | integration | `cargo test -p xgent-gateway --test registry_integration_test -- --ignored` | ⬜ pending |
| 03-03-T2 | 03 | 3 | NODE-06 | build | `cargo build -p xgent-gateway --bin xgent-agent` | ⬜ pending |

*Status: ⬜ pending / ✅ green / ❌ red / ⚠️ flaky*

---

## Wave 0 Requirements

No separate Wave 0 plan needed. All plan tasks include inline `<automated>` verify blocks:

- Plan 01 Task 1: `cargo build` + `cargo test --lib` (catches proto codegen and unit test failures)
- Plan 01 Task 2: `cargo build` (verifies wiring compiles)
- Plan 02 Task 1: `cargo test --lib` (verifies node health CRUD compiles)
- Plan 02 Task 2: `cargo build` (verifies RPC + poll loop compiles)
- Plan 03 Task 1: `cargo test --test registry_integration_test -- --ignored` (full integration)
- Plan 03 Task 2: `cargo build --bin xgent-agent` (verifies agent compiles)

All tasks satisfy the Nyquist rule with automated verification commands.

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| gRPC DrainNode RPC end-to-end | NODE-05 | Requires running gateway + agent process | Start gateway, start agent, send SIGTERM to agent, verify drain RPC called and no new tasks dispatched |
| Service config survives restart | SRVC-04 | Requires process restart | Register service, restart gateway, verify service still listed |

---

## Validation Sign-Off

- [x] All tasks have `<automated>` verify blocks
- [x] Sampling continuity: no 3 consecutive tasks without automated verify
- [x] No watch-mode flags
- [x] Feedback latency < 15s
- [x] `nyquist_compliant: true` set in frontmatter
- [x] `wave_0_complete: true` set in frontmatter (inline tests satisfy coverage)

**Approval:** pending
