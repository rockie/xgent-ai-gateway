# Phase 12: Dashboard and Metrics Visualization - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Admin sees a live operational dashboard with metrics charts and service health indicators on first login. This phase builds: (1) `GET /v1/admin/metrics/summary` endpoint for overview card data, (2) `GET /v1/admin/metrics/history` endpoint serving time-series data from an in-memory ring buffer, (3) dashboard page with overview cards, Recharts area charts, and compact service health list.

</domain>

<decisions>
## Implementation Decisions

### Metrics backend API
- **D-01:** New `GET /v1/admin/metrics/summary` endpoint returns a combined JSON response with: service_count, active_nodes, total_queue_depth, throughput (submitted_per_min, completed_per_min), and per-service health array (name, health, active_nodes, total_nodes, queue_depth). Single fetch for all 4 overview cards + health section.
- **D-02:** New `GET /v1/admin/metrics/history` endpoint returns time-series data from a server-side in-memory ring buffer. Each data point includes timestamp, tasks_submitted, tasks_completed, tasks_failed, per-service queue_depth, and per-service nodes_active.
- **D-03:** Ring buffer stores snapshots every 10 seconds, retaining 30 minutes of history (180 data points, ~50KB memory). History resets on gateway restart.
- **D-04:** Throughput rate (tasks/min) is computed server-side from Prometheus counter deltas over the last minute.

### Chart library and visuals
- **D-05:** Recharts library for all charts. Not using shadcn/ui chart wrappers — direct Recharts for more control.
- **D-06:** Two time-series area charts side-by-side: (1) Task Throughput — stacked areas for submitted (blue) vs completed (green), (2) Queue Depth — stacked areas per service with distinct colors.
- **D-07:** Charts auto-update via TanStack Query polling using the existing auto-refresh interval from the header control (Phase 8 D-22).

### Overview cards
- **D-08:** Four overview cards in a row: Services (count), Active Nodes (count), Queue Depth (aggregate), Throughput (tasks/min rate).
- **D-09:** Each card shows a delta arrow trend indicator (▲/▼) comparing current value to 5 minutes ago. Green for positive trends (more throughput, more nodes), red for negative. Uses ring buffer history data for delta computation.
- **D-10:** Service count card has no trend indicator (services change rarely).

### Dashboard page layout
- **D-11:** Top-to-bottom layout: overview cards row → two charts side-by-side → compact service health list. Natural hierarchy from summary to detail.
- **D-12:** Compact service health list at bottom: color dot + service name + active/total node count per row. Clicking a service navigates to `/services/$name`. Reuses Phase 9 health logic (green Healthy, yellow Degraded, red Down).
- **D-13:** Service health section heading: "Service Health".

### Claude's Discretion
- Ring buffer implementation details (VecDeque, custom struct, etc.)
- Exact Recharts styling (colors, axis formatting, tooltip design)
- Overview card icon choices and styling
- Loading skeleton design for dashboard page
- Empty state when no services are registered
- Error handling for metrics endpoints
- Chart responsive behavior at different viewport widths
- Delta arrow formatting details (percentage vs absolute change)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — DASH-01, DASH-02, DASH-03

### Existing metrics infrastructure
- `gateway/src/metrics.rs` — All 8 Prometheus metrics: tasks_submitted_total, tasks_completed_total, errors_total, callback_delivery_total, queue_depth, nodes_active, task_duration_seconds, node_poll_latency_seconds. Also `refresh_gauges()` function that reads Redis for current queue depths and active node counts.
- `gateway/src/http/admin.rs` line 478 — Existing `GET /metrics` handler returning Prometheus exposition format

### Frontend foundation (Phase 8)
- `admin-ui/src/routes/_authenticated/index.tsx` — Current dashboard stub (EmptyState placeholder)
- `admin-ui/src/lib/api.ts` — `apiClient()` fetch wrapper with cookie auth
- `admin-ui/src/hooks/use-auto-refresh.tsx` — Auto-refresh hook used by all data-fetching pages
- `admin-ui/src/components/empty-state.tsx` — EmptyState component
- `admin-ui/src/components/error-alert.tsx` — ErrorAlert with retry button

### Related patterns
- `admin-ui/src/lib/services.ts` — TanStack Query hook pattern for data fetching (follow for metrics hooks)
- `admin-ui/src/routes/_authenticated/services.tsx` — Card grid layout pattern (reference for overview cards)
- `gateway/src/registry/node_health.rs` — `derive_health_state()`, `NodeHealthState` enum (reuse for health badges)
- `gateway/src/main.rs` lines 249-277 — Admin route registration (add new metrics routes here)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `apiClient()`: Typed fetch wrapper with auth — use for metrics API calls
- `useAutoRefresh()`: Hook that triggers TanStack Query refetch at configured interval — charts and cards auto-update
- `EmptyState` component: For "no services registered" state on dashboard
- `ErrorAlert` component: For metrics endpoint failures
- `Metrics` struct in `gateway/src/metrics.rs`: All Prometheus counters/gauges already tracked — ring buffer reads from these
- `refresh_gauges()`: Already iterates services for queue depth and active nodes — similar logic needed for summary endpoint

### Established Patterns
- TanStack Query hooks in dedicated `lib/*.ts` files (services.ts, tasks.ts, credentials.ts) — create `lib/metrics.ts` for dashboard hooks
- shadcn/ui Card component used throughout — reuse for overview cards
- Color-coded health badges (green/yellow/red) from Phase 9 service cards — same logic for dashboard health list
- Data table and card grid patterns established — dashboard uses neither (custom layout with cards + charts + list)

### Integration Points
- `gateway/src/main.rs` admin routes: Add `/v1/admin/metrics/summary` and `/v1/admin/metrics/history`
- `gateway/src/state.rs` AppState: Ring buffer storage needs to be added to shared state
- `admin-ui/src/routes/_authenticated/index.tsx`: Replace EmptyState stub with dashboard implementation
- Background task in `main.rs` that calls `refresh_gauges()` periodically: Extend or add parallel task for ring buffer snapshots

</code_context>

<specifics>
## Specific Ideas

- Summary endpoint shape confirmed: `{ service_count, active_nodes, total_queue_depth, throughput: { submitted_per_min, completed_per_min }, services: [{ name, health, active_nodes, total_nodes, queue_depth }] }`
- History endpoint shape confirmed: `{ interval_secs, points: [{ timestamp, tasks_submitted, tasks_completed, tasks_failed, queue_depth: {svc: N}, nodes_active: {svc: N} }] }`
- Delta arrows compare current vs 5-minute-ago values — not percentage for counts (nodes, queue depth), percentage for rates (throughput)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 12-dashboard-and-metrics-visualization*
*Context gathered: 2026-03-23*
