# Phase 11: Credential Management - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-23
**Phase:** 11-credential-management
**Areas discussed:** Create flow & secret reveal, Revoke flow

---

## Create Flow & Secret Reveal

### Secret reveal mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| Modal with warning | Dialog shows raw key with copy button, warning text, "I've copied it" dismiss button, no close-X | ✓ |
| Two-step dialog | Step 1: form, Step 2: secret reveal. Must click "Done" to close | |
| Inline reveal | New credential appears at top of list with secret visible, auto-copies, disappears after 30s | |

**User's choice:** Modal with warning
**Notes:** None

### Create form inputs

| Option | Description | Selected |
|--------|-------------|----------|
| Service selector only | API key: multi-select services. Node token: single service + optional label. Minimal form | |
| Service + optional label for both | Both types get optional label alongside service selection | |
| Service + expiry date | Add optional expiry date field (needs backend TTL support) | |
| Other (custom) | Service + optional label + optional expiry date + optional callback URL | ✓ |

**User's choice:** Custom — service + optional label + optional expiry date + optional callback URL
**Notes:** Richer form than minimal. Backend needs extensions for label on API keys and TTL/expiry on both types. Callback URL already supported for API keys.

### Secret display format

| Option | Description | Selected |
|--------|-------------|----------|
| Full secret visible | Secret fully visible immediately with copy button. Simpler — one-time view | ✓ |
| Masked with reveal toggle | Secret shown as dots with eye icon toggle. Prevents shoulder-surfing | |

**User's choice:** Full secret visible
**Notes:** None

---

## Revoke Flow

### Revocation mechanism

| Option | Description | Selected |
|--------|-------------|----------|
| Single revoke with dialog | One-at-a-time via row action. Confirmation dialog consistent with Phase 9/10 patterns | ✓ |
| Bulk + single revoke | Checkbox column + "Revoke selected" bulk action + per-row revoke | |
| Revoke with grace period | Mark as "revoking" for 24h before deletion. Needs backend transitional state | |

**User's choice:** Single revoke with dialog
**Notes:** None

### Post-revoke list update

| Option | Description | Selected |
|--------|-------------|----------|
| Optimistic removal | Row disappears immediately with success toast. Reappears with error toast if fails | ✓ |
| Strikethrough then remove | Row stays with strikethrough for 3s, then fades out | |
| Refresh entire list | Full list reload after revoke. Simpler but causes flash | |

**User's choice:** Optimistic removal
**Notes:** Matches deregister pattern from Phase 9

---

## Claude's Discretion

- Backend SCAN listing approach for credentials
- Tab component styling and placement
- Create dialog form layout
- Loading skeletons and empty states
- Expiry display formatting
- Callback URL field validation

## Deferred Ideas

None
