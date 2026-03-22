# Phase 8: Frontend Foundation and Backend Auth - Research

**Researched:** 2026-03-22
**Domain:** Rust backend session auth + React SPA scaffolding with UI patterns
**Confidence:** HIGH

## Summary

This phase spans two distinct domains: (1) backend auth endpoints in Rust/Axum with cookie-based sessions stored in Redis, and (2) a new React SPA project with routing, auth guards, and reusable UI patterns. The backend work modifies existing code (AdminConfig, admin_auth_middleware, route registration) and adds three new endpoints. The frontend is a greenfield project in a separate repository using Vite + React 19 + TanStack Router + TanStack Query + shadcn/ui.

The backend side is straightforward -- axum-extra provides cookie extraction, the existing Redis MultiplexedConnection handles session storage, and the argon2/password-hash ecosystem handles password verification. The frontend side requires careful scaffolding decisions since all subsequent phases (9-12) build on the patterns established here. TanStack Router's file-based routing with `beforeLoad` auth guards and TanStack Query's mutation/query patterns for auth state are well-documented and mature.

**Primary recommendation:** Scaffold the SPA project first with full routing skeleton and auth flow, then implement the backend auth endpoints, then connect them. This prevents frontend work from blocking on backend changes.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Admin credentials defined in `gateway.toml` -- `admin.username` and `admin.password_hash` (bcrypt or argon2 hash). Single admin, no Redis credential storage.
- **D-02:** Existing `admin.token` config field is replaced by `admin.username` + `admin.password_hash` for the new session-based auth.
- **D-03:** HttpOnly cookie with `SameSite=None; Secure` for cross-origin auth (SPA hosted separately from gateway).
- **D-04:** Sessions stored in Redis with a 24-hour sliding window TTL. Each authenticated API request resets the TTL.
- **D-05:** Explicit `/v1/admin/auth/refresh` endpoint exists (API-02) alongside the sliding window -- belt-and-suspenders approach.
- **D-06:** `/v1/admin/auth/login` returns the session cookie. `/v1/admin/auth/logout` deletes the session from Redis and clears the cookie.
- **D-07:** Session key format in Redis: `admin_session:<session_id>` with value containing username and created_at.
- **D-08:** Configurable allowed origin in `gateway.toml` via `admin.cors_origin` (e.g., `"https://admin.example.com"`). Single explicit origin, not wildcard (required for credentials).
- **D-09:** CORS must work in dev too -- SPA uses `VITE_API_URL` env var pointing at gateway directly.
- **D-10:** SPA lives in a separate repository, fully decoupled from the gateway repo. Independent deploy cycles.
- **D-11:** Hosted on Aliyun OSS + CDN. Vanilla `vite build` output (`dist/` with `index.html` + assets).
- **D-12:** Dev workflow uses `VITE_API_URL=http://localhost:8080` -- no Vite proxy, direct fetch to gateway.
- **D-13:** Tech stack: Vite + React 19 + TanStack Router + TanStack Query + shadcn/ui + Tailwind CSS.
- **D-14:** Collapsible sidebar -- icon + label, can minimize to icon-only mode.
- **D-15:** All 4 nav items stubbed from day one: Dashboard, Services, Tasks, Credentials. Clicking unbuilt pages shows a placeholder.
- **D-16:** Split layout login page -- left side branding/project name, right side login form.
- **D-17:** TanStack Router auth guard redirects unauthenticated users to `/login`. After successful login, redirect back to the originally requested page.
- **D-18:** Loading skeletons use shadcn/ui `Skeleton` component. Each page creates its own skeleton layout matching its content shape.
- **D-19:** Error states are inline alert banners with error message + "Retry" button within the page content area. Page shell (sidebar, header) stays visible.
- **D-20:** Empty states show helpful guidance text within the content area.
- **D-21:** Toast notifications use Sonner (shadcn/ui integration).
- **D-22:** Auto-refresh is a global control in the header -- interval picker (5s/15s/30s/off) + pause toggle. Applies to whichever page is active.
- **D-23:** Dark mode toggle with persisted preference (localStorage). Default to dark.
- **D-24:** Responsive layout targeting 1280px+ screens.

