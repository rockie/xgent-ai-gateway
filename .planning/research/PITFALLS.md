# Pitfalls Research

**Domain:** Adding React admin UI to existing Rust/Axum gateway
**Researched:** 2026-03-22
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: CORS Misconfiguration Between Axum API and React Dev Server

**What goes wrong:**
During development, the React dev server runs on `localhost:5173` and the Axum gateway on a different port. Every API request from the browser is cross-origin. Developers add `CorsLayer::permissive()` to get it working, ship it to production, and now the admin API is open to cross-origin attacks from any website. Alternatively, they configure CORS but forget to allow `Authorization` in `allowed_headers`, so auth requests fail with opaque browser errors. Or they set `allow_credentials(true)` with `allow_origin(Any)`, which is forbidden by the CORS spec -- the browser silently rejects every response.

**Why it happens:**
CORS errors in the browser console are cryptic. The instinct is to make them go away with the most permissive settings. The `tower-http` CorsLayer API makes it easy to set `.allow_origin(Any)` and move on. The `allow_credentials + wildcard origin` incompatibility is a spec detail that does not produce a compile error or clear runtime message. Developers also forget that preflight OPTIONS requests must be handled -- Axum does not handle them by default without the CORS layer.

**How to avoid:**
1. In development: use Vite's proxy (`server.proxy` in `vite.config.ts`) to forward `/api/*` requests to the gateway. This eliminates CORS entirely during development because the browser sees same-origin requests.
2. In production: configure `CorsLayer` with explicit `allow_origin` matching only the admin UI's actual origin. Never use `Any` with credentials. Explicitly list `Authorization`, `Content-Type` in `allowed_headers`. Add the CORS layer only to admin routes, not to the existing client API routes (which are called server-to-server and do not need CORS).
3. Use Axum's route nesting to apply CORS middleware only to the admin router, keeping the existing client/node endpoints unaffected.

**Warning signs:**
- `CorsLayer::permissive()` or `allow_origin(Any)` in production code
- CORS middleware applied globally instead of scoped to admin routes
- Preflight (OPTIONS) requests returning 404 or 405
- Auth headers being stripped in cross-origin responses

**Phase to address:**
Phase 1 (Project Setup / Dev Environment). Vite proxy must be configured before any API integration work begins.

---

### Pitfall 2: Auth Token Storage in localStorage Exposes Admin Credentials to XSS

**What goes wrong:**
The admin login returns a JWT or session token. The developer stores it in `localStorage` because it is the simplest approach and works across page refreshes. A single XSS vulnerability anywhere in the app (or in any npm dependency) allows an attacker to exfiltrate the admin token with `localStorage.getItem('token')` and gain full admin access to the gateway -- registering/deleting services, viewing task payloads, cancelling tasks.

**Why it happens:**
localStorage is the most commonly taught approach in React tutorials. It "just works" -- survives page refreshes, easy to attach to fetch headers. The risk feels abstract because "we don't have XSS." But admin panels are high-value targets, and the npm supply chain is a real XSS vector (compromised dependencies injecting scripts).

**How to avoid:**
Use HttpOnly, Secure, SameSite=Strict cookies for the session token. The Axum backend sets the cookie on login; the browser sends it automatically on every request. The token is never accessible to JavaScript. Implement CSRF protection (SameSite=Strict handles most cases; add a CSRF token header for mutation requests as defense-in-depth). On the React side, use `credentials: 'include'` in fetch/TanStack Query config. If JWTs are required (e.g., for stateless auth), store the short-lived access token in memory (React context/state) and the refresh token in an HttpOnly cookie. Accept that the access token is lost on page refresh -- the refresh token cookie will re-establish the session silently.

**Warning signs:**
- `localStorage.setItem('token', ...)` anywhere in the codebase
- `Authorization: Bearer` header constructed from stored token in frontend code
- No `HttpOnly` flag on auth cookies
- No SameSite attribute on cookies

**Phase to address:**
Phase 2 (Admin Auth). Must be correct from the start -- migrating token storage after users exist requires forced re-login and is disruptive.

---

### Pitfall 3: SPA Client-Side Routing Breaks on Page Refresh / Direct URL Access

