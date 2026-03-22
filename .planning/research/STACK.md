# Stack Research: Admin Web UI Frontend

**Domain:** Admin dashboard frontend for Rust API gateway
**Researched:** 2026-03-22
**Confidence:** HIGH

> This document covers ONLY the new frontend stack additions for the v1.1 Admin Web UI.
> The existing Rust backend stack (Axum, Tonic, Redis, etc.) is documented in CLAUDE.md and is not re-researched here.

## Recommended Stack

### Core Technologies (User-Selected, Versions Verified)

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **Vite** | 8.x | Build tool | Latest stable. Ships Rolldown (Rust-based bundler) for 10-30x faster builds. User-selected. |
| **React** | 19.x | UI framework | User-selected. Required by shadcn/ui. |
| **TailwindCSS** | 4.2.x | Utility CSS | v4 uses CSS-first config (no tailwind.config.js needed). 100x faster incremental builds. User-selected. |
| **shadcn/ui** | CLI v4.1.x | Component library | Copy-paste components (not a runtime dependency). Install via `npx shadcn@latest init`. Built on Radix UI primitives. User-selected. |
| **TanStack Router** | 1.168.x | Client routing | Fully type-safe routing with file-based route generation via Vite plugin. User-selected. |
| **TanStack Query** | 5.94.x | Server state management | Caching, refetching, polling interval -- ideal for admin dashboard that polls gateway status. User-selected. |

### Metrics Visualization

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **Recharts** | 3.8.x | Charts and graphs | shadcn/ui's built-in chart components ARE Recharts wrappers. Run `npx shadcn@latest add chart` to install chart primitives (AreaChart, BarChart, LineChart). 53 pre-built variants. Auto-themes with dark mode via CSS variables. No separate charting library needed -- Recharts comes in through shadcn. |

### Supporting Libraries

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| **zustand** | 5.x | Client state (auth) | Lightweight store for auth state (isAuthenticated, adminToken). 1KB gzipped. No boilerplate, no providers. Persist middleware backs to sessionStorage. |
| **date-fns** | 4.x | Date formatting | Format task timestamps, node last-seen times. Tree-shakable, functional API. |
| **sonner** | latest | Toast notifications | shadcn/ui's recommended toast component. `npx shadcn@latest add sonner`. Use for mutation feedback (service created, task cancelled, key revoked). |
| **lucide-react** | latest | Icons | shadcn/ui uses Lucide. Installed during `shadcn init`. Tree-shakable icon set. |
| **class-variance-authority** | latest | Variant styling | Dependency of shadcn/ui. Installed during init. |
| **clsx** + **tailwind-merge** | latest | Class merging | Powers shadcn's `cn()` utility. Installed during init. |

### Development Dependencies

| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| **TypeScript** | 5.x | Type safety | Strict mode. TanStack Router provides full type inference for routes, params, search. |
| **@vitejs/plugin-react** | latest | Vite React plugin | Fast Refresh HMR for React in Vite 8. |
| **@tanstack/router-plugin** | latest | Route codegen | Vite plugin for file-based route generation. Auto-generates route tree from `src/routes/`. |
| **@tanstack/react-router-devtools** | 1.168.x | Router debugging | Dev only. Inspect route matches, params, search state. |
| **@tanstack/react-query-devtools** | 5.94.x | Query debugging | Dev only. Inspect cache, refetch states, mutation status. |
| **@tanstack/eslint-plugin-query** | latest | Query linting | Catches common TanStack Query mistakes (missing query keys, etc). |

## Installation

