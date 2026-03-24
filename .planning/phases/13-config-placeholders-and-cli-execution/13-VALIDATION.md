---
phase: 13
slug: config-placeholders-and-cli-execution
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 13 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `gateway/Cargo.toml` (dev-dependencies section) |
| **Quick run command** | `cargo test -p gateway --lib -- agent` |
| **Full suite command** | `cargo test -p gateway` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p gateway --lib -- agent`
- **After every plan wave:** Run `cargo test -p gateway`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 13-01-01 | 01 | 1 | CFG-01 | unit | `cargo test -p gateway --lib -- agent::config` | ❌ W0 | ⬜ pending |
| 13-01-02 | 01 | 1 | CFG-02 | unit | `cargo test -p gateway --lib -- agent::placeholder` | ❌ W0 | ⬜ pending |
| 13-01-03 | 01 | 1 | CFG-03 | unit | `cargo test -p gateway --lib -- agent::config::env` | ❌ W0 | ⬜ pending |
| 13-01-04 | 01 | 1 | CFG-04 | unit | `cargo test -p gateway --lib -- agent::placeholder::metadata` | ❌ W0 | ⬜ pending |
| 13-02-01 | 02 | 2 | CLI-01 | integration | `cargo test -p gateway --lib -- agent::cli_executor::arg` | ❌ W0 | ⬜ pending |
| 13-02-02 | 02 | 2 | CLI-02 | integration | `cargo test -p gateway --lib -- agent::cli_executor::stdin` | ❌ W0 | ⬜ pending |
| 13-02-03 | 02 | 2 | CLI-03 | integration | `cargo test -p gateway --lib -- agent::cli_executor::timeout` | ❌ W0 | ⬜ pending |
| 13-02-04 | 02 | 2 | CLI-04 | unit | `cargo test -p gateway --lib -- agent::cli_executor::exit_code` | ❌ W0 | ⬜ pending |
| 13-02-05 | 02 | 2 | CLI-05 | unit | `cargo test -p gateway --lib -- agent::response` | ❌ W0 | ⬜ pending |
| 13-02-06 | 02 | 2 | SAFE-01 | unit | `cargo test -p gateway --lib -- agent::response::max_bytes` | ❌ W0 | ⬜ pending |
| 13-03-01 | 03 | 2 | CFG-05 | integration | `cargo test -p gateway --lib -- agent::cli_executor::cwd` | ❌ W0 | ⬜ pending |
| 13-03-02 | 03 | 2 | CFG-06 | integration | `cargo test -p gateway --lib -- agent::cli_executor::env_vars` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `gateway/src/agent/mod.rs` — module declaration with test stubs
- [ ] Test fixtures: sample `agent.yaml` configs in `gateway/tests/fixtures/`
- [ ] `serde_yaml_ng` added to `gateway/Cargo.toml` dev-dependencies

*Existing `cargo test` infrastructure covers framework needs — no new test framework required.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Large payload stdin without deadlock | CLI-02 | Requires > 64KB payload to trigger pipe buffer pressure | Run agent with 1MB payload in stdin mode, verify no hang |

*Most behaviors have automated verification via unit/integration tests.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