### Claude's Discretion
- Session ID generation strategy (UUID v4, random bytes, etc.)
- Password hashing verification library choice (bcrypt vs argon2)
- Exact sidebar width and collapse animation
- Dark mode implementation approach (CSS variables, Tailwind `dark:` classes, etc.)
- TanStack Router file structure and route organization
- Error alert banner styling details
- Placeholder page content and layout

### Deferred Ideas (OUT OF SCOPE)
- Multiple admin accounts -- out of scope per PROJECT.md (no RBAC)
- Serving SPA from Axum via ServeDir -- explicitly rejected; SPA hosted independently on Aliyun OSS + CDN
- Admin password change from the UI -- could be a future enhancement, currently config-file only
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AUTH-01 | Admin can log in with username and password | Backend: POST /v1/admin/auth/login with argon2 verification. Frontend: login form + TanStack Query mutation |
| AUTH-02 | Admin session persists via secure HttpOnly cookie | Backend: axum-extra cookie extraction, Redis session storage with 24h TTL. Frontend: fetch with `credentials: 'include'` |
| AUTH-03 | Admin session auto-refreshes before expiry | Backend: sliding window TTL reset on every request + explicit /refresh endpoint. Frontend: TanStack Query interval or beforeLoad check |
| AUTH-04 | Admin can log out with session cleanup | Backend: DELETE session from Redis, clear cookie. Frontend: logout mutation + redirect to /login |
| API-01 | POST /v1/admin/auth/login endpoint | Axum handler: validate credentials against config, create session in Redis, set HttpOnly cookie |
| API-02 | POST /v1/admin/auth/refresh endpoint | Axum handler: validate session cookie, reset TTL in Redis, return success |
| UI-01 | Loading skeletons, error states with retry, empty states | shadcn/ui Skeleton component, Alert component for errors, custom empty state pattern |
| UI-02 | Toast notifications for mutations | Sonner library via shadcn/ui toast integration |
| UI-03 | Dark mode toggle with persisted preference | Tailwind `dark:` class strategy with localStorage persistence, default dark |
| UI-04 | Auto-refresh with configurable interval | TanStack Query `refetchInterval` controlled by global React context/store |
| UI-05 | Responsive layout for 1280px+ screens | Tailwind responsive breakpoints, sidebar collapse on smaller viewports |
</phase_requirements>

## Standard Stack

### Backend (Rust -- additions to existing gateway)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| axum-extra | 0.12.5 | Cookie extraction | Official Axum companion crate. `CookieJar` extractor for reading/writing cookies in handlers. Pairs with axum 0.8. |
| argon2 (RustCrypto) | 0.5.3 (stable) | Password hash verification | RustCrypto ecosystem, implements `PasswordVerifier` trait with PHC string format. Pure Rust, no C dependencies. Use stable 0.5.x, not 0.6.0-rc. |
| password-hash | 0.5.0 | Password hash traits | Companion to argon2 crate. Provides `PasswordHash::new()` for parsing PHC strings and `PasswordVerifier` trait. |

**Note on argon2 version:** The `argon2` crate 0.6.0-rc.8 is a release candidate. Use the stable 0.5.3 release which pairs with `password-hash` 0.5.0. The RC versions pair with `password-hash` 0.6.0 which is also in RC. Stick with stable.

**Note on session IDs:** Use 32 random bytes (via `rand::thread_rng().gen::<[u8; 32]>()`) hex-encoded. This gives 256 bits of entropy -- far more than UUID v4's 122 bits. The `rand` and `hex` crates are already in Cargo.toml.

### Frontend (New SPA project)

| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Vite | 8.0.1 | Build tool | Fast HMR, native ESM, standard React tooling |
| React | 19.2.4 | UI framework | Latest stable, locked by decision D-13 |
| @tanstack/react-router | 1.168.2 | File-based routing | Type-safe routing, `beforeLoad` auth guards, automatic code splitting |
| @tanstack/router-plugin | 1.167.3 | Vite plugin for router | Generates route tree from file structure, must be loaded before @vitejs/plugin-react |
| @tanstack/react-query | 5.94.5 | Server state management | Caching, refetching, mutations, `refetchInterval` for auto-refresh |
| @tanstack/react-router-devtools | 1.166.11 | Dev tooling | Route debugging in development |
| Tailwind CSS | 4.2.2 | Utility-first CSS | Locked by D-13. v4 uses CSS-first configuration (no tailwind.config.js) |
| shadcn/ui (CLI) | 4.1.0 | Component library | Copy-paste components: Sidebar, Skeleton, Alert, Button, Input, etc. |
| Sonner | 2.0.7 | Toast notifications | shadcn/ui's recommended toast library (D-21) |
| lucide-react | 0.577.0 | Icons | shadcn/ui's icon library, used throughout sidebar and UI |
| class-variance-authority | 0.7.1 | Component variants | Used by shadcn/ui for component styling variants |
| clsx | 2.1.1 | Class merging | Conditional className building |
| tailwind-merge | 3.5.0 | Tailwind class dedup | Resolves conflicting Tailwind classes, used in shadcn/ui `cn()` utility |
| TypeScript | 5.x | Type safety | Required by TanStack Router for type-safe routes |

### Alternatives Considered

| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| argon2 (RustCrypto) | bcrypt crate | Argon2 is memory-hard (better brute-force resistance). Both work for config-stored hashes. Argon2 recommended per modern best practices. |
| argon2 0.5.3 stable | argon2 0.6.0-rc | RC has breaking changes to password-hash trait. Not worth the risk for a single verify call. |
| 32-byte random session ID | UUID v4 | 256 bits vs 122 bits entropy. Random bytes are simpler and more secure for session tokens. |
| Tailwind `dark:` classes | CSS custom properties | Tailwind dark mode is built-in and zero-config with `class` strategy. shadcn/ui already uses this pattern. |

### Installation

**Backend (add to gateway/Cargo.toml):**
```toml
axum-extra = { version = "0.12", features = ["cookie"] }
argon2 = "0.5"
password-hash = "0.5"
```

**Frontend (new project):**
```bash
npm create vite@latest xgent-admin -- --template react-ts
cd xgent-admin
npm install @tanstack/react-router @tanstack/react-query sonner lucide-react clsx tailwind-merge class-variance-authority
npm install -D @tanstack/router-plugin @tanstack/react-router-devtools tailwindcss @tailwindcss/vite
npx shadcn@latest init
```

## Architecture Patterns

### Backend: Session Auth Endpoints

**Pattern: Cookie-based session middleware replacing Bearer token middleware.**

The existing `admin_auth_middleware` in `admin.rs` checks `Authorization: Bearer <token>`. Replace it with:

1. Extract session cookie from request using `axum_extra::extract::cookie::CookieJar`
2. Look up `admin_session:<session_id>` in Redis
3. If found, reset TTL (sliding window) and proceed
4. If not found or no cookie, return 401

**Login flow:**
```
POST /v1/admin/auth/login
Body: { "username": "admin", "password": "plaintext" }
1. Compare username against config.admin.username
2. Verify password against config.admin.password_hash using argon2::PasswordVerifier
3. Generate 32-byte random session ID
4. HSET admin_session:<hex_id> username <username> created_at <timestamp>
5. EXPIRE admin_session:<hex_id> 86400
6. Set-Cookie: session=<hex_id>; HttpOnly; Secure; SameSite=None; Path=/; Max-Age=86400
7. Return 200 { "username": "admin" }
```

**Logout flow:**
```
POST /v1/admin/auth/logout
1. Extract session cookie
2. DEL admin_session:<session_id> from Redis
3. Clear cookie (Set-Cookie with Max-Age=0)
4. Return 200
```

**Refresh flow:**
```
POST /v1/admin/auth/refresh
1. Extract session cookie
2. EXPIRE admin_session:<session_id> 86400 (reset TTL)
3. Return 200
```

**CORS configuration:**
```rust
// In main.rs, add CorsLayer to the HTTP app
use tower_http::cors::{CorsLayer, AllowOrigin};

let cors = CorsLayer::new()
    .allow_origin(AllowOrigin::exact(
        config.admin.cors_origin.parse().unwrap()
    ))
    .allow_methods([Method::GET, Method::POST, Method::PATCH, Method::DELETE])
    .allow_headers([CONTENT_TYPE, COOKIE])
    .allow_credentials(true);
```

**Important:** `allow_credentials(true)` is required for cross-origin cookie sending. This also requires a specific origin (not wildcard), which aligns with D-08.

### Backend: Config Changes

