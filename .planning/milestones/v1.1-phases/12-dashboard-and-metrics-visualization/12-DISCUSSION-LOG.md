# Phase 12: Dashboard and Metrics Visualization - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-23
**Phase:** 12-dashboard-and-metrics-visualization
**Areas discussed:** Metrics backend API, Chart library & visuals, Overview cards content, Health badges & layout

---

## Metrics Backend API

### Time-series data source

| Option | Description | Selected |
|--------|-------------|----------|
| Server-side ring buffer | New GET /v1/admin/metrics/history endpoint. Gateway stores last 30min of snapshots in memory (every 10s = 180 points). Resets on restart. | ✓ |
| Redis-backed history | Store metric snapshots in Redis sorted sets. Survives restarts but adds storage overhead. | |
| Frontend-only accumulation | SPA accumulates from polling. No history endpoint. Lost on page refresh. | |

**User's choice:** Server-side ring buffer
**Notes:** None

### Summary endpoint

| Option | Description | Selected |
|--------|-------------|----------|
| Combined endpoint | Single GET /v1/admin/metrics/summary returns all card data + per-service health. One fetch for all 4 cards. | ✓ |
| Reuse existing endpoints | No new endpoint. Dashboard fetches /v1/admin/services + per-service detail. More API calls. | |

**User's choice:** Combined endpoint
**Notes:** None

### Ring buffer retention

| Option | Description | Selected |
|--------|-------------|----------|
| 30 minutes | 180 data points at 10s intervals. ~50KB memory. | ✓ |
| 1 hour | 360 data points. ~100KB memory. | |
| 15 minutes | 90 data points. ~25KB memory. | |

**User's choice:** 30 minutes
**Notes:** None

---

## Chart Library & Visuals

### Charting library

| Option | Description | Selected |
|--------|-------------|----------|
| Recharts | React-native, built on D3. Most popular React charting lib. | ✓ |
| shadcn/ui Charts | Wrapper around Recharts with design tokens. | |
| uPlot | Ultra-fast canvas-based. Less React-native. | |

**User's choice:** Recharts
**Notes:** None

### Chart types

| Option | Description | Selected |
|--------|-------------|----------|
| Area charts | Filled area under line. Throughput: stacked submitted/completed. Queue depth: area per service. | ✓ |
| Line charts | Clean lines without fill. Better for precise value comparison. | |
| You decide | Let Claude pick during implementation. | |

**User's choice:** Area charts
**Notes:** None

---

## Overview Cards Content

### Throughput display

| Option | Description | Selected |
|--------|-------------|----------|
| Tasks/minute rate | Current rate like '45.2 tasks/min' from counter deltas. | ✓ |
| Submitted/completed counts | Raw totals. Numbers only go up. | |
| Both rate and totals | Primary: rate. Secondary: total counts. | |

**User's choice:** Tasks/minute rate
**Notes:** None

### Trend indicator

| Option | Description | Selected |
|--------|-------------|----------|
| Delta arrow | ▲/▼ with change vs 5 minutes ago. Green positive, red negative. | ✓ |
| No trend indicator | Just current value. Trends visible in charts. | |
| Mini sparkline | Tiny inline chart per card. More visual but complex. | |

**User's choice:** Delta arrow
**Notes:** None

---

## Health Badges & Layout

### Service health display

| Option | Description | Selected |
|--------|-------------|----------|
| Compact service list | Below charts, color dot + name + node count. Clickable to /services/$name. | ✓ |
| Summary only | Single aggregate badge: '2 healthy, 1 degraded'. | |
| Mini service cards | Small card per service with health, queue depth, node count. | |

**User's choice:** Compact service list
**Notes:** None

### Dashboard page layout

| Option | Description | Selected |
|--------|-------------|----------|
| Cards → Charts → Health | Top: 4 cards. Middle: 2 charts side-by-side. Bottom: service health list. | ✓ |
| Cards + Charts only | Cards on top, charts below. No health section on dashboard. | |
| Two-column layout | Left: cards + health. Right: charts stacked. | |

**User's choice:** Cards → Charts → Health
**Notes:** None

---

## Claude's Discretion

- Ring buffer implementation details
- Exact Recharts styling
- Overview card icon choices
- Loading skeleton design
- Empty state content
- Error handling patterns
- Chart responsive behavior
- Delta arrow formatting details
