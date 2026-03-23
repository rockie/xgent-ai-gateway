---
phase: 10
slug: task-management-and-data-endpoints
status: draft
nyquist_compliant: false
wave_0_complete: false
created: 2026-03-23
---

# Phase 10 — Validation Strategy

> Per-phase validation contract for feedback sampling during execution.

---

## Test Infrastructure

| Property | Value |
|----------|-------|
| **Framework** | cargo test (Rust backend) / vitest (frontend) |
| **Config file** | `gateway/Cargo.toml` / `admin-ui/vitest.config.ts` |
| **Quick run command** | `cargo test --manifest-path gateway/Cargo.toml -- task` |
| **Full suite command** | `cargo test --manifest-path gateway/Cargo.toml && cd admin-ui && npx vitest run` |
| **Estimated runtime** | ~30 seconds |

---

## Sampling Rate

- **After every task commit:** Run `cargo test --manifest-path gateway/Cargo.toml -- task`
- **After every plan wave:** Run full suite command
- **Before `/gsd:verify-work`:** Full suite must be green
- **Max feedback latency:** 30 seconds

---

## Per-Task Verification Map

| Task ID | Plan | Wave | Requirement | Test Type | Automated Command | File Exists | Status |
|---------|------|------|-------------|-----------|-------------------|-------------|--------|
| 10-01-01 | 01 | 1 | API-05 | unit | `cargo test -- task_list` | ❌ W0 | ⬜ pending |
| 10-01-02 | 01 | 1 | API-06 | unit | `cargo test -- task_cancel` | ❌ W0 | ⬜ pending |
| 10-02-01 | 02 | 2 | TASK-01 | manual | browser check | N/A | ⬜ pending |
| 10-02-02 | 02 | 2 | TASK-02 | manual | browser check | N/A | ⬜ pending |
| 10-02-03 | 02 | 2 | TASK-03 | manual | browser check | N/A | ⬜ pending |

*Status: ⬜ pending · ✅ green · ❌ red · ⚠️ flaky*

---

## Wave 0 Requirements

- [ ] Backend task endpoint tests — stubs for API-05, API-06
- [ ] Existing test infrastructure covers framework needs

*If none: "Existing infrastructure covers all phase requirements."*

---

## Manual-Only Verifications

| Behavior | Requirement | Why Manual | Test Instructions |
|----------|-------------|------------|-------------------|
| Paginated task list with filters | TASK-01 | UI interaction requires browser | Load tasks page, apply service/status filters, verify pagination |
| Task detail slide-out panel | TASK-02 | Visual UI component | Click task row, verify panel shows metadata/payload/result |
| Task cancel confirmation flow | TASK-03 | UI interaction + backend mutation | Click cancel on pending task, confirm dialog, verify status change |

---

## Validation Sign-Off

- [ ] All tasks have `<automated>` verify or Wave 0 dependencies
- [ ] Sampling continuity: no 3 consecutive tasks without automated verify
- [ ] Wave 0 covers all MISSING references
- [ ] No watch-mode flags
- [ ] Feedback latency < 30s
- [ ] `nyquist_compliant: true` set in frontmatter

**Approval:** pending