Extend `AdminConfig` in `config.rs`:
```rust
pub struct AdminConfig {
    pub username: Option<String>,
    pub password_hash: Option<String>,
    pub cors_origin: Option<String>,
    #[serde(default = "default_session_ttl")]
    pub session_ttl_secs: u64, // default 86400 (24h)
}
```

The `token` field is removed. When `username` and `password_hash` are both None, admin endpoints are unauthenticated (dev mode) -- preserving existing dev behavior.

### Backend: Auth Endpoint Registration

New auth routes go OUTSIDE the admin_auth_middleware layer (login must be unauthenticated):
```rust
// Auth routes (no auth required)
let auth_routes = Router::new()
    .route("/v1/admin/auth/login", post(auth::login))
    .route("/v1/admin/auth/logout", post(auth::logout))
    .route("/v1/admin/auth/refresh", post(auth::refresh));

// Protected admin routes (existing, with new cookie middleware)
let admin_routes = Router::new()
    // ... existing routes ...
    .layer(from_fn_with_state(state.clone(), session_auth_middleware));

let app = Router::new()
    .merge(api_routes)
    .merge(auth_routes)      // unauthenticated
    .merge(admin_routes)     // session-protected
    .layer(cors)             // CORS on all routes
    .with_state(state);
```

### Frontend: Recommended Project Structure

```
xgent-admin/
├── src/
│   ├── routes/
│   │   ├── __root.tsx            # Root layout (providers, global error boundary)
│   │   ├── _authenticated.tsx    # Auth guard layout (sidebar + header)
│   │   ├── _authenticated/
│   │   │   ├── index.tsx         # Dashboard (placeholder)
│   │   │   ├── services.tsx      # Services (placeholder)
│   │   │   ├── tasks.tsx         # Tasks (placeholder)
│   │   │   └── credentials.tsx   # Credentials (placeholder)
│   │   └── login.tsx             # Login page (split layout)
│   ├── components/
│   │   ├── ui/                   # shadcn/ui components (auto-generated)
│   │   ├── app-sidebar.tsx       # Sidebar with nav items
│   │   ├── auto-refresh.tsx      # Header auto-refresh control
│   │   ├── theme-toggle.tsx      # Dark mode toggle
│   │   ├── page-skeleton.tsx     # Reusable skeleton wrapper
│   │   ├── error-alert.tsx       # Reusable error state with retry
│   │   └── empty-state.tsx       # Reusable empty state
│   ├── lib/
│   │   ├── api.ts                # Fetch wrapper with credentials: 'include'
│   │   ├── auth.ts               # Auth query/mutation hooks
│   │   └── utils.ts              # cn() utility from shadcn/ui
│   ├── hooks/
│   │   └── use-auto-refresh.ts   # Auto-refresh interval context/hook
│   ├── routeTree.gen.ts          # Auto-generated by TanStack Router plugin
│   ├── main.tsx                  # Entry point
│   └── index.css                 # Tailwind imports + CSS variables for dark mode
├── .env                          # VITE_API_URL=http://localhost:8080
├── vite.config.ts
├── tsconfig.json
├── components.json               # shadcn/ui config
└── package.json
```

### Frontend: Auth Guard Pattern (TanStack Router)

The `_authenticated.tsx` layout route uses `beforeLoad` to check auth state:

```typescript
// src/routes/_authenticated.tsx
import { createFileRoute, redirect, Outlet } from '@tanstack/react-router'

export const Route = createFileRoute('/_authenticated')({
  beforeLoad: async ({ context, location }) => {
    // Check if user is authenticated (via TanStack Query cache or /auth/me endpoint)
    if (!context.auth.isAuthenticated) {
      throw redirect({
        to: '/login',
        search: { redirect: location.href },
      })
    }
  },
  component: AuthenticatedLayout,
})

function AuthenticatedLayout() {
  return (
    <SidebarProvider>
      <AppSidebar />
      <main>
        <Header /> {/* auto-refresh control, dark mode toggle */}
        <Outlet />
      </main>
    </SidebarProvider>
  )
}
```

### Frontend: API Client Pattern

