# Feature Research: Admin Web UI for xgent-ai-gateway

**Domain:** Infrastructure gateway admin dashboard (task queue management + monitoring)
**Researched:** 2026-03-22
**Confidence:** HIGH (existing API is well-understood; UI patterns are mature domain)

## Existing Backend API Surface

Before defining UI features, here is what the gateway already exposes that the UI will consume:

| Endpoint | Method | Purpose | UI-Ready? |
|----------|--------|---------|-----------|
| `POST /v1/admin/api-keys` | POST | Create API key | YES |
| `POST /v1/admin/api-keys/revoke` | POST | Revoke API key | YES |
| `PATCH /v1/admin/api-keys/{key_hash}` | PATCH | Update callback URL | YES |
| `POST /v1/admin/node-tokens` | POST | Create node token | YES |
| `POST /v1/admin/node-tokens/revoke` | POST | Revoke node token | YES |
| `POST /v1/admin/services` | POST | Register service | YES |
| `GET /v1/admin/services` | GET | List services | YES |
| `GET /v1/admin/services/{name}` | GET | Service detail + nodes | YES |
| `DELETE /v1/admin/services/{name}` | DELETE | Deregister service | YES |
| `GET /v1/admin/health` | GET | Per-service node health | YES |
| `GET /metrics` | GET | Prometheus exposition format | YES (needs client-side parsing) |
| `GET /v1/tasks/{task_id}` | GET | Single task status | Partial (API-key protected, not admin-scoped) |

### Backend Gaps (New Endpoints Needed for UI)

| Missing Endpoint | Why Needed | Complexity |
|-----------------|-----------|------------|
| `GET /v1/admin/api-keys` -- List API keys | UI needs to show existing keys (hash + services, never raw key) | LOW |
| `GET /v1/admin/node-tokens?service=` -- List node tokens | UI needs to show existing tokens per service (hash + label, never raw token) | LOW |
| `GET /v1/admin/tasks?service=&status=&page=&limit=` -- List tasks | Task management page needs task listing by service/status with pagination | MEDIUM |
| `POST /v1/admin/tasks/{task_id}/cancel` -- Cancel task | Mark task failed, store "cancelled" result so polling clients get notified | MEDIUM |
| `POST /v1/admin/auth/login` -- Admin login | Session-based auth for UI (current Bearer token is static config value) | MEDIUM |
| `POST /v1/admin/auth/refresh` -- Token refresh | JWT refresh for persistent sessions via httpOnly cookie | LOW |
| `PATCH /v1/admin/services/{name}` -- Update service config | Edit timeout, max_nodes, etc. without delete/recreate cycle | LOW |

## Feature Landscape

### Table Stakes (Users Expect These)

Features any admin of an infrastructure gateway expects from a web UI. Missing these makes the UI feel like a demo, not a tool.

| Feature | Why Expected | Complexity | Depends On (API) |
|---------|--------------|------------|------------------|
| **Admin login page** | Cannot ship admin UI without auth. Every infrastructure tool has a login gate. Celery Flower has basic auth, Temporal UI defers to proxy auth. | MEDIUM | New: login/session endpoints on backend |
| **Dashboard overview** | First thing admins see. Must answer "is my gateway healthy?" in under 3 seconds. Show total services, active nodes, aggregate queue depth, task throughput counters. | MEDIUM | Existing: `/v1/admin/health` + `/metrics` |
| **Service list page** | CRUD for services is the primary admin action. List with status indicators (healthy/degraded/no-nodes). | LOW | Existing: `GET /v1/admin/services` |
| **Service detail page** | Drill into a service: config values, connected nodes with health badges, queue depth. | LOW | Existing: `GET /v1/admin/services/{name}` |
| **Service create/delete** | Register new services, deregister old ones. Form with config fields (timeout, max_nodes, etc.) + confirmation dialog for destructive delete. | LOW | Existing: `POST/DELETE /v1/admin/services` |
| **Node list per service** | See which nodes are connected, their health state (healthy/stale/disconnected), in-flight task count, drain status. | LOW | Existing: embedded in service detail response |
| **Task list page** | View tasks by service, filterable by status (pending/assigned/running/completed/failed). Paginated. This is the most-used page in Celery Flower and Bull Board. | MEDIUM | **New: `GET /v1/admin/tasks` with pagination/filter** |
| **Task detail view** | See task metadata, timestamps (created, assigned, completed), assigned node, current status, result payload (if completed/failed). | LOW | Existing: `GET /v1/tasks/{task_id}` (needs admin-accessible variant) |
| **Task cancellation** | Cancel a pending/running task. Returns failed result to polling client. Confirmation dialog required -- destructive action. | MEDIUM | **New: `POST /v1/admin/tasks/{task_id}/cancel`** |
| **API key management** | List existing keys (masked hash only), create new ones (show raw key once with copy button), revoke keys. | LOW | Existing: create/revoke. **New: list endpoint** |
| **Node token management** | List tokens per service (masked hash + label), create new ones (show raw token once), revoke tokens. | LOW | Existing: create/revoke. **New: list endpoint** |
| **Responsive layout** | Must work on laptop screens (1280px+). Mobile not required for infrastructure admin tools, but 1024px tablet should not break. | LOW | Frontend only |
| **Loading and error states** | Skeleton loaders, error banners with retry action, empty states with guidance. Infrastructure UIs that show blank screens or raw error JSON are unusable. | LOW | Frontend only |
| **Toast notifications** | Success/failure feedback on mutations (create service, revoke key, cancel task). Standard in every modern admin panel. | LOW | Frontend only |

