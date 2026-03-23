# Phase 9: Service and Node Management - Research

**Researched:** 2026-03-23
**Domain:** React frontend — service CRUD UI with TanStack Router/Query against existing REST endpoints
**Confidence:** HIGH

## Summary

Phase 9 builds the service management UI — a card-grid list page, a detail page with node health table, a registration dialog, and a deregistration flow. All four backend endpoints already exist and are verified: `GET /v1/admin/services` (list), `POST /v1/admin/services` (register), `GET /v1/admin/services/{name}` (detail with nodes), `DELETE /v1/admin/services/{name}` (deregister, returns 202).

The frontend foundation from Phase 8 provides TanStack Router file-based routing, TanStack Query for data fetching, shadcn/ui v4 components, Sonner toasts, and an auto-refresh context. The existing `services.tsx` route is a placeholder stub ready to be replaced. A new dynamic route `services/$name.tsx` is needed for the detail page.

**Primary recommendation:** Build incrementally — service list page first (with API hooks), then detail page with node table, then registration dialog, then deregistration flow. Each piece is independently testable against the existing backend.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Card grid layout — one card per service showing name, description, node count (active/total), queue depth, and created date.
- **D-02:** Health indicator on each card: color dot + label — green "Healthy" (all nodes active), yellow "Degraded" (some stale), red "Down" (no healthy nodes).
- **D-03:** "Register Service" primary button in the top-right of the page header, next to the "Services" title.
- **D-04:** Empty state uses EmptyState component with Server icon, "No services registered" heading, and a prominent "Register your first service" CTA button.
- **D-05:** Clicking a service card navigates to `/services/$name` (TanStack Router dynamic route).
- **D-06:** Single scrollable page with two sections — service config at top, then node list table below.
- **D-07:** Breadcrumb navigation: "Services > {service-name}" at the top for back navigation.
- **D-08:** Deregister button on the detail page (destructive action with confirmation dialog).
- **D-09:** Node table columns: Node ID, Health, In-flight Tasks, Last Seen.
- **D-10:** Node health displayed as color dot + status text — green "Healthy", yellow "Stale", red "Disconnected", blue "Draining".
- **D-11:** Node table is sufficient for NODE-02 requirements — no separate node detail page needed.
- **D-12:** Registration opens as a dialog/sheet from the "Register Service" button. Form fields: name (required), description (optional), task_timeout_secs, max_retries, max_nodes, node_stale_after_secs, drain_timeout_secs (all optional with server defaults).

### Claude's Discretion
- Service registration form layout and field grouping
- Deregister confirmation dialog exact wording and behavior
- Card grid responsive breakpoints (2-col, 3-col, etc.)
- How to handle 202 Accepted after deregister (optimistic removal vs poll for cleanup)
- Loading skeleton design for service list and detail pages
- "Last seen" time formatting (relative vs absolute)
- Queue depth display on cards (how to fetch — list endpoint returns config only, detail endpoint has nodes but no queue depth)

### Deferred Ideas (OUT OF SCOPE)
- Service config editing (inline edit timeout/max_nodes) — deferred per REQUIREMENTS.md EDIT-01
- Node drain/disconnect actions from the UI — not in current requirements
- Queue depth history/sparkline on cards — could be a dashboard enhancement (Phase 12)
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SVC-01 | Admin can view list of all registered services | `GET /v1/admin/services` returns `ListServicesResponse` with array of `ServiceResponse` objects. Card grid layout per D-01. |
| SVC-02 | Admin can view service detail (config, connected nodes, queue depth) | `GET /v1/admin/services/{name}` returns `ServiceDetailResponse` with flattened service config + `nodes[]` array. Queue depth not in response — see Open Questions. |
| SVC-03 | Admin can register a new service via form | `POST /v1/admin/services` accepts `RegisterServiceRequest` with name (required) + 6 optional fields. Returns 201 + `ServiceResponse`. |
| SVC-04 | Admin can deregister a service with confirmation dialog | `DELETE /v1/admin/services/{name}` returns 202 Accepted (async cleanup). Frontend needs confirmation dialog before calling. |
| NODE-01 | Admin can view per-service node list with health status | Detail endpoint returns `nodes[]` with `health` field (string: "healthy", "unhealthy", "disconnected") and `draining` boolean. |
| NODE-02 | Admin can see node details (in-flight tasks, drain status, last seen) | Each `NodeStatusResponse` includes `node_id`, `health`, `in_flight_tasks` (u32), `draining` (bool), `last_seen` (RFC3339 string). Node table per D-09/D-11 suffices. |
</phase_requirements>

