---
phase: 3
slug: service-registry-and-node-health
status: draft
nyquist_compliant: false
wave_0_complete: false
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
| **Quick run command** | `cargo test -p gateway --lib` |
| **Full suite command** | `cargo test -p gateway` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p gateway --lib`
- **After every plan wave:** Run `cargo test -p gateway`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 03-01-01 | 01 | 1 | SRVC-01 | unit | `cargo test -p gateway service_registry` | ❌ W0 | ⬜ pending |
| 03-01-02 | 01 | 1 | SRVC-03 | unit | `cargo test -p gateway service_deregister` | ❌ W0 | ⬜ pending |
| 03-01-03 | 01 | 1 | SRVC-04 | unit | `cargo test -p gateway service_persist` | ❌ W0 | ⬜ pending |
| 03-02-01 | 02 | 2 | NODE-03 | unit | `cargo test -p gateway node_health` | ❌ W0 | ⬜ pending |
| 03-02-02 | 02 | 2 | NODE-05 | unit | `cargo test -p gateway node_drain` | ❌ W0 | ⬜ pending |
| 03-02-03 | 02 | 2 | NODE-06 | unit | `cargo test -p gateway node_heartbeat` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `gateway/src/service/mod.rs` — service registry module stubs
- [ ] `gateway/src/service/tests.rs` — unit test stubs for SRVC-01, SRVC-03, SRVC-04
- [ ] `gateway/src/health/mod.rs` — node health module stubs
- [ ] `gateway/src/health/tests.rs` — unit test stubs for NODE-03, NODE-05, NODE-06

*Existing cargo test infrastructure covers framework needs. No new test framework required.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| gRPC DrainNode RPC end-to-end | NODE-05 | Requires running gateway + agent process | Start gateway, start agent, send SIGTERM to agent, verify drain RPC called and no new tasks dispatched |
| Service config survives restart | SRVC-04 | Requires process restart | Register service, restart gateway, verify service still listed |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
