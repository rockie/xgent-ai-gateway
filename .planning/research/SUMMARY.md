# Project Research Summary

**Project:** xgent-ai-gateway Admin Web UI (v1.1)
**Domain:** Infrastructure gateway admin dashboard — React SPA integrated with existing Rust/Axum gateway
**Researched:** 2026-03-22
**Confidence:** HIGH

## Executive Summary

The xgent-ai-gateway already has a working Rust/Axum backend with a mature admin API surface. The v1.1 milestone adds a React-based admin web UI on top of that existing backend. This is a well-understood problem domain — infrastructure admin dashboards analogous to Celery Flower or Bull Board — providing task queue management, service configuration, node health monitoring, and operational metrics visualization. The recommended approach is a Vite + React 19 + TanStack Router/Query + shadcn/ui SPA, served from the gateway process itself in production to eliminate CORS complexity and maintain the single-binary deployment constraint.

The backend requires 7 new endpoints before the full UI can be built: an auth/login flow, list endpoints for API keys and node tokens, a paginated task list, a task cancellation endpoint, and a JSON metrics summary endpoint. Three of these are MEDIUM complexity (login, task list, task cancel) due to Redis stream pagination semantics and state machine edge cases. The remaining four are LOW complexity. The frontend can scaffold and implement pages backed by existing endpoints in parallel with backend work, but the auth endpoints (login + /me) are the critical-path dependency that must exist before any authenticated UI page can be end-to-end tested.

The primary risks are architectural and must be resolved in Phase 1 before feature work begins: CORS strategy (Vite proxy in dev, same-origin serving in production), SPA fallback routing (ServeDir with not_found_service), auth token storage (HttpOnly cookies, not localStorage), and TanStack Query key structure (centralized factory from day one). Deferring these to "fix later" carries MEDIUM-to-HIGH recovery costs. All other pitfalls are preventable with standard patterns that are well-documented.

## Key Findings

### Recommended Stack

The frontend stack is entirely additive to the existing Rust codebase — no Rust dependencies change. The user pre-selected the core technologies (Vite, React, TailwindCSS, shadcn/ui, TanStack Router, TanStack Query), and research confirmed all are at current stable versions with verified compatibility. The key insight is that Recharts is not a separate dependency: shadcn/ui chart components are Recharts wrappers, installed via `npx shadcn@latest add chart`. Prometheus metrics should be served as a JSON endpoint from the gateway (reading directly from `state.metrics` handles) rather than parsed from raw `/metrics` text in the browser — this keeps auth requirements simple and avoids exposing operational data unauthenticated.

See `.planning/research/STACK.md` for full version tables, installation commands, and alternatives considered.

**Core technologies:**
- **Vite 8.x**: Build tool with Rolldown (Rust-based bundler) for fast builds; dev server proxy eliminates CORS during development
- **React 19.x + TypeScript 5.x**: UI framework with full type inference enabled by TanStack Router
- **TailwindCSS 4.2.x + shadcn/ui CLI v4.1.x**: CSS-first config (no tailwind.config.js); copy-paste components built on Radix UI primitives
- **TanStack Router 1.168.x**: Fully type-safe file-based routing with `beforeLoad` auth guards and `_authenticated` layout route convention
- **TanStack Query 5.94.x**: Server state with automatic caching, background refetch, and `refetchInterval` polling for live dashboard data
- **Recharts 3.8.x** (via shadcn chart): Time-series and bar charts auto-themed with dark mode via CSS variables — no separate charting library needed
- **zustand 5.x**: 1KB client state store for auth token with sessionStorage persistence via built-in middleware

### Expected Features

The existing backend already covers services CRUD, node health, API key and node token create/revoke, and Prometheus metrics. The UI gaps driving backend work are task listing with pagination, task cancellation, key/token listing, and an admin auth flow. See `.planning/research/FEATURES.md` for full competitor analysis (Celery Flower, Bull Board, Temporal UI) and backend work itemization.

**Must have (table stakes) — v1.1 launch:**
- Admin login page — all admin functionality gated behind auth
- Dashboard overview — service count, active nodes, aggregate queue depth, task throughput counters
- Service list + detail pages — CRUD for services, config values, connected nodes with health badges
- Task list page — paginated, filterable by service and status (highest-traffic page in comparable tools)
- Task detail + cancellation — view metadata, assigned node, result; cancel pending/running tasks with confirmation dialog
- API key management — list (masked hash), create (show-once with copy), revoke
- Node token management — list (masked hash + label), create (show-once), revoke
- Loading/error/empty states and toast notifications — non-negotiable for usable infrastructure tooling