## Standard Stack

### Core (already installed in admin-ui)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| React | 19.2.x | UI framework | Already installed, project standard |
| TanStack Router | 1.168.x | File-based routing | Already set up with `_authenticated` layout, dynamic routes via `$param` |
| TanStack Query | 5.95.x | Data fetching + cache | Already configured with QueryClient, auto-refresh via `refetchInterval` |
| shadcn/ui | v4 (base-nova) | Component library | Already installed — Card, Button, Dialog, Sheet, Input, Table needed |
| Sonner | 2.0.x | Toast notifications | Already configured in root layout, use `toast.success()`/`toast.error()` |
| Lucide React | 0.577.x | Icons | Already installed — Server, Plus, Trash2, ArrowLeft, Circle for health dots |

### shadcn/ui Components Needed (not yet installed)
| Component | Purpose | Install Command |
|-----------|---------|-----------------|
| **dialog** | Registration form modal, deregister confirmation | `npx shadcn@latest add dialog` |
| **table** | Node list on detail page | `npx shadcn@latest add table` |
| **badge** | Health status badges on cards and node rows | `npx shadcn@latest add badge` |
| **label** | Form field labels in registration dialog | `npx shadcn@latest add label` |
| **breadcrumb** | "Services > service-name" navigation on detail page | `npx shadcn@latest add breadcrumb` |
| **alert-dialog** | Destructive action confirmation for deregister | `npx shadcn@latest add alert-dialog` |

**Already installed:** alert, button, card, dropdown-menu, input, separator, sheet, sidebar, skeleton, sonner, tooltip

**Installation (all at once):**
```bash
cd admin-ui && npx shadcn@latest add dialog table badge label breadcrumb alert-dialog
```

## Architecture Patterns

### Recommended File Structure
```
admin-ui/src/
├── routes/_authenticated/
│   ├── services.tsx                  # Service list page (replace stub)
│   └── services/$name.tsx            # Service detail page (new dynamic route)
├── lib/
│   ├── api.ts                        # Existing fetch wrapper
│   └── services.ts                   # NEW: Service API hooks (queries + mutations)
├── components/
│   ├── service-card.tsx              # NEW: Service card for grid
│   ├── service-registration-dialog.tsx # NEW: Register form in dialog
│   ├── deregister-dialog.tsx         # NEW: Confirmation alert dialog
│   ├── node-table.tsx                # NEW: Node health table
│   ├── health-badge.tsx              # NEW: Reusable health dot + label
│   ├── empty-state.tsx               # Existing (extend with action prop)
│   ├── error-alert.tsx               # Existing
│   └── page-skeleton.tsx             # Existing
└── hooks/
    └── use-auto-refresh.tsx          # Existing — used for refetchInterval
```

