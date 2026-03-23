# Phase 12: Dashboard and Metrics Visualization - Research

**Researched:** 2026-03-23
**Domain:** Rust backend metrics API + React dashboard with Recharts charting
**Confidence:** HIGH

## Summary

Phase 12 adds two backend endpoints (`/v1/admin/metrics/summary` and `/v1/admin/metrics/history`) and a frontend dashboard page replacing the current EmptyState stub at the authenticated index route. The backend work involves an in-memory ring buffer (VecDeque of snapshots) that captures Prometheus counter/gauge values every 10 seconds, plus two Axum handlers that serialize this data as JSON. The frontend work involves overview cards, two Recharts AreaChart components, and a compact service health list.

The existing codebase provides strong foundations: `refresh_gauges()` already iterates services for queue depth and active node counts (reuse for summary), the `health_handler` already computes per-service health (pattern to follow), and the admin-ui has established TanStack Query hook patterns with auto-refresh. Recharts 3.8.0 is the current stable release, fully compatible with React 19, and should be used instead of Recharts 2.x.

**Primary recommendation:** Use Recharts 3.x (latest stable), add ring buffer to AppState behind `Arc<Mutex<VecDeque<MetricsSnapshot>>>`, snapshot every 10s from the existing background gauge refresh task, and follow the established `lib/*.ts` hook pattern for `lib/metrics.ts`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** New `GET /v1/admin/metrics/summary` endpoint returns combined JSON: service_count, active_nodes, total_queue_depth, throughput (submitted_per_min, completed_per_min), and per-service health array
- **D-02:** New `GET /v1/admin/metrics/history` endpoint returns time-series from server-side in-memory ring buffer
- **D-03:** Ring buffer stores snapshots every 10 seconds, retaining 30 minutes (180 data points, ~50KB)
- **D-04:** Throughput rate computed server-side from Prometheus counter deltas over last minute
- **D-05:** Recharts library for all charts (direct, not shadcn/ui chart wrappers)
- **D-06:** Two time-series area charts side-by-side: Task Throughput (stacked submitted/completed) and Queue Depth (stacked per service)
- **D-07:** Charts auto-update via TanStack Query polling using existing auto-refresh interval
- **D-08:** Four overview cards: Services, Active Nodes, Queue Depth, Throughput
- **D-09:** Delta arrow trend indicators comparing current vs 5 minutes ago
- **D-10:** Service count card has no trend indicator
- **D-11:** Top-to-bottom layout: cards -> charts -> service health list
- **D-12:** Compact service health list with color dots, clickable to service detail
- **D-13:** Service health section heading: "Service Health"

### Claude's Discretion
- Ring buffer implementation details (VecDeque, custom struct, etc.)
- Exact Recharts styling (colors, axis formatting, tooltip design)
- Overview card icon choices and styling
- Loading skeleton design for dashboard page
- Empty state when no services are registered
- Error handling for metrics endpoints
- Chart responsive behavior at different viewport widths
- Delta arrow formatting details (percentage vs absolute change)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| DASH-01 | Admin sees overview cards (service count, active nodes, queue depth, task throughput) | Summary endpoint provides all card data in single fetch; Card component from shadcn/ui; delta arrows from ring buffer history |
| DASH-02 | Admin sees live time-series charts for throughput and queue depth (polling every 10-15s) | History endpoint serves ring buffer data; Recharts 3.x AreaChart with stacked areas; useAutoRefresh hook drives polling |
| DASH-03 | Admin sees color-coded service health badges (green/yellow/red) | Summary endpoint includes per-service health; HealthBadge component already exists; deriveServiceHealth logic reusable |
</phase_requirements>

## Standard Stack

### Core
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| recharts | 3.8.0 | React charting | Current stable. Recharts 3 rewrote state management, added 3500+ tests, fixed React 19 ResponsiveContainer issues that plagued 2.x. Direct AreaChart API -- no wrapper needed. |
| @tanstack/react-query | 5.95.0 (already installed) | Data fetching + polling | Already used for all admin-ui data; refetchInterval drives auto-refresh |
| prometheus (Rust) | 0.14 (already installed) | Metrics source | CounterVec.with_label_values().get() reads current totals for ring buffer snapshots |

