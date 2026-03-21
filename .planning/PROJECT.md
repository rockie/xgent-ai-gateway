# xgent-ai-gateway

## What This Is

A Rust-based pull-model task gateway that sits on the public internet and brokers work between external clients and internal compute nodes. Clients submit tasks via gRPC or HTTPS and receive a task ID immediately. Internal nodes — running behind NAT/firewalls — reverse-poll the gateway to pick up tasks from their service's queue. Each registered service maintains its own node pool, making it a queue-based alternative to traditional load balancers where nodes pull work rather than having it pushed to them.

## Core Value

Tasks submitted by clients reliably reach internal nodes and results reliably flow back, regardless of network topology — nodes behind NAT can serve work without inbound connectivity.

## Requirements

### Validated

(None yet — ship to validate)

### Active

- [ ] Clients can submit tasks via gRPC and receive a task ID
- [ ] Clients can submit tasks via HTTPS and receive a task ID
- [ ] Clients can poll task status/result by task ID (gRPC and HTTPS)
- [ ] Clients can optionally provide a callback URL for result delivery
- [ ] Services can be registered with the gateway, each with its own task queue
- [ ] Internal nodes reverse-poll the gateway to pick up tasks for their service
- [ ] Nodes authenticate with pre-shared tokens scoped to a service
- [ ] Client authentication via API key (HTTPS) and mTLS (gRPC)
- [ ] Task queue state persisted in Redis/Valkey for durability across restarts
- [ ] Nodes report task results back through the gateway
- [ ] Gateway delivers results to polling clients and fires optional callbacks
- [ ] Service-level node pool management (register, deregister, health status)
- [ ] Task lifecycle tracking (pending → assigned → running → completed/failed)
- [ ] Task timeout and retry handling for unresponsive nodes

### Out of Scope

- Web UI dashboard — CLI and API are sufficient for v1
- Multi-region/federation — single gateway instance (behind LB) for v1
- Task priority queues — FIFO per service for v1
- Streaming/websocket result delivery — poll + callback covers v1 needs
- Rate limiting per client — defer to v2
- Task scheduling (cron/delayed) — v1 is immediate dispatch only

## Context

- **Workload types:** AI inference (LLM, image gen), agent job execution, CI pipelines — tasks run seconds to minutes
- **Network topology:** Gateway on public internet, nodes on private networks behind NAT. Nodes cannot receive inbound connections — the pull model solves this.
- **Scale target (v1):** ~100 concurrent nodes, thousands of tasks/hour
- **Deployment:** Single binary for development, Docker/K8s for production
- **Similar systems:** Conceptually like Celery (Python) or Bull (Node.js) but protocol-native (gRPC/HTTP), language-agnostic, and designed for cross-network-boundary operation

## Constraints

- **Language:** Rust — chosen for performance, safety, and native gRPC support (tonic)
- **Protocol:** Must support both gRPC and HTTPS on the same gateway
- **Storage:** Redis/Valkey for task queue state — balances speed with durability
- **Auth:** API key for HTTPS clients, mTLS for gRPC clients, pre-shared tokens for internal nodes
- **Deployment:** Must produce a single static binary and a Docker image

## Key Decisions

| Decision | Rationale | Outcome |
|----------|-----------|---------|
| Pull model over push | Nodes behind NAT can't receive inbound connections; pull inverts the connection direction | — Pending |
| Rust over Go/Node | Performance-critical gateway with gRPC; Rust's tonic crate provides excellent gRPC support | — Pending |
| Redis/Valkey over PostgreSQL | Queue operations need low latency; Redis pub/sub can notify nodes of new tasks | — Pending |
| Async-first task model | AI/CI tasks take seconds-minutes; blocking callers is impractical | — Pending |
| API key + mTLS dual auth | Different security postures for HTTP (simpler) vs gRPC (stronger) clients | — Pending |

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
*Last updated: 2026-03-21 — Phase 01 (core-queue-loop) complete: Cargo workspace, gRPC/HTTP dual-port gateway, Redis Streams queue, node runner agent, 6 integration tests passing*