**What goes wrong:**
TanStack Router handles routes like `/admin/services/123` client-side. The user refreshes the page or shares a direct URL. The browser requests `/admin/services/123` from the Axum server. Axum has no route for this path and returns 404. The admin UI is inaccessible on any URL except the root.

**Why it happens:**
SPAs rely on the server returning `index.html` for all frontend routes so the client-side router can take over. Axum's `ServeDir` serves static files by exact path match. Without a fallback, any path that does not match a physical file returns 404.

**How to avoid:**
Configure Axum's `ServeDir` with a `not_found_service` that falls back to `index.html`:
```rust
use tower_http::services::{ServeDir, ServeFile};

let spa = ServeDir::new("dist")
    .not_found_service(ServeFile::new("dist/index.html"));
```
Mount this as a fallback after all API routes so API 404s still return proper error responses. Order matters: API routes first, then SPA fallback. If the admin UI is served under a sub-path (e.g., `/admin/`), configure TanStack Router's `basePath` to match and scope the fallback accordingly. For production Docker builds, ensure the `dist/` directory is copied into the container image.

**Warning signs:**
- Direct URL navigation returns a blank page or Axum's default 404
- Browser shows raw JSON error instead of the React app on frontend routes
- Refresh on any page except "/" breaks the app
- No `ServeDir` or equivalent static file serving in Axum config

**Phase to address:**
Phase 1 (Project Setup). SPA serving must work before any route is built, or every demo and test cycle is broken.

---

### Pitfall 4: TanStack Query Cache Invalidation Mismatches Cause Stale Admin UI

**What goes wrong:**
An admin registers a new service via a mutation. The service list page still shows the old data because the query cache was not invalidated. Or: an admin cancels a task, but the task detail page still shows "running" because only the list query was invalidated, not the detail query. Over time, cache staleness bugs multiply as the number of related queries grows. Users see phantom services, stale node counts, and tasks in wrong states.

**Why it happens:**
TanStack Query caches aggressively by default (`staleTime: 0` means refetch on mount, but `gcTime: 5min` keeps old data). Developers invalidate the obvious query key after a mutation but miss related queries. Query keys are strings/arrays -- typos and mismatches are invisible until runtime. As the admin UI grows, the graph of "which mutations affect which queries" becomes complex.

**How to avoid:**
1. Establish a query key factory pattern from day one. Define all query keys in a single file:
   ```typescript
   export const queryKeys = {
     services: {
       all: ['services'] as const,
       detail: (id: string) => ['services', id] as const,
     },
     tasks: {
       all: ['tasks'] as const,
       byService: (svcId: string) => ['tasks', { service: svcId }] as const,
       detail: (id: string) => ['tasks', id] as const,
     },
     nodes: {
       all: ['nodes'] as const,
       byService: (svcId: string) => ['nodes', { service: svcId }] as const,
     },
   };
   ```
2. Use hierarchical invalidation: `queryClient.invalidateQueries({ queryKey: ['services'] })` invalidates both `services.all` and `services.detail(id)` because TanStack Query matches prefixes.
3. In mutation `onSuccess`, invalidate by prefix rather than exact key.
4. Set `staleTime` to a short but nonzero value (e.g., 30 seconds) for dashboard data that does not need to be real-time. This prevents redundant refetches on rapid navigation.

**Warning signs:**
- Query keys defined as inline string arrays scattered across components
- Mutation `onSuccess` callbacks invalidating a single exact query key
- Users reporting "I just did X but the list still shows the old state"
- No shared query key module/factory

**Phase to address:**
Phase 1 (Project Setup / API Client Layer). The query key factory must exist before the first query is written.

---

### Pitfall 5: Vite Dev Proxy Works But Production Deploy Fails

**What goes wrong:**
During development, Vite proxies `/api/*` to the Axum gateway. Everything works. In production, the React app is built to static files and served by Axum (or a CDN). But the API calls still go to `/api/*` -- if the static files are served from a different origin (CDN) or the Axum routes are not mounted under `/api`, every request 404s. Or the developer hardcodes `http://localhost:3000` as the API base URL and it reaches production.

**Why it happens:**
The Vite proxy masks the difference between development and production URL resolution. In dev, `/api/services` proxies to `localhost:8080/api/services`. In production, that same fetch goes to wherever the static files are served from. If the SPA is on a CDN and the API is on `api.example.com`, the request fails.

