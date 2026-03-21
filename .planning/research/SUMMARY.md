# Project Research Summary

**Project:** xgent-ai-gateway
**Domain:** Pull-model task gateway / distributed job queue (Rust, gRPC/HTTPS, Redis-backed)
**Researched:** 2026-03-21
**Confidence:** HIGH

## Executive Summary

The xgent-ai-gateway is a pull-model task broker that sits on the public internet and allows worker nodes behind NAT/firewalls to claim and execute tasks without requiring inbound connectivity. Experts build this type of system using reliable queue patterns (Redis Streams or BRPOPLPUSH/BLMOVE with processing lists), heartbeat-based lease management, and separated trust boundaries between task submitters (clients) and task executors (nodes). The closest comparable systems are Temporal, Hatchet, and Celery -- but xgent differentiates by being a single static Rust binary with only Redis as an external dependency, supporting both gRPC and HTTP natively, and being language-agnostic for workers.

The recommended approach is to build on the Tokio/Axum/Tonic stack, co-hosting gRPC and HTTP on a single port via content-type multiplexing. Redis serves as the sole data store using lists with BLMOVE for reliable atomic dequeue and hashes for task state. The architecture cleanly separates into protocol layer, core engine, and storage layer, with three distinct auth mechanisms (API keys for HTTP clients, mTLS for gRPC clients, pre-shared tokens for nodes). The build order follows strict dependencies: proto definitions and storage first, then core engine, then protocol and auth layers, then operational reliability features.

The primary risks are task loss on worker crash (solved by the reliable queue pattern with processing lists), duplicate execution from fixed timeouts (solved by heartbeat-based leases and fencing tokens), and silent connection death through network intermediaries killing idle gRPC connections (solved by HTTP/2 keepalive pings configured from day one). Redis as a single point of failure is an accepted tradeoff for v1 simplicity, mitigated by AOF persistence, noeviction policy, result TTLs, and graceful degradation on Redis unavailability.

## Key Findings

### Recommended Stack

The stack is entirely Rust-native and unified around the Tokio ecosystem. Every library shares the same async runtime and Tower middleware layer, eliminating the interop problems that plague mixed-ecosystem stacks. See [STACK.md](STACK.md) for full details.

**Core technologies:**
- **Rust (stable 1.85+):** Language -- performance, memory safety, static binary compilation
- **Tokio 1.43+ LTS:** Async runtime -- the foundation everything else builds on
- **Tonic 0.14.x:** gRPC server -- Tokio-native, supports mTLS via rustls, streaming
- **Axum 0.8.x:** HTTP server -- same Hyper/Tower foundation as Tonic, enabling single-port co-hosting
- **redis-rs 1.0.x:** Redis client -- MultiplexedConnection is clone-safe and cancellation-safe, no external pool needed
- **rustls 0.23.x:** TLS -- pure Rust, no OpenSSL dependency, simpler cross-compilation and mTLS

**Critical version constraint:** Tonic 0.14.x requires Prost 0.14.x (must match minor versions). Axum 0.8.x requires Tower 0.5.x. rustls 0.23.x pairs with tokio-rustls 0.26.x. These are non-negotiable pairings.

### Expected Features

See [FEATURES.md](FEATURES.md) for full analysis including competitor comparison and dependency graph.

**Must have (table stakes -- P1):**
- Task submission via both gRPC and HTTPS (dual protocol is the stated requirement)
- Async task lifecycle state machine (pending, assigned, running, completed, failed)
- Task status polling and result storage in Redis with TTL
- Node reverse-polling to claim tasks (the core differentiator)
- Service-scoped queues with per-service isolation
- Client auth (API keys + mTLS) and node auth (per-service tokens)
- Task timeout detection with reassignment
- Retry with exponential backoff and dead letter queue
- Node health tracking via heartbeat
- Structured JSON logging and Prometheus metrics

**Should have (differentiators -- P2):**
- Callback/webhook delivery with retry
- OpenTelemetry distributed tracing
- Task metadata/labels for routing
- Bulk task submission
- Graceful node drain
- Task cancellation

