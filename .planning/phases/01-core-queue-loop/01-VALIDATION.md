---
phase: 1
slug: core-queue-loop
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 1 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace root) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test --all` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test --all`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 01-01-01 | 01 | 1 | INFR-01, INFR-02 | unit | `cargo test --lib` | ❌ W0 | ⬜ pending |
| 01-02-01 | 02 | 1 | TASK-01, TASK-02, TASK-03 | unit+integration | `cargo test --all` | ❌ W0 | ⬜ pending |
| 01-03-01 | 03 | 2 | NODE-01, NODE-02, NODE-04 | unit+integration | `cargo test --all` | ❌ W0 | ⬜ pending |
| 01-04-01 | 04 | 2 | RSLT-01, RSLT-02, RSLT-05 | unit+integration | `cargo test --all` | ❌ W0 | ⬜ pending |
| 01-05-01 | 05 | 3 | LIFE-01, LIFE-02, TASK-04 | integration | `cargo test --all` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/` directory — integration test infrastructure
- [ ] Test helpers — Redis test fixtures, mock server setup
- [ ] `cargo test` — verify test framework runs on empty project

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Redis BLMOVE reliability on restart | LIFE-01 | Requires process kill during operation | 1. Submit task 2. Kill gateway mid-poll 3. Restart 4. Verify task recoverable |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
