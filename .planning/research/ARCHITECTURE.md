# Architecture Research: Admin Web UI Integration

**Domain:** Frontend-to-backend integration for admin UI on existing Rust/Axum gateway
**Researched:** 2026-03-22
**Confidence:** HIGH

## System Overview

```
                DEVELOPMENT                                  PRODUCTION

  ┌─────────────────────────┐               ┌─────────────────────────────────────┐
  │   Vite Dev Server :5173 │               │          Gateway Process :8080       │
  │   ┌───────────────────┐ │               │                                     │
  │   │  React Admin SPA  │ │               │  ┌─────────────────────────────────┐│
  │   │  TanStack Router  │ │               │  │  Axum Router                    ││
  │   │  TanStack Query   │ │               │  │                                 ││
  │   └────────┬──────────┘ │               │  │  /admin/*  ──> ServeDir(dist/)  ││
  │            │ /api/*     │               │  │               fallback index.html││
  │   ┌────────▼──────────┐ │               │  │                                 ││
  │   │  Vite Proxy       │─┼──┐            │  │  /v1/admin/* ──> Admin API      ││
  │   │  /api/* -> :8080  │ │  │            │  │  /v1/tasks   ──> Client API     ││
  │   └───────────────────┘ │  │            │  │  /metrics    ──> Prometheus      ││
  └─────────────────────────┘  │            │  └─────────────────────────────────┘│
                               │            │                                     │
  ┌─────────────────────────┐  │            │  ┌──────────┐  ┌──────────────────┐│
  │   Gateway :8080         │◀─┘            │  │  Auth    │  │  Redis/Valkey    ││
  │   /v1/admin/*           │               │  │  Layer   │  │  (state store)   ││
  │   /metrics              │               │  └──────────┘  └──────────────────┘│
  └─────────────────────────┘               └─────────────────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Implementation |
|-----------|----------------|----------------|
| **Vite Dev Proxy** | Forward `/api/*` to gateway during development, avoid CORS in dev | `server.proxy` in `vite.config.ts` |
| **Axum Static File Server** | Serve built SPA assets in production, fallback to `index.html` for client-side routing | `tower_http::services::ServeDir` with `fallback(ServeFile)` |
| **CORS Layer** | Allow cross-origin requests (only needed if frontend served from different origin) | `tower_http::cors::CorsLayer` on admin API routes |
| **Admin Auth Middleware** | Validate admin bearer token on all `/v1/admin/*` requests | Existing `admin_auth_middleware` -- already implemented |
| **API Client (fetch wrapper)** | Typed HTTP client for frontend, attaches auth token, handles errors | Thin wrapper around `fetch` used by TanStack Query |
| **TanStack Query** | Server state cache, automatic refetching, loading/error states | `queryClient` with queries per API endpoint |
| **TanStack Router** | Client-side routing, auth guards, layout nesting | File-based routes with `beforeLoad` auth check |

## Integration Architecture

### Decision: Serve from Gateway in Production

**Recommendation:** Serve the built SPA from the gateway process in production. Separate origin in development only.

**Rationale:**
- Eliminates CORS entirely in production (same origin)
- No separate frontend process/container to deploy
- Single Docker image remains (constraint from PROJECT.md)
- Admin UI is low-traffic; no CDN needed
- Vite dev proxy handles the development case cleanly

**How it works:**
- Development: Vite dev server on `:5173` proxies `/api/*` to gateway on `:8080`
- Production: Gateway serves SPA from `dist/` directory at `/admin/*`, API at `/v1/admin/*`

### New Axum Routes Needed

| Route Pattern | Method | Purpose | Auth | New/Modified |
|---------------|--------|---------|------|--------------|
| `/admin/*` | GET | Serve SPA static assets + index.html fallback | None (SPA is public shell, auth in JS) | **NEW** |
| `/v1/admin/auth/login` | POST | Admin login, returns bearer token | None (login endpoint) | **NEW** |
| `/v1/admin/auth/me` | GET | Validate current token, return admin info | Admin token | **NEW** |
| `/v1/admin/tasks` | GET | List tasks with pagination/filtering | Admin token | **NEW** |
| `/v1/admin/tasks/{task_id}` | GET | Get task detail (reuse existing result endpoint logic) | Admin token | **NEW** (admin version with full detail) |
| `/v1/admin/tasks/{task_id}/cancel` | POST | Cancel a pending/running task | Admin token | **NEW** |
| `/v1/admin/metrics/query` | POST | Proxy PromQL queries to internal metrics | Admin token | **NEW** |
| `/v1/admin/api-keys` | GET | List all API keys (hashes only) | Admin token | **NEW** (existing POST stays) |
| `/v1/admin/node-tokens` | GET | List all node tokens per service | Admin token | **NEW** (existing POST stays) |
| `/v1/admin/services` | GET/POST | Already exists | Admin token | Existing |
| `/v1/admin/services/{name}` | GET/DELETE | Already exists | Admin token | Existing |
| `/v1/admin/health` | GET | Already exists | Admin token | Existing |
| `/metrics` | GET | Already exists (Prometheus exposition) | Admin token | Existing |

### Endpoints NOT Needed

| Endpoint | Why Not |
|----------|---------|
| WebSocket for live updates | Polling with TanStack Query refetchInterval is simpler and sufficient for an admin dashboard. Out of scope per PROJECT.md. |
| `/v1/admin/logs` | Explicitly deferred in PROJECT.md out of scope |
| Session/cookie auth | Bearer token is simpler, matches existing admin auth pattern |

## Recommended Project Structure

```
admin-ui/                        # Separate directory at repo root (NOT in gateway/)
├── index.html                   # Vite entry point
├── package.json
├── vite.config.ts               # Dev proxy config
├── tsconfig.json
├── tailwind.config.ts
├── components.json              # shadcn/ui config
├── src/
│   ├── main.tsx                 # React entry, QueryClientProvider, RouterProvider
│   ├── routeTree.gen.ts         # TanStack Router generated tree
│   ├── api/
│   │   ├── client.ts            # fetch wrapper with auth token injection
│   │   ├── types.ts             # TypeScript types matching gateway JSON responses
│   │   ├── services.ts          # TanStack Query hooks: useServices, useServiceDetail
│   │   ├── tasks.ts             # TanStack Query hooks: useTasks, useTaskDetail, useCancelTask
│   │   ├── nodes.ts             # TanStack Query hooks: useNodes
│   │   ├── api-keys.ts          # TanStack Query hooks: useApiKeys, useCreateApiKey
│   │   ├── metrics.ts           # TanStack Query hooks: useMetrics, useMetricQuery
│   │   └── auth.ts              # TanStack Query hooks: useLogin, useAuthMe
│   ├── routes/
│   │   ├── __root.tsx           # Root layout: sidebar nav, auth guard
│   │   ├── login.tsx            # Login page (no auth required)
│   │   ├── _authenticated.tsx   # Layout route: checks auth, redirects to login
│   │   ├── _authenticated/
│   │   │   ├── index.tsx        # Dashboard (metrics overview)
│   │   │   ├── services/
│   │   │   │   ├── index.tsx    # Service list
│   │   │   │   └── $name.tsx    # Service detail + nodes
│   │   │   ├── tasks/
│   │   │   │   ├── index.tsx    # Task list with filters
│   │   │   │   └── $taskId.tsx  # Task detail
│   │   │   ├── api-keys.tsx     # API key management
│   │   │   └── settings.tsx     # Node tokens, gateway config view
│   ├── components/
│   │   ├── ui/                  # shadcn/ui components (generated)
│   │   ├── layout/
│   │   │   ├── sidebar.tsx
│   │   │   ├── header.tsx
│   │   │   └── page-container.tsx
│   │   ├── dashboard/
│   │   │   ├── metric-card.tsx
│   │   │   ├── queue-depth-chart.tsx
│   │   │   └── node-status-grid.tsx
│   │   ├── services/
│   │   │   ├── service-table.tsx
│   │   │   ├── register-service-dialog.tsx
│   │   │   └── node-health-badge.tsx
│   │   └── tasks/
│   │       ├── task-table.tsx
│   │       ├── task-status-badge.tsx
│   │       └── cancel-task-dialog.tsx
│   ├── lib/
│   │   ├── auth.ts              # Token storage (localStorage), auth context
│   │   └── utils.ts             # shadcn/ui cn() helper, formatters
│   └── hooks/
│       └── use-auth.ts          # Auth state hook
└── dist/                        # Build output (gitignored, copied into Docker image)
```

### Structure Rationale

- **`admin-ui/` at repo root:** Keeps frontend completely separate from the Rust workspace. No cargo interaction. Clean build boundary.
- **`api/` directory:** One file per resource domain. Each file exports TanStack Query hooks, keeping data fetching organized and co-located with types.
- **`_authenticated` layout route:** TanStack Router convention for layout routes. All child routes automatically get auth checking without repeating it.
- **`components/` by feature:** UI components grouped by the page/feature they serve, not by type (no `atoms/molecules/organisms` pattern -- that adds indirection without value at this scale).

## Architectural Patterns

### Pattern 1: Vite Dev Proxy

**What:** During development, Vite's built-in proxy forwards API requests to the gateway. No CORS configuration needed in dev.

**When to use:** Always during `npm run dev`. This is the standard Vite pattern for frontend+backend development.

**Trade-offs:** Only works in development. Production needs a different solution (SPA serving from gateway).

**Configuration:**
```typescript
// vite.config.ts
export default defineConfig({
  server: {
    proxy: {
      '/v1': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
      '/metrics': {
        target: 'http://localhost:8080',
        changeOrigin: true,
      },
    },
  },
  base: '/admin/',  // SPA lives under /admin/ in production
});
```

### Pattern 2: SPA Serving from Axum with Fallback

**What:** In production, Axum serves the built SPA files. Any request to `/admin/*` that does not match a static file returns `index.html`, enabling client-side routing.

**When to use:** Production deployment. The gateway binary includes a path to the static files directory (configurable).

**Trade-offs:** Adds a few lines to the Axum router. Static files are small (~500KB-2MB for a React SPA). No performance concern for an admin tool.

**Implementation (Rust):**
```rust
use tower_http::services::{ServeDir, ServeFile};

// In the HTTP router setup:
let spa_service = ServeDir::new(&config.admin.static_dir)
    .fallback(ServeFile::new(
        format!("{}/index.html", config.admin.static_dir)
    ));

let app = Router::new()
    .merge(api_routes)
    .merge(admin_routes)
    .nest_service("/admin", spa_service)  // SPA assets
    .with_state(http_state);
```

**Config addition needed:**
```rust
// In AdminConfig:
pub struct AdminConfig {
    pub token: Option<String>,
    /// Path to built SPA assets directory. None = UI disabled.
    pub static_dir: Option<String>,
}
```

### Pattern 3: API Client with Token Injection

**What:** A thin fetch wrapper that automatically attaches the admin bearer token from localStorage to every request. TanStack Query hooks use this client, never raw `fetch`.

**When to use:** Every API call from the frontend.

**Trade-offs:** Simple and transparent. No axios dependency needed -- native `fetch` is sufficient for JSON APIs.

**Implementation:**
```typescript
// api/client.ts
const API_BASE = import.meta.env.DEV ? '' : '/admin/..';
// In dev, Vite proxy handles /v1/*. In prod, same origin.

export async function apiClient<T>(
  path: string,
  options: RequestInit = {},
): Promise<T> {
  const token = localStorage.getItem('admin_token');
  const headers: HeadersInit = {
    'Content-Type': 'application/json',
    ...(token ? { Authorization: `Bearer ${token}` } : {}),
    ...options.headers,
  };

  const response = await fetch(`${API_BASE}${path}`, {
    ...options,
    headers,
  });

  if (response.status === 401) {
    localStorage.removeItem('admin_token');
    window.location.href = '/admin/login';
    throw new Error('Unauthorized');
  }

  if (!response.ok) {
    throw new Error(`API error: ${response.status}`);
  }

  return response.json();
}
```

### Pattern 4: TanStack Query for Server State

**What:** Each API resource gets a query hook. Automatic caching, background refetching, and stale-while-revalidate. Mutations for write operations.

**When to use:** Every data fetch in the UI. Never use `useEffect` + `useState` for API data.

**Trade-offs:** Adds a dependency but eliminates manual loading/error/cache state management. TanStack Query is the established standard for this.

**Example:**
```typescript
// api/services.ts
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query';
import { apiClient } from './client';

export function useServices() {
  return useQuery({
    queryKey: ['services'],
    queryFn: () => apiClient<ListServicesResponse>('/v1/admin/services'),
    refetchInterval: 30_000, // Refresh every 30s for dashboard
  });
}

export function useServiceDetail(name: string) {
  return useQuery({
    queryKey: ['services', name],
    queryFn: () => apiClient<ServiceDetailResponse>(`/v1/admin/services/${name}`),
    refetchInterval: 10_000, // More frequent for live node health
  });
}

export function useRegisterService() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (data: RegisterServiceRequest) =>
      apiClient('/v1/admin/services', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['services'] });
    },
  });
}
```

### Pattern 5: Prometheus Metrics via Gateway Proxy Endpoint

**What:** The frontend does NOT query Prometheus directly. Instead, the gateway exposes a `/v1/admin/metrics/query` endpoint that reads from its internal `prometheus::Registry` and returns formatted data. No external Prometheus server needed.

**When to use:** Dashboard metrics visualization.

**Why this approach:**
- The gateway already has all metrics in its `prometheus::Registry` (8 metric families)
- No external Prometheus server dependency for the admin UI
- The `/metrics` endpoint already exposes Prometheus text format
- Admin auth protects metric data

**Implementation options (pick one):**

**Option A -- Parse /metrics text format in frontend (simplest):**
```typescript
// Fetch raw Prometheus text, parse client-side
export function useRawMetrics() {
  return useQuery({
    queryKey: ['metrics', 'raw'],
    queryFn: async () => {
      const token = localStorage.getItem('admin_token');
      const res = await fetch('/metrics', {
        headers: token ? { Authorization: `Bearer ${token}` } : {},
      });
      return res.text(); // Prometheus exposition format
    },
    refetchInterval: 15_000,
  });
}
// Use a library like `prom-client-parser` or write a simple parser
// for the text exposition format (it is line-based and simple)
```

**Option B -- Add JSON metrics endpoint to gateway (recommended):**
```rust
// New endpoint: GET /v1/admin/metrics/summary
// Returns pre-computed dashboard-friendly JSON
#[derive(Serialize)]
pub struct MetricsSummary {
    pub queue_depths: HashMap<String, f64>,
    pub active_nodes: HashMap<String, f64>,
    pub tasks_submitted_total: HashMap<String, f64>,
    pub tasks_completed_total: HashMap<String, f64>,
    pub error_rate: HashMap<String, f64>,
}

pub async fn metrics_summary(
    State(state): State<Arc<AppState>>,
) -> Json<MetricsSummary> {
    // Read directly from state.metrics gauge/counter handles
    // No Prometheus text parsing needed
}
```

**Recommendation:** Option B. The gateway already holds all metric handles in `state.metrics`. Reading them directly and returning JSON is trivial, type-safe, and avoids text format parsing on both sides. The existing `/metrics` endpoint continues to serve Prometheus scrapers.

## Data Flow

### Admin Authentication Flow

```
Browser                     Gateway                      Redis
  |                           |                            |
  |-- POST /v1/admin/auth/login                            |
  |   { username, password }  |                            |
  |                           |-- Validate against config  |
  |                           |   (admin.token or          |
  |                           |    admin.credentials)      |
  |                           |                            |
  |<-- 200 { token }         |                            |
  |                           |                            |
  | Store token in            |                            |
  | localStorage              |                            |
  |                           |                            |
  |-- GET /v1/admin/services  |                            |
  |   Authorization: Bearer X |                            |
  |                           |-- admin_auth_middleware     |
  |                           |   validates Bearer token   |
  |                           |                            |
  |<-- 200 { services: [...] }                             |
```

**Auth token design decision:**

The existing `admin_auth_middleware` already validates `Authorization: Bearer <token>` against `config.admin.token`. Two approaches for login:

**Approach A -- Static token (simplest, recommended for v1.1):**
- The admin token in `gateway.toml` IS the bearer token
- Login endpoint validates username/password against config, returns the same static token
- No JWT, no expiry, no Redis session state
- Matches the existing auth model exactly
- To "logout", frontend just clears localStorage

**Approach B -- JWT tokens (future, if multi-admin needed):**
- Login returns a signed JWT with expiry
- Middleware validates JWT signature + expiry
- Adds `jsonwebtoken` crate dependency
- Only needed if multiple admin users with different permissions are required

**Recommendation:** Approach A for v1.1. The gateway already has a single admin token. The login endpoint just verifies credentials and hands back that token. Zero new backend state.

### Dashboard Data Refresh Flow

```
React Component          TanStack Query Cache        Gateway API
  |                           |                          |
  | useServices()             |                          |
  |-------------------------->|                          |
  |                           |-- Cache MISS             |
  |                           |-- GET /v1/admin/services |
  |                           |                          |
  |                           |<-- { services: [...] }   |
  |<-- data (loading=false)   |                          |
  |                           |                          |
  |   ... 30s passes ...      |                          |
  |                           |-- Background refetch     |
  |                           |-- GET /v1/admin/services |
  |                           |<-- { services: [...] }   |
  |<-- data (updated silently)|                          |
```

### Key Data Flows

1. **Login:** User submits credentials -> Gateway validates -> Returns admin token -> Frontend stores in localStorage -> All subsequent requests include `Authorization: Bearer` header
2. **Dashboard load:** Multiple parallel TanStack Query fetches (services, health, metrics summary) -> Gateway reads Redis + internal metrics -> Returns JSON -> React renders
3. **Service registration:** User fills form -> `useMutation` POSTs to `/v1/admin/services` -> On success, `invalidateQueries(['services'])` triggers automatic refetch of service list
4. **Task cancellation:** User clicks cancel -> `useMutation` POSTs to `/v1/admin/tasks/{id}/cancel` -> Gateway marks task as failed in Redis -> Invalidate task queries
5. **Metrics refresh:** `refetchInterval: 15000` on metrics query -> Gateway reads from `state.metrics` Prometheus handles -> Returns JSON summary -> Chart components re-render

## CORS Configuration

### Production: Not Needed

Same-origin serving eliminates CORS entirely. The SPA is served from `/admin/*` on the same gateway process that serves `/v1/admin/*`. No `Access-Control-*` headers needed.

### Development: Not Needed (Vite Proxy)

The Vite dev proxy forwards requests from `:5173` to `:8080` server-side. The browser sees all requests going to `:5173`. No CORS.

### Fallback: If Separate Deployment Ever Needed

If someone runs the frontend on a different origin (e.g., during testing), add CORS only to admin routes:

```rust
use tower_http::cors::{CorsLayer, Any};

let cors = CorsLayer::new()
    .allow_origin(["http://localhost:5173".parse().unwrap()])
    .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
    .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION])
    .allow_credentials(true);

// Apply ONLY to admin routes, not client API routes
let admin_routes = admin_routes.layer(cors);
```

**Recommendation:** Do not add CORS by default. The proxy + same-origin pattern means it is never needed in the normal workflow. Add it only if the deployment model changes.

## Gateway Config Changes

```toml
# gateway.toml additions for v1.1:

[admin]
token = "your-admin-token"            # Existing
static_dir = "./admin-ui/dist"        # NEW: path to built SPA assets
# username = "admin"                  # NEW: optional, for login endpoint
# password_hash = "sha256:..."        # NEW: hashed admin password
```

## Docker Build Integration

```dockerfile
# Multi-stage: build frontend, then build Rust, then combine
FROM node:22-alpine AS frontend
WORKDIR /app/admin-ui
COPY admin-ui/package*.json ./
RUN npm ci
COPY admin-ui/ ./
RUN npm run build

FROM rust:1.85-alpine AS backend
# ... existing Rust build ...

FROM scratch
COPY --from=backend /app/target/release/xgent-gateway /gateway
COPY --from=frontend /app/admin-ui/dist /admin-ui/dist
# Single binary + SPA assets
```

## Anti-Patterns

### Anti-Pattern 1: Embedding SPA in Rust Binary with include_bytes!

**What people do:** Use `include_bytes!` or `rust-embed` to compile the SPA into the Rust binary.
**Why it is wrong:** Every frontend change requires recompiling the Rust binary. Adds 1-2MB to binary size. Makes hot-reload impossible during development. Complicates CI (Rust build depends on Node build).
**Do this instead:** Serve from a configurable directory path. The Docker multi-stage build places the files alongside the binary. In development, point to the Vite build output directory.

### Anti-Pattern 2: Adding a Full Prometheus Server Dependency

**What people do:** Deploy a Prometheus server alongside the gateway and have the frontend query Prometheus directly.
**Why it is wrong:** Massive operational overhead for an admin dashboard. The gateway already has all the metrics in memory. Adding Prometheus adds a container, storage, and another network hop.
**Do this instead:** Read metrics directly from the gateway's `prometheus::Registry` handles and return JSON. Keep the `/metrics` endpoint for external Prometheus scrapers if desired.

### Anti-Pattern 3: JWT with Refresh Tokens for Single-Admin Tool

**What people do:** Build a full JWT auth system with access tokens, refresh tokens, and rotation.
**Why it is wrong:** Over-engineered for a single-admin gateway tool. Adds crypto dependencies, token storage, and refresh logic. The gateway already has a static admin token.
**Do this instead:** Use the existing static admin token pattern. The login endpoint validates credentials and returns the configured token. If multi-admin is needed later, upgrade to JWT then.

### Anti-Pattern 4: Polling with setInterval Instead of TanStack Query

**What people do:** Use `setInterval` + `fetch` + `useState` for periodic data refresh.
**Why it is wrong:** No request deduplication, no caching, no background refetch, no stale-while-revalidate, no automatic error retry, no garbage collection of unused queries.
**Do this instead:** TanStack Query with `refetchInterval`. It handles all of the above automatically. One line of config vs 30+ lines of manual state management.

### Anti-Pattern 5: CORS Permissive Mode in Production

**What people do:** Add `CorsLayer::permissive()` to allow all origins.
**Why it is wrong:** Allows any website to make authenticated requests to the admin API if the user has a token stored.
**Do this instead:** Serve from the same origin (no CORS needed) or whitelist specific origins.

## Build Order (Dependencies Between Frontend and Backend Changes)

The following order respects dependencies -- backend endpoints must exist before frontend can consume them.

### Phase 1: Backend Auth + New Endpoints
1. Add `static_dir` to `AdminConfig`
2. Add login endpoint (`/v1/admin/auth/login`) that validates credentials against config
3. Add `/v1/admin/auth/me` endpoint
4. Add `/v1/admin/metrics/summary` JSON endpoint (reads from `state.metrics`)
5. Add `/v1/admin/tasks` list endpoint with pagination
6. Add `/v1/admin/tasks/{task_id}/cancel` endpoint
7. Add `/v1/admin/api-keys` GET (list) endpoint
8. Add `/v1/admin/node-tokens` GET (list) endpoint
9. Add SPA static file serving route (`/admin/*` with fallback)

### Phase 2: Frontend Scaffolding
1. Initialize Vite + React + TypeScript project in `admin-ui/`
2. Configure Vite dev proxy, TailwindCSS, shadcn/ui
3. Set up TanStack Router with `__root.tsx` and `_authenticated` layout
4. Build API client (`api/client.ts`) and auth hooks
5. Build login page

### Phase 3: Frontend Feature Pages
1. Dashboard page (metrics summary, queue depths, node counts)
2. Services list + detail pages (uses existing endpoints)
3. Task list + detail + cancel (uses new endpoints)
4. API key management page
5. Node token management page

### Phase 4: Production Integration
1. Docker multi-stage build (Node + Rust)
2. Gateway config documentation for `admin.static_dir`
3. Integration testing (build frontend, serve from gateway, verify routes)

**Critical dependency:** Phase 2 step 4 (API client) depends on Phase 1 steps 2-3 (auth endpoints). The frontend cannot test auth flow without the login endpoint. Build auth endpoints first.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Redis/Valkey | Existing -- no changes | All admin data already in Redis |
| Vite Dev Server | Dev proxy to gateway | Only during `npm run dev` |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| SPA <-> Admin API | HTTP JSON via `fetch` | All requests go through `apiClient` wrapper with auth token |
| Admin API <-> Redis | Existing `auth_conn` | New list endpoints add read-only Redis queries |
| Admin API <-> Metrics | Direct Rust struct access | `state.metrics` is `Arc<Metrics>`, read gauge/counter values directly |
| SPA Router <-> Auth State | localStorage + React context | Token in localStorage, auth state in context, router guards in `beforeLoad` |

## Sources

- [Axum SPA fallback discussion](https://github.com/tokio-rs/axum/discussions/2486) -- ServeDir with fallback for React client-side routing
- [tower-http CORS](https://docs.rs/tower-http/latest/tower_http/cors/struct.CorsLayer.html) -- CorsLayer configuration reference
- [Vite server proxy docs](https://vite.dev/config/server-options#server-proxy) -- Dev proxy configuration
- [TanStack Query auth patterns](https://github.com/TanStack/query/discussions/3253) -- Authentication with TanStack Query
- [Prometheus HTTP API](https://prometheus.io/docs/prometheus/latest/querying/api/) -- Query API reference
- [Axum static file serving](https://github.com/tokio-rs/axum/discussions/1309) -- Embedding and serving static files

---
*Architecture research for: Admin Web UI integration with Rust/Axum gateway*
*Researched: 2026-03-22*