**Should have (competitive) — v1.1.x patches after validation:**
- Live metrics charts — Recharts area/bar/line charts polling `/v1/admin/metrics/summary` every 10-15s
- Dark mode — shadcn/ui native CSS toggle, persisted in localStorage
- Service config editing — inline edit without delete/recreate cycle (requires `PATCH /v1/admin/services/{name}`)
- Auto-refresh with pause toggle — TanStack Query `refetchInterval` configuration
- Copy-to-clipboard — for API keys, node tokens, task IDs
- Service health badges — color-coded (green/yellow/red) on dashboard service cards

**Defer (v2+):**
- Keyboard shortcuts (`g d`, `g s`, `g t` navigation)
- Task latency percentiles (p50/p95/p99 from histogram metrics)
- Node health timeline (needs backend history tracking)
- Bulk task cancellation
- Exportable Grafana dashboard JSON

**Anti-features (do not build):**
- Embedded Grafana iframe — auth complexity, CORS/CSP issues, massive operational overhead for a simple panel
- Log viewer — high-volume streaming data, explicitly out of scope in PROJECT.md
- Real-time WebSocket updates — explicitly descoped in PROJECT.md; polling is sufficient for admin dashboards
- Role-based access control — single-admin gateway, no value at current scale

### Architecture Approach

The recommended production architecture serves the built SPA from the gateway process itself using `tower_http::services::ServeDir` with `not_found_service` fallback to `index.html`. This eliminates CORS entirely (same origin), requires no separate frontend process or container, and maintains the single Docker image constraint. Development uses a Vite dev server proxy to forward `/v1/*` and `/metrics` to the gateway — the browser sees same-origin requests, so no CORS configuration is needed in dev either.

Auth uses the existing static Bearer token pattern for v1.1: the login endpoint validates credentials against the config file and returns the configured admin token. No JWT, no Redis sessions, no new state — this matches `admin_auth_middleware` exactly. For metrics, a `GET /v1/admin/metrics/summary` JSON endpoint reads directly from `state.metrics` (`Arc<Metrics>` Prometheus handles) rather than having the browser parse Prometheus text format. Keep `/metrics` for external scrapers.

See `.planning/research/ARCHITECTURE.md` for the full system diagram, recommended project structure (`admin-ui/` at repo root), code patterns for all major components, and the Docker multi-stage build definition.

**Major components:**
1. **Vite Dev Proxy** — forwards `/v1/*` and `/metrics` to gateway during development; eliminates CORS in dev
2. **Axum ServeDir with fallback** — serves built SPA in production; returns `index.html` for all unmatched paths under `/admin/*`
3. **Admin Auth Middleware** (existing) — validates `Authorization: Bearer` on all `/v1/admin/*` routes
4. **API Client (`api/client.ts`)** — thin fetch wrapper that injects auth token; all TanStack Query hooks use this, never raw fetch
5. **TanStack Router file-based routes** — `_authenticated` layout route enforces auth guard across all protected pages without repetition
6. **TanStack Query hooks** — one file per resource domain (`services.ts`, `tasks.ts`, `metrics.ts`, etc.) with centralized query key factory
7. **Metrics JSON endpoint** (`/v1/admin/metrics/summary`) — reads from `state.metrics` handles, returns dashboard-friendly JSON with admin auth

### Critical Pitfalls

See `.planning/research/PITFALLS.md` for full details, warning signs, recovery costs, and a phase-to-pitfall mapping table.

1. **CORS misconfiguration** — Use Vite proxy in dev (eliminates CORS entirely); serve SPA from gateway in production (same origin, no CORS headers needed). Never use `CorsLayer::permissive()` in production. Recovery cost is LOW but the failure mode (admin API open to cross-origin attacks) is HIGH severity.

2. **Auth token in localStorage** — Store the session token in an HttpOnly, Secure, SameSite=Strict cookie, not localStorage. localStorage is accessible to any JavaScript on the page (XSS/supply chain attack vector). Recovery requires backend changes plus forced re-login of all admin users — MEDIUM recovery cost, best avoided from the start.

3. **SPA fallback routing missing** — Configure `ServeDir::not_found_service(ServeFile::new("dist/index.html"))`. Without this, any direct URL access or page refresh returns Axum's 404 instead of the React app. Mount API routes before the SPA fallback to preserve proper API 404 responses. Recovery cost is LOW but it breaks basic browser behavior immediately.

4. **TanStack Query cache staleness from scattered keys** — Define a centralized query key factory module on day one. Use hierarchical keys so `invalidateQueries({ queryKey: ['services'] })` invalidates both list and detail queries. Scattered inline string literals cause invisible typo-driven cache bugs that multiply as the UI grows. Recovery cost is MEDIUM — touching every query and mutation in the codebase.

