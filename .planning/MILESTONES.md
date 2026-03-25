# Milestones

## v1.2 Flexible Agent Execution (Shipped: 2026-03-25)

**Phases completed:** 7 phases, 16 plans, 31 tasks

**Key accomplishments:**

- YAML agent config with env var interpolation, single-pass placeholder engine preventing injection, Executor trait with async_trait, and response body template resolver with max_bytes enforcement
- CliExecutor with arg/stdin modes, concurrent I/O deadlock prevention, timeout enforcement via SIGKILL, and exit code mapping through Executor trait
- Agent binary refactored from CLI-arg HTTP POST dispatch to YAML-config-driven Executor trait dispatch with CliExecutor wiring
- SyncApiExecutor with HTTP dispatch, connection retry, dot-notation JSON extraction, and configurable URL/method/headers/body templates
- SyncApiExecutor wired into agent binary with error-handling construction, dry-run sync-api display, and separate AsyncApi stub
- Extracted http_common module with shared JSON extraction and prefixed placeholder scanning, restructured ResponseSection into success/failed sub-sections with header fields, and wired failure-path body template resolution into CLI and sync-api executors
- AsyncApiExecutor with two-phase submit+poll lifecycle, condition-based completion/failure detection, timeout enforcement, and response template mapping
- Extended sample_service with /sync and /async endpoints, created example YAML configs and CLI echo script for all three agent execution modes
- Agent --dry-run validates command/URL accessibility, previews response templates with sample values, and prints pass/fail summary
- Three zero-dependency Node.js client scripts with tutorial READMEs covering all example directories
- Zero-warning clippy/check baseline via FromStr trait impl, Default impls, clamp(), and dead code removal across 8 files
- Replaced manual Redis SMEMBERS/HGETALL/derive_health_state calls in admin.rs and metrics.rs with canonical get_nodes_for_service, removing ~65 lines of duplicated code
- Deduplicated init_tracing from 4-arm match to 2 branches with shared file layer; standardized all admin handlers to return GatewayError
- Proto payload/result fields changed from bytes to string; Redis queue, executor, response resolver, and placeholder builder all use String types without base64
- HTTP handlers accept/return native JSON values, all executors produce String results, base64 encoding removed from HTTP layer
- Integration tests, auth tests, Node.js clients, and README updated from base64 to JSON payloads throughout

---

## v1.1 Admin Web UI (Shipped: 2026-03-23)

**Phases completed:** 5 phases, 12 plans, 28 tasks

**Key accomplishments:**

- Argon2 password-verified session auth with Redis-backed HttpOnly cookies, CORS, and login/logout/refresh endpoints replacing Bearer token admin middleware
- Vite + React 19 SPA with TanStack Router auth guards, shadcn/ui components, API client with HttpOnly cookie auth, and split-layout login page
- Collapsible sidebar navigation, dark/light theme, auto-refresh controls, and reusable UI pattern components (ErrorAlert, EmptyState, PageSkeleton)
- Service list page with card grid showing per-card health badges and node counts, registration dialog with all config fields, and shared hooks/types for service management
- Service detail page with config card, node health table, breadcrumb navigation, and deregister confirmation dialog
- Admin task list/detail/cancel endpoints with SCAN-based pagination, service/status filters, and state-validated cancel with XACK
- TanStack Query hooks for task CRUD with base64 payload decoder and colored status badges
- Task list page with filterable data table, slide-out detail sheet with payload viewer, and cancel confirmation dialog
- Backend list endpoints for API keys and node tokens with label/expiry fields and auth-time expiry enforcement
- Tabbed credential management page with API key/node token CRUD, one-time secret reveal dialog, and optimistic revoke
- In-memory ring buffer capturing Prometheus snapshots every 10s with two JSON endpoints for dashboard overview cards and time-series charts
- Operational dashboard with Recharts 3.x area charts, overview cards with trend arrows, and service health list replacing EmptyState stub

---

## v1.0 MVP (Shipped: 2026-03-22)

**Phases completed:** 7 phases, 20 plans, 41 tasks

**Key accomplishments:**

- Cargo workspace with proto codegen, Redis Streams queue layer, task state machine, and layered config
- gRPC TaskService and NodeService with server-streaming poll, HTTP REST endpoints, and dual-port server startup via Axum and Tonic
- Runner agent binary with gRPC streaming poll, HTTP task dispatch, exponential backoff reconnection, and integration test suite proving end-to-end submit-poll-report-retrieve lifecycle
- SHA-256 API key and node token auth modules with rustls TLS config builders for HTTP and gRPC mTLS
- TLS termination, API key auth middleware, node token validation, admin CRUD endpoints, and HTTP/2 keepalive wired into gateway servers
- 12 integration tests proving Phase 2 auth success criteria plus runner agent with Bearer token and TLS support
- Service registry with Redis-backed CRUD, admin HTTP endpoints, proto Heartbeat/DrainNode RPCs, and submit_task gating on registered services
- Node registry CRUD with Heartbeat/DrainNode RPCs and drain-aware poll loop using Redis hash-based node tracking
- 9 Redis integration tests proving service registry CRUD, node health, and drain lifecycle; runner agent SIGTERM handler with DrainNode RPC and in-flight task wait
- Background reaper with XPENDING IDLE scan for timed-out task detection, callback delivery with exponential backoff, and foundational config/state changes
- Callback URL support wired through API key creation, task submission with per-task override, admin PATCH endpoint, and delivery triggers from both report_result and reaper
- Prometheus metrics registry with 8 metric families, structured logging with JSON/text/file support, /metrics and /v1/admin/health endpoints behind admin auth
- All 8 Prometheus metrics wired into live code paths: task submission counters, completion histograms, poll latency, callback delivery, error counters, and 15-second background gauge refresh for queue depth and active nodes
- Multi-stage Dockerfile producing static musl binary on alpine:3.19, with jemalloc allocator and default gateway.toml configuration
- Fixed integration test compilation for 5-arg AppState::new and added structured success logging on HTTP/gRPC task submission
- Tower Service auth wrappers (ApiKeyAuthLayer, NodeTokenAuthLayer) enforcing API key and node token authentication on all gRPC RPCs with per-service authorization in handlers
- 13 integration tests proving gRPC auth enforcement on all RPCs -- negative tests verify Unauthenticated/PermissionDenied, positive tests confirm valid credentials accepted
- Proto callback_url/node_id fields, in_flight_tasks decrement on report_result, gRPC callback_url storage, agent X-Meta- header forwarding, plain HTTP keepalive
- Config-based mTLS cert fingerprint-to-service authorization in NodeTokenAuthLayer, plus full-loop reaper integration test that invokes reap_timed_out_tasks and verifies task state transitions
- Standalone echo service (hyper 1.x) on POST /execute with optional X-Meta-simulate_delay_ms delay, plus full tech debt audit closure across all 9 items

---