### Differentiators (Competitive Advantage)

Features that make this admin UI notably better than Celery Flower, Bull Board, or basic CRUD admin panels. Not required for v1.1 launch but high value-to-effort ratio.

| Feature | Value Proposition | Complexity | Depends On |
|---------|-------------------|------------|------------|
| **Live metrics charts** | Visualize task throughput rate, queue depth over time, and active node count using existing Prometheus `/metrics` endpoint. Recharts line/area charts polling every 10-15s. Vastly better than raw counter numbers. Neither Flower nor Bull Board has built-in charts. | MEDIUM | Existing `/metrics`; frontend charting only |
| **Task latency percentile display** | Show p50/p95/p99 from `gateway_task_duration_seconds` histogram. Answers "how fast is my gateway?" without needing Grafana. | LOW | Existing histogram metric; parse Prometheus exposition format client-side |
| **Service health badges on dashboard** | Color-coded service cards: green (all nodes healthy), yellow (some stale), red (no active nodes). At-a-glance cluster health without drilling into each service. | LOW | Existing: `/v1/admin/health` provides all needed data |
| **Copy-to-clipboard for tokens/keys** | One-click copy for newly created API keys and node tokens. Critical UX because keys are shown exactly once and can never be retrieved again. | LOW | Frontend only |
| **Dark mode** | Infrastructure tools are used during incident response at odd hours. Dark mode reduces eye strain. shadcn/ui supports this natively via Tailwind dark variant. | LOW | Frontend only (CSS toggle) |
| **Service config editing** | Edit task_timeout, max_nodes, node_stale_after inline without delete/recreate cycle. Flower does not support this; Temporal UI does. | LOW | **New: PATCH endpoint for service config** |
| **Auto-refresh with pause** | Metrics and task lists auto-refresh on configurable interval with a visible pause/resume toggle. Common in monitoring UIs (Grafana, Datadog). Pausing is essential when inspecting a specific task. | LOW | Frontend only (TanStack Query refetchInterval) |
| **Keyboard shortcuts** | Quick navigation: `g d` for dashboard, `g s` for services, `g t` for tasks. Power users managing infrastructure expect keyboard-driven workflows. | LOW | Frontend only |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem useful but should NOT be built for this admin UI.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Embedded Grafana iframe** | "We already use Grafana for metrics" | Adds auth complexity (Grafana session + gateway session), CORS/CSP issues, mixed content on HTTPS, iframe UIs feel janky and break on resize. Two auth systems to maintain. | Parse `/metrics` directly in React with Recharts. Provide an exportable Grafana dashboard JSON for users who want Grafana alongside. |
| **Log viewer in admin UI** | "I want to see gateway logs in the UI" | Explicitly descoped in PROJECT.md. Logs are high-volume streaming data needing a separate backend (Loki, CloudWatch, ELK). Building log aggregation is a separate product. | Link to external log tool from the UI. Document structured log format for easy Loki/ELK integration. |
| **Real-time WebSocket updates** | "Tasks should update live without polling" | Explicitly descoped in PROJECT.md ("poll + callback covers all practical use cases"). WebSocket adds server-side connection management, reconnection logic, and a new protocol to maintain. | TanStack Query with `refetchInterval` (5-10s polling). Simple, reliable, zero backend work. Sufficient for admin UI refresh rates. |
| **Task payload editor / resubmit** | "Let me edit and retry failed tasks" | Gateway treats payloads as opaque bytes (core design constraint). Editing opaque bytes in a UI is meaningless. Retry is explicitly not supported -- clients resubmit on failure. | Show payload as base64/hex read-only dump with copy button. Link to docs on client-side resubmission pattern. |
| **Role-based access control** | "Different admins should see different things" | Single-admin gateway does not need RBAC. User management, role definitions, permission matrices -- massive complexity for zero value at current scale. | Single admin account with full access. Revisit only if multi-tenant admin requirements emerge. |
| **Multi-gateway federation view** | "Manage all my gateways from one UI" | Each gateway is independent by design. Cross-gateway coordination requires a meta-service. Different product. | Each gateway runs its own admin UI. Bookmark multiple instances behind a reverse proxy. |
| **Editable Prometheus alert rules** | "Set alerting thresholds from the UI" | Alert management is Alertmanager's job. Reimplementing it poorly doubles maintenance. | Document Alertmanager setup with gateway metrics. Ship example alert rules in the repo. |
| **Task scheduling / cron** | "Schedule recurring tasks from the admin UI" | Explicitly out of scope in PROJECT.md. External schedulers submit to gateway. | Document integration with cron, Kubernetes CronJobs, or Temporal for scheduled task submission. |

