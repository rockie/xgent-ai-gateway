# Phase 9: Service and Node Management - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Admin can view, create, and manage services and inspect node health from the UI. This phase builds the service list page, service detail page, service registration form, and deregistration flow using existing backend endpoints (`GET/POST /v1/admin/services`, `GET/DELETE /v1/admin/services/{name}`).

</domain>

<decisions>
## Implementation Decisions

### Service list layout
- **D-01:** Card grid layout — one card per service showing name, description, node count (active/total), queue depth, and created date.
- **D-02:** Health indicator on each card: color dot + label — green "Healthy" (all nodes active), yellow "Degraded" (some stale), red "Down" (no healthy nodes).
- **D-03:** "Register Service" primary button in the top-right of the page header, next to the "Services" title.
- **D-04:** Empty state uses EmptyState component with Server icon, "No services registered" heading, and a prominent "Register your first service" CTA button.
- **D-05:** Clicking a service card navigates to `/services/$name` (TanStack Router dynamic route).

### Service detail page
- **D-06:** Single scrollable page with two sections — service config at top (name, description, timeout, max_nodes, stale_after, drain_timeout, created_at), then node list table below.
- **D-07:** Breadcrumb navigation: "Services > {service-name}" at the top for back navigation.
- **D-08:** Deregister button on the detail page (destructive action with confirmation dialog).
- **D-09:** Node table columns: Node ID, Health, In-flight Tasks, Last Seen.
- **D-10:** Node health displayed as color dot + status text — green "Healthy", yellow "Stale", red "Disconnected", blue "Draining". Consistent with card health badges.
- **D-11:** Node table is sufficient for NODE-02 requirements — no separate node detail page or expandable rows needed.

### Service registration form
- **D-12:** Registration opens as a dialog/sheet from the "Register Service" button. Form fields match `RegisterServiceRequest`: name (required), description (optional), task_timeout_secs, max_retries, max_nodes, node_stale_after_secs, drain_timeout_secs (all optional with server defaults).

### Claude's Discretion
- Service registration form layout and field grouping
- Deregister confirmation dialog exact wording and behavior
- Card grid responsive breakpoints (2-col, 3-col, etc.)
- How to handle 202 Accepted after deregister (optimistic removal vs poll for cleanup)
- Loading skeleton design for service list and detail pages
- "Last seen" time formatting (relative vs absolute)
- Queue depth display on cards (how to fetch — existing list endpoint returns service config, may need to call detail endpoint per service or add queue depth to list response)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — SVC-01 through SVC-04, NODE-01, NODE-02

### Backend endpoints (all already implemented)
- `gateway/src/http/admin.rs` lines 260-390 — `register_service`, `deregister_service`, `list_services`, `get_service_detail` handlers with request/response types
- `gateway/src/registry/service.rs` — Service CRUD operations in Redis, `ServiceConfig` struct
- `gateway/src/registry/node_health.rs` — `derive_health_state`, `NodeHealthState` enum, node health tracking
- `gateway/src/main.rs` lines 249-255 — Admin route registration for service endpoints

### Frontend foundation (Phase 8)
- `admin-ui/src/lib/api.ts` — `apiClient()` fetch wrapper with cookie auth
- `admin-ui/src/components/empty-state.tsx` — Reusable EmptyState component
- `admin-ui/src/components/error-alert.tsx` — ErrorAlert with retry button
- `admin-ui/src/components/page-skeleton.tsx` — PageSkeleton loading component
- `admin-ui/src/routes/_authenticated/services.tsx` — Existing stub route (placeholder)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EmptyState` component: Accepts icon, heading, description — extend with optional action button for CTA
- `ErrorAlert` component: Error message + retry button pattern — reuse for API failures
- `PageSkeleton` component: Loading skeleton — customize for card grid and detail page shapes
- `apiClient()`: Typed fetch wrapper with auth — use for all service/node API calls
- TanStack Query: Already set up for data fetching with auto-refresh from header control
- shadcn/ui components: Already installed — Table, Card, Button, Dialog, Alert available

### Established Patterns
- TanStack Router file-based routes under `_authenticated/` with auth guard
- `credentials: 'include'` on all API calls for cookie session
- Sonner for toast notifications on mutations
- Auto-refresh control in header applies to active page via TanStack Query `refetchInterval`

### Integration Points
- `_authenticated/services.tsx` — Replace placeholder with service list page
- Add `_authenticated/services/$name.tsx` — New dynamic route for service detail
- Sidebar nav already links to `/services` — no navigation changes needed
- Registration form will POST to `/v1/admin/services` and invalidate the service list query

</code_context>

<specifics>
## Specific Ideas

- Health badge colors should be consistent between service cards and node table rows — same green/yellow/red/blue scheme used everywhere
- Card grid should feel clean and scannable, not cluttered — key stats visible at a glance

</specifics>

<deferred>
## Deferred Ideas

- Service config editing (inline edit timeout/max_nodes) — deferred per REQUIREMENTS.md EDIT-01
- Node drain/disconnect actions from the UI — not in current requirements
- Queue depth history/sparkline on cards — could be a dashboard enhancement (Phase 12)

</deferred>

---

*Phase: 09-service-and-node-management*
*Context gathered: 2026-03-23*
