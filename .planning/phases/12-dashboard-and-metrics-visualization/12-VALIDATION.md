---
phase: 12
slug: dashboard-and-metrics-visualization
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 12 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust backend) / vitest (frontend) |
| **Config file** | `gateway/Cargo.toml` / `admin-ui/vitest.config.ts` |
| **Quick run command** | `cargo test --manifest-path gateway/Cargo.toml -- metrics && cd admin-ui && npx vitest run --reporter=verbose` |
| **Full suite command** | `cargo test --manifest-path gateway/Cargo.toml && cd admin-ui && npx vitest run` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --manifest-path gateway/Cargo.toml -- metrics`
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 12-01-01 | 01 | 1 | DASH-01 | unit | `cargo test --manifest-path gateway/Cargo.toml -- metrics::ring_buffer` | ❌ W0 | ⬜ pending |
| 12-01-02 | 01 | 1 | DASH-01 | unit | `cargo test --manifest-path gateway/Cargo.toml -- metrics::summary` | ❌ W0 | ⬜ pending |
| 12-01-03 | 01 | 1 | DASH-02 | unit | `cargo test --manifest-path gateway/Cargo.toml -- metrics::history` | ❌ W0 | ⬜ pending |
| 12-02-01 | 02 | 2 | DASH-01 | integration | `cd admin-ui && npx vitest run src/__tests__/dashboard` | ❌ W0 | ⬜ pending |
| 12-02-02 | 02 | 2 | DASH-02 | integration | `cd admin-ui && npx vitest run src/__tests__/dashboard` | ❌ W0 | ⬜ pending |
| 12-02-03 | 02 | 2 | DASH-03 | integration | `cd admin-ui && npx vitest run src/__tests__/dashboard` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `gateway/src/metrics.rs` — ring buffer tests (snapshot creation, capacity limit, delta computation)
- [ ] `gateway/src/http/admin.rs` — summary and history endpoint tests
- [ ] `admin-ui/src/__tests__/dashboard/` — dashboard component render tests

*Existing test infrastructure covers both Rust and frontend — no new framework installation needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Charts render correctly with live data | DASH-02 | Visual rendering requires browser | Start gateway + admin-ui, verify charts update every 10-15s |
| Color-coded health badges display correctly | DASH-03 | CSS color verification | Inspect service health dots for correct green/yellow/red |
| Delta arrow trends show correct direction | DASH-01 | Requires time-series state | Wait 5+ minutes, verify arrows match value changes |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
