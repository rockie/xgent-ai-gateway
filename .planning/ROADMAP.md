# Roadmap: xgent-ai-gateway

## Overview

Build a Rust pull-model task gateway from storage layer up through core queue loop, authentication, service/node management, task reliability, and finally observability and packaging. Each phase delivers a coherent, testable capability. The first phase establishes the complete submit-poll-execute-return loop without auth (for testability); subsequent phases layer security, fleet management, reliability guarantees, and operational maturity on top.

## Phases

**Phase Numbering:**
- Integer phases (1, 2, 3): Planned milestone work
- Decimal phases (2.1, 2.2): Urgent insertions (marked with INSERTED)

Decimal phases appear between their surrounding integers in numeric order.

- [ ] **Phase 1: Core Queue Loop** - Redis storage, task state machine, reliable queue, dual-protocol submit and poll, node reverse-polling
- [ ] **Phase 2: Authentication and TLS** - API key, mTLS, and node token auth with TLS termination and keepalive
- [ ] **Phase 3: Service Registry and Node Health** - Service CRUD, node heartbeat tracking, graceful drain
- [ ] **Phase 4: Task Reliability and Callbacks** - Timeout detection, retry with backoff, dead letter queue, callback delivery
- [ ] **Phase 5: Observability and Packaging** - Structured logging, Prometheus metrics, admin health API, static binary and Docker image

## Phase Details

### Phase 1: Core Queue Loop
**Goal**: A client can submit a task via gRPC or HTTPS, an internal node can poll and claim that task via gRPC server-streaming, execute it, report the result via unary RPC, and the client can retrieve the result by polling -- all backed by Redis Streams with consumer group semantics for reliable delivery
**Depends on**: Nothing (first phase)
**Requirements**: TASK-01, TASK-02, TASK-03, TASK-04, RSLT-01, RSLT-02, RSLT-05, NODE-01, NODE-02, NODE-04, LIFE-01, LIFE-02, SRVC-02, INFR-01, INFR-02
**Success Criteria** (what must be TRUE):
  1. Client can submit a task with opaque payload and metadata via both gRPC and HTTPS and receive a unique task ID
  2. Internal node can reverse-poll the gateway and receive a queued task for its service
  3. Node can report task completion (success or failure) with a result payload back to the gateway
  4. Client can poll by task ID via both gRPC and HTTPS and retrieve the task status and result
  5. Tasks are persisted in Redis using reliable queue pattern (BLMOVE to processing list) so no task is lost if the gateway restarts mid-operation
**Plans**: 3 plans

Plans:
- [x] 01-01-PLAN.md -- Cargo workspace, proto codegen, types, config, Redis Streams queue layer
- [x] 01-02-PLAN.md -- gRPC services (TaskService + NodeService), HTTP REST handlers, dual-port server startup
- [x] 01-03-PLAN.md -- Runner agent binary, integration tests, end-to-end verification

### Phase 2: Authentication and TLS
**Goal**: All connections to the gateway are authenticated and encrypted -- HTTPS clients use API keys, gRPC clients use mTLS, internal nodes use per-service tokens, and all traffic runs over TLS with HTTP/2 keepalive
**Depends on**: Phase 1
**Requirements**: AUTH-01, AUTH-02, AUTH-03, INFR-05, INFR-06
**Success Criteria** (what must be TRUE):
  1. HTTPS client requests without a valid API key are rejected with 401
  2. gRPC client connections without a valid client certificate are rejected at the TLS handshake
  3. Node poll requests with an invalid or wrong-service token are rejected
  4. Gateway serves all traffic over TLS and maintains HTTP/2 keepalive pings to prevent silent connection death
**Plans**: 3 plans

Plans:
- [x] 02-01-PLAN.md -- Auth module foundation: API key + node token CRUD, TLS config builders, extended config/state/error
- [x] 02-02-PLAN.md -- Wire TLS, auth middleware, admin endpoints, keepalive into server startup
- [x] 02-03-PLAN.md -- Auth integration tests with rcgen certs, runner agent auth support

### Phase 3: Service Registry and Node Health
**Goal**: Admins can register and manage services, and the gateway tracks node health per service so it knows which nodes are alive and can gracefully handle node departures
**Depends on**: Phase 2
**Requirements**: SRVC-01, SRVC-03, SRVC-04, NODE-03, NODE-05, NODE-06
**Success Criteria** (what must be TRUE):
  1. Admin can register a new service with its configuration and node auth tokens, and that service gets its own task queue
  2. Admin can deregister a service, draining its queue and cleaning up config
  3. Service configuration survives gateway restarts (persisted in Redis)
  4. Gateway detects stale nodes via heartbeat (last poll time) and marks them unhealthy
  5. A node can signal graceful drain, after which it receives no new tasks but completes in-flight work
**Plans**: TBD

Plans:
- [ ] 03-01: TBD
- [ ] 03-02: TBD

### Phase 4: Task Reliability and Callbacks
**Goal**: Tasks that fail or time out are automatically retried, permanently failed tasks land in a dead letter queue, and clients can optionally receive results via callback URL instead of polling
**Depends on**: Phase 3
**Requirements**: LIFE-03, LIFE-04, LIFE-05, RSLT-03, RSLT-04
**Success Criteria** (what must be TRUE):
  1. A task assigned to an unresponsive node is detected by the background reaper and re-queued for another node
  2. Failed tasks are retried with configurable max retries and exponential backoff
  3. Tasks that exhaust all retries are moved to a per-service dead letter queue
  4. Client can provide a callback URL at submission and receive the result delivered to that URL with exponential backoff retries on failure
**Plans**: TBD

Plans:
- [ ] 04-01: TBD
- [ ] 04-02: TBD

### Phase 5: Observability and Packaging
**Goal**: The gateway emits structured logs, exposes Prometheus metrics, provides admin health data, and ships as a single static binary and Docker image ready for production deployment
**Depends on**: Phase 4
**Requirements**: OBSV-01, OBSV-02, OBSV-03, INFR-03, INFR-04
**Success Criteria** (what must be TRUE):
  1. Every log line is structured JSON with task ID, service name, and node context where applicable
  2. Prometheus metrics endpoint exposes queue depth, task latency, node counts, and error rates
  3. Admin API endpoint returns node health data (active nodes per service, last seen time, in-flight task counts)
  4. Gateway compiles to a single static binary (musl target) and ships as a Docker image
**Plans**: TBD

Plans:
- [ ] 05-01: TBD
- [ ] 05-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 1 -> 2 -> 3 -> 4 -> 5

| Phase | Plans Complete | Status | Completed |
|-------|----------------|--------|-----------|
| 1. Core Queue Loop | 3/3 | Complete | 2026-03-21 |
| 2. Authentication and TLS | 0/3 | Not started | - |
| 3. Service Registry and Node Health | 0/2 | Not started | - |
| 4. Task Reliability and Callbacks | 0/2 | Not started | - |
| 5. Observability and Packaging | 0/2 | Not started | - |
