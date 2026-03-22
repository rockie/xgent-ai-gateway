---
phase: 5
slug: observability-and-packaging
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 5 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust built-in) |
| **Config file** | gateway/Cargo.toml (dev-dependencies section) |
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
| 05-01-01 | 01 | 1 | OBSV-01 | unit | `cargo test -p gateway logging` | ❌ W0 | ⬜ pending |
| 05-01-02 | 01 | 1 | OBSV-02 | unit | `cargo test -p gateway metrics` | ❌ W0 | ⬜ pending |
| 05-02-01 | 02 | 1 | OBSV-03 | unit | `cargo test -p gateway admin_health` | ❌ W0 | ⬜ pending |
| 05-02-02 | 02 | 2 | INFR-03 | integration | `cargo test -p gateway --test build_test` | ❌ W0 | ⬜ pending |
| 05-02-03 | 02 | 2 | INFR-04 | integration | `docker build -t xgent-gateway .` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `gateway/tests/logging_tests.rs` — stubs for OBSV-01 structured logging verification
- [ ] `gateway/tests/metrics_tests.rs` — stubs for OBSV-02 Prometheus metrics verification
- [ ] `gateway/tests/admin_health_tests.rs` — stubs for OBSV-03 admin health endpoint
- [ ] prometheus 0.14 dev-dependency — if not already present

*Existing cargo test infrastructure covers framework needs.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Docker image runs and serves traffic | INFR-04 | Requires Docker daemon | `docker run -p 8080:8080 xgent-gateway` and verify health endpoint responds |
| musl static binary has no dynamic deps | INFR-03 | Requires musl target toolchain | `file target/x86_64-unknown-linux-musl/release/xgent-gateway` shows "statically linked" |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
