# Requirements: xgent-ai-gateway

**Defined:** 2026-03-21
**Core Value:** Tasks submitted by clients reliably reach internal nodes and results reliably flow back, regardless of network topology

## v1 Requirements

### Task Submission

- [x] **TASK-01**: Client can submit a task via gRPC with an opaque payload and receive a task ID
- [x] **TASK-02**: Client can submit a task via HTTPS REST with an opaque payload and receive a task ID
- [x] **TASK-03**: Client can attach arbitrary key-value metadata/labels to a task at submission
- [x] **TASK-04**: Task payloads are treated as opaque bytes — the gateway does not interpret payload content

### Task Results

- [x] **RSLT-01**: Client can poll task status and result by task ID via gRPC
- [x] **RSLT-02**: Client can poll task status and result by task ID via HTTPS REST
- [x] **RSLT-03**: Client can optionally provide a callback URL at submission for result delivery
- [x] **RSLT-04**: Gateway delivers results to callback URL with exponential backoff retries on failure
- [x] **RSLT-05**: Task results are stored in Redis with a configurable TTL

### Service Registry

- [x] **SRVC-01**: Admin can register a new service with the gateway (name, config, node auth tokens)
- [x] **SRVC-02**: Each registered service gets its own isolated task queue
- [x] **SRVC-03**: Admin can deregister a service (drains queue, removes config)
- [x] **SRVC-04**: Service configuration is persisted in Redis and survives gateway restarts

### Node Management

- [x] **NODE-01**: Internal nodes can reverse-poll the gateway via gRPC to pick up tasks for their service
- [x] **NODE-02**: Internal nodes can reverse-poll the gateway via HTTPS to pick up tasks for their service
- [x] **NODE-03**: Nodes authenticate with pre-shared tokens scoped to their service
- [x] **NODE-04**: Nodes report task completion (success or failure) with result payload back to the gateway
- [x] **NODE-05**: Gateway tracks node health via heartbeat (last poll time, stale detection)
- [x] **NODE-06**: Nodes can signal graceful drain — gateway stops assigning new tasks, waits for in-flight completion

### Task Lifecycle

- [x] **LIFE-01**: Tasks follow a state machine: pending → assigned → running → completed/failed
- [x] **LIFE-02**: Gateway uses reliable queue pattern (atomic move to processing list) to prevent task loss
- [x] **LIFE-03**: Background reaper detects timed-out tasks (node died) and re-queues them
- [ ] **LIFE-04**: Failed tasks retry with configurable max retries and exponential backoff
- [ ] **LIFE-05**: Tasks exhausting retries move to a per-service dead letter queue

### Authentication

- [x] **AUTH-01**: HTTPS clients authenticate via API key (bearer token)
- [x] **AUTH-02**: gRPC clients authenticate via mTLS (mutual TLS certificates)
- [x] **AUTH-03**: Internal nodes authenticate via pre-shared tokens validated on each poll

### Observability

- [x] **OBSV-01**: Gateway emits structured JSON logs with task ID, service, and node context in every log line
- [x] **OBSV-02**: Gateway exposes Prometheus metrics endpoint (queue depth, task latency, node counts, error rates)
- [x] **OBSV-03**: Node health dashboard data available via admin API (active nodes, last seen, in-flight tasks)

### Infrastructure

- [x] **INFR-01**: Gateway connects to Redis/Valkey for all persistent state (queues, results, config)
- [x] **INFR-02**: Gateway is configurable via environment variables with optional TOML config file override
- [ ] **INFR-03**: Gateway builds as a single static binary (musl target)
- [ ] **INFR-04**: Gateway ships as a Docker image
- [x] **INFR-05**: Gateway supports TLS termination for HTTPS and gRPC
- [x] **INFR-06**: Gateway configures HTTP/2 keepalive pings to prevent silent connection death through NAT/LB

## v2 Requirements

### Enhanced Auth

- **EAUTH-01**: Node authentication via mTLS certificates (replace pre-shared tokens)
- **EAUTH-02**: API key rotation and revocation without downtime

### Advanced Features

- **ADVF-01**: Bulk task submission in a single RPC call
- **ADVF-02**: Task cancellation by task ID
- **ADVF-03**: OpenTelemetry tracing with W3C trace context propagation across task lifecycle
- **ADVF-04**: Task metadata-based routing (send to nodes matching label selectors)

### Operations

- **OPS-01**: CLI tool for service management, task inspection, and node status
- **OPS-02**: Admin API for service CRUD, node management, and queue inspection
- **OPS-03**: Task data TTL policies with automatic cleanup

## Out of Scope

| Feature | Reason |
|---------|--------|
| Priority queues | Use separate services per priority tier instead — simpler, no starvation risk |
| Task scheduling (cron/delayed) | Different product — use external schedulers (K8s CronJobs) that submit to gateway |
| Workflow orchestration / DAGs | Turns gateway into workflow engine (Temporal territory) — callers orchestrate |
| Web UI dashboard | Prometheus + Grafana provides better observability — avoid frontend maintenance |
| Multi-region federation | Run independent instances per region — cross-region is caller's problem |
| Rate limiting per client | Defer to API gateway (nginx/Envoy) in front — infrastructure concern |
| Streaming/WebSocket results | Poll + callback covers all practical use cases |
| Dynamic service loading (.so/.dylib) | Opaque payloads with universal API is simpler and more secure |
| Payload encryption at rest | Callers encrypt before submission — gateway treats payloads as opaque bytes |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| TASK-01 | Phase 1 | Complete |
| TASK-02 | Phase 1 | Complete |
| TASK-03 | Phase 1 | Complete |
| TASK-04 | Phase 1 | Complete |
| RSLT-01 | Phase 1 | Complete |
| RSLT-02 | Phase 1 | Complete |
| RSLT-03 | Phase 4 | Complete |
| RSLT-04 | Phase 4 | Complete |
| RSLT-05 | Phase 1 | Complete |
| SRVC-01 | Phase 3 | Complete |
| SRVC-02 | Phase 1 | Complete |
| SRVC-03 | Phase 3 | Complete |
| SRVC-04 | Phase 3 | Complete |
| NODE-01 | Phase 1 | Complete |
| NODE-02 | Phase 1 | Complete |
| NODE-03 | Phase 3 | Complete |
| NODE-04 | Phase 1 | Complete |
| NODE-05 | Phase 3 | Complete |
| NODE-06 | Phase 3 | Complete |
| LIFE-01 | Phase 1 | Complete |
| LIFE-02 | Phase 1 | Complete |
| LIFE-03 | Phase 4 | Complete |
| LIFE-04 | Phase 4 | Pending |
| LIFE-05 | Phase 4 | Pending |
| AUTH-01 | Phase 2 | Complete |
| AUTH-02 | Phase 2 | Complete |
| AUTH-03 | Phase 2 | Complete |
| OBSV-01 | Phase 5 | Complete |
| OBSV-02 | Phase 5 | Complete |
| OBSV-03 | Phase 5 | Complete |
| INFR-01 | Phase 1 | Complete |
| INFR-02 | Phase 1 | Complete |
| INFR-03 | Phase 5 | Pending |
| INFR-04 | Phase 5 | Pending |
| INFR-05 | Phase 2 | Complete |
| INFR-06 | Phase 2 | Complete |

**Coverage:**
- v1 requirements: 36 total
- Mapped to phases: 36
- Unmapped: 0

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after roadmap creation*
