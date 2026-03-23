# Phase 8: Frontend Foundation and Backend Auth - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Admin can log in to a working React app shell with established UI patterns for all subsequent pages. Scaffold the SPA project, implement auth endpoints (login/refresh/logout), implement the login flow, and establish reusable UI patterns (loading, error, empty states, toasts, auto-refresh, dark mode, responsive layout).

</domain>

<decisions>
## Implementation Decisions

### Admin credential source
- **D-01:** Admin credentials defined in `gateway.toml` — `admin.username` and `admin.password_hash` (bcrypt or argon2 hash). Single admin, no Redis credential storage.
- **D-02:** Existing `admin.token` config field is replaced by `admin.username` + `admin.password_hash` for the new session-based auth.

### Session mechanism
- **D-03:** HttpOnly cookie with `SameSite=None; Secure` for cross-origin auth (SPA hosted separately from gateway).
- **D-04:** Sessions stored in Redis with a 24-hour sliding window TTL. Each authenticated API request resets the TTL.
- **D-05:** Explicit `/v1/admin/auth/refresh` endpoint exists (API-02) alongside the sliding window — belt-and-suspenders approach.
- **D-06:** `/v1/admin/auth/login` returns the session cookie. `/v1/admin/auth/logout` deletes the session from Redis and clears the cookie.
- **D-07:** Session key format in Redis: `admin_session:<session_id>` with value containing username and created_at.

### CORS configuration
- **D-08:** Configurable allowed origin in `gateway.toml` via `admin.cors_origin` (e.g., `"https://admin.example.com"`). Single explicit origin, not wildcard (required for credentials).
- **D-09:** CORS must work in dev too — SPA uses `VITE_API_URL` env var pointing at gateway directly.

### SPA project structure
- **D-10:** SPA lives in a **separate repository**, fully decoupled from the gateway repo. Independent deploy cycles.
- **D-11:** Hosted on Aliyun OSS + CDN. Vanilla `vite build` output (`dist/` with `index.html` + assets).
- **D-12:** Dev workflow uses `VITE_API_URL=http://localhost:8080` — no Vite proxy, direct fetch to gateway.
- **D-13:** Tech stack: Vite + React 19 + TanStack Router + TanStack Query + shadcn/ui + Tailwind CSS.

### App shell and navigation
- **D-14:** Collapsible sidebar — icon + label, can minimize to icon-only mode.
- **D-15:** All 4 nav items stubbed from day one: Dashboard, Services, Tasks, Credentials. Clicking unbuilt pages shows a placeholder.
- **D-16:** Split layout login page — left side branding/project name, right side login form.
- **D-17:** TanStack Router auth guard redirects unauthenticated users to `/login`. After successful login, redirect back to the originally requested page.

### UI pattern foundations
- **D-18:** Loading skeletons use shadcn/ui `Skeleton` component. Each page creates its own skeleton layout matching its content shape.
- **D-19:** Error states are inline alert banners with error message + "Retry" button within the page content area. Page shell (sidebar, header) stays visible.
- **D-20:** Empty states show helpful guidance text within the content area.
- **D-21:** Toast notifications use Sonner (shadcn/ui integration).
- **D-22:** Auto-refresh is a global control in the header — interval picker (5s/15s/30s/off) + pause toggle. Applies to whichever page is active.
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

</decisions>

<specifics>
## Specific Ideas

- SPA is NOT served from Axum — this overrides the earlier STATE.md decision about ServeDir. Gateway is API-only; SPA is independently hosted.
- Login page should feel professional but not over-designed — split layout gives it presence without being a full marketing page.
- The collapsible sidebar should remember its collapsed/expanded state (localStorage).
- Auto-refresh control should visually indicate when it's active (e.g., a spinning icon or countdown).

</specifics>

<canonical_refs>
## Canonical References

### Requirements
- `.planning/REQUIREMENTS.md` — AUTH-01 through AUTH-04, API-01, API-02, UI-01 through UI-05

### Existing backend patterns
- `gateway/src/http/admin.rs` — Current admin auth middleware (Bearer token), admin endpoint patterns, response types
- `gateway/src/config.rs` — `AdminConfig` struct (currently `token: Option<String>`), config loading with TOML + env var overrides
- `gateway/src/state.rs` — `AppState` with Redis `MultiplexedConnection`, config, metrics
- `gateway/src/auth/api_key.rs` — SHA-256 hashing pattern, Redis key storage pattern, Axum middleware pattern
- `gateway/src/main.rs` lines 210-253 — Admin route registration with middleware layer

### Existing gateway config
- `gateway.toml` — Current `[admin]` section structure, TOML conventions used throughout

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `admin_auth_middleware` in `admin.rs`: Must be replaced with cookie-session-based middleware. The pattern (extract from request → validate → call next) is reusable.
- `api_key::hash_api_key` pattern: SHA-256 hashing with `sha2` crate — reusable for session ID generation.
- `AdminConfig` in `config.rs`: Needs extension from `{ token }` to `{ username, password_hash, cors_origin, session_ttl_secs }`.
- Redis `MultiplexedConnection` in `AppState`: Already available for session storage — no new connections needed.

### Established Patterns
- Axum middleware via `from_fn_with_state` — used for both API key auth and current admin auth.
- Redis hash storage pattern: `HSET key field value` for structured data (see `api_key.rs`).
- Config layering: TOML defaults → file → env vars (`GATEWAY__ADMIN__CORS_ORIGIN`).
- Response types: `Result<(StatusCode, Json<T>), GatewayError>` pattern throughout admin handlers.

### Integration Points
- `main.rs` admin_routes block: New auth endpoints (`/v1/admin/auth/login`, `/v1/admin/auth/refresh`, `/v1/admin/auth/logout`) added here.
- `admin_auth_middleware`: Replaced to check session cookie against Redis instead of Bearer token.
- CORS layer: Added to the HTTP app router (tower-http `CorsLayer`).
- `gateway.toml` `[admin]` section: Extended with new fields.

</code_context>

<deferred>
## Deferred Ideas

- Multiple admin accounts — out of scope per PROJECT.md (no RBAC)
- Serving SPA from Axum via ServeDir — explicitly rejected; SPA hosted independently on Aliyun OSS + CDN
- Admin password change from the UI — could be a future enhancement, currently config-file only

</deferred>

---

*Phase: 08-frontend-foundation-backend-auth*
*Context gathered: 2026-03-22*
