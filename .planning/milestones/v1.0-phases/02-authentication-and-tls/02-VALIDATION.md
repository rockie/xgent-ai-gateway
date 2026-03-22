---
phase: 2
slug: authentication-and-tls
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-21
---

# Phase 2 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | Rust `cargo test` (built-in) |
| **Config file** | `Cargo.toml` (workspace root) |
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
| 02-01-01 | 01 | 1 | AUTH-01 | integration | `cargo test api_key` | ❌ W0 | ⬜ pending |
| 02-01-02 | 01 | 1 | INFR-05 | integration | `cargo test tls` | ❌ W0 | ⬜ pending |
| 02-02-01 | 02 | 1 | AUTH-02 | integration | `cargo test mtls` | ❌ W0 | ⬜ pending |
| 02-02-02 | 02 | 1 | AUTH-03 | integration | `cargo test node_token` | ❌ W0 | ⬜ pending |
| 02-02-03 | 02 | 1 | INFR-06 | integration | `cargo test keepalive` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `tests/auth_tests.rs` — stubs for AUTH-01, AUTH-02, AUTH-03
- [ ] `tests/tls_tests.rs` — stubs for INFR-05, INFR-06
- [ ] `tests/helpers/certs.rs` — rcgen-based test cert generation fixtures

*Existing cargo test infrastructure covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| HTTP/2 keepalive prevents silent death | INFR-06 | Requires long-running connection monitoring | Start gateway with TLS, connect client, wait >keepalive interval, verify ping frames via wireshark or connection stays alive |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
