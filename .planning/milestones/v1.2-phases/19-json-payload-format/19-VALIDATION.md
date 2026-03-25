---
phase: 19
slug: json-payload-format
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-25
---

# Phase 19 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | Cargo.toml (workspace) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test --workspace` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test --workspace`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 19-01-01 | 01 | 1 | EXMP-04 | unit | `cargo test --lib -p xgent-ai-gateway` | ✅ | ⬜ pending |
| 19-01-02 | 01 | 1 | EXMP-04 | unit | `cargo test --lib -p xgent-ai-gateway` | ✅ | ⬜ pending |
| 19-02-01 | 02 | 1 | EXMP-04 | unit | `cargo test --lib -p xgent-agent` | ✅ | ⬜ pending |
| 19-03-01 | 03 | 2 | EXMP-04 | integration | `cargo test --workspace` | ✅ | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

*Existing infrastructure covers all phase requirements.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Node.js clients send/receive JSON payloads | EXMP-04 | Requires running gateway + agent + Node.js client | Start gateway, start agent, run each Node.js client example |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
