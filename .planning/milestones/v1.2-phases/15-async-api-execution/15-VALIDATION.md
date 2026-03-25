---
phase: 15
slug: async-api-execution
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 15 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | gateway/Cargo.toml |
| **Quick run command** | `cargo test -p gateway --lib` |
| **Full suite command** | `cargo test -p gateway` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p gateway --lib`
- **After every plan wave:** Run `cargo test -p gateway`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 15-01-01 | 01 | 1 | AAPI-01, AAPI-02, AAPI-03, AAPI-04 | unit | `cargo test -p gateway async_api` | ❌ W0 | ⬜ pending |
| 15-01-02 | 01 | 1 | AAPI-05 | unit | `cargo test -p gateway async_api` | ❌ W0 | ⬜ pending |
| 15-01-03 | 01 | 1 | AAPI-06 | unit | `cargo test -p gateway async_api` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `gateway/src/agent/async_api_executor.rs` — tests module with stubs for AAPI-01 through AAPI-06
- [ ] Test fixtures for async-api YAML configs

*Existing infrastructure (cargo test, existing test patterns in cli_executor.rs and sync_api_executor.rs) covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Poll loop timeout enforcement under real timing | AAPI-05 | Timing-sensitive, unit tests use mocked time | Start agent with short timeout config, verify timeout error within expected window |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