### Pattern 1: TanStack Query Hooks for Service API
**What:** Centralize all service API calls in `lib/services.ts` using `useQuery` and `useMutation`.
**When to use:** Every component that reads or writes service/node data.
**Example:**
```typescript
// lib/services.ts
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { apiClient } from './api'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'

// Types matching backend responses
export interface ServiceResponse {
  name: string
  description: string
  created_at: string
  task_timeout_secs: number
  max_retries: number
  max_nodes: number | null
  node_stale_after_secs: number
  drain_timeout_secs: number
}

export interface NodeStatusResponse {
  node_id: string
  health: string        // "healthy" | "unhealthy" | "disconnected"
  last_seen: string     // RFC3339
  in_flight_tasks: number
  draining: boolean
}

export interface ServiceDetailResponse extends ServiceResponse {
  nodes: NodeStatusResponse[]
}

export interface ListServicesResponse {
  services: ServiceResponse[]
}

export function useServices() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['services'],
    queryFn: () => apiClient<ListServicesResponse>('/v1/admin/services'),
    refetchInterval: effectiveInterval,
  })
}

export function useServiceDetail(name: string) {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['services', name],
    queryFn: () => apiClient<ServiceDetailResponse>(`/v1/admin/services/${encodeURIComponent(name)}`),
    refetchInterval: effectiveInterval,
  })
}

export function useRegisterService() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (data: RegisterServiceRequest) =>
      apiClient<ServiceResponse>('/v1/admin/services', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['services'] })
    },
  })
}

export function useDeregisterService() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (name: string) =>
      apiClient<void>(`/v1/admin/services/${encodeURIComponent(name)}`, {
        method: 'DELETE',
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['services'] })
    },
  })
}
```

### Pattern 2: TanStack Router Dynamic Route with Params
**What:** File-based dynamic route using `$name` param for service detail.
**When to use:** Service detail page.
**Example:**
```typescript
// routes/_authenticated/services/$name.tsx
import { createFileRoute } from '@tanstack/react-router'

export const Route = createFileRoute('/_authenticated/services/$name')({
  component: ServiceDetailPage,
})

function ServiceDetailPage() {
  const { name } = Route.useParams()
  // use name to fetch service detail
}
```

### Pattern 3: EmptyState with Action Button
**What:** Extend EmptyState component to accept an optional `action` prop for CTA buttons.
**When to use:** D-04 requires "Register your first service" button in empty state.
**Example:**
```typescript
interface EmptyStateProps {
  icon?: LucideIcon
  heading: string
  description: string
  action?: ReactNode    // NEW: optional action slot
}
```

### Pattern 4: Health Badge Component
**What:** Reusable colored dot + label for service and node health states.
**When to use:** Service cards (D-02) and node table rows (D-10).
**Example:**
```typescript
// D-02 service-level health: derived from node statuses
type ServiceHealth = 'healthy' | 'degraded' | 'down'
// D-10 node-level health: from API response + draining check
type NodeHealth = 'healthy' | 'stale' | 'disconnected' | 'draining'

// Color mapping:
// healthy    -> green  (bg-green-500)
// degraded   -> yellow (bg-yellow-500)
// stale      -> yellow (bg-yellow-500)
// down       -> red    (bg-red-500)
// disconnected -> red  (bg-red-500)
// draining   -> blue   (bg-blue-500)
```

