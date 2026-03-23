---
phase: 11
slug: credential-management
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 11 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust backend) + vitest (frontend, if configured) |
| **Config file** | `gateway/Cargo.toml` (backend), `admin-ui/vitest.config.ts` (frontend, if exists) |
| **Quick run command** | `cargo test --manifest-path gateway/Cargo.toml` |
| **Full suite command** | `cargo test --manifest-path gateway/Cargo.toml && (cd admin-ui && npm test 2>/dev/null || true)` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --manifest-path gateway/Cargo.toml`
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 11-01-01 | 01 | 1 | API-03 | integration | `cargo test list_api_keys` | ❌ W0 | ⬜ pending |
| 11-01-02 | 01 | 1 | API-04 | integration | `cargo test list_node_tokens` | ❌ W0 | ⬜ pending |
| 11-01-03 | 01 | 1 | CRED-04 | unit | `cargo test store_api_key_with_label` | ❌ W0 | ⬜ pending |
| 11-02-01 | 02 | 2 | CRED-01 | manual | Browser: credentials page loads | N/A | ⬜ pending |
| 11-02-02 | 02 | 2 | CRED-02 | manual | Browser: create flow shows secret | N/A | ⬜ pending |
| 11-02-03 | 02 | 2 | CRED-03 | manual | Browser: revoke with confirmation | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Backend integration tests for list endpoints (API-03, API-04)
- [ ] Backend unit tests for label/expiry extensions (CRED-04)

*Frontend tests are manual browser verification — no additional test infrastructure needed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Credential list renders with tabs | CRED-01 | Browser UI rendering | Navigate to /credentials, verify tabs switch between API keys and node tokens |
| Secret reveal dialog | CRED-02 | Interactive modal flow | Create a credential, verify secret shown once with copy button, dialog requires "I've copied it" |
| Revoke confirmation dialog | CRED-03 | Interactive confirmation | Click revoke, verify dialog text, confirm, verify row removed |
| Copy-to-clipboard | CRED-05 | Browser clipboard API | Create credential, click copy, verify clipboard contents |
| Expiry display | CRED-06 | Visual formatting | Create credential with expiry, verify date shown in table |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