```bash
# 1. Create project
npm create vite@latest admin-ui -- --template react-ts
cd admin-ui

# 2. Core routing + data fetching
npm install @tanstack/react-router @tanstack/react-query

# 3. Client state (auth only)
npm install zustand

# 4. Date formatting
npm install date-fns

# 5. Dev tools + build plugins
npm install -D @tanstack/react-router-devtools @tanstack/react-query-devtools
npm install -D @tanstack/router-plugin @tanstack/eslint-plugin-query
npm install -D @vitejs/plugin-react

# 6. Initialize shadcn/ui (auto-installs tailwindcss, Recharts, Lucide, Radix, etc.)
npx shadcn@latest init

# 7. Add shadcn components for admin dashboard
npx shadcn@latest add card table button input badge dialog alert
npx shadcn@latest add chart sonner sidebar tabs dropdown-menu separator
npx shadcn@latest add sheet command popover select label switch
```

## Auth Integration Pattern

**No additional auth library needed.** The existing gateway uses a simple Bearer token check via `admin_auth_middleware`:

```
Authorization: Bearer {admin.token}
```

Where `admin.token` is a pre-shared secret from the gateway's TOML config. This is NOT a user/password system -- it's a single admin token.

**Frontend auth flow:**

1. Login page: user enters the admin token
2. Store token in zustand with sessionStorage persistence (cleared on tab close)
3. Create a shared `fetch` wrapper that adds `Authorization: Bearer {token}` to all requests
4. TanStack Query uses this wrapper as its `queryFn` transport
5. On 401 response from any request: clear zustand state, redirect to login route
6. Protected routes use TanStack Router's `beforeLoad` guard to check auth state

**Why this approach:**
- Matches the existing `admin_auth_middleware` exactly (Bearer token in header)
- sessionStorage is appropriate: admin tokens should require re-entry per browser session
- No JWT, no refresh tokens, no httpOnly cookies -- the admin token is static and long-lived
- zustand's persist middleware handles sessionStorage serialization automatically

**Gateway-side changes needed:**
- Add CORS headers via `tower-http::CorsLayer` on admin routes (already a dependency)
- Or use Vite proxy in dev + reverse proxy in prod to avoid CORS entirely (recommended)

## Prometheus Metrics Strategy

**Do NOT add a Prometheus server, Grafana, or prometheus-query-api dependency.** The gateway IS the metrics source, not a Prometheus server.

**Approach: Direct fetch + parse + Recharts**

1. **Fetch `/metrics`** via TanStack Query with `refetchInterval: 10000` (10s polling)
2. **Parse Prometheus text format** client-side with a simple custom parser (~50 lines of TypeScript)
3. **Accumulate time-series** in React state: store last N snapshots (e.g., 60 points at 10s = 10 min history)
4. **Render with Recharts** via shadcn chart components: area charts for rates, bar charts for queue depth, number cards for counters

**Why a custom parser instead of `parse-prometheus-text-format` (npm)?**
The npm package is 7 years unmaintained. The Prometheus text exposition format is simple and well-specified:
```
# HELP metric_name Description
# TYPE metric_name counter
metric_name{label="value"} 42.0
```
A regex-based parser handling comments, metric names, labels, and values is trivial in TypeScript. Avoids a dead dependency.

**Why NOT embedded Grafana or a Prometheus query API client?**
- The gateway exposes raw `/metrics`, not a Prometheus query endpoint (`/api/v1/query`)
- Embedding Grafana requires running a Grafana instance -- massive operational overhead for a simple admin panel
- Direct fetch gives full control over visualization with zero infrastructure dependencies