## Feature Dependencies

```
[Admin Login / Session Auth]
    └──requires──> [Backend: login + JWT/session endpoints]
                       └──enables──> [All other admin pages]

[Dashboard Overview]
    └──requires──> [Service List data] (aggregated view of services)
    └──requires──> [Metrics Parsing utility] (chart data from /metrics)

[Service Detail Page]
    └──requires──> [Service List] (navigation: click service in list)
    └──contains──> [Node List per Service]

[Task List Page]
    └──requires──> [Backend: paginated task list endpoint]
    └──enables──> [Task Detail View]
    └──enables──> [Task Cancellation]

[Task Cancellation]
    └──requires──> [Backend: cancel task endpoint]
    └──requires──> [Task Detail View] (cancel button lives on detail page)

[API Key Management]
    └──requires──> [Backend: list API keys endpoint]

[Node Token Management]
    └──requires──> [Backend: list node tokens endpoint]

[Live Metrics Charts]
    └──requires──> [Dashboard Overview] (charts are embedded in dashboard)
    └──requires──> [Metrics Parsing utility] (shared with dashboard counters)

[Dark Mode] ──independent──> [All pages] (CSS-only toggle, no dependencies)

[Copy-to-Clipboard] ──independent──> [API Key + Node Token create dialogs]
```

### Dependency Notes

- **Admin Login requires backend session endpoints:** The current admin auth is a static Bearer token from `gateway.toml`. The UI needs a login flow: POST credentials, receive short-lived JWT (in-memory) + long-lived refresh token (httpOnly cookie). This is the critical-path backend work that must be built first.
- **Task List requires new backend endpoint:** No task listing endpoint exists today. `GET /v1/admin/tasks?service=X&status=pending&page=1&limit=50` requires scanning Redis streams with filtering. MEDIUM complexity due to Redis stream pagination semantics (XRANGE with cursor-based paging).
- **Task Cancellation requires new backend endpoint:** No cancel endpoint exists. Cancelling means updating the task state machine to "failed" with reason "cancelled", storing a result so polling clients are notified. MEDIUM complexity due to state machine edge cases (what if task is already completed?).
- **API Key / Node Token listing requires new endpoints:** Currently create and revoke exist but not list. LOW complexity -- SCAN Redis for `apikey:*` and `nodetoken:*` patterns, return hash + associated services/labels.
- **Metrics parsing is frontend-only:** The `/metrics` endpoint already emits Prometheus text exposition format. The UI parses this text into numbers/series for charts. No backend changes needed. A lightweight parser or the `prom-client` parse utility handles this.

## MVP Definition

### Launch With (v1.1)

Minimum viable admin UI -- what makes it worth deploying over curl commands.