```typescript
// src/lib/api.ts
const API_URL = import.meta.env.VITE_API_URL

export async function apiClient<T>(
  path: string,
  options?: RequestInit,
): Promise<T> {
  const response = await fetch(`${API_URL}${path}`, {
    ...options,
    credentials: 'include', // Send cookies cross-origin
    headers: {
      'Content-Type': 'application/json',
      ...options?.headers,
    },
  })

  if (response.status === 401) {
    // Session expired -- will be caught by auth guard
    throw new AuthError('Unauthorized')
  }

  if (!response.ok) {
    throw new ApiError(response.status, await response.text())
  }

  return response.json()
}
```

### Frontend: Auto-Refresh Pattern

```typescript
// Use React context for global auto-refresh state
// TanStack Query's refetchInterval option reads from this context

const { refetchInterval } = useAutoRefresh() // 5000 | 15000 | 30000 | false

useQuery({
  queryKey: ['services'],
  queryFn: fetchServices,
  refetchInterval, // From global context
})
```

### Frontend: Dark Mode Pattern

Use Tailwind's `class` strategy (add/remove `dark` class on `<html>` element):

```typescript
// On mount, check localStorage. Default to dark (D-23).
const theme = localStorage.getItem('theme') ?? 'dark'
document.documentElement.classList.toggle('dark', theme === 'dark')
```

shadcn/ui components already use `dark:` variants throughout. No additional configuration needed beyond ensuring the `dark` class is on `<html>`.

### Anti-Patterns to Avoid

- **Storing session token in localStorage on frontend:** The session is an HttpOnly cookie -- the frontend never sees it. Do not extract it or store it anywhere.
- **Using `SameSite=Strict` with cross-origin:** `Strict` prevents cookies from being sent on cross-origin requests. Must be `None` since SPA and gateway are on different origins.
- **Putting login route inside auth guard:** The `/v1/admin/auth/login` endpoint and `/login` frontend route must both be accessible without authentication.
- **Using wildcard CORS origin with credentials:** Browsers reject `Access-Control-Allow-Origin: *` when `credentials: include` is used. Must be a specific origin.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Cookie parsing/setting | Manual header manipulation | axum-extra `CookieJar` | Cookie encoding, attribute handling, security flags are subtle |
| Password hash verification | Custom argon2 implementation | `argon2` + `password-hash` crates | PHC string format parsing, constant-time comparison, algorithm parameters |
| CORS headers | Manual header injection | tower-http `CorsLayer` | Preflight requests, credential handling, header allowlists are complex |
| Component styling system | Custom CSS framework | shadcn/ui + Tailwind | Accessible, consistent, dark-mode-ready components |
| Route code splitting | Manual dynamic imports | @tanstack/router-plugin `autoCodeSplitting` | Automatic per-route code splitting via Vite plugin |
| Toast notifications | Custom notification system | Sonner | Positioning, animation, stacking, auto-dismiss timing |
| Sidebar collapse | Custom sidebar | shadcn/ui Sidebar component | `SidebarProvider`, `useSidebar` hook, mobile responsive, collapsible modes already built |

**Key insight:** shadcn/ui provides a complete Sidebar component with collapsible behavior, mobile responsiveness, and state management via `useSidebar()` hook. Do not build a custom sidebar -- install it with `npx shadcn@latest add sidebar`.

## Common Pitfalls

### Pitfall 1: CORS Preflight Failures with Credentials
**What goes wrong:** Browser sends OPTIONS preflight request, gateway returns 401 because admin auth middleware intercepts it.
**Why it happens:** Preflight requests don't carry cookies. If the CORS layer is inside the auth middleware layer, preflights fail.
**How to avoid:** Apply CORS layer at the outermost level of the router (after all route merges), not within any middleware-protected group. The `CorsLayer` handles OPTIONS automatically.
**Warning signs:** Browser console shows "CORS error" on every request, even though credentials are correct.

### Pitfall 2: Cookie Not Sent Cross-Origin
**What goes wrong:** Login succeeds but subsequent requests return 401. Cookie is set but not included in requests.
**Why it happens:** Missing `credentials: 'include'` in fetch calls, or `SameSite` attribute is wrong, or CORS `allow_credentials` is false.
**How to avoid:** Always use `credentials: 'include'` in the API client. Set `SameSite=None; Secure` on the cookie. Set `allow_credentials(true)` on CorsLayer. During dev, ensure HTTPS or localhost (browsers may relax Secure for localhost).
**Warning signs:** Cookie visible in browser DevTools after login but not in request headers.