**How to avoid:**
1. Use a relative URL for all API calls (e.g., `/api/services`). Never hardcode an absolute URL.
2. In production, serve the React SPA from the same Axum process that serves the API. This makes relative URLs work without CORS. Mount API routes under `/api` and the SPA fallback on everything else.
3. If the SPA must be served separately (CDN), use an environment variable (`VITE_API_BASE_URL`) baked into the build at compile time via Vite's `import.meta.env`. Set this per deployment.
4. In `vite.config.ts`, configure the proxy to match the exact path prefix used in production:
   ```typescript
   server: {
     proxy: {
       '/api': { target: 'http://localhost:8080', changeOrigin: true }
     }
   }
   ```
5. Test the production build locally before deploying: `vite build && cargo run` -- serve the built SPA from the Rust binary.

**Warning signs:**
- Hardcoded `localhost` URLs in API client code
- API calls working in dev but 404ing in production
- No `VITE_API_BASE_URL` or equivalent env-based URL configuration
- No local production build testing step

**Phase to address:**
Phase 1 (Project Setup). The API base URL strategy must be decided before writing the first API call.

---

### Pitfall 6: Prometheus Metrics Exposed to Browser Without Auth

**What goes wrong:**
The existing gateway exposes Prometheus metrics on `/metrics`. The admin dashboard fetches these metrics from the browser to display charts. But `/metrics` has no authentication (standard for Prometheus scraping). Now anyone with the gateway URL can read operational metrics -- queue depths, error rates, node counts, latency distributions -- revealing system internals.

**Why it happens:**
Prometheus metrics endpoints are conventionally unauthenticated because Prometheus itself scrapes them server-to-server. When a browser-based dashboard needs the same data, developers expose the same endpoint to the browser without considering that the security model has changed from server-to-server to client-facing.

**How to avoid:**
Do NOT fetch `/metrics` from the browser. Instead:
1. Create dedicated admin API endpoints that return pre-processed metrics data (e.g., `GET /api/admin/metrics/dashboard`). These endpoints require admin auth and return JSON formatted for the charts.
2. The admin API endpoints read from the same metric registry (Prometheus client library) but format the data as JSON, not Prometheus text format.
3. Alternatively, if a Prometheus server is running, query the Prometheus HTTP API (`/api/v1/query`) from the admin backend and relay results to the frontend. Never have the browser talk directly to Prometheus.
4. Keep the `/metrics` endpoint for Prometheus scraping but ensure it is not accessible from the public internet (bind to internal interface, or require a specific header that Prometheus sends).

**Warning signs:**
- Frontend code fetching `/metrics` directly
- Prometheus text format being parsed in JavaScript
- `/metrics` endpoint accessible without authentication from the public internet
- No dedicated admin metrics API

**Phase to address:**
Phase 3 (Dashboard / Metrics). Must be designed before building the metrics visualization components.

---

### Pitfall 7: Bolting Admin Auth Onto Existing API Key Auth Creates a Confused System

**What goes wrong:**
The gateway already has API key auth for clients and pre-shared token auth for nodes. The developer reuses the API key mechanism for admin login, or creates a third auth mechanism with no unified middleware. The result is three separate auth systems with different token formats, different validation paths, and different error responses. Auth bugs become hard to reason about. Middleware ordering issues cause one auth system to interfere with another.

**Why it happens:**
Expediency. The API key system already works, and adding a `role: admin` field to API keys seems faster than building proper session-based auth. Or the admin auth is built completely independently, duplicating middleware and error handling.

**How to avoid:**
1. Keep admin auth separate from client/node auth -- they serve different purposes and have different security requirements. Admin auth is session-based (human users), client auth is token-based (automated systems).
2. Use Axum's nested routers to isolate middleware stacks:
   ```
   /api/client/*   -> API key auth middleware
   /api/node/*     -> Pre-shared token middleware
   /api/admin/*    -> Session/cookie auth middleware
   /admin/*        -> SPA static files (no auth needed for HTML/JS/CSS)
   ```
3. Each router gets its own Tower auth layer. No shared "super middleware" that tries to handle all three auth types.
4. Admin sessions use HttpOnly cookies with expiration. Admin accounts are configured in the gateway's config file or a dedicated admin table in Redis.
5. Return consistent error responses across all auth types (same JSON shape, different error codes).

