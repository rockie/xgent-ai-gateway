# xgent-ai-gateway

## What This Is

A Rust-based pull-model task gateway that sits on the public internet and brokers work between external clients and internal compute nodes. Clients submit tasks via gRPC or HTTPS and receive a task ID immediately. Internal nodes — running behind NAT/firewalls — reverse-poll the gateway to pick up tasks from their service's queue. Each registered service maintains its own node pool with health tracking, making it a queue-based alternative to traditional load balancers where nodes pull work rather than having it pushed to them.

## Core Value

Tasks submitted by clients reliably reach internal nodes and results reliably flow back, regardless of network topology — nodes behind NAT can serve work without inbound connectivity.

## Requirements

### Validated

- ✓ Client task submission via gRPC and HTTPS with opaque payload — v1.0
- ✓ Task status/result polling by task ID via gRPC and HTTPS — v1.0
- ✓ Arbitrary key-value metadata/labels attached at submission — v1.0
- ✓ Opaque payload treatment (gateway doesn't interpret content) — v1.0
- ✓ Optional callback URL for result delivery with exponential backoff — v1.0
- ✓ Task results stored in Redis with configurable TTL — v1.0
- ✓ Service registration with isolated task queues per service — v1.0
- ✓ Service deregistration with queue drain and config cleanup — v1.0
- ✓ Service configuration persisted in Redis across restarts — v1.0
- ✓ Internal nodes reverse-poll via gRPC server-streaming — v1.0
- ✓ Node authentication with pre-shared tokens scoped to service — v1.0
- ✓ Node task completion reporting (success/failure) with result payload — v1.0
- ✓ Node health tracking via heartbeat (last poll time, stale detection) — v1.0
- ✓ Graceful node drain (no new tasks, complete in-flight work) — v1.0
- ✓ Task state machine: pending → assigned → running → completed/failed — v1.0
- ✓ Reliable queue pattern (atomic move to processing list) — v1.0
- ✓ Background reaper detects timed-out tasks and marks as failed — v1.0
- ✓ API key auth for HTTPS clients — v1.0
- ✓ mTLS auth for gRPC clients with cert fingerprint-to-service mapping — v1.0
- ✓ Node token auth validated on every poll — v1.0
- ✓ Structured JSON logging with task/service/node context — v1.0
- ✓ Prometheus metrics (queue depth, latency, node counts, error rates) — v1.0
- ✓ Admin health API (active nodes, last seen, in-flight tasks) — v1.0
- ✓ Redis/Valkey for all persistent state — v1.0
- ✓ Configurable via env vars with optional TOML config file — v1.0
- ✓ Single static binary (musl target) — v1.0
- ✓ Docker image — v1.0
- ✓ TLS termination for HTTPS and gRPC — v1.0
- ✓ HTTP/2 keepalive pings on all connection modes — v1.0
- ✓ gRPC auth hardening (API key on client RPCs, node token on node RPCs) — v1.0

- ✓ Admin authentication with Argon2 password hashing and HttpOnly cookie sessions — v1.1
- ✓ Admin dashboard with Prometheus metrics visualization (overview cards, time-series charts, service health) — v1.1
- ✓ Service registration and management UI (list, detail, create, deregister) — v1.1
- ✓ Node health monitoring UI (per-service node list, detail, health badges) — v1.1
- ✓ Task management UI (paginated list, filters, detail, cancel) — v1.1
- ✓ Credential management UI (API keys and node tokens: list, create, revoke) — v1.1

### Active

## Current Milestone: v1.2 Flexible Agent Execution

**Goal:** Make the runner agent a configurable execution engine supporting HTTP and CLI invocation modes with templated request/response mapping and async two-phase polling.

**Target features:**
- Configurable execution modes: `cli`, `sync-api`, `async-api` per service in agent.toml
- CLI mode with arg-based and stdin-pipe execution, placeholder system (`<payload>`, `<stdout>`, `<stderr>`)
- sync-api mode with configurable URL, method, headers, body template, env var interpolation
- async-api mode with two-phase submit + poll, key-path extraction, completion conditions, timeout
- Templated response body mapping for all modes
- Example services covering all three modes
- Node.js client example for full end-to-end flow

### Out of Scope

- Multi-region/federation — run independent instances per region; cross-region is caller's problem
- Task priority queues — use separate services per priority tier; simpler, no starvation risk
- Streaming/WebSocket results — poll + callback covers all practical use cases
- Rate limiting per client — defer to API gateway (nginx/Envoy) in front
- Task scheduling (cron/delayed) — different product; use external schedulers that submit to gateway
- Workflow orchestration / DAGs — turns gateway into workflow engine (Temporal territory)
- Dynamic service loading (.so/.dylib) — opaque payloads with universal API is simpler and more secure
- Payload encryption at rest — callers encrypt before submission; gateway treats payloads as opaque bytes
- Task retry with exponential backoff — never for this project; clients resubmit on failure (D-07)
- Dead letter queues — descoped v1; failed state is terminal (D-08/D-09)
- Log viewer in admin UI — deferred from v1.1; revisit later
- HTTP node polling — deferred; runner agent proxy unifies node protocol to gRPC (D-13)

## Context

Shipped v1.1 with ~6,600 LOC Rust (gateway) + ~6,600 LOC TypeScript/TSX (admin-ui).
Tech stack: Rust (Tokio, Tonic, Axum, Redis Streams, rustls) + Vite + React 19 + TailwindCSS v4 + shadcn/ui + TanStack Router & Query + Recharts 3.x.
32 plans across 12 phases completed in 2 days (v1.0 + v1.1).
34 integration tests cover auth, registry, health, reaper, and gRPC auth flows.
Admin UI serves as a single-page app with session-based auth, dark mode, auto-refresh, and full CRUD for services, nodes, tasks, and credentials.
Runner agent currently hardcodes HTTP POST to echo service; v1.2 replaces this with a configurable execution engine.

## Constraints

- **Language:** Rust — chosen for performance, safety, and native gRPC support (tonic)
- **Protocol:** Must support both gRPC and HTTPS on the same gateway
- **Storage:** Redis/Valkey for task queue state — balances speed with durability
- **Auth:** API key for HTTPS clients, mTLS for gRPC clients, pre-shared tokens for internal nodes
- **Deployment:** Must produce a single static binary and a Docker image

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Pull model over push | Nodes behind NAT can't receive inbound connections; pull inverts the connection direction | ✓ Good — core architecture validated |
| Rust over Go/Node | Performance-critical gateway with gRPC; Rust's tonic crate provides excellent gRPC support | ✓ Good — 8.4k LOC, static binary |
| Redis Streams over BLMOVE | Consumer group semantics give reliable delivery, XPENDING for timeout detection | ✓ Good — reaper uses XPENDING IDLE |
| Async-first task model | AI/CI tasks take seconds-minutes; blocking callers is impractical | ✓ Good — validated |
| API key + mTLS dual auth | Different security postures for HTTP (simpler) vs gRPC (stronger) clients | ✓ Good — validated Phase 2 |
| Dual-port HTTP + gRPC | Separate TLS configs needed; simpler than co-hosting on single port | ✓ Good — clean separation |
| Tower auth wrappers for gRPC | NamedService delegation pattern for per-RPC auth enforcement | ✓ Good — Phase 6 |
| Manual TLS accept loop | hyper-util for per-connection HTTP/2 keepalive control | ✓ Good — keepalive parity |
| Descope retries/DLQ (v1) | Clients can resubmit; keeps gateway simple and predictable | ✓ Good — simplicity preserved |
| Defer HTTP node polling | Runner agent proxy unifies all nodes to gRPC; avoids duplicate protocol | ✓ Good — single protocol path |
| Config-based mTLS identity | HashMap<fingerprint, Vec<service>> in gateway.toml, empty=disabled | ✓ Good — Phase 7 |
| jemalloc for musl binary | Default musl allocator has poor performance; jemalloc fixes this | ✓ Good — Phase 5 |
| Argon2id + HttpOnly cookie sessions | Industry-standard password hashing; cookies avoid XSS token theft | ✓ Good — Phase 8 |
| SameSite=None + Secure cookies | Required for cross-origin SPA session delivery during dev | ✓ Good — Phase 8 |
| Vite + React 19 + TanStack | Modern SPA stack with file-based routing and query caching | ✓ Good — Phase 8 |
| shadcn/ui v4 with oklch defaults | Accepted framework defaults over custom zinc HSL from UI-SPEC | ✓ Good — consistent theming |
| SCAN-based pagination | App-layer filtering with Redis SCAN; simple, works for admin scale | ✓ Good — Phase 10 |
| In-memory ring buffer for metrics | std::sync::Mutex with microsecond locks; captures Prometheus snapshots every 10s | ✓ Good — Phase 12 |
| Forced-dismissal secret dialog | Prevents accidental dismiss of one-time secret reveal | ✓ Good — Phase 11 |

## Evolution

This document evolves at phase transitions and milestone boundaries.

**After each phase transition** (via `/gsd:transition`):
1. Requirements invalidated? → Move to Out of Scope with reason
2. Requirements validated? → Move to Validated with phase reference
3. New requirements emerged? → Add to Active
4. Decisions to log? → Add to Key Decisions
5. "What This Is" still accurate? → Update if drifted

**After each milestone** (via `/gsd:complete-milestone`):
1. Full review of all sections
2. Core Value check — still the right priority?
3. Audit Out of Scope — reasons still valid?
4. Update Context with current state

---
*Last updated: 2026-03-24 after v1.2 milestone started*
