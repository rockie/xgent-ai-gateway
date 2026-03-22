---
phase: 8
slug: frontend-foundation-backend-auth
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-22
---

# Phase 8 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | vitest (frontend), cargo test (backend) |
| **Config file** | `admin-ui/vitest.config.ts` (Wave 0 installs), `Cargo.toml` (existing) |
| **Quick run command** | `cd admin-ui && npx vitest run --reporter=verbose` + `cargo test --lib` |
| **Full suite command** | `cd admin-ui && npx vitest run` + `cargo test` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run quick run command
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 08-01-01 | 01 | 1 | AUTH-01 | unit | `cargo test auth` | ❌ W0 | ⬜ pending |
| 08-01-02 | 01 | 1 | AUTH-02 | unit | `cargo test session` | ❌ W0 | ⬜ pending |
| 08-01-03 | 01 | 1 | AUTH-03 | unit | `cargo test auth` | ❌ W0 | ⬜ pending |
| 08-01-04 | 01 | 1 | AUTH-04 | unit | `cargo test cors` | ❌ W0 | ⬜ pending |
| 08-02-01 | 02 | 1 | UI-01 | unit | `cd admin-ui && npx vitest run` | ❌ W0 | ⬜ pending |
| 08-02-02 | 02 | 1 | UI-02 | unit | `cd admin-ui && npx vitest run` | ❌ W0 | ⬜ pending |
| 08-02-03 | 02 | 2 | UI-03 | unit | `cd admin-ui && npx vitest run` | ❌ W0 | ⬜ pending |
| 08-02-04 | 02 | 2 | UI-04 | unit | `cd admin-ui && npx vitest run` | ❌ W0 | ⬜ pending |
| 08-02-05 | 02 | 2 | UI-05 | unit | `cd admin-ui && npx vitest run` | ❌ W0 | ⬜ pending |
| 08-03-01 | 03 | 2 | API-01 | integration | `cargo test api` | ❌ W0 | ⬜ pending |
| 08-03-02 | 03 | 2 | API-02 | integration | `cargo test api` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] `admin-ui/vitest.config.ts` — vitest configuration
- [ ] `admin-ui/src/test/setup.ts` — test setup with jsdom
- [ ] `tests/auth_tests.rs` — stubs for AUTH-01 through AUTH-04
- [ ] `tests/api_tests.rs` — stubs for API-01, API-02

*Frontend test infrastructure must be created from scratch (new SPA). Backend test infrastructure exists but needs auth-specific test modules.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Dark mode toggle persists preference | UI-05 | Visual verification + localStorage | Toggle dark mode, refresh page, verify theme persists |
| Responsive layout at 1280px+ | UI-03 | Visual viewport testing | Resize browser to 1280px, verify sidebar collapses correctly |
| Loading skeletons render correctly | UI-04 | Visual appearance | Throttle network, navigate between pages, verify skeleton appearance |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