**Warning signs:**
- A single auth middleware trying to handle API keys, node tokens, and admin sessions
- Admin login endpoint returning an API key
- Different JSON error formats from different auth paths
- Auth middleware applied globally instead of per-router

**Phase to address:**
Phase 2 (Admin Auth). Design the auth boundary before implementing login.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Store admin JWT in localStorage | Quick to implement, survives refresh | XSS vulnerability exposes full admin access | Never for admin panel -- use HttpOnly cookies |
| `CorsLayer::permissive()` in production | No more CORS errors | Any website can make authenticated requests to admin API | Never in production -- dev only |
| Inline query keys as string literals | Faster initial development | Typo-driven cache bugs, no invalidation guarantees | Never -- query key factory takes 10 minutes to set up |
| Fetch Prometheus `/metrics` from browser | No backend changes needed | Metrics exposed without auth, Prometheus text parsing in JS is fragile | Never -- create admin API endpoints |
| Skip SPA fallback routing | Frontend "works" navigating from root | Every direct URL access or page refresh returns 404 | Never -- breaks basic browser behavior |
| Single Dockerfile for gateway + SPA build | Simpler CI pipeline | Slow builds -- Rust rebuild triggers SPA rebuild and vice versa. Cache invalidation issues | Only for initial prototype. Split to multi-stage build with separate caching within first week |
| Embed SPA assets in Rust binary with `include_dir!` | Single binary deployment | Rust recompile on every CSS change during dev. Binary size bloat. | Acceptable for production builds only, never for dev workflow |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Axum + SPA static files | Mounting SPA fallback before API routes -- all API 404s return index.html | Mount API routes first, then SPA fallback. Use `Router::merge` ordering or nested routers |
| Axum + admin cookies | Forgetting `SameSite` and `Secure` flags on auth cookies | Always set `SameSite=Strict` (or `Lax` if cross-site links needed), `Secure=true`, `HttpOnly=true` |
| Vite + Axum proxy | Setting `changeOrigin: true` but not `rewrite` -- API path prefix doubled | Match the proxy path prefix to the actual backend route prefix, use `rewrite` only if they differ |
| TanStack Router + Vite | Not adding `@tanstack/router-plugin/vite` to Vite plugins -- file-based routing silently broken | Add the plugin and verify `routeTree.gen.ts` is generated. Add the generated file to `.gitignore` if using auto-generation |
| TanStack Query + auth | Not setting `credentials: 'include'` on fetch calls -- HttpOnly cookies not sent | Configure a global `queryFn` or wrap `fetch` with `credentials: 'include'` applied universally |
| shadcn/ui + Tailwind v4 | Using Tailwind v3 class syntax with v4 config -- styles silently missing | Verify which Tailwind version shadcn init scaffolded. v4 uses `@theme` directives instead of `tailwind.config.js` `extend` |
| shadcn/ui dark mode | Defining dark mode colors in CSS but not wrapping app in `ThemeProvider` | Use shadcn's `ThemeProvider` component and `useTheme` hook. Without it, the `.dark` class is never applied |
| Redis admin endpoints | Fetching full task lists without pagination | Always paginate. Redis SCAN for keys, XRANGE with COUNT for streams. Admin UI must send limit/offset |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Polling for real-time dashboard updates every 1 second | Hammers admin API with requests, wastes bandwidth, gateway load increases per open dashboard tab | Use `refetchInterval: 5000` (5s) for dashboard, `refetchInterval: 30000` for less dynamic pages. Disable polling when tab is not visible (`refetchIntervalInBackground: false`) | >5 admin tabs open simultaneously |
| Fetching all tasks/nodes in a single API call | Browser freezes parsing huge JSON, gateway memory spike serializing thousands of tasks | Paginate all list endpoints from day one. Default page size of 50. Use cursor-based pagination for tasks (UUID v7 enables this) | >1000 tasks or >100 nodes |
| Unoptimized Vite production build | Large JS bundle, slow initial load, poor lighthouse score | Enable code splitting per route (TanStack Router supports lazy routes). Use `vite-plugin-compression` for gzip/brotli. Verify tree-shaking works with `npx vite-bundle-visualizer` | Always -- admin UIs still need fast loads |
| Re-rendering entire dashboard on every metrics update | UI jank, high CPU usage in browser, sluggish interactions | Isolate metrics components with React.memo or separate TanStack Query subscriptions per widget. Do not lift metrics state to a shared parent | Dashboard with >10 metric widgets |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Admin login endpoint with no rate limiting | Brute-force password attacks against admin accounts | Rate limit login endpoint: 5 attempts per minute per IP. Use Tower rate-limiting middleware on the admin auth router only |
| Admin session tokens with no expiration | Stolen session token grants permanent admin access | Set session expiration (e.g., 8 hours). Implement session refresh. Store session metadata in Redis with TTL |
| Serving admin UI on the same port as the public client API | Admin UI exposed to the same audience as the public API | Acceptable if admin routes require auth. Better: bind admin routes to a separate port or require VPN/internal network access |
| No Content-Security-Policy header | XSS attacks via inline scripts, external script injection | Set CSP headers on admin UI responses: `default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'` (unsafe-inline needed for Tailwind) |
| Admin API returns full Redis data including internal fields | Leaking internal implementation details (Redis keys, internal IDs, timestamps in unexpected formats) | Define explicit API response DTOs. Never serialize internal structs directly to JSON. Filter sensitive fields |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| No loading states during API calls | Admin clicks "Cancel Task" -- nothing happens for 2 seconds, they click again | Use TanStack Query's `isPending` and `isLoading` states. Disable buttons during mutations. Show skeleton loaders |
| Optimistic updates without rollback | Admin deletes a service, UI removes it instantly, but the API call fails -- service disappears from UI but still exists | Use TanStack Query's `onMutate`/`onError` for optimistic updates with proper rollback. Or skip optimistic updates for destructive actions and use loading spinners instead |
| No confirmation for destructive actions | Admin accidentally deletes a service with 50 active nodes | Require confirmation dialog for service deletion, task cancellation, and node drain. Show impact count ("This will affect 50 nodes and 12 pending tasks") |
| Dashboard shows metrics without context | "Queue depth: 47" -- is that normal or a crisis? | Show trends (sparklines), thresholds (color-coded: green/yellow/red), and comparisons to historical baselines |
| Table-only views with no search or filter | Admin managing 200 services scrolls endlessly to find one | Add search/filter to every list view from the start. Filter by status, service name, date range |

