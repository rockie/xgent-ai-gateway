# Phase 5: Observability and Packaging - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

The gateway emits structured JSON logs, exposes Prometheus metrics, provides admin health data via API, and ships as a single static binary and Docker image ready for production deployment.

**In scope:**
- OBSV-01 (structured JSON logs with task ID, service, node context)
- OBSV-02 (Prometheus metrics endpoint: queue depth, task latency, node counts, error rates)
- OBSV-03 (admin health API: active nodes per service, last seen, in-flight tasks)
- INFR-03 (single static binary, musl target)
- INFR-04 (Docker image)

</domain>

<decisions>
## Implementation Decisions

### Structured logging
- **D-01:** Always-present fields: `timestamp`, `level`, `message`, `target` (Rust module path), plus contextual `task_id`/`service`/`node_id` when available. No request_id or span_id — minimal approach.
- **D-02:** Log level policy (conservative): ERROR = process-threatening (Redis disconnect, TLS failure), WARN = recoverable failures (callback delivery failed, auth rejected), INFO = lifecycle events (server start, task state transitions), DEBUG = per-request detail.
- **D-03:** JSON activation controlled via TOML config: `logging.format = "json" | "text"`. Default to `"text"` for dev ergonomics, production configs set `"json"`. Env var override via `GATEWAY__LOGGING__FORMAT`.
- **D-04:** Log output to stdout by default. Optional file output via `logging.file` config path for non-container deployments. When file is set, logs go to both stdout and file.

### Prometheus metrics
- **D-05:** Use the `prometheus` crate (official Prometheus client for Rust). `TextEncoder` for `/metrics` endpoint. Register metrics at startup, pass registry through `AppState`.
- **D-06:** Metrics surface (OBSV-02 + operational extras):
  - `gateway_queue_depth{service}` (gauge) — pending tasks per service
  - `gateway_task_duration_seconds{service,status}` (histogram) — time from submit to terminal state
  - `gateway_nodes_active{service}` (gauge) — healthy node count per service
  - `gateway_errors_total{service,type}` (counter) — auth failures, callback failures, reaper timeouts
  - `gateway_tasks_submitted_total{service,protocol}` (counter) — gRPC vs HTTP submission rate
  - `gateway_tasks_completed_total{service,status}` (counter) — completed vs failed breakdown
  - `gateway_callback_delivery_total{status}` (counter) — success/failure/exhausted
  - `gateway_node_poll_latency_seconds{service}` (histogram) — time from task queued to node pickup
- **D-07:** Exponential histogram buckets for both duration and latency: 0.1s, 0.25s, 0.5s, 1s, 2.5s, 5s, 10s, 30s, 60s, 120s, 300s. Single bucket config covers seconds-to-minutes AI inference workloads.
- **D-08:** `/metrics` endpoint on the HTTP listener, behind admin auth (existing `admin.token` guard). Prometheus scraper configured with bearer token.

### Admin health API (OBSV-03)
- **D-09:** `GET /v1/admin/health` endpoint returning JSON with per-service node health data: active nodes, last seen timestamps, in-flight task counts. Reuses existing `registry/node_health.rs` data. Behind admin auth like other admin endpoints.

### Static binary and Docker packaging
- **D-10:** jemalloc allocator via `tikv-jemallocator` crate with `#[global_allocator]`. Battle-tested for async Rust workloads under musl.
- **D-11:** Docker base image: `FROM alpine:3.19`. Includes CA certs, timezone data, and shell for debugging. Small attack surface (~7MB base).
- **D-12:** Ship a default `gateway.toml` embedded in the image at `/etc/xgent/gateway.toml`. Works out of the box with env var overrides (`GATEWAY__REDIS__URL`, etc.). Zero-config start for simple deployments, mount a custom config to override.
- **D-13:** Multi-stage Dockerfile with `FROM rust:latest AS builder`. Install musl target and build inside Docker. Copy binary to Alpine final stage. Self-contained — works in any CI without external toolchain setup.

### Claude's Discretion
- Exact `tracing-subscriber` layer composition for JSON + file output
- `prometheus` crate registry setup and metric registration pattern
- How to compute `queue_depth` gauge (Redis XLEN vs XPENDING count)
- How to compute `task_duration_seconds` (timestamp diff from task hash fields)
- `poll_latency` measurement approach (created_at vs claimed_at diff)
- Multi-stage Dockerfile exact structure (cargo-chef for layer caching vs plain build)
- `.dockerignore` contents
- OBSV-03 response JSON schema
- Whether jemalloc needs feature flags for musl compatibility
- Graceful handling of file logger errors (fallback to stdout-only)

</decisions>

<specifics>
## Specific Ideas

