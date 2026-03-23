---
phase: 11-credential-management
plan: 02
subsystem: ui
tags: [react, tanstack-query, shadcn, tabs, credential-management, api-key, node-token]

# Dependency graph
requires:
  - phase: 11-credential-management
    provides: GET /v1/admin/api-keys and GET /v1/admin/node-tokens list endpoints, create/revoke endpoints
  - phase: 08-frontend-foundation-backend-auth
    provides: admin auth session, apiClient, useAutoRefresh hook
  - phase: 09-service-node-management
    provides: useServices hook for service selection
provides:
  - Credential management page with tabbed API Keys and Node Tokens views
  - Create credential flow with one-time secret reveal dialog
  - Revoke credential flow with optimistic removal
  - TanStack Query hooks for credential CRUD (useApiKeys, useNodeTokens, useCreateApiKey, useCreateNodeToken, useRevokeApiKey, useRevokeNodeToken)
affects: [admin-ui]

# Tech tracking
tech-stack:
  added: ["@base-ui/react/tabs (via shadcn)"]
  patterns:
    - "Forced-dismissal dialog pattern: disablePointerDismissal + onOpenChange reason filtering + showCloseButton=false"
    - "Optimistic mutation with rollback: onMutate snapshots data, onError restores, onSettled invalidates"
    - "Secret reveal after create: mutation onSuccess triggers reveal dialog, dismiss triggers cache invalidation"

key-files:
  created:
    - admin-ui/src/lib/credentials.ts
    - admin-ui/src/components/credential-table.tsx
    - admin-ui/src/components/create-credential-dialog.tsx
    - admin-ui/src/components/secret-reveal-dialog.tsx
    - admin-ui/src/components/revoke-credential-dialog.tsx
    - admin-ui/src/components/ui/tabs.tsx
  modified:
    - admin-ui/src/routes/_authenticated/credentials.tsx

key-decisions:
  - "Used base-ui Dialog disablePointerDismissal + onOpenChange reason filtering for forced-dismissal secret reveal"
  - "Popover+Checkbox multi-select for API key service selection, single Select for node token service"
  - "Native date input for expiry (no external date picker dependency)"

patterns-established:
  - "Forced-dismissal dialog: disablePointerDismissal, filter escape-key/outside-press/close-press in onOpenChange, showCloseButton=false"
  - "Credential CRUD hooks follow services.ts/tasks.ts pattern with auto-refresh"

requirements-completed: [CRED-01, CRED-02, CRED-03, CRED-04, CRED-05, CRED-06]

# Metrics
duration: 7min
completed: 2026-03-23
---

# Phase 11 Plan 02: Credential Management UI Summary

**Tabbed credential management page with API key/node token CRUD, one-time secret reveal dialog, and optimistic revoke**

## Performance

- **Duration:** 7 min
- **Started:** 2026-03-23T07:39:08Z
- **Completed:** 2026-03-23T07:46:07Z
- **Tasks:** 2 of 2 auto tasks (Task 3 checkpoint pending)
- **Files created:** 6
- **Files modified:** 1

## Accomplishments
- Built credential data layer with types, query hooks (auto-refresh), mutation hooks (optimistic revoke)
- Created five UI components: CredentialTable, CreateCredentialDialog, SecretRevealDialog, RevokeCredentialDialog, and shadcn Tabs
- Replaced placeholder credentials page with full tabbed CRUD interface
- Secret reveal dialog prevents all dismiss mechanisms except the "I've copied it" button

## Task Commits

Each task was committed atomically:

1. **Task 1: Install Tabs component and create credential data layer** - `7e962da` (feat)
2. **Task 2: Build credential UI components and replace placeholder page** - `6c62ebf` (feat)

## Files Created/Modified
- `admin-ui/src/lib/credentials.ts` - Types, query hooks (useApiKeys, useNodeTokens), mutation hooks (create, revoke with optimistic updates), utility functions (maskHash, isExpired)
- `admin-ui/src/components/ui/tabs.tsx` - shadcn Tabs component (base-ui v4)
- `admin-ui/src/components/credential-table.tsx` - Shared data table for API keys and node tokens with masked hashes and revoke action
- `admin-ui/src/components/create-credential-dialog.tsx` - Dialog form with service selection (multi for API keys, single for tokens), label, expiry, callback URL
- `admin-ui/src/components/secret-reveal-dialog.tsx` - Forced-dismissal dialog showing raw secret once with copy button
- `admin-ui/src/components/revoke-credential-dialog.tsx` - AlertDialog confirmation for credential revocation
- `admin-ui/src/routes/_authenticated/credentials.tsx` - Full credentials page with tabs, loading/error/empty states, create and revoke flows

## Decisions Made
- Used base-ui Dialog `disablePointerDismissal` and `onOpenChange` reason filtering to implement forced-dismissal instead of the older radix `onInteractOutside`/`onEscapeKeyDown` pattern (adapted to shadcn v4 API)
- Used Popover+Checkbox pattern for API key multi-service selection (consistent with tasks page status filter)
- Used native `<input type="date">` for expiry to avoid adding a date picker dependency
- Fixed Select `onValueChange` null handling for base-ui compatibility

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 1 - Bug] Fixed Select onValueChange type mismatch**
- **Found during:** Task 2
- **Issue:** base-ui Select's `onValueChange` passes `string | null` but React state setter expects `string`
- **Fix:** Wrapped with `(value) => setSelectedService(value ?? '')`
- **Files modified:** admin-ui/src/components/create-credential-dialog.tsx
- **Verification:** `npm run build` passes
- **Committed in:** 6c62ebf (Task 2 commit)

**2. [Rule 3 - Blocking] Adapted forced-dismissal to base-ui Dialog API**
- **Found during:** Task 2
- **Issue:** Plan specified `onInteractOutside` and `onEscapeKeyDown` (radix API) but shadcn v4 uses base-ui which has different dismiss prevention API
- **Fix:** Used `disablePointerDismissal` prop and `onOpenChange` reason filtering for `escape-key`/`outside-press`/`close-press`, plus `showCloseButton={false}`
- **Files modified:** admin-ui/src/components/secret-reveal-dialog.tsx
- **Verification:** TypeScript compiles, build succeeds
- **Committed in:** 6c62ebf (Task 2 commit)

---

**Total deviations:** 2 auto-fixed (1 bug, 1 blocking)
**Impact on plan:** Both fixes necessary for shadcn v4 / base-ui compatibility. No scope creep.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- Task 3 (checkpoint:human-verify) pending: end-to-end verification of credential CRUD flows
- All TypeScript compiles and production build succeeds

---
*Phase: 11-credential-management*
*Completed: 2026-03-23 (pending checkpoint verification)*

## Self-Check: PASSED