### Pitfall 3: Secure Cookie on localhost During Development
**What goes wrong:** Cookie with `Secure` flag is rejected by browser on `http://localhost`.
**Why it happens:** `Secure` flag requires HTTPS. While most modern browsers allow `Secure` cookies on `localhost`, behavior varies.
**How to avoid:** In development, conditionally set `Secure=false` when the gateway is running on localhost/HTTP. Use a config flag or detect the listen address scheme. Alternatively, rely on the fact that Chrome/Firefox/Safari all treat localhost as secure context.
**Warning signs:** Cookie doesn't appear in browser DevTools after login response.

### Pitfall 4: TanStack Router Plugin Order in Vite Config
**What goes wrong:** Route tree not generated, or routes not found at runtime.
**Why it happens:** `@tanstack/router-plugin/vite` must be listed BEFORE `@vitejs/plugin-react` in the Vite plugins array.
**How to avoid:** Always put TanStackRouter plugin first in vite.config.ts plugins array.
**Warning signs:** Missing `routeTree.gen.ts` file, or "route not found" errors.

### Pitfall 5: Sliding Window TTL Race Condition
**What goes wrong:** Multiple concurrent requests each try to EXPIRE the session, causing unnecessary Redis round-trips.
**Why it happens:** Every authenticated request resets the TTL per D-04.
**How to avoid:** This is acceptable for a single-admin gateway. The EXPIRE command is idempotent and cheap. Do not add complexity (e.g., only refresh if < 50% TTL remaining) -- it's premature optimization for a single-user system.
**Warning signs:** None -- this is a non-problem for this use case.

### Pitfall 6: shadcn/ui with Tailwind CSS v4
**What goes wrong:** shadcn/ui components don't style correctly, or `components.json` configuration fails.
**Why it happens:** Tailwind v4 changed from `tailwind.config.js` to CSS-first configuration. shadcn/ui CLI may need the `--css` flag or specific configuration.
**How to avoid:** Use `npx shadcn@latest init` which auto-detects Tailwind v4. Ensure the `@tailwindcss/vite` plugin is installed. Check that `src/index.css` has proper `@import "tailwindcss"` instead of `@tailwind` directives.
**Warning signs:** Components render without styles, or shadcn CLI errors about missing config.

## Code Examples

### Backend: Password Verification with argon2

```rust
// Source: RustCrypto password-hashes documentation
use argon2::Argon2;
use password_hash::{PasswordHash, PasswordVerifier};

fn verify_admin_password(password: &str, stored_hash: &str) -> bool {
    let parsed_hash = match PasswordHash::new(stored_hash) {
        Ok(h) => h,
        Err(_) => return false,
    };
    Argon2::default()
        .verify_password(password.as_bytes(), &parsed_hash)
        .is_ok()
}
```

### Backend: Session Creation with Cookie

```rust
// Source: axum-extra docs + axum cookie discussion
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};

async fn login(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> Result<(CookieJar, Json<LoginResponse>), StatusCode> {
    // ... validate credentials ...

    let session_id = generate_session_id(); // 32 random bytes, hex-encoded
    store_session_in_redis(&mut state.auth_conn.clone(), &session_id).await?;

    let cookie = Cookie::build(("session", session_id))
        .path("/")
        .http_only(true)
        .secure(true)
        .same_site(SameSite::None)
        .max_age(time::Duration::seconds(state.config.admin.session_ttl_secs as i64))
        .build();

    Ok((jar.add(cookie), Json(LoginResponse { username: req.username })))
}
```

### Backend: Session Auth Middleware

```rust
use axum_extra::extract::cookie::CookieJar;

pub async fn session_auth_middleware(
    State(state): State<Arc<AppState>>,
    jar: CookieJar,
    req: axum::extract::Request,
    next: axum::middleware::Next,
) -> Result<axum::response::Response, StatusCode> {
    // Dev mode: no credentials configured = pass through
    if state.config.admin.username.is_none() {
        return Ok(next.run(req).await);
    }

    let session_id = jar
        .get("session")
        .map(|c| c.value().to_string())
        .ok_or(StatusCode::UNAUTHORIZED)?;

    let key = format!("admin_session:{}", session_id);
    let exists: bool = redis::cmd("EXISTS")
        .arg(&key)
        .query_async(&mut state.auth_conn.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    if !exists {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Sliding window: reset TTL
    let ttl = state.config.admin.session_ttl_secs;
    let _: () = redis::cmd("EXPIRE")
        .arg(&key)
        .arg(ttl)
        .query_async(&mut state.auth_conn.clone())
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    Ok(next.run(req).await)
}
```