### Supporting
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| lucide-react | 0.577.0 (already installed) | Card icons | Overview card icons (Activity, Server, Layers, Zap or similar) |
| shadcn/ui Card | already installed | Card layout | Overview cards reuse existing Card component |

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| Recharts 3 | Chart.js / react-chartjs-2 | Chart.js has smaller bundle but imperative API; Recharts is declarative React-native. Decision D-05 locks Recharts. |
| Recharts 3 | Recharts 2.x | Recharts 2.x has React 19 ResponsiveContainer bugs. Recharts 3 fixes these and is stable. |
| In-memory ring buffer | Redis time-series | Overkill for 180 data points; adds Redis complexity. In-memory is correct per D-03. |

**Installation (admin-ui only -- no new Rust deps needed):**
```bash
cd admin-ui && npm install recharts
```

**Version verification:** recharts 3.8.0 confirmed via `npm view recharts version` on 2026-03-23.

## Architecture Patterns

### Backend: Ring Buffer + Snapshot Task

**Ring buffer storage in AppState:**
```rust
// Add to state.rs
use std::collections::VecDeque;
use std::sync::Mutex;

pub struct MetricsSnapshot {
    pub timestamp: i64,  // Unix epoch seconds
    pub tasks_submitted: f64,  // counter total at snapshot time
    pub tasks_completed: f64,
    pub tasks_failed: f64,
    pub queue_depth: HashMap<String, f64>,  // service -> depth
    pub nodes_active: HashMap<String, f64>, // service -> count
}

// In AppState:
pub metrics_history: Arc<Mutex<VecDeque<MetricsSnapshot>>>,
```

**Why `Mutex<VecDeque>` not `RwLock`:** Writes happen every 10s from one task; reads happen on API request. The critical section is tiny (push_back + pop_front). Mutex simplicity wins over RwLock for this access pattern. Use `std::sync::Mutex` (not tokio::sync::Mutex) because the lock is held for microseconds with no async operations inside.

**Snapshot capture:** Extend the existing background gauge refresh task in main.rs (lines 137-150). After `refresh_gauges()` completes, read counter totals and gauge values from the Prometheus registry, construct a `MetricsSnapshot`, and push to the ring buffer. If buffer exceeds 180 entries, pop_front.

**Reading counter values from prometheus crate:**
```rust
// CounterVec: iterate metric families from registry.gather()
// Or directly: metrics.tasks_submitted_total.with_label_values(&["svc", "http"]).get()
// For total across all labels, use registry.gather() and sum metric values
```

**Throughput computation (D-04):** For the summary endpoint, compute tasks/min by comparing the current counter total to the counter total from 60 seconds ago (6 snapshots back in the ring buffer). Formula: `(current_total - total_60s_ago) / 1.0` gives tasks per minute.

### Backend: Summary Endpoint

The `/v1/admin/metrics/summary` handler combines:
1. Service count from `list_services()` (same as health_handler)
2. Per-service node health from `get_nodes_for_service()` (same as health_handler)
3. Queue depth from current gauge values `metrics.queue_depth.with_label_values(&[svc]).get()`
4. Throughput from ring buffer counter deltas

Derive per-service health using the same logic as `deriveServiceHealth` on frontend: all healthy = green, some healthy = yellow/degraded, none = red/down.

**Response shape (confirmed in CONTEXT.md):**
```json
{
  "service_count": 3,
  "active_nodes": 7,
  "total_queue_depth": 42,
  "throughput": {
    "submitted_per_min": 120.0,
    "completed_per_min": 115.0
  },
  "services": [
    {
      "name": "image-resize",
      "health": "healthy",
      "active_nodes": 3,
      "total_nodes": 3,
      "queue_depth": 12
    }
  ]
}
```

### Backend: History Endpoint

Returns the full ring buffer contents. No query parameters needed (always returns all available history).

**Response shape (confirmed in CONTEXT.md):**
```json
{
  "interval_secs": 10,
  "points": [
    {
      "timestamp": 1711180800,
      "tasks_submitted": 1500.0,
      "tasks_completed": 1480.0,
      "tasks_failed": 5.0,
      "queue_depth": {"image-resize": 12, "ocr": 5},
      "nodes_active": {"image-resize": 3, "ocr": 2}
    }
  ]
}
```

Note: `tasks_submitted`/`tasks_completed`/`tasks_failed` in the history are counter totals, not rates. The frontend computes deltas between consecutive points to derive rates for the throughput chart.

### Frontend: Dashboard Page Structure