**Defer (v2+):**
- Priority queues (use separate services instead)
- Task scheduling/cron (use external schedulers)
- Workflow orchestration/DAGs (not a workflow engine)
- Web UI (use Grafana + CLI)
- Multi-region federation

### Architecture Approach

The gateway is a single process with three layers: protocol (Axum + Tonic multiplexed on one port), core engine (auth, task router, queue manager, node registry, result dispatcher), and storage (Redis). Clients and nodes connect through separate gRPC service definitions with different auth mechanisms. The queue uses Redis lists with BLMOVE for atomic reliable dequeue into a processing list, with a background reaper recovering stuck tasks. Task state lives in Redis hashes keyed by task_id; queue lists contain only task_id strings. See [ARCHITECTURE.md](ARCHITECTURE.md) for full details.

**Major components:**
1. **Protocol Layer** -- Axum + Tonic on shared Hyper server, content-type routing
2. **Auth Middleware** -- Three Tower middleware layers: API key, mTLS, node token
3. **Task Router** -- Validates submissions, generates IDs, enqueues to service queue
4. **Queue Manager** -- Per-service FIFO queues via Redis BLMOVE, background reaper for timeouts
5. **Node Registry** -- Tracks nodes per service, heartbeat-based health, stale node reaping
6. **Task Lifecycle Tracker** -- State machine in Redis hashes with timestamps and retry counts
7. **Result Dispatcher** -- Synchronous polling reads + async callback delivery with retries

### Critical Pitfalls

See [PITFALLS.md](PITFALLS.md) for full analysis including recovery strategies and phase mapping.

1. **Task loss on worker crash** -- Use BLMOVE to atomically move tasks to a processing list; reaper recovers stuck tasks. Never use bare BRPOP. Must be correct in Phase 1.
2. **Duplicate execution from timeout races** -- Implement heartbeat-based leases instead of fixed timeouts. Add fencing tokens (unique assignment IDs) so stale results are rejected. Require idempotent callbacks.
3. **Thundering herd on task arrival** -- Use BLMOVE (only one consumer unblocks per item) instead of pub/sub notification. Add jitter to polling intervals. Never notify all idle nodes.
4. **gRPC connections killed by intermediaries** -- Configure HTTP/2 keepalive pings (30s interval, 10s timeout) on both server and client from day one. Deploy behind TCP-passthrough LB (NLB), not HTTP-terminating LB (ALB).
5. **Redis data loss / memory exhaustion** -- Enable AOF persistence, set noeviction policy, TTL all results, cap payload sizes. Gateway must gracefully reject submissions when Redis is unavailable.

## Implications for Roadmap

Based on combined research, the following phase structure respects dependency ordering, groups architecturally related work, and addresses pitfalls at the earliest possible point.

### Phase 1: Foundation -- Proto, Storage, Core Queue Loop

**Rationale:** Everything depends on proto definitions, the Redis storage layer, and the reliable queue mechanism. The ARCHITECTURE.md build order and FEATURES.md dependency graph both converge here. Getting the queue pattern wrong in Phase 1 is catastrophically expensive to fix later (HIGH recovery cost per PITFALLS.md).

**Delivers:** Proto definitions (gateway.proto, worker.proto), Redis storage layer with Store trait, task state machine, reliable queue (BLMOVE + processing list + reaper), basic task submission and node polling (without auth), result reporting and storage with TTL.

**Addresses features:** Service-scoped queues, async task lifecycle, task submission (gRPC + HTTP), node reverse-polling, task result storage, task timeout detection and reassignment, retry with backoff, dead letter queue.

**Avoids pitfalls:** Task loss on worker crash (reliable queue from day one), thundering herd (BLMOVE-based polling), Redis data loss (TTLs and key structure designed correctly).

### Phase 2: Protocol and Auth -- Production-Ready Interfaces

