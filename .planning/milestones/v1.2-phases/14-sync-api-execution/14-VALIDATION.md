---
phase: 14
slug: sync-api-execution
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-24
---

# Phase 14 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | `gateway/Cargo.toml` |
| **Quick run command** | `cargo test -p gateway --lib agent` |
| **Full suite command** | `cargo test -p gateway` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test -p gateway --lib agent`
- **After every plan wave:** Run `cargo test -p gateway`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 14-01-01 | 01 | 1 | SAPI-01 | unit | `cargo test -p gateway sync_api` | ❌ W0 | ⬜ pending |
| 14-01-02 | 01 | 1 | SAPI-02 | unit | `cargo test -p gateway sync_api` | ❌ W0 | ⬜ pending |
| 14-01-03 | 01 | 1 | SAPI-03 | unit | `cargo test -p gateway sync_api` | ❌ W0 | ⬜ pending |
| 14-01-04 | 01 | 1 | SAPI-04 | unit | `cargo test -p gateway sync_api` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `gateway/src/agent/sync_api_executor.rs` — test module with unit tests for SAPI-01 through SAPI-04
- [ ] Mock HTTP server setup for testing (wiremock or similar, or manual mock)

*Existing test infrastructure (cargo test) covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| TLS skip verify with self-signed cert | D-06 | Requires real TLS endpoint | Start HTTPS server with self-signed cert, configure `tls_skip_verify: true`, verify request succeeds |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