- [ ] **Admin login page** -- Gate all admin functionality behind authentication
- [ ] **Dashboard overview** -- Service count, total active nodes, aggregate queue depth, task throughput counters (numeric cards, not charts yet)
- [ ] **Service list + detail pages** -- View all services, drill into config + node list
- [ ] **Service create/delete** -- Register and deregister services via forms with validation
- [ ] **Node health view** -- Per-service node list with health badges (healthy/stale/disconnected)
- [ ] **Task list page** -- Paginated task list filtered by service and status
- [ ] **Task detail + cancel** -- View task details and metadata, cancel pending/running tasks
- [ ] **API key management** -- List (masked), create (show-once), revoke
- [ ] **Node token management** -- List (masked), create (show-once), revoke
- [ ] **Loading/error/empty states** -- Skeleton loaders, error boundaries with retry, empty state guidance
- [ ] **Toast notifications** -- Success/failure feedback on all mutations

### Add After Validation (v1.1.x)

Features to add once core UI is stable and in daily use.

- [ ] **Live metrics charts** -- Recharts line charts for throughput rate, queue depth, and latency over time (polling /metrics every 10-15s)
- [ ] **Dark mode** -- Light/dark theme toggle persisted in localStorage
- [ ] **Service config editing** -- Inline edit of service parameters without delete/recreate
- [ ] **Auto-refresh with pause** -- Configurable poll interval (5s/15s/30s/off) with visible toggle
- [ ] **Copy-to-clipboard** -- For API keys, node tokens, and task IDs
- [ ] **Service health badges** -- Color-coded health indicators on dashboard overview cards

### Future Consideration (v2+)

- [ ] **Keyboard shortcuts** -- Power-user navigation (`g d`, `g s`, `g t`)
- [ ] **Task latency percentiles** -- p50/p95/p99 display parsed from histogram metrics
- [ ] **Node health timeline** -- Historical health state visualization (needs backend history tracking)
- [ ] **Exportable Grafana dashboard** -- JSON dashboard definition for Grafana users
- [ ] **Bulk task cancellation** -- Select multiple tasks and cancel in batch

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority | Backend Changes? |
|---------|------------|---------------------|----------|-----------------|
| Admin login | HIGH | MEDIUM | P1 | YES: login/session endpoints |
| Dashboard overview | HIGH | MEDIUM | P1 | NO: uses existing health + metrics |
| Service list + detail | HIGH | LOW | P1 | NO: fully covered by existing API |
| Service create/delete | HIGH | LOW | P1 | NO: fully covered by existing API |
| Node health view | HIGH | LOW | P1 | NO: embedded in service detail |
| Task list page | HIGH | MEDIUM | P1 | YES: paginated list endpoint |
| Task detail + cancel | HIGH | MEDIUM | P1 | YES: cancel endpoint + admin task access |
| API key management | MEDIUM | LOW | P1 | YES: list endpoint (trivial SCAN) |
| Node token management | MEDIUM | LOW | P1 | YES: list endpoint (trivial SCAN) |
| Loading/error states | HIGH | LOW | P1 | NO: frontend patterns only |
| Toast notifications | MEDIUM | LOW | P1 | NO: frontend patterns only |
| Live metrics charts | HIGH | MEDIUM | P2 | NO: parses existing /metrics |
| Dark mode | MEDIUM | LOW | P2 | NO: CSS toggle |
| Service config editing | MEDIUM | LOW | P2 | YES: PATCH endpoint |
| Auto-refresh w/ pause | MEDIUM | LOW | P2 | NO: TanStack Query config |
| Copy-to-clipboard | MEDIUM | LOW | P2 | NO: frontend utility |
| Service health badges | MEDIUM | LOW | P2 | NO: derived from existing data |
| Keyboard shortcuts | LOW | LOW | P3 | NO |
| Task latency percentiles | MEDIUM | LOW | P3 | NO |
| Node health timeline | LOW | HIGH | P3 | YES: backend history tracking |

**Priority key:**
- P1: Must have for v1.1 launch
- P2: Should have, add in v1.1.x patches
- P3: Nice to have, future consideration

## Competitor Feature Analysis

