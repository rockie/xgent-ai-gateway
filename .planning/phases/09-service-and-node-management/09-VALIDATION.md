---
phase: 9
slug: service-and-node-management
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 9 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | vitest (not installed — admin-ui has no test framework yet) |
| **Config file** | none — Wave 0 installs |
| **Quick run command** | `cd admin-ui && npx tsc --noEmit` |
| **Full suite command** | `cd admin-ui && npx tsc --noEmit && npm run build` |
| **Estimated runtime** | ~15 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cd admin-ui && npx tsc --noEmit`
- **After every plan wave:** Run `cd admin-ui && npx tsc --noEmit && npm run build`
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 15 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 09-01-01 | 01 | 1 | SVC-01 | build | `cd admin-ui && npx tsc --noEmit` | ❌ W0 | ⬜ pending |
| 09-01-02 | 01 | 1 | SVC-02 | build | `cd admin-ui && npx tsc --noEmit` | ❌ W0 | ⬜ pending |
| 09-01-03 | 01 | 1 | SVC-03 | build | `cd admin-ui && npx tsc --noEmit` | ❌ W0 | ⬜ pending |
| 09-01-04 | 01 | 1 | SVC-04 | build | `cd admin-ui && npx tsc --noEmit` | ❌ W0 | ⬜ pending |
| 09-02-01 | 02 | 1 | NODE-01 | build | `cd admin-ui && npx tsc --noEmit` | ❌ W0 | ⬜ pending |
| 09-02-02 | 02 | 1 | NODE-02 | build | `cd admin-ui && npx tsc --noEmit` | ❌ W0 | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] No test framework in admin-ui — TypeScript compilation and build checks serve as validation
- [ ] All pages must compile without TypeScript errors
- [ ] Build must succeed with no warnings treated as errors

*Existing build infrastructure covers basic compilation verification. No unit test framework installed.*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Service list renders card grid with health badges | SVC-01 | Visual layout verification | Start dev server, navigate to /services, verify cards display name, node count, queue depth, health dot |
| Service detail shows config + node table | SVC-02 | Visual layout + data binding | Click a service card, verify breadcrumb, config section, node table with health indicators |
| Register service form validates and submits | SVC-03 | Form interaction + toast feedback | Click "Register Service", fill form, submit, verify toast + new card appears |
| Deregister confirmation dialog works | SVC-04 | Dialog interaction + optimistic removal | On detail page, click Deregister, confirm dialog, verify service removed from list |
| Node health indicators show correct colors | NODE-01 | Visual color verification | View service detail with nodes in various states, verify green/yellow/red/blue dots |
| Node details show in-flight tasks, drain, last seen | NODE-02 | Data display verification | View node row, verify all columns populated correctly |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 15s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