5. **Prometheus metrics exposed without auth** — Do not fetch `/metrics` from the browser. The `/metrics` endpoint is conventionally unauthenticated (Prometheus scraping model). Create `/v1/admin/metrics/summary` with admin auth that returns pre-processed JSON. Recovery cost is MEDIUM (new backend endpoints plus frontend component rework).

## Implications for Roadmap

The natural build order is driven by two constraints: (1) backend endpoints must exist before the frontend can test against them end-to-end, and (2) foundational architectural decisions (CORS, auth storage, SPA routing, query key structure) must be locked in before feature work begins or they become expensive to retrofit across the entire codebase.

### Phase 1: Project Setup and Architectural Foundation

**Rationale:** Several architectural choices are zero-cost to get right upfront but MEDIUM-to-HIGH cost to fix later. CORS strategy, SPA fallback routing, auth token storage model, and TanStack Query key structure must all be established before writing the first feature page. This phase de-risks the entire project.

**Delivers:** Working dev environment (Vite proxy configured, `admin-ui/` scaffolded), TanStack Router layout with `_authenticated` route, centralized query key factory module, API client wrapper with auth injection, Axum SPA static file serving with fallback, and a shell app that routes correctly and loads (with placeholder content). Confirms the production build path works locally.

**Addresses:** App routing structure, login page shell, dev/prod URL strategy
**Avoids:** CORS misconfiguration (Pitfall 1), SPA fallback routing broken (Pitfall 3), TanStack Query cache staleness (Pitfall 4), Vite dev vs production URL mismatch (Pitfall 5)

### Phase 2: Backend Auth and New Endpoints

**Rationale:** The frontend cannot complete end-to-end testing of any authenticated page without the login endpoint. This is the critical-path backend work. Build all 7 new endpoints together in dependency order so the frontend proceeds against a stable, complete API surface.

**Delivers:** 7 new backend endpoints fully implemented and tested in build order:
- `POST /v1/admin/auth/login` + `GET /v1/admin/auth/me` — auth gate for all UI pages (MEDIUM)
- `GET /v1/admin/api-keys` + `GET /v1/admin/node-tokens` — Redis SCAN list endpoints (LOW)
- `GET /v1/admin/tasks` with pagination/filtering — Redis stream pagination (MEDIUM)
- `POST /v1/admin/tasks/{task_id}/cancel` — state machine transition with edge cases (MEDIUM)
- `GET /v1/admin/metrics/summary` — reads from `Arc<Metrics>` handles, returns JSON (LOW)

**Uses:** Existing redis-rs `MultiplexedConnection`, existing Axum middleware patterns, existing `Arc<Metrics>` Prometheus handles
**Avoids:** Confused auth systems — three auth paths (client, node, admin) tested independently with per-router Tower middleware (Pitfall 7); Prometheus metrics exposure without auth (Pitfall 6); missing pagination (HIGH recovery cost if added later)

### Phase 3: Core UI Feature Pages (P1 Features)

**Rationale:** With the backend API surface complete, all P1 features can be built against real endpoints. Build in the order an admin would use them: login first (required to access anything), dashboard second (first thing seen), then the most operationally critical pages (services and tasks).

**Delivers:** Full v1.1 launch-ready admin UI including:
- Login page with auth flow (token storage per auth model decided in Phase 2)
- Dashboard overview (service count, node count, queue depth, task throughput as numeric cards)
- Service list + detail pages (uses existing `GET /v1/admin/services*` endpoints — no backend changes)
- Task list + detail + cancel (uses new paginated task endpoints from Phase 2)
- API key management (list, create-show-once with copy button, revoke)
- Node token management (list, create-show-once, revoke)
- Loading/error/empty states and toast notifications on all mutations

**Implements:** All shadcn/ui components, TanStack Query hooks per resource domain, TanStack Router protected routes, all P1 features from FEATURES.md
**Avoids:** No loading states (UX pitfall — users click twice thinking nothing happened), optimistic updates without rollback (use loading spinners for destructive actions), no confirmation dialogs on destructive actions

### Phase 4: Production Integration and Docker

**Rationale:** Once the UI is feature-complete, validate the full production deployment path before calling v1.1 done. The multi-stage Docker build is new and must be verified to confirm the gateway binary and SPA assets coexist correctly in the `FROM scratch` final image.