- `tracing-subscriber` already has the `json` feature enabled in Cargo.toml — just needs activation in the subscriber builder
- The `prometheus` crate is the most common choice in the Rust ecosystem and maps directly to Prometheus text format without intermediaries
- Admin auth guard already exists from Phase 3 — `/metrics` behind the same `admin.token` keeps the security model consistent
- Alpine gives a debugging shell (`docker exec -it ... sh`) which is valuable for a v1 where operators may need to inspect the running container
- Default embedded config means `docker run xgent-gateway` works immediately with just `GATEWAY__REDIS__URL` set — good developer experience
- jemalloc is what Redis itself uses — appropriate for a Redis-backed gateway

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/PROJECT.md` — Core constraints: Rust, dual protocol, Redis, static binary, Docker image
- `.planning/REQUIREMENTS.md` — Phase 5 covers OBSV-01, OBSV-02, OBSV-03, INFR-03, INFR-04
- `.planning/ROADMAP.md` — Phase 5 success criteria (4 items)

### Technology stack
- `CLAUDE.md` §Technology Stack — tracing 0.1 + tracing-subscriber 0.3 already in deps, musl + mimalloc/jemalloc recommended, `cross` for cross-compilation, `FROM scratch` pattern
- `CLAUDE.md` §Blockers/Concerns — "Static musl binary + rustls: Edge cases with certificate loading under musl. Test in CI early."

### Prior phase context
- `.planning/phases/01-core-queue-loop/01-CONTEXT.md` — Dual-port architecture (D-05..D-07), 2-crate workspace (D-08..D-09), tracing for logging (D-10)
- `.planning/phases/02-authentication-and-tls/02-CONTEXT.md` — Admin auth with `admin.token` config, TLS config patterns
- `.planning/phases/03-service-registry-and-node-health/03-CONTEXT.md` — Node health tracking, `node_health.rs` data model, heartbeat/stale detection
- `.planning/phases/04-task-reliability-and-callbacks/04-CONTEXT.md` — Reaper background task, callback delivery, per-service `failed_count` in Redis

### Existing code (critical integration points)
- `gateway/src/main.rs` — Tracing subscriber init (line 21-26, needs JSON/file upgrade), server startup, reaper spawn, HTTP router with admin routes
- `gateway/src/config.rs` — `GatewayConfig` struct needs `LoggingConfig` section. Existing pattern for nested config with defaults.
- `gateway/src/state.rs` — `AppState` struct needs `prometheus::Registry` (or individual metric handles)
- `gateway/src/http/admin.rs` — Admin endpoint pattern, add `/v1/admin/health` and `/metrics` here
- `gateway/src/registry/node_health.rs` — Node health data for OBSV-03 health endpoint
- `gateway/src/queue/redis.rs` — Task state transitions where metrics should be recorded (submit, assign, complete, fail)
- `gateway/src/grpc/submit.rs` — gRPC submission path, needs `tasks_submitted_total` counter increment
- `gateway/src/http/submit.rs` — HTTP submission path, needs `tasks_submitted_total` counter increment
- `gateway/src/grpc/poll.rs` — Node poll path, needs `poll_latency` histogram observation
- `gateway/src/callback/mod.rs` — Callback delivery, needs `callback_delivery_total` counter increment
- `gateway/src/reaper/mod.rs` — Reaper timeout detection, needs `errors_total{type="timeout"}` counter increment
- `gateway/src/auth/mod.rs` — Auth rejection paths, needs `errors_total{type="auth_*"}` counter increment
- `gateway/Cargo.toml` — Add `prometheus`, `tikv-jemallocator` deps. tracing-subscriber `json` feature already present.

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `tracing_subscriber::fmt()` init in `main.rs` — replace with layered subscriber (JSON + optional file)
- `admin.token` auth pattern in `http/admin.rs` — reuse for `/metrics` endpoint protection
- `registry/node_health.rs` — `get_node_health` / `list_nodes` for OBSV-03 health data
- `registry/service.rs:list_services` — iterate services for gauge collection
- `queue/redis.rs` — XLEN/XPENDING commands available for queue depth gauge

### Established Patterns
- `Arc<AppState>` shared state — metrics registry/handles added here, accessible from all handlers
- `GatewayConfig` with nested sections + `#[serde(default)]` — same pattern for `LoggingConfig`
- Admin routes merged into HTTP router without auth middleware — same for `/metrics`
- Background `tokio::spawn` for periodic work — could use for periodic gauge refresh if needed

### Integration Points
- `main.rs` — Tracing subscriber upgrade, metric registration, health endpoint wiring
- `config.rs` — Add `LoggingConfig` with `format` and `file` fields
- `state.rs` — Add metric handles to `AppState`
- `queue/redis.rs` — Instrument submit/assign/complete/fail with counters and histograms
- `grpc/submit.rs` + `http/submit.rs` — Increment `tasks_submitted_total` with protocol label
- `grpc/poll.rs` — Observe `poll_latency` histogram
- `callback/mod.rs` — Increment `callback_delivery_total`
- `reaper/mod.rs` — Increment `errors_total{type="timeout"}`
- `auth/` — Increment `errors_total{type="auth_*"}` on rejection

</code_context>

<deferred>
## Deferred Ideas

- OpenTelemetry tracing with W3C trace context propagation — v2 requirement (ADVF-03)
- Request-id header propagation — useful but not required for v1
- Per-endpoint latency histograms for HTTP/gRPC handlers — can add in v2
- Metrics push gateway support (for environments without Prometheus pull) — out of scope
- Health check endpoint for load balancers (separate from admin health) — could be added but not in requirements
- Grafana dashboard JSON export — operational concern, not gateway code
- Log rotation for file output — use external tools (logrotate) in production

</deferred>

---

*Phase: 05-observability-and-packaging*
*Context gathered: 2026-03-22*
