---
phase: 4
slug: task-reliability-and-callbacks
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 4 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | gateway/Cargo.toml |
| **Quick run command** | `cargo test -p xgent-gateway --lib` |
| **Full suite command** | `cargo test -p xgent-gateway -- --ignored` |
| **Estimated runtime** | ~30 seconds (integration tests require Redis) |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p xgent-gateway --lib`
- **After every plan wave:** Run `cargo test -p xgent-gateway -- --ignored`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 04-01-01 | 01 | 1 | LIFE-03 | integration | `cargo test -p xgent-gateway reaper -- --ignored` | ❌ W0 | ⬜ pending |
| 04-01-02 | 01 | 1 | LIFE-03 | unit | `cargo test -p xgent-gateway --lib state_transition` | ❌ W0 | ⬜ pending |
| 04-02-01 | 02 | 2 | RSLT-03 | integration | `cargo test -p xgent-gateway callback -- --ignored` | ❌ W0 | ⬜ pending |
| 04-02-02 | 02 | 2 | RSLT-04 | integration | `cargo test -p xgent-gateway callback_retry -- --ignored` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test stubs for reaper (LIFE-03) in `gateway/tests/reaper_integration_test.rs`
- [ ] Test stubs for callback delivery (RSLT-03, RSLT-04) in `gateway/tests/callback_integration_test.rs`
- [ ] Existing test infrastructure (cargo test, Redis test helpers) covers framework needs

*Existing infrastructure covers framework requirements. Only new test files needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Reaper timing accuracy | LIFE-03 | 30s interval difficult to test precisely in CI | Start gateway, submit task, kill node, wait 30s+timeout, verify task marked failed |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