**Delivers:** Working Docker multi-stage build (Node stage + Rust stage, `FROM scratch` final image with binary + `dist/`), `admin.static_dir` config key in `gateway.toml`, deployment documentation, integration test confirming the built SPA serves correctly from the Axum process (not just the Vite dev server).

**Uses:** `tower_http::services::ServeDir`, multi-stage Dockerfile, `cross` for musl static binary if cross-compiling
**Avoids:** Vite dev vs production URL mismatch being caught at release time instead of during development (Pitfall 5)

### Phase 5: Metrics Visualization and P2 Features (v1.1.x)

**Rationale:** After v1.1 launch validation, add the differentiating features that set this UI apart from Celery Flower and Bull Board. Metrics charts are the highest-value P2 item — neither competitor has built-in time-series charting. The remaining P2 features are all low-complexity frontend-only changes.

**Delivers:** Live metrics charts (Recharts area/bar charts via `shadcn chart`, polling `/v1/admin/metrics/summary` every 10-15s with history accumulation in React state), dark mode toggle, service config editing (`PATCH /v1/admin/services/{name}` backend endpoint + frontend form), auto-refresh with pause toggle, copy-to-clipboard, service health color badges on dashboard.

**Addresses:** Metrics charts differentiate vs Celery Flower/Bull Board per competitor research in FEATURES.md
**Avoids:** 1-second polling hammering the admin API (use 10-15s interval; disable polling when browser tab is not visible via `refetchIntervalInBackground: false`)

### Phase Ordering Rationale

- **Phase 1 before Phase 2:** Architectural decisions (CORS, auth storage, SPA routing, query keys) affect how backend endpoints set cookies, how routes are mounted, and how auth middleware is structured. Lock these in before the API contract is finalized.
- **Phase 2 before Phase 3:** Every P1 UI page depends on at least one new backend endpoint. The auth endpoints (login + /me) block all authenticated pages from end-to-end testing.
- **Phase 3 before Phase 4:** Integration testing requires a feature-complete UI to exercise all routes and API calls through the production serving path, not just the Vite dev server.
- **Phase 4 before Phase 5:** P2 features should be added to a validated production baseline. Adding metrics charts to an untested deployment creates a larger blast radius for debugging.
- **Pitfall alignment:** Every critical pitfall is addressed in Phases 1-2, preventing expensive retrofits during feature work in Phase 3.

### Research Flags

Phases likely needing deeper research during planning:

- **Phase 2 (Redis stream pagination for task list):** `GET /v1/admin/tasks` requires cursor-based pagination over Redis data with service and status filtering. The Redis XRANGE/SCAN cursor semantics for paginating stream entries with filter predicates are non-obvious. Recommend a focused implementation spike before writing this endpoint to validate the approach against the actual Redis data structures used for tasks.
- **Phase 2 (Task cancellation state machine):** Edge cases need explicit definition before coding — what happens when cancel is called on a task that is already completing? What if the node reports completion at the same time the admin cancels? Idempotency requirements need to be spelled out.
- **Phase 2 (Auth cookie vs Bearer token decision):** The existing `admin_auth_middleware` validates Bearer tokens. Switching to HttpOnly cookies means the middleware must also validate cookies, or the login endpoint must return both (Bearer for API scripts, cookie for browser). This design choice has cascading effects on frontend fetch configuration (`credentials: 'include'`) and CSRF protection. Resolve at the start of Phase 2, not mid-implementation.

Phases with standard patterns (skip research-phase):

- **Phase 1 (Vite + React + TanStack + shadcn scaffolding):** All versions verified and compatible; setup is well-documented with official guides.
- **Phase 3 (UI feature pages):** Standard CRUD UI patterns against a known API surface using established TanStack Query + shadcn/ui patterns.
- **Phase 4 (Docker multi-stage build):** Pattern is fully documented in ARCHITECTURE.md and is standard for Rust + Node projects.
- **Phase 5 (Recharts via shadcn chart components):** Verified integration; shadcn chart = Recharts wrappers, documented and widely used.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified via npm registries and GitHub releases as of 2026-03-22. Vite 8 + TanStack Router 1.168 compatibility with Rolldown confirmed. shadcn CLI v4 + Tailwind v4 pairing verified. React 19 compatibility with all libraries confirmed. |
| Features | HIGH | Derived from actual gateway codebase review (`admin.rs`, `metrics.rs`, `main.rs`) plus competitor feature analysis (Celery Flower, Bull Board, Temporal UI). Backend gaps are precise because the existing API surface is fully known. |
| Architecture | HIGH | Integration patterns (Vite proxy, Axum ServeDir fallback, TanStack Query, auth middleware structure) are all well-documented with code references. Docker multi-stage build follows established Rust + Node pattern. |
| Pitfalls | HIGH | Sourced from official documentation, GitHub issue threads, and security literature. Recovery costs assessed against known codebase structure. Phase mapping is explicit. |

