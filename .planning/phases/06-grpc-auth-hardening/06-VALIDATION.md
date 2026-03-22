---
phase: 6
slug: grpc-auth-hardening
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 6 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust built-in `#[tokio::test]` + integration tests |
| **Config file** | `Cargo.toml` (workspace test config) |
| **Quick run command** | `cargo test --lib` |
| **Full suite command** | `cargo test` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --lib`
- **After every plan wave:** Run `cargo test`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 6-01-01 | 01 | 1 | AUTH-01 | integration | `cargo test grpc_client_auth` | ❌ W0 | ⬜ pending |
| 6-01-02 | 01 | 1 | AUTH-03 | integration | `cargo test grpc_node_auth` | ❌ W0 | ⬜ pending |
| 6-02-01 | 02 | 1 | TASK-01 | integration | `cargo test grpc_submit_task_auth` | ❌ W0 | ⬜ pending |
| 6-02-02 | 02 | 1 | RSLT-01 | integration | `cargo test grpc_report_result_auth` | ❌ W0 | ⬜ pending |
| 6-03-01 | 03 | 2 | NODE-03, NODE-04, NODE-06 | integration | `cargo test grpc_node_rpc_auth` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Integration test stubs for gRPC auth positive/negative paths
- [ ] Test fixtures for API key and node token setup
- [ ] gRPC test client helper (tonic client with configurable auth metadata)

*Existing `cargo test` infrastructure covers framework needs — no new framework install required.*

---

## Manual-Only Verifications

*All phase behaviors have automated verification.*

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
