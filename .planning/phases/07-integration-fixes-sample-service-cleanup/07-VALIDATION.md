---
phase: 7
slug: integration-fixes-sample-service-cleanup
status: draft
nyquist_compliant: false
wave_0_complete: false
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

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 07-01-01 | 01 | 1 | OBSV-03 | unit | `cargo test --lib -- in_flight` | ✅ | ⬜ pending |
| 07-01-02 | 01 | 1 | RSLT-03 | unit | `cargo test --lib -- callback` | ❌ W0 | ⬜ pending |
| 07-02-01 | 02 | 1 | INFR-06 | unit | `cargo test --lib -- revoke` | ❌ W0 | ⬜ pending |
| 07-02-02 | 02 | 1 | INFR-06 | unit | `cargo test --lib -- keepalive` | ❌ W0 | ⬜ pending |
| 07-03-01 | 03 | 2 | NODE-05 | integration | `cargo test --test sample_service` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Test stubs for in_flight_tasks decrement verification
- [ ] Test stubs for callback_url proto field
- [ ] Test stubs for DELETE revoke routes
- [ ] Test stubs for plain HTTP keepalive
- [ ] Integration test harness for sample service binary

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| mTLS identity extraction | NODE-05 | Requires TLS handshake with client cert | Start gateway with mTLS, connect with grpcurl using client cert, verify logs show fingerprint |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