**Rationale:** With the core loop working, add the security and protocol layers that make it deployable. Auth middleware is independent of core logic (Tower layers) but must exist before any public exposure. mTLS and API key validation are medium complexity and architecturally separable from queue logic.

**Delivers:** Axum HTTP routes + Tonic gRPC services on single port, API key auth for HTTP clients, mTLS for gRPC clients, per-service token auth for nodes, TLS on all connections, HTTP/2 keepalive configuration.

**Addresses features:** Client authentication, node authentication, dual-protocol submission, status polling via both protocols.

**Avoids pitfalls:** Token-based auth weaknesses (per-service scoping, hashed storage), gRPC connection death (keepalive configuration), cloud LB issues (document NLB requirement).

### Phase 3: Node Management and Health

**Rationale:** With auth in place, nodes have identity. Now build the fleet management layer: service registration, node registry, heartbeat processing, stale node detection. This is architecturally separate from queue logic and depends on auth for node identity.

**Delivers:** Service registration API, node registry with metadata, heartbeat-based health tracking, stale node reaping, fencing tokens for duplicate execution prevention, node capacity declaration.

**Addresses features:** Node health tracking, service-level node pool management.

**Avoids pitfalls:** Duplicate execution race (fencing tokens), orphaned tasks on node deregistration (cleanup logic).

### Phase 4: Operational Reliability -- Callbacks, Observability, Hardening

**Rationale:** The system is functional and secure after Phase 3. Now add operational maturity: callback delivery (a distinct reliability problem), structured logging, Prometheus metrics, and OpenTelemetry tracing. Callbacks are deliberately deferred because polling works in Phase 1-3, and callbacks require their own retry/dead-letter infrastructure.

**Delivers:** Callback/webhook delivery with exponential backoff retries, callback dead-letter log, SSRF protection on callback URLs, Prometheus metrics endpoint, OpenTelemetry distributed tracing, enhanced structured logging with task/service/node context.

**Addresses features:** Callback delivery, Prometheus metrics, structured logging, OpenTelemetry tracing.

**Avoids pitfalls:** Callback delivery failures (retry with backoff, dead-letter, polling as fallback), SSRF via callback URLs (URL validation, private IP blocking).

### Phase 5: Developer Experience and Extensions

**Rationale:** With a production-ready gateway, add features that improve day-to-day usability. These are low-complexity additions that layer on top of existing infrastructure without modifying core paths.

**Delivers:** Task metadata/labels with node-side filtering, bulk task submission, task cancellation, graceful node drain, CLI tool for task/node inspection.

**Addresses features:** Task metadata, bulk submission, task cancellation, graceful drain, CLI tooling.

### Phase Ordering Rationale