```
admin-ui/src/
  routes/_authenticated/
    index.tsx          # Dashboard page (replace EmptyState)
  lib/
    metrics.ts         # Types + TanStack Query hooks (useMetricsSummary, useMetricsHistory)
  components/
    overview-card.tsx  # Reusable card with value, label, trend arrow
    throughput-chart.tsx  # Recharts AreaChart for task throughput
    queue-depth-chart.tsx  # Recharts AreaChart for queue depth
    service-health-list.tsx  # Compact health list
```

### Frontend: Recharts 3 AreaChart Pattern

```tsx
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts'

// Stacked area chart for throughput
<ResponsiveContainer width="100%" height={300}>
  <AreaChart data={chartData}>
    <CartesianGrid strokeDasharray="3 3" />
    <XAxis dataKey="time" />
    <YAxis width="auto" />
    <Tooltip />
    <Area
      type="monotone"
      dataKey="submitted"
      stackId="1"
      stroke="#3b82f6"
      fill="#3b82f6"
      fillOpacity={0.3}
    />
    <Area
      type="monotone"
      dataKey="completed"
      stackId="1"
      stroke="#22c55e"
      fill="#22c55e"
      fillOpacity={0.3}
    />
  </AreaChart>
</ResponsiveContainer>
```

**Recharts 3 notes:**
- `ResponsiveContainer` works correctly with React 19 in Recharts 3 (fixed from 2.x)
- `YAxis width="auto"` is new in 3.x -- auto-sizes to label width
- `CartesianGrid` may need `xAxisId`/`yAxisId` props if using multiple axes
- Accessibility layer is enabled by default in 3.x

### Frontend: Data Transform Pattern

History endpoint returns counter totals. Frontend must compute per-interval deltas:
```typescript
function computeRates(points: HistoryPoint[]): ChartDataPoint[] {
  return points.slice(1).map((point, i) => {
    const prev = points[i]
    const dtSecs = point.timestamp - prev.timestamp
    return {
      time: formatTime(point.timestamp),
      submitted: ((point.tasks_submitted - prev.tasks_submitted) / dtSecs) * 60,
      completed: ((point.tasks_completed - prev.tasks_completed) / dtSecs) * 60,
    }
  })
}
```

### Anti-Patterns to Avoid
- **Do NOT use `tokio::sync::Mutex` for the ring buffer:** The lock is held for microseconds with no async operations inside. `std::sync::Mutex` is correct and avoids unnecessary `.await` at every access.
- **Do NOT recompute health on frontend from raw node data:** The summary endpoint should return a health string per service, pre-computed server-side, matching the existing `deriveServiceHealth` logic.
- **Do NOT poll history and summary separately at different intervals:** Both should use the same `effectiveInterval` from `useAutoRefresh` to avoid visual desync.
- **Do NOT use Recharts 2.x:** React 19 ResponsiveContainer bugs are unfixed in 2.x. Use 3.x.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Responsive charts | Manual SVG resize logic | Recharts ResponsiveContainer | Handles resize observer, debouncing, container detection |
| Time axis formatting | Custom date string formatting | Recharts XAxis tickFormatter | Built-in tick formatting with auto-interval |
| Polling/caching | Manual setInterval + fetch | TanStack Query refetchInterval | Deduplication, cache invalidation, error retry built-in |
| Color-coded health dots | Custom CSS classes | Existing HealthBadge component | Already has green/yellow/red mapping for all health states |
| Card layout | Custom div styling | shadcn/ui Card component | Already used throughout the app |

## Common Pitfalls

### Pitfall 1: Ring Buffer Lock Contention
**What goes wrong:** Using `tokio::sync::Mutex` and holding it across async operations causes unnecessary contention.
**Why it happens:** Developers default to tokio Mutex in async code.
**How to avoid:** Use `std::sync::Mutex`. Clone the VecDeque data out immediately (or collect into Vec) and drop the lock before doing any serialization.
**Warning signs:** Handler response time increases under load.

### Pitfall 2: Counter Delta Going Negative
**What goes wrong:** If the gateway restarts, Prometheus counters reset to 0, causing negative deltas.
**Why it happens:** Ring buffer is also in-memory and resets, but if you compute deltas across a restart boundary from stale data, you get nonsense.
**How to avoid:** Ring buffer resets on restart (per D-03), so this is naturally handled. But if you ever persist history, check for negative deltas and treat them as 0.
**Warning signs:** Negative throughput values in charts.

