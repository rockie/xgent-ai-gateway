---
quick_id: 260323-sgb
description: Fix canCancel to include assigned state and check off stale UI checkboxes
tasks: 2
---

# Quick Plan: Fix canCancel + Stale UI Checkboxes

## Task 1: Add assigned state to canCancel

- **files:** [admin-ui/src/lib/tasks.ts]
- **action:** Add `|| state === 'assigned'` to the canCancel function return statement
- **verify:** grep for `assigned` in canCancel function
- **done:** canCancel returns true for pending, running, AND assigned states

## Task 2: Check off stale UI requirement checkboxes

- **files:** [.planning/REQUIREMENTS.md]
- **action:** Change `[ ]` to `[x]` for UI-01, UI-02, UI-03, UI-04 (audit confirmed satisfied)
- **verify:** grep for unchecked UI requirements
- **done:** All 33 v1.1 requirements show [x] in REQUIREMENTS.md