**Dashboard metrics to display (from existing `/metrics` endpoint):**
- `gateway_tasks_total` -- task submission rate (counter, show as rate/min)
- `gateway_queue_depth` -- per-service queue depth (gauge, show as bar chart)
- `gateway_task_latency_seconds` -- task processing latency (histogram, show as area chart)
- `gateway_active_nodes` -- per-service active node count (gauge, show as number card)
- `gateway_errors_total` -- error rate by type (counter, show as rate chart)

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| **Recharts** (via shadcn) | Chart.js, Victory, Nivo | Never for this project. shadcn chart components ARE Recharts wrappers. Using anything else fights the component library. |
| **zustand** (auth state) | React Context | React Context works for this use case (just one boolean + one string). Use zustand because persist middleware to sessionStorage is built-in and avoids boilerplate. |
| **Custom metrics parser** | parse-prometheus-text-format (npm) | Use npm package only if you want to avoid writing any parser code. It works but is unmaintained. |
| **TanStack Query polling** | WebSocket/SSE live metrics | Only if gateway adds SSE support. Polling every 10s is standard for admin dashboards and matches Prometheus scrape intervals. |
| **sessionStorage** (auth) | httpOnly cookies | httpOnly cookies only if you later add a proper user/password auth with server-issued sessions. For a static admin token, sessionStorage is simpler and appropriate. |
| **date-fns** | dayjs | dayjs if you prefer a moment-like chainable API. date-fns is more tree-shakable. Either works. |
| **Native fetch** | Axios | Axios adds a dependency for what `fetch()` does natively. TanStack Query works with any promise-returning function. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **Embedded Grafana** | Requires running a Grafana server instance. Massive operational overhead for a simple admin panel. | Recharts via shadcn chart components |
| **prometheus-query (npm)** | Requires a running Prometheus server with query API. The gateway exposes raw `/metrics`, not `/api/v1/query`. | Direct fetch of `/metrics` + custom text parser |
| **react-admin** | Full admin framework with its own routing, data providers, auth. Conflicts with TanStack Router/Query and shadcn. | Build with shadcn components + TanStack stack |
| **Redux / Redux Toolkit** | Massive boilerplate for one boolean + one string of client state. TanStack Query handles all server state. | zustand for the tiny amount of client state |
| **Axios** | Unnecessary dependency. `fetch()` is native and TanStack Query is transport-agnostic. | Native `fetch()` with a thin wrapper |
| **localStorage for auth** | Persists across sessions. If someone walks away, the token remains. Admin tokens should expire with the session. | sessionStorage via zustand persist middleware |
| **SWR** | TanStack Query already selected. Query has more features (mutations, optimistic updates, better devtools). | TanStack Query |
| **parse-prometheus-text-format** | Unmaintained for 7 years. The format is simple enough to parse in ~50 lines. | Custom TypeScript parser |
| **moment.js** | Enormous bundle size (300KB+), mutable API, officially in maintenance mode. | date-fns |

## Stack Patterns

### Development: Vite Proxy (Recommended)

Avoids CORS issues entirely during development. Configure Vite's dev server to proxy API calls to the local gateway:

```typescript
// vite.config.ts
import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { TanStackRouterVite } from '@tanstack/router-plugin/vite'

export default defineConfig({
  plugins: [TanStackRouterVite(), react()],
  server: {
    proxy: {
      '/v1': 'http://localhost:8080',
      '/metrics': 'http://localhost:8080',
    }
  }
})
```

### Production Deployment Options

**Option A -- Reverse proxy (recommended):**
Nginx/Caddy serves the static SPA build and proxies `/v1/*` and `/metrics` to the gateway. Single domain, no CORS needed. Standard deployment pattern.

**Option B -- Gateway serves static files:**
Add an Axum static file handler (`tower-http::services::ServeDir`) for the built admin UI. Keeps single-process deployment. Adds ~2-5MB to the Docker image. Requires rebuilding the gateway image when the UI changes.

**Option C -- Separate origin (CDN):**
Deploy admin UI on CDN, add `CorsLayer` to gateway admin routes. Simplest deployment but requires CORS configuration. Use if admin UI and gateway are on different infrastructure.

### Auth Guard Pattern with TanStack Router

```typescript
// src/routes/__root.tsx -- root layout with auth check
// Use beforeLoad to redirect unauthenticated users to /login
// Check zustand store for token presence
// /login route is the only unprotected route
```

### Metrics Polling Pattern with TanStack Query