### Pattern 5: Deregister with 202 Handling
**What:** DELETE returns 202 (async cleanup). Handle optimistically.
**Recommendation:** On 202, show success toast "Service deregistration started", invalidate queries, and navigate back to list. The service will disappear from the list once cleanup completes. The `apiClient` already handles non-JSON responses for 204 — 202 also returns no body, so the mutation needs to handle this.
**Important:** `apiClient` currently tries `response.json()` for any non-204 success. Since 202 has no body, either:
  - (a) Add 202 handling to `apiClient` (like the existing 204 check), OR
  - (b) The DELETE handler could return a JSON body (but it currently doesn't — it returns bare `StatusCode::ACCEPTED`)

**Recommendation:** Add `response.status === 202` check to `apiClient` alongside the 204 check. Minimal change, consistent pattern.

### Anti-Patterns to Avoid
- **Fetching detail per-card for node counts:** The list endpoint does NOT return node counts. Do NOT call `GET /v1/admin/services/{name}` for every card — this creates N+1 API calls. Instead, either (a) show node count only on the detail page, or (b) add a lightweight enrichment endpoint. See Open Questions.
- **Storing derived health in component state:** Health is computed from node data. Use the query response directly; do not duplicate in useState.
- **Building custom form validation:** Use HTML5 required/pattern attributes for the simple registration form. No need for react-hook-form or zod for 7 fields.
- **Polling too aggressively:** Auto-refresh from header context already handles interval. Do NOT add a separate setInterval.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Confirmation dialog | Custom modal with portal management | shadcn AlertDialog | Handles focus trap, escape, overlay click, accessible by default |
| Form dialog | Custom overlay + form | shadcn Dialog + native form elements | Dialog manages open/close state, form onSubmit handles validation |
| Data table | Custom div-based table | shadcn Table (HTML table with styles) | Semantic HTML, accessible, consistent styling. No need for TanStack Table — the node list is static columns, no sorting/filtering. |
| Relative time display | Custom date math | Simple utility function (see Code Examples) | Only need "X minutes ago" format — not worth a library dependency for one use |
| Toast notifications | Custom notification system | Sonner (already installed) | Already configured in root layout |
| Breadcrumbs | Custom nav links | shadcn Breadcrumb | Consistent with shadcn design system, accessible markup |

**Key insight:** This phase is pure CRUD UI — list, detail, create, delete. Every pattern is standard and well-served by the existing stack. No exotic requirements.

## Common Pitfalls

### Pitfall 1: Service Name URL Encoding
**What goes wrong:** Service names containing special characters (dots, slashes, spaces) break URL routing.
**Why it happens:** `$name` param in TanStack Router captures the raw URL segment.
**How to avoid:** Always use `encodeURIComponent(name)` when building API URLs. TanStack Router's `useParams()` auto-decodes, but API calls need explicit encoding.
**Warning signs:** 404 errors for services with dots in names (e.g., "ml.inference").

### Pitfall 2: 202 Response Parsing
**What goes wrong:** `apiClient` calls `response.json()` on 202 responses which have no body, causing a parse error.
**Why it happens:** The DELETE endpoint returns bare `StatusCode::ACCEPTED` with no JSON body. The current `apiClient` only handles 204 as a no-body case.
**How to avoid:** Add `response.status === 202` to the no-body check in `apiClient` alongside the existing 204 check.
**Warning signs:** "Unexpected end of JSON input" error after successful deregister.

### Pitfall 3: Service Health Derivation on List Page
**What goes wrong:** The list endpoint returns `ServiceResponse[]` which has NO node information. You cannot compute service-level health (D-02) from the list response alone.
**Why it happens:** `list_services` only reads `ServiceConfig` from Redis — it does not enumerate nodes.
**How to avoid:** Two options: (a) Fetch each service's detail to get node health (N+1 problem), or (b) show a simpler indicator on list cards (e.g., just node count from a lightweight endpoint). See Open Questions for recommendation.
**Warning signs:** Cards showing stale or missing health data.

### Pitfall 4: Stale Query Cache After Deregister
**What goes wrong:** After deregistering a service and navigating back to the list, the deleted service still appears briefly.
**Why it happens:** TanStack Query serves stale cache while refetching. The 202 means cleanup is async — the service may still exist in Redis for a moment.
**How to avoid:** Use `queryClient.invalidateQueries({ queryKey: ['services'] })` in the mutation's `onSuccess`, AND use optimistic updates to immediately remove the card from the list. Or simply accept a brief flash and let the refetch clean it up.
**Warning signs:** Deleted service briefly visible after navigation.

### Pitfall 5: EmptyState Component Extension
**What goes wrong:** Modifying EmptyState breaks existing usages (dashboard, credentials, tasks stubs).
**Why it happens:** Adding required props to an existing component.
**How to avoid:** Make the `action` prop optional with `action?: ReactNode`. Existing usages without an action prop continue to work unchanged.
**Warning signs:** TypeScript errors in other route files after modifying EmptyState.

## Code Examples

### Relative Time Formatter (for "Last seen")
```typescript
// Simple relative time — no library needed
export function relativeTime(isoString: string): string {
  const date = new Date(isoString)
  const now = Date.now()
  const diffMs = now - date.getTime()

  if (diffMs < 0) return 'just now'

  const seconds = Math.floor(diffMs / 1000)
  if (seconds < 60) return `${seconds}s ago`

  const minutes = Math.floor(seconds / 60)
  if (minutes < 60) return `${minutes}m ago`

  const hours = Math.floor(minutes / 60)
  if (hours < 24) return `${hours}h ago`

  const days = Math.floor(hours / 24)
  return `${days}d ago`
}
```

### Service-Level Health Derivation (from nodes array)
```typescript
// Derive service health from its nodes for D-02
// Must be called on detail response (which includes nodes)
export function deriveServiceHealth(
  nodes: NodeStatusResponse[]
): 'healthy' | 'degraded' | 'down' | 'unknown' {
  if (nodes.length === 0) return 'unknown'

  const healthyCount = nodes.filter(
    (n) => n.health === 'healthy' && !n.draining
  ).length

  if (healthyCount === nodes.length) return 'healthy'
  if (healthyCount === 0) return 'down'
  return 'degraded'
}
```

### Card Grid Layout (responsive breakpoints)
```typescript
// Tailwind grid with responsive breakpoints
// 1 col on mobile, 2 cols on md, 3 cols on lg
<div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-3">
  {services.map((svc) => (
    <ServiceCard key={svc.name} service={svc} />
  ))}
</div>
```

### Node Health Color Map
```typescript
const healthConfig: Record<string, { color: string; label: string }> = {
  healthy:      { color: 'bg-green-500', label: 'Healthy' },
  unhealthy:    { color: 'bg-yellow-500', label: 'Stale' },      // backend "unhealthy" = UI "Stale" per D-10
  disconnected: { color: 'bg-red-500', label: 'Disconnected' },
  draining:     { color: 'bg-blue-500', label: 'Draining' },
}

// Note: backend returns "unhealthy" but D-10 says display "Stale"
// Also: draining is a separate boolean field, not a health value from the API
// A node with draining=true should show "Draining" regardless of health field
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| shadcn/ui v0 (HSL colors, Radix primitives) | shadcn/ui v4 (oklch colors, Base UI primitives) | 2025 | Component APIs differ — use v4 patterns. This project already uses v4. |
| TanStack Router v1 `@tanstack/router` | TanStack Router v1 `@tanstack/react-router` | stable | File-based routing with `createFileRoute` — already in use. |
| React Query v4 `useQuery({ queryKey, queryFn })` | React Query v5 (same API surface) | 2024 | Minimal change — v5 used here. `suspense` option removed; use separate Suspense boundary if needed. |

## Open Questions

1. **Queue depth on service cards (D-01 mentions queue depth)**
   - What we know: The list endpoint (`GET /v1/admin/services`) returns only `ServiceConfig` fields — no queue depth, no node count. Queue depth is tracked via Redis Streams (`XLEN tasks:{name}`) and exposed via Prometheus metrics gauge, not via the admin API.
   - What's unclear: Whether to add queue depth to the list response (backend change) or accept showing it only on the detail page.
   - Recommendation: **Show queue depth only on the detail page for now.** The detail endpoint can be extended with a single XLEN call. Alternatively, the card can show "X nodes" (from a quick SMEMBERS count added to the list endpoint) instead of queue depth. Changing the backend is a minor addition but crosses phase boundaries — flag for planner to decide. If the planner wants queue depth on cards, a backend task must be added: extend `list_services` response to include `node_count` and `queue_depth` per service.

2. **Node count on service cards (D-01 mentions "node count active/total")**
   - What we know: Same issue as queue depth — list endpoint has no node info. The detail endpoint has the full node list.
   - What's unclear: Whether fetching detail per service (N+1) is acceptable for small service counts.
   - Recommendation: **For MVP, fetch detail per service only if service count is small (< 20).** Alternatively, add `node_count` and `active_node_count` to the list endpoint. The latter is cleaner but requires a backend change. The planner should decide: (a) N+1 detail fetches, (b) backend enrichment, or (c) omit node count from cards and show only on detail page.

3. **Backend "unhealthy" vs UI "Stale" naming (D-10)**
   - What we know: Backend `NodeHealthState` has three variants: `Healthy`, `Unhealthy`, `Disconnected`. The API serializes as lowercase strings. D-10 wants the UI to show "Stale" instead of "Unhealthy".
   - What's unclear: Nothing — this is just a frontend display mapping.
   - Recommendation: Map `"unhealthy"` to display label `"Stale"` in the health badge component. The `draining` boolean is a separate field — when `draining === true`, override the health display to show "Draining" (blue) regardless of the `health` string.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Vite + no test runner detected in admin-ui |
| Config file | none — no vitest/jest config in admin-ui |
| Quick run command | N/A — no test infrastructure |
| Full suite command | N/A |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SVC-01 | Service list renders cards from API response | manual-only | Manual: visit /services with running gateway | N/A |
| SVC-02 | Service detail shows config + nodes | manual-only | Manual: click service card, verify detail page | N/A |
| SVC-03 | Register service form submits and creates service | manual-only | Manual: fill form, submit, verify new card appears | N/A |
| SVC-04 | Deregister with confirmation removes service | manual-only | Manual: click deregister, confirm, verify removal | N/A |
| NODE-01 | Node table shows health status per node | manual-only | Manual: visit detail page for service with nodes | N/A |
| NODE-02 | Node details show in-flight, drain, last seen | manual-only | Manual: inspect node row on detail page | N/A |

**Justification for manual-only:** admin-ui has no test framework configured (no vitest, jest, or playwright). Adding test infrastructure would be a separate effort. All requirements are UI-rendering behaviors best verified by visual inspection against a running gateway.

### Sampling Rate
- **Per task commit:** `cd admin-ui && npm run build` (TypeScript + Vite build catches type errors)
- **Per wave merge:** Full build + manual verification against running gateway
- **Phase gate:** All 6 requirements manually verified with screenshots

### Wave 0 Gaps
- No test framework installed — out of scope for this phase (UI-only CRUD)
- Build verification (`npm run build`) is the automated safety net

## Sources

### Primary (HIGH confidence)
- `gateway/src/http/admin.rs` lines 199-390 — All request/response types and endpoint implementations verified by direct code reading
- `gateway/src/registry/node_health.rs` — NodeHealthState enum (Healthy/Unhealthy/Disconnected), derive_health_state logic verified
- `gateway/src/registry/service.rs` — list_services uses `services:index` SMEMBERS, confirmed no node/queue enrichment
- `gateway/src/metrics.rs` lines 140-180 — Queue depth uses `XLEN tasks:{name}`, confirmed not exposed via admin API
- `admin-ui/package.json` — All dependency versions verified from file
- `admin-ui/src/` — Existing components, routes, hooks, and patterns verified by direct code reading

### Secondary (MEDIUM confidence)
- shadcn/ui v4 component install commands — based on `components.json` config showing `base-nova` style and v4 schema
- TanStack Router dynamic route `$name` pattern — standard file-based routing convention, consistent with existing route structure

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH — all libraries already installed and verified in package.json
- Architecture: HIGH — follows established Phase 8 patterns exactly, all backend endpoints verified
- Pitfalls: HIGH — identified from direct code analysis (202 handling, missing node data in list endpoint, URL encoding)
- API contracts: HIGH — response types read directly from Rust source code

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (stable — no moving parts, all backend endpoints frozen)