### Frontend: TanStack Query Auth Hooks

```typescript
// src/lib/auth.ts
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiClient } from './api'

export function useAuth() {
  return useQuery({
    queryKey: ['auth', 'session'],
    queryFn: () => apiClient('/v1/admin/auth/refresh', { method: 'POST' }),
    retry: false,
    staleTime: 5 * 60 * 1000, // 5 minutes
  })
}

export function useLogin() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (data: { username: string; password: string }) =>
      apiClient('/v1/admin/auth/login', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['auth'] })
    },
  })
}

export function useLogout() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: () =>
      apiClient('/v1/admin/auth/logout', { method: 'POST' }),
    onSuccess: () => {
      queryClient.clear()
    },
  })
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Bearer token in header | HttpOnly cookie session | Decision D-02/D-03 | SPA cannot access token; server manages session lifecycle |
| tailwind.config.js | Tailwind CSS v4 CSS-first config | Tailwind v4 (2025) | No config file; use `@import "tailwindcss"` and CSS variables |
| React Router v6 | TanStack Router | 2024-2025 | Type-safe routes, file-based routing, `beforeLoad` guards |
| `@shadcn/ui` CLI | `shadcn` CLI (v4.x) | 2025 | CLI renamed, init detects framework automatically |
| Separate toast library | Sonner via shadcn/ui | 2024 | shadcn/ui officially integrates Sonner, replaces react-hot-toast |

**Deprecated/outdated:**
- `admin.token` config field: Replaced by `admin.username` + `admin.password_hash`
- `@tailwind base/components/utilities` directives: Replaced by `@import "tailwindcss"` in Tailwind v4
- `tailwind.config.js/ts`: Replaced by `@theme` directive in CSS for Tailwind v4

## Open Questions

1. **Generating admin password hash for gateway.toml**
   - What we know: Admin stores a PHC-format hash in `gateway.toml`. The gateway verifies against it.
   - What's unclear: How does the admin generate this hash initially? Need a CLI command or documentation.
   - Recommendation: Add a `xgent-gateway hash-password` subcommand using clap that outputs a PHC string. This is a small addition and prevents admins from needing external tools.

2. **Dev HTTPS for Secure cookies**
   - What we know: `Secure` cookies require HTTPS. Browsers generally treat localhost as a secure context.
   - What's unclear: Whether all target browsers consistently allow `Secure` cookies on `http://localhost:8080`.
   - Recommendation: Default `Secure=true` but make it configurable via `admin.cookie_secure` in gateway.toml. In dev, set to `false` if issues arise. Chrome, Firefox, and Safari all support Secure on localhost as of 2025.

3. **SPA repository location**
   - What we know: D-10 says separate repository. Phase 8 needs to create this project.
   - What's unclear: Where exactly to create it. Presumably adjacent to the gateway repo.
   - Recommendation: Create the SPA as a new directory within the same workspace temporarily, or document that the planner should scaffold the project structure and the user moves it to a separate repo. For planning purposes, scaffold it in a `admin-ui/` directory at the repo root and note it should be extracted to a separate repo for production.

## Validation Architecture

### Test Framework (Backend)

| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | gateway/Cargo.toml (dev-dependencies section) |
| Quick run command | `cd gateway && cargo test --lib` |
| Full suite command | `cd gateway && cargo test` |

### Test Framework (Frontend)

| Property | Value |
|----------|-------|
| Framework | Vitest (recommended with Vite) |
| Config file | To be created in Wave 0 |
| Quick run command | `cd admin-ui && npx vitest run --reporter=verbose` |
| Full suite command | `cd admin-ui && npx vitest run` |