```typescript
// Poll /metrics every 10 seconds, accumulate history in component state
// useQuery with refetchInterval: 10_000
// On each fetch, parse text -> extract values -> append to history array
// Cap history at 60 entries (10 min at 10s interval)
// Pass history array to Recharts AreaChart for sparklines
```

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| Vite 8.x | @tanstack/router-plugin | TanStack Router added Vite 8 support (rolldownOptions) in March 2026 releases |
| shadcn CLI v4.x | TailwindCSS v4.x | shadcn v4 requires Tailwind v4. Uses CSS-first config, no tailwind.config.js |
| shadcn chart | Recharts 3.x | shadcn chart components work with Recharts 3.8.x |
| TanStack Router 1.168.x | TanStack Query 5.94.x | Designed for integration. Router loaders can prefetch queries. |
| React 19.x | All above | All libraries verified compatible with React 19 |
| TypeScript 5.x | All above | Required for TanStack Router type inference |

## Gateway-Side Changes Required

These are small Axum additions, not frontend stack items, but needed for the admin UI to function:

| Change | Why | Effort |
|--------|-----|--------|
| **CORS layer on admin routes** | Admin UI in dev runs on different port (Vite 5173 vs gateway 8080) | Trivial -- `tower-http::CorsLayer` already a dependency |
| **Task cancellation endpoint** | PROJECT.md requires task cancellation in v1.1. No endpoint exists yet. | New endpoint: `POST /v1/admin/tasks/{task_id}/cancel` |
| **Task listing endpoint** | Need to browse/search tasks in the admin UI. No list endpoint exists. | New endpoint: `GET /v1/admin/tasks` with pagination + filters |
| **Node listing endpoint** | Need to see all nodes across services. Currently only per-service via health. | New endpoint: `GET /v1/admin/nodes` or enhance existing health endpoint |

## Confidence Assessment

| Area | Confidence | Reasoning |
|------|------------|-----------|
| Core stack versions | HIGH | All verified via npm registries and GitHub releases within days of this research |
| Recharts via shadcn | HIGH | Documented integration, shadcn chart = Recharts wrapper, verified |
| Auth pattern | HIGH | Exactly matches existing `admin_auth_middleware` (Bearer token, same header format) |
| Prometheus metrics parsing | MEDIUM | Custom parser approach is sound and the format is well-specified, but untested at scale. The parser itself is trivial; the question is whether 10s polling + client-side accumulation provides enough dashboard resolution |
| Production deployment | MEDIUM | Multiple viable patterns; choice depends on deployment environment preferences |
| Gateway-side changes | HIGH | Identified from actual code review of `main.rs` and `admin.rs` |

## Sources

- [shadcn/ui Chart docs](https://ui.shadcn.com/docs/components/radix/chart) -- Recharts integration verified
- [shadcn CLI v4 changelog](https://ui.shadcn.com/docs/changelog/2026-03-cli-v4) -- CLI v4.1.x confirmed
- [TanStack Router releases](https://github.com/TanStack/router/releases) -- v1.168.x confirmed (March 2026)
- [@tanstack/react-query npm](https://www.npmjs.com/package/@tanstack/react-query) -- v5.94.x confirmed
- [Recharts npm](https://www.npmjs.com/package/recharts) -- v3.8.0 confirmed
- [Vite releases](https://vite.dev/releases) -- v8.0.1 confirmed
- [TailwindCSS releases](https://github.com/tailwindlabs/tailwindcss/releases) -- v4.2.2 confirmed
- [Prometheus text exposition format](https://prometheus.io/docs/instrumenting/clientlibs/) -- format specification
- [parse-prometheus-text-format npm](https://www.npmjs.com/package/parse-prometheus-text-format) -- evaluated and rejected (7yr unmaintained)
- [shadcn/ui dashboard guide (2026)](https://designrevision.com/blog/shadcn-dashboard-tutorial) -- dashboard patterns
- [TanStack ecosystem guide 2026](https://www.codewithseb.com/blog/tanstack-ecosystem-complete-guide-2026) -- Router + Query integration patterns

---
*Stack research for: xgent-ai-gateway Admin Web UI (v1.1)*
*Researched: 2026-03-22*