## "Looks Done But Isn't" Checklist

- [ ] **SPA routing:** Often missing fallback -- verify direct URL access to `/admin/services/123` works after browser refresh
- [ ] **Auth cookies:** Often missing `HttpOnly` -- verify `document.cookie` in browser console does NOT show the session token
- [ ] **CORS in production:** Often left permissive from dev -- verify `Access-Control-Allow-Origin` is not `*` in production responses
- [ ] **Pagination:** Often missing on list endpoints -- verify the tasks list with 1000+ tasks does not crash the browser
- [ ] **Error handling:** Often missing global error boundary -- verify a 500 from the API shows a friendly error, not a white screen
- [ ] **Session expiration:** Often missing -- verify that after 8 hours of inactivity, the admin is redirected to login
- [ ] **Mobile responsiveness:** Often ignored for admin UIs -- verify the dashboard is usable on a tablet (common for on-call)
- [ ] **Dark mode consistency:** Often partially implemented -- verify all shadcn/ui components respect the theme toggle, especially custom components
- [ ] **Build output:** Often untested -- verify `vite build` output is served correctly by Axum, not just the dev server
- [ ] **API error format:** Often inconsistent -- verify all admin API errors return the same JSON structure with status code, message, and error type

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| localStorage token storage | MEDIUM | Migrate to HttpOnly cookies. Requires backend changes (set-cookie on login, cookie validation middleware). Force all admin users to re-login. Update all frontend fetch calls to use `credentials: 'include'` |
| Permissive CORS in production | LOW | Update CorsLayer configuration, deploy. No data breach if caught quickly, but audit access logs for suspicious cross-origin requests |
| Missing SPA fallback | LOW | Add `ServeDir::not_found_service(ServeFile::new("dist/index.html"))` to Axum. One-line fix plus deploy |
| Stale cache bugs from scattered query keys | MEDIUM | Introduce query key factory. Refactor all `useQuery`/`useMutation` calls to use factory keys. Audit all mutation `onSuccess` callbacks. Tedious but not architecturally breaking |
| Prometheus metrics exposed | MEDIUM | Remove browser-facing metrics fetch. Create admin API endpoints. Requires new backend endpoints and frontend components. Straightforward but touches multiple layers |
| Confused auth middleware | HIGH | Refactor to per-router middleware stacks. May require changing URL structure. Test all three auth paths (client, node, admin) for regressions. High risk of breaking existing client/node auth |
| Missing pagination | HIGH | Requires backend API changes (add limit/offset params), frontend changes (add pagination UI), and possibly Redis query changes (add SCAN/COUNT). Affects every list endpoint |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| CORS misconfiguration | Phase 1 (Project Setup) | Verify Vite proxy works in dev and CorsLayer is scoped to admin routes only |
| Auth token in localStorage | Phase 2 (Admin Auth) | Run `document.cookie` in browser console -- session token must NOT be visible. `localStorage` must be empty of auth data |
| SPA fallback routing | Phase 1 (Project Setup) | Navigate directly to `/admin/services` -- React app must load, not 404 |
| TanStack Query cache staleness | Phase 1 (Project Setup) | Query key factory exists as a module. All queries use factory keys. Mutations invalidate by prefix |
| Vite dev vs production URL mismatch | Phase 1 (Project Setup) | Run `vite build`, serve from Axum, verify all API calls succeed |
| Prometheus metrics exposure | Phase 3 (Dashboard) | `/metrics` is not fetched from browser. Dedicated `/api/admin/metrics` endpoints exist with auth |
| Confused auth systems | Phase 2 (Admin Auth) | Three auth paths tested independently. No middleware shared between client, node, and admin routers |
| Missing pagination | Phase 2 (API Endpoints) | Every list endpoint accepts `limit`/`offset`. Default page size enforced |
| No rate limiting on login | Phase 2 (Admin Auth) | Submit 10 rapid login attempts -- 429 returned after 5 |
| Missing loading/error states | Phase 3 (UI Polish) | Every mutation shows loading indicator. Network error shows error boundary, not white screen |

