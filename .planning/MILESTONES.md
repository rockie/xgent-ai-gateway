# Milestones

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