### Pitfall 3: Recharts ResponsiveContainer Needs Parent Height
**What goes wrong:** Chart renders with 0 height or doesn't appear.
**Why it happens:** ResponsiveContainer measures parent element. If parent has no explicit height, it collapses.
**How to avoid:** Always wrap ResponsiveContainer in a div with explicit height (e.g., `h-[300px]` with Tailwind).
**Warning signs:** Chart area is blank but no errors in console.

### Pitfall 4: Summary Endpoint Duplicating Health Handler Logic
**What goes wrong:** Two endpoints with near-identical Redis queries, diverging over time.
**Why it happens:** Copy-paste from health_handler.
**How to avoid:** Extract shared service health computation into a helper function that both handlers call. The summary endpoint adds throughput and queue depth on top.
**Warning signs:** Health status differs between /health and /metrics/summary.

### Pitfall 5: Stale Prometheus Gauges in Summary
**What goes wrong:** Summary endpoint reads gauge values that haven't been refreshed recently.
**Why it happens:** `refresh_gauges()` runs every 15s. Between refreshes, gauges show stale values.
**How to avoid:** The summary endpoint should call `refresh_gauges()` (or its extracted helper) directly for fresh data. Alternatively, read Redis directly in the summary handler rather than relying on cached gauge values.
**Warning signs:** Queue depth shows 0 when tasks are actually queued.

## Code Examples

### Rust: MetricsSnapshot Struct
```rust
// Source: Project-specific design based on D-02, D-03
use std::collections::HashMap;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct MetricsSnapshot {
    pub timestamp: i64,
    pub tasks_submitted: f64,
    pub tasks_completed: f64,
    pub tasks_failed: f64,
    pub queue_depth: HashMap<String, f64>,
    pub nodes_active: HashMap<String, f64>,
}
```

### Rust: Reading Counter Totals via registry.gather()
```rust
// Source: prometheus crate API (https://docs.rs/prometheus)
// Summing counter values across all label combinations
fn sum_counter(registry: &prometheus::Registry, name: &str) -> f64 {
    let families = registry.gather();
    for family in &families {
        if family.name() == name {
            return family
                .get_metric()
                .iter()
                .map(|m| m.get_counter().get_value())
                .sum();
        }
    }
    0.0
}

// Usage:
let submitted = sum_counter(&state.metrics.registry, "gateway_tasks_submitted_total");
let completed = sum_counter(&state.metrics.registry, "gateway_tasks_completed_total");
```

### TypeScript: Metrics TanStack Query Hook
```typescript
// Source: Follows lib/services.ts pattern exactly
import { useQuery } from '@tanstack/react-query'
import { apiClient } from './api'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'

export function useMetricsSummary() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['metrics', 'summary'],
    queryFn: () => apiClient<MetricsSummaryResponse>('/v1/admin/metrics/summary'),
    refetchInterval: effectiveInterval,
  })
}

export function useMetricsHistory() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['metrics', 'history'],
    queryFn: () => apiClient<MetricsHistoryResponse>('/v1/admin/metrics/history'),
    refetchInterval: effectiveInterval,
  })
}
```

### TypeScript: Overview Card with Trend Arrow
```tsx
// Source: Project-specific, follows shadcn/ui Card pattern from service-card.tsx
import { Card, CardHeader, CardTitle, CardContent } from '@/components/ui/card'
import { type LucideIcon } from 'lucide-react'

interface OverviewCardProps {
  title: string
  value: string | number
  icon: LucideIcon
  trend?: { delta: number; positive: 'up' | 'down' } // 'up' means increase is good
}

function OverviewCard({ title, value, icon: Icon, trend }: OverviewCardProps) {
  return (
    <Card>
      <CardHeader className="flex flex-row items-center justify-between pb-2">
        <CardTitle className="text-sm font-medium text-muted-foreground">
          {title}
        </CardTitle>
        <Icon className="h-4 w-4 text-muted-foreground" />
      </CardHeader>
      <CardContent>
        <div className="text-2xl font-bold">{value}</div>
        {trend && <TrendIndicator {...trend} />}
      </CardContent>
    </Card>
  )
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Recharts 2.x | Recharts 3.x | 2025 | React 19 compatibility fixed, state management rewritten, ResponsiveContainer reliable |
| External Grafana iframe | Custom Recharts dashboard | N/A (project decision) | Avoids auth/CORS/CSP complexity per REQUIREMENTS.md Out of Scope |
| WebSocket real-time | TanStack Query polling | N/A (project decision) | Simpler, sufficient for 10-15s update cadence per REQUIREMENTS.md |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework (Rust) | cargo test (built-in) |
| Framework (Frontend) | None configured -- no vitest/jest in admin-ui |
| Config file | gateway/Cargo.toml (Rust), none for frontend |
| Quick run command | `cd gateway && cargo test --lib` |
| Full suite command | `cd gateway && cargo test` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| DASH-01 | Summary endpoint returns correct card data | unit | `cargo test metrics_ring_buffer` | No -- Wave 0 |
| DASH-01 | Overview cards render with data | manual | Visual inspection in browser | N/A |
| DASH-02 | History endpoint returns ring buffer contents | unit | `cargo test metrics_history` | No -- Wave 0 |
| DASH-02 | Charts render and auto-update | manual | Visual inspection in browser | N/A |
| DASH-03 | Health badges show correct colors | manual | Visual inspection in browser | N/A |
| DASH-03 | Service health derived correctly server-side | unit | `cargo test metrics_summary_health` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cd gateway && cargo test --lib`
- **Per wave merge:** `cd gateway && cargo test`
- **Phase gate:** Full suite green + visual verification of dashboard