### Phase Requirements -> Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AUTH-01 | Login with valid/invalid credentials | integration | `cd gateway && cargo test auth_login` | No -- Wave 0 |
| AUTH-02 | Session cookie set correctly | integration | `cd gateway && cargo test session_cookie` | No -- Wave 0 |
| AUTH-03 | Session TTL refreshed on request | unit | `cd gateway && cargo test session_refresh` | No -- Wave 0 |
| AUTH-04 | Logout clears session | integration | `cd gateway && cargo test auth_logout` | No -- Wave 0 |
| API-01 | POST /v1/admin/auth/login returns cookie | integration | `cd gateway && cargo test login_endpoint` | No -- Wave 0 |
| API-02 | POST /v1/admin/auth/refresh resets TTL | integration | `cd gateway && cargo test refresh_endpoint` | No -- Wave 0 |
| UI-01 | Skeleton/error/empty states render | unit (frontend) | `cd admin-ui && npx vitest run --reporter=verbose` | No -- Wave 0 |
| UI-02 | Toast appears on mutation | manual-only | Visual verification during dev | N/A |
| UI-03 | Dark mode toggles and persists | manual-only | Visual verification | N/A |
| UI-04 | Auto-refresh triggers at interval | unit (frontend) | `cd admin-ui && npx vitest run` | No -- Wave 0 |
| UI-05 | Layout responds at 1280px+ | manual-only | Visual verification | N/A |

### Sampling Rate
- **Per task commit:** `cd gateway && cargo test --lib` (backend) or `cd admin-ui && npx vitest run` (frontend)
- **Per wave merge:** `cd gateway && cargo test` (full backend suite)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] Backend auth integration test file (`gateway/tests/admin_auth_test.rs`)
- [ ] Frontend project with Vitest configured
- [ ] Frontend test for auth hooks (`admin-ui/src/lib/__tests__/auth.test.ts`)
- [ ] Frontend test for auto-refresh hook (`admin-ui/src/hooks/__tests__/use-auto-refresh.test.ts`)

## Sources

### Primary (HIGH confidence)
- [axum-extra on crates.io](https://crates.io/crates/axum-extra) - version 0.12.5 verified via `cargo search`
- [TanStack Router official docs - Installation with Vite](https://tanstack.com/router/latest/docs/installation/with-vite) - file-based routing setup
- [TanStack Router - Authenticated Routes](https://tanstack.com/router/latest/docs/guide/authenticated-routes) - beforeLoad auth guard pattern
- [shadcn/ui - Vite installation](https://ui.shadcn.com/docs/installation/vite) - official setup guide
- [shadcn/ui - Sidebar component](https://ui.shadcn.com/docs/components/radix/sidebar) - collapsible sidebar with useSidebar hook
- [RustCrypto password-hashes](https://github.com/RustCrypto/password-hashes) - argon2 + password-hash crate ecosystem
- npm registry (verified via `npm view`): vite 8.0.1, react 19.2.4, @tanstack/react-router 1.168.2, @tanstack/react-query 5.94.5, sonner 2.0.7, tailwindcss 4.2.2, shadcn 4.1.0

### Secondary (MEDIUM confidence)
- [axum-extra cookie extraction](https://docs.rs/axum-extra/latest/axum_extra/extract/cookie/index.html) - CookieJar API
- [tower-http CORS](https://docs.rs/tower-http/latest/tower_http/cors/index.html) - CorsLayer configuration
- [Axum cookie discussion #2546](https://github.com/tokio-rs/axum/discussions/2546) - HttpOnly/Secure/SameSite cookie setup patterns

### Tertiary (LOW confidence)
- shadcn/ui + Tailwind v4 compatibility: Multiple community reports confirm it works but official shadcn docs may lag behind Tailwind v4 changes. Test during scaffolding.

## Metadata

**Confidence breakdown:**
- Standard stack (backend): HIGH - axum-extra, argon2 are well-established. Versions verified via cargo search.
- Standard stack (frontend): HIGH - All versions verified via npm view. TanStack ecosystem is mature.
- Architecture (backend): HIGH - Existing codebase patterns are clear. Session middleware follows established admin_auth_middleware pattern.
- Architecture (frontend): MEDIUM - TanStack Router file-based routing is well-documented but the exact file structure is a recommendation, not verified from a production reference.
- Pitfalls: HIGH - CORS + credentials + SameSite interaction is a well-documented pain point. Cookie on localhost is a known dev issue.

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (30 days -- stable ecosystem, no imminent breaking changes expected)