- **Phase 1 before Phase 2:** Auth is meaningless without a working queue. The core loop must be testable end-to-end before adding security layers. This also lets Phase 1 integration tests run without auth complexity.
- **Phase 2 before Phase 3:** Node management requires authenticated node identity. Cannot track "which node has this task" without knowing who the node is.
- **Phase 3 before Phase 4:** Callbacks and observability benefit from node identity context. OpenTelemetry spans are more useful when they include node metadata.
- **Phase 4 before Phase 5:** Operational reliability must precede convenience features. Callbacks are a higher priority than bulk submission.
- **Pitfall alignment:** Every critical pitfall (task loss, duplicate execution, thundering herd, connection death, Redis durability) is addressed in Phases 1-2, preventing expensive retrofits.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 1:** Redis Streams vs BLMOVE decision needs benchmarking. PITFALLS.md recommends Streams (XREADGROUP/XACK) while ARCHITECTURE.md recommends BLMOVE. Both are viable; the team should prototype both and measure. This is the most consequential technical decision.
- **Phase 2:** mTLS implementation with Tonic has documented edge cases (see tonic#511). Research the exact rustls + tonic TLS configuration, certificate loading, and client certificate validation flow.
- **Phase 4:** SSRF protection for callback URLs needs research on Rust DNS resolution libraries and private IP range detection.

Phases with standard patterns (skip research-phase):
- **Phase 3:** Node registry and heartbeat tracking are well-documented patterns with clear Redis data structures.
- **Phase 5:** Task metadata, bulk submission, and cancellation are straightforward CRUD extensions of existing structures.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | All versions verified on crates.io. Compatibility matrix confirmed. The Tokio/Axum/Tonic stack is the uncontested Rust standard for this type of service. |
| Features | HIGH | Competitor analysis covers Temporal, Hatchet, Celery, BullMQ. Feature priorities align with the domain. Anti-features are well-reasoned. |
| Architecture | HIGH | Reliable queue pattern (BLMOVE + processing list) is a proven Redis pattern. Protocol multiplexing is documented with examples. Component boundaries are clean. |
| Pitfalls | HIGH | Sources include Redis official docs, gRPC performance guides, and post-mortem-style articles. Pitfalls are specific, actionable, and mapped to phases. |

**Overall confidence:** HIGH

### Gaps to Address

- **Redis Streams vs Lists:** PITFALLS.md advocates Streams (XREADGROUP/XACK with PEL); ARCHITECTURE.md advocates lists (BLMOVE with processing list and reaper). Both work. Streams are more robust out of the box but add Redis Streams-specific complexity. Lists with BLMOVE are simpler but require a manual reaper. Decide during Phase 1 planning by prototyping both.
- **redis-rs 1.0 MultiplexedConnection under load:** Stack research rates this MEDIUM confidence. With 100+ nodes doing blocking BLMOVE, each holding a connection for up to 30 seconds, the multiplexed connection may not suffice. Plan to benchmark early and add a second connection or pool if needed.
- **Static musl binary + rustls:** Rated MEDIUM confidence due to edge cases with certificate loading under musl. Test this in CI early, not at release time.
- **Payload size limits:** The architecture assumes small payloads in Redis. AI workloads can produce large outputs (images, long text). Define and enforce a max payload size in Phase 1 (recommend 1MB). Plan object storage references for v2.
- **Node concurrent task capacity:** Nodes may want to declare capacity (e.g., "I have 4 GPUs, send me 4 tasks"). The polling model naturally handles this (node issues 4 concurrent polls), but the gateway should track and respect declared capacity. Address in Phase 3.

## Sources

### Primary (HIGH confidence)
- [tonic 0.14.x on crates.io](https://crates.io/crates/tonic) -- gRPC framework version and features
- [Axum 0.8.0 announcement](https://tokio.rs/blog/2025-01-01-announcing-axum-0-8-0) -- HTTP framework compatibility
- [redis-rs 1.0.x on crates.io](https://crates.io/crates/redis) -- Redis client API and MultiplexedConnection
- [Redis Streams documentation](https://redis.io/docs/latest/develop/data-types/streams/) -- Consumer groups, PEL, XAUTOCLAIM
- [Redis reliable queue patterns](https://redis.io/docs/latest/commands/rpoplpush/) -- BRPOPLPUSH/BLMOVE documentation
- [gRPC Performance Best Practices](https://grpc.io/docs/guides/performance/) -- Keepalive, connection management
- [Temporal Task Queues](https://docs.temporal.io/task-queue) -- Competitor architecture reference

### Secondary (MEDIUM confidence)
- [Axum + Tonic co-hosting example](https://github.com/sunsided/http-grpc-cohosting) -- Multiplexing implementation reference
- [Svix: How to Build a Reliable Queue in Redis](https://www.svix.com/resources/redis/reliable-queue/) -- Practical reliable queue patterns
- [Hatchet features](https://hatchet.run/) -- Competitor feature comparison
- [tonic mTLS discussion (#511)](https://github.com/hyperium/tonic/issues/511) -- mTLS authorization patterns

### Tertiary (LOW confidence)
- Static musl binary edge cases with rustls -- anecdotal reports, needs early testing
- redis-rs MultiplexedConnection behavior under 100+ blocking operations -- needs benchmarking

---
*Research completed: 2026-03-21*
*Ready for roadmap: yes*
