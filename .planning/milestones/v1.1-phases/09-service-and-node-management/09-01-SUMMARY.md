---
phase: 09-service-and-node-management
plan: 01
subsystem: ui
tags: [react, tanstack-query, tanstack-router, shadcn, admin-ui, services]

requires:
  - phase: 08-frontend-foundation-backend-auth
    provides: "Vite+React app shell, auth hooks pattern, apiClient, EmptyState, PageSkeleton, ErrorAlert"
provides:
  - "Service list page at /services with card grid, empty state, and registration dialog"
  - "Shared lib/services.ts with all TypeScript types and TanStack Query hooks for service management"
  - "HealthBadge reusable component for service and node health visualization"
  - "apiClient 202 response handling for DELETE endpoints"
  - "shadcn dialog, table, badge, label, breadcrumb, alert-dialog components installed"
  - "services.$name route stub for type-safe navigation"
affects: [09-02-service-detail, 10-tasks-management]

tech-stack:
  added: [shadcn-dialog, shadcn-table, shadcn-badge, shadcn-label, shadcn-breadcrumb, shadcn-alert-dialog]
  patterns: [per-card-detail-fetch, derive-health-from-nodes, controlled-dialog-form]

key-files:
  created:
    - admin-ui/src/lib/services.ts
    - admin-ui/src/components/health-badge.tsx
    - admin-ui/src/components/service-card.tsx
    - admin-ui/src/components/service-registration-dialog.tsx
    - admin-ui/src/routes/_authenticated/services.$name.tsx
  modified:
    - admin-ui/src/lib/api.ts
    - admin-ui/src/components/empty-state.tsx
    - admin-ui/src/routes/_authenticated/services.tsx

key-decisions:
  - "Per-card detail fetch (N+1 pattern) acceptable for admin UI with small service count"
  - "Queue depth omitted from cards — backend admin API does not expose it (Prometheus only)"
  - "Created services.$name route stub to enable TanStack Router type-safe Link navigation"

patterns-established:
  - "Per-card detail fetch: ServiceCard calls useServiceDetail for node data not in list response"
  - "Derived health: deriveServiceHealth computes service-level health from node array"
  - "Controlled dialog pattern: parent manages open state, dialog handles form and mutation"

requirements-completed: [SVC-01, SVC-03, SVC-04]

duration: 6min
completed: 2026-03-23
---

# Phase 09 Plan 01: Service List Page Summary

**Service list page with card grid showing per-card health badges and node counts, registration dialog with all config fields, and shared hooks/types for service management**

## Performance

- **Duration:** 6 min
- **Started:** 2026-03-23T03:14:01Z
- **Completed:** 2026-03-23T03:20:01Z
- **Tasks:** 2
- **Files modified:** 13

## Accomplishments
- Built service list page at /services replacing placeholder stub with card grid, empty state with CTA, and header register button
- Created shared lib/services.ts with all TypeScript types matching backend response shapes and 4 TanStack Query hooks (useServices, useServiceDetail, useRegisterService, useDeregisterService)
- Built HealthBadge component supporting all health states (healthy, stale, degraded, disconnected, down, draining, unknown) with color-coded dots
- Created ServiceRegistrationDialog with all 7 RegisterServiceRequest fields including advanced settings section
- Installed 6 shadcn components needed by both Plan 01 and Plan 02

## Task Commits

Each task was committed atomically:

1. **Task 1: Create shared service types, API hooks, health badge, and fix apiClient for 202** - `81ed832` (feat)
2. **Task 2: Install shadcn components and build service list page with registration dialog** - `c6e888c` (feat)

## Files Created/Modified
- `admin-ui/src/lib/api.ts` - Added 202 status handling alongside existing 204
- `admin-ui/src/lib/services.ts` - All TypeScript types and TanStack Query hooks for service management
- `admin-ui/src/components/health-badge.tsx` - Reusable colored dot + label for health status
- `admin-ui/src/components/empty-state.tsx` - Extended with optional action prop
- `admin-ui/src/components/service-card.tsx` - Card with per-card detail fetch for health and node count
- `admin-ui/src/components/service-registration-dialog.tsx` - Controlled dialog with all RegisterServiceRequest fields
- `admin-ui/src/routes/_authenticated/services.tsx` - Full service list page replacing placeholder
- `admin-ui/src/routes/_authenticated/services.$name.tsx` - Route stub for type-safe navigation
- `admin-ui/src/components/ui/dialog.tsx` - shadcn dialog component
- `admin-ui/src/components/ui/table.tsx` - shadcn table component
- `admin-ui/src/components/ui/badge.tsx` - shadcn badge component
- `admin-ui/src/components/ui/label.tsx` - shadcn label component
- `admin-ui/src/components/ui/breadcrumb.tsx` - shadcn breadcrumb component
- `admin-ui/src/components/ui/alert-dialog.tsx` - shadcn alert-dialog component

## Decisions Made
- **Per-card detail fetch (N+1):** The list endpoint does not include node data. Each ServiceCard calls useServiceDetail to get nodes for health badge and node count. Acceptable for admin UI with small service count (<20).
- **Queue depth omitted:** The backend admin API does not expose queue depth (only available via Prometheus metrics). Cards show node count only.
- **Route stub for navigation:** Created services.$name.tsx placeholder so TanStack Router type-safe Links work. Plan 02 will replace with full detail page.

## Deviations from Plan

### Auto-fixed Issues

**1. [Rule 3 - Blocking] Created services.$name route stub for type-safe navigation**
- **Found during:** Task 2 (service list page build)
- **Issue:** TanStack Router Link to `/services/$name` caused TypeScript error because the route file didn't exist yet (planned for Plan 02)
- **Fix:** Created minimal route stub at services.$name.tsx that Plan 02 will replace
- **Files modified:** admin-ui/src/routes/_authenticated/services.$name.tsx
- **Verification:** Build passes with type-safe Link navigation
- **Committed in:** c6e888c (Task 2 commit)

---

**Total deviations:** 1 auto-fixed (1 blocking)
**Impact on plan:** Essential for build to pass with TanStack Router's type-safe routing. No scope creep.

## Known Stubs
- `admin-ui/src/routes/_authenticated/services.$name.tsx` - Placeholder detail page showing service name only. Will be replaced by Plan 02 with full detail view.

## Issues Encountered
None

## User Setup Required
None - no external service configuration required.

## Next Phase Readiness
- All shared types, hooks, and components ready for Plan 02 (service detail page)
- shadcn components (table, breadcrumb, alert-dialog) pre-installed for Plan 02
- Route stub in place at services.$name.tsx ready to be replaced with full implementation

---
*Phase: 09-service-and-node-management*
*Completed: 2026-03-23*

## Self-Check: PASSED
- All 11 created files verified present
- Both task commits (81ed832, c6e888c) verified in git log