**Overall confidence:** HIGH

### Gaps to Address

- **Auth cookie vs Bearer token decision:** The research recommends HttpOnly cookies over localStorage for security, but the existing `admin_auth_middleware` validates Bearer tokens. Supporting both requires middleware changes. Alternatively, the login endpoint returns a Bearer token that the frontend keeps in memory (lost on page refresh, but the cookie can silently re-establish the session). This must be resolved at the start of Phase 2 — it affects frontend fetch configuration, CSRF protection requirements, and whether any backend session state is needed.

- **Redis SCAN performance for key listing:** `GET /v1/admin/api-keys` and `GET /v1/admin/node-tokens` likely use Redis SCAN to enumerate keys. SCAN is O(N) over the full keyspace. If Redis holds many keys from task data, this may be slow. Consider whether API key and node token hashes are tracked in a dedicated Redis Set (enabling O(1) listing) or must be discovered via SCAN. Confirm the data structure decision at the start of Phase 2.

- **Metrics polling resolution vs dashboard utility:** The research recommends a 10-15s polling interval for `/metrics/summary`. For dashboards showing task throughput rates during high-traffic periods, this may be too coarse to show meaningful trends. Validate the interval against real traffic patterns during Phase 5 and tune accordingly.

- **`admin-ui/` Docker image size:** The `FROM scratch` final image currently contains only the Rust binary and TLS certs (~15-25MB). Adding the SPA `dist/` directory adds ~0.5-2MB (typical Vite-built React SPA). This is acceptable, but confirm the image size constraint is not a hard requirement before Phase 4.

## Sources

### Primary (HIGH confidence)

- Existing gateway codebase (`gateway/src/http/admin.rs`, `gateway/src/metrics.rs`, `gateway/src/main.rs`) — API surface, Prometheus metric registry, middleware structure
- [Vite 8.0.1 release notes](https://vite.dev/releases) — version and Rolldown bundler status confirmed
- [TanStack Router releases](https://github.com/TanStack/router/releases) — v1.168.x confirmed, Vite 8 compatibility verified
- [@tanstack/react-query npm](https://www.npmjs.com/package/@tanstack/react-query) — v5.94.x confirmed
- [shadcn CLI v4 changelog](https://ui.shadcn.com/docs/changelog/2026-03-cli-v4) — v4.1.x, Tailwind v4 requirement confirmed
- [shadcn/ui Chart docs](https://ui.shadcn.com/docs/components/radix/chart) — Recharts 3.x integration verified
- [Recharts npm](https://www.npmjs.com/package/recharts) — v3.8.0 confirmed
- [TailwindCSS releases](https://github.com/tailwindlabs/tailwindcss/releases) — v4.2.2 CSS-first config confirmed
- [Axum SPA fallback discussion](https://github.com/tokio-rs/axum/discussions/2486) — ServeDir with not_found_service pattern
- [Vite server proxy docs](https://vite.dev/config/server-options#server-proxy) — dev proxy configuration reference
- [tower-http CORS docs](https://docs.rs/tower-http/latest/tower_http/cors/struct.CorsLayer.html) — CorsLayer configuration

### Secondary (MEDIUM confidence)

- [JWT storage security analysis](https://dev.to/cotter/localstorage-vs-cookies-all-you-need-to-know-about-storing-jwt-tokens-securely-in-the-front-end-15id) — localStorage vs HttpOnly cookie security tradeoffs
- [TanStack Query cache invalidation patterns](https://www.buncolak.com/posts/avoiding-common-mistakes-with-tanstack-query-part-1/) — query key factory pattern and invalidation strategies
- [Celery Flower](https://github.com/mher/flower) — competitor feature reference for task list, worker monitoring, cancel
- [Prometheus security model](https://prometheus.io/docs/operating/security/) — metrics endpoint conventional auth model
- [Fullstack Rust + React + Vite integration](https://dev.to/alexeagleson/how-to-set-up-a-fullstack-rust-project-with-axum-react-vite-and-shared-types-429e) — development integration patterns

### Tertiary (LOW confidence / needs validation)

- Redis SCAN performance at scale for key listing — O(N) behavior assumed; validate against actual Redis keyspace size in production
- 10-15s metrics polling resolution — assumed sufficient for admin dashboards; validate against actual traffic patterns during Phase 5 work

---
*Research completed: 2026-03-22*
*Ready for roadmap: yes*