| Feature | Celery Flower | Bull Board | Temporal UI | Our Approach |
|---------|---------------|------------|-------------|--------------|
| Task list + status filter | YES: real-time task list with state tabs | YES: per-queue job list with status filters | YES: workflow execution list with filters | Paginated list filtered by service + status |
| Task detail view | YES: args, result, traceback, timestamps | YES: job data, logs, progress bar | YES: detailed execution history + events | Task metadata, timestamps, assigned node, result payload |
| Task cancellation | YES: revoke/terminate workers | YES: remove/retry individual jobs | YES: cancel/terminate running workflows | Cancel marks as failed with reason, result returned to client |
| Worker/node monitoring | YES: worker list with active task count | NO: queue-focused, no worker view | YES: worker identity + task slots + pollers | Node list per service with health badges + in-flight count |
| Built-in metrics/charts | LIMITED: basic task counters, no time-series | NO: no built-in metrics visualization | YES: latency histograms, execution counts | Parse Prometheus /metrics, Recharts time-series charts |
| Service/queue management | NO: Celery config manages queues | YES: pause/resume queues | YES: namespace management + search attributes | Full CRUD for service registration with config |
| Authentication | Basic HTTP auth or --auth flag | None (intended for local access) | None by default (proxy auth recommended) | JWT session auth with dedicated login page |
| Dark mode | NO | NO | YES (built-in toggle) | YES: shadcn/ui native dark mode support |
| Tech stack | Python/Tornado (server-rendered) | React embedded panel | React SPA with gRPC-web | React + Vite + shadcn/ui + TanStack |

**Key takeaway:** Celery Flower and Bull Board are the closest analogues -- simple, focused admin UIs. Temporal UI is more sophisticated but targets workflow orchestration. Our UI should match Flower-level completeness (task list, node monitoring, cancel) with modern design (React + shadcn/ui) and better metrics visualization (built-in charts from Prometheus data). The gap we fill: neither Flower nor Bull Board has built-in time-series charting, and neither has proper service management CRUD.

## Backend Work Summary

New endpoints the UI requires, in build order based on dependencies:

| # | Endpoint | Complexity | Blocks |
|---|----------|-----------|--------|
| 1 | `POST /v1/admin/auth/login` | MEDIUM | All authenticated UI pages |
| 2 | `POST /v1/admin/auth/refresh` | LOW | Session persistence |
| 3 | `GET /v1/admin/api-keys` | LOW | API key management page |
| 4 | `GET /v1/admin/node-tokens?service=` | LOW | Node token management page |
| 5 | `GET /v1/admin/tasks?service=&status=&page=&limit=` | MEDIUM | Task list page |
| 6 | `POST /v1/admin/tasks/{task_id}/cancel` | MEDIUM | Task cancellation |
| 7 | `PATCH /v1/admin/services/{name}` | LOW | Service config editing (P2) |

**Total: 7 new endpoints. 3 MEDIUM, 4 LOW complexity. Items 1-6 are P1; item 7 is P2.**

## Sources

- Existing gateway codebase: `gateway/src/http/admin.rs` (all current admin endpoints), `gateway/src/metrics.rs` (8 Prometheus metrics), `gateway/src/main.rs` (route definitions)
- [Celery Flower -- task queue monitoring UI](https://github.com/mher/flower) -- feature reference for task list, worker monitoring, cancel
- [Prometheus 3.0 UI rewrite](https://promlabs.com/blog/2024/09/11/a-look-at-the-new-prometheus-3-0-ui/) -- modern React-based metrics UI patterns (Mantine framework)
- [JWT authentication best practices for SPAs](https://blog.logrocket.com/jwt-authentication-best-practices/) -- httpOnly cookie pattern, token refresh
- [JWT Storage: Local Storage vs Cookies](https://cybersierra.co/blog/react-jwt-storage-guide/) -- security tradeoffs for admin auth
- [React chart libraries 2026](https://www.syncfusion.com/blogs/post/top-5-react-chart-libraries) -- Recharts for lightweight SVG charting, uPlot for high-performance time-series
- [Admin Dashboard UX Best Practices 2025](https://medium.com/@CarlosSmith24/admin-dashboard-ui-ux-best-practices-for-2025-8bdc6090c57d) -- inverted pyramid layout, information hierarchy
- [Grafana dashboard best practices](https://grafana.com/docs/grafana/latest/visualizations/dashboards/build-dashboards/best-practices/) -- metrics visualization patterns

---
*Feature research for: xgent-ai-gateway Admin Web UI (v1.1 milestone)*
*Researched: 2026-03-22*
