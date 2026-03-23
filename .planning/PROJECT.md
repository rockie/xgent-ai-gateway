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

### Active

<!-- Milestone v1.1: Admin Web UI -->

- [ ] Admin authentication (login with admin account)
- [ ] Dashboard with Prometheus metrics visualization
- [x] Service registration and management — Phase 9
- [x] Node agents management — Phase 9
- [x] Credential management (list, create, revoke API keys and node tokens) — Phase 11
- [ ] Tasks management (view status/details, cancel tasks)

## Current Milestone: v1.1 Admin Web UI

**Goal:** Add an admin web UI as a separate frontend app for managing and monitoring the gateway.

**Target features:**
- Admin login/authentication
- Dashboard with Prometheus metrics (integrate existing Prometheus UI lib if available)
- Service registration and CRUD management
- Node agents management
- Task viewing, status details, and cancellation (returns failed result to client)

### Out of Scope

- ~~Web UI dashboard~~ — now building custom admin UI in v1.1
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

Shipped v1.0 with 8,429 LOC Rust across 3 crates (gateway, proto, runner-agent) + sample service.
Tech stack: Rust, Tokio, Tonic (gRPC), Axum (HTTP), Redis Streams, rustls (TLS/mTLS).
20 plans across 7 phases completed in 2 days.
34 integration tests cover auth, registry, health, reaper, and gRPC auth flows.
v1.1 adds a separate frontend app: Vite + React + TailwindCSS + shadcn/ui + TanStack Router & Query.

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
*Last updated: 2026-03-23 after Phase 11 (Credential Management) complete*