## Sources

- [Axum CORS configuration with tower-http](https://learning.angarsa.com/rust-axum/using-tower-http-middleware-cors-and-compression/) - CorsLayer configuration
- [Axum SPA fallback routing discussion](https://github.com/tokio-rs/axum/discussions/2486) - ServeDir with not_found_service pattern
- [Axum SPA routing issue #87](https://github.com/tokio-rs/axum/issues/87) - Official discussion on SPA support
- [Vite proxy configuration](https://medium.com/@kychok98/frontend-tips-2-local-cors-issues-with-vite-proxy-6b2cf4ca3672) - Dev proxy setup and limitations
- [TanStack Query cache invalidation patterns](https://www.buncolak.com/posts/avoiding-common-mistakes-with-tanstack-query-part-1/) - Common mistakes and prevention
- [TanStack Query invalidation discussion](https://github.com/TanStack/query/discussions/1691) - Cache invalidation strategies
- [JWT storage security: localStorage vs HttpOnly cookies](https://dev.to/cotter/localstorage-vs-cookies-all-you-need-to-know-about-storing-jwt-tokens-securely-in-the-front-end-15id) - Token storage security analysis
- [DigitalOcean: Securing React with HttpOnly cookies](https://www.digitalocean.com/community/tutorials/how-to-secure-react-applications-against-xss-attacks-with-http-only-cookies) - HttpOnly cookie implementation
- [Prometheus security model](https://prometheus.io/docs/operating/security/) - Metrics endpoint security considerations
- [Prometheus CORS issues](https://github.com/prometheus/prometheus/issues/15406) - CORS implementation pitfalls
- [shadcn/ui theming documentation](https://ui.shadcn.com/docs/theming) - CSS variable conventions and dark mode setup
- [TanStack Router file-based routing with Vite](https://tanstack.com/router/latest/docs/installation/with-vite) - Vite plugin configuration
- [TanStack Router Vite config root issue](https://github.com/TanStack/router/issues/3624) - Route generation directory problems
- [Fullstack Rust + React + Vite setup](https://dev.to/alexeagleson/how-to-set-up-a-fullstack-rust-project-with-axum-react-vite-and-shared-types-429e) - Integration patterns

---
*Pitfalls research for: Adding React admin UI to existing Rust/Axum gateway*
*Researched: 2026-03-22*