### Wave 0 Gaps
- [ ] `gateway/src/metrics_history.rs` -- ring buffer struct + snapshot logic unit tests
- [ ] `gateway/src/http/admin.rs` -- summary and history handler tests (may use existing test patterns from admin.rs)
- [ ] No frontend test framework configured -- all frontend validation is manual/visual

## Open Questions

1. **Throughput across service labels**
   - What we know: `tasks_submitted_total` is labeled by `["service", "protocol"]`. Summary needs aggregate throughput across all services.
   - What's unclear: Whether to sum across all labels or provide per-service throughput too.
   - Recommendation: Sum across all labels for the aggregate cards. The history endpoint already captures per-service data for future use.

2. **Service health derivation on backend**
   - What we know: Frontend has `deriveServiceHealth()` in `lib/services.ts`. Backend has `derive_health_state()` per-node in `node_health.rs`.
   - What's unclear: Whether to add service-level health derivation to Rust or replicate the frontend logic.
   - Recommendation: Add `derive_service_health(nodes: &[NodeStatus]) -> &str` to Rust (`node_health.rs`) and use it in both the summary endpoint and the existing health_handler.

3. **Empty state timing**
   - What we know: Dashboard should handle "no services registered" gracefully.
   - What's unclear: Whether ring buffer should still run snapshots when no services exist.
   - Recommendation: Ring buffer snapshots still run (just record zeros). Dashboard shows EmptyState component if `service_count == 0`, hiding charts and health list.

## Sources

### Primary (HIGH confidence)
- `gateway/src/metrics.rs` -- All 8 Prometheus metrics, `refresh_gauges()` function
- `gateway/src/state.rs` -- AppState structure
- `gateway/src/http/admin.rs` lines 478-554 -- Existing metrics_handler and health_handler patterns
- `gateway/src/main.rs` lines 137-150 -- Background gauge refresh task (10s snapshot insertion point)
- `admin-ui/src/lib/services.ts` -- TanStack Query hook pattern with useAutoRefresh
- `admin-ui/src/components/health-badge.tsx` -- Color-coded health dot component
- `admin-ui/src/components/service-card.tsx` -- Card + HealthBadge usage pattern
- [recharts npm](https://www.npmjs.com/package/recharts) -- version 3.8.0 confirmed
- [Recharts 3.0 migration guide](https://github.com/recharts/recharts/wiki/3.0-migration-guide) -- API changes from 2.x
- [prometheus docs.rs](https://docs.rs/prometheus) -- Counter.get() and registry.gather() API

### Secondary (MEDIUM confidence)
- [Recharts API docs](https://recharts.github.io/en-US/api/) -- AreaChart, ResponsiveContainer props
- [React 19 ResponsiveContainer fix](https://github.com/recharts/recharts/issues/4590) -- Confirmed fixed in Recharts 3

### Tertiary (LOW confidence)
- None

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- Recharts 3 version verified via npm, all other deps already in project
- Architecture: HIGH -- Ring buffer pattern is straightforward, all integration points identified in existing code
- Pitfalls: HIGH -- React 19 + Recharts issues well-documented, Prometheus API patterns verified

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (stable domain, 30-day validity)
