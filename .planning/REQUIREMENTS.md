# Requirements: xgent-ai-gateway

**Defined:** 2026-03-21
**Core Value:** Tasks submitted by clients reliably reach internal nodes and results reliably flow back, regardless of network topology

## v1 Requirements

### Task Submission

- [ ] **TASK-01**: Client can submit a task via gRPC with an opaque payload and receive a task ID
- [ ] **TASK-02**: Client can submit a task via HTTPS REST with an opaque payload and receive a task ID
- [ ] **TASK-03**: Client can attach arbitrary key-value metadata/labels to a task at submission
- [ ] **TASK-04**: Task payloads are treated as opaque bytes — the gateway does not interpret payload content

### Task Results

- [ ] **RSLT-01**: Client can poll task status and result by task ID via gRPC
- [ ] **RSLT-02**: Client can poll task status and result by task ID via HTTPS REST
- [ ] **RSLT-03**: Client can optionally provide a callback URL at submission for result delivery
- [ ] **RSLT-04**: Gateway delivers results to callback URL with exponential backoff retries on failure
- [ ] **RSLT-05**: Task results are stored in Redis with a configurable TTL

### Service Registry

- [ ] **SRVC-01**: Admin can register a new service with the gateway (name, config, node auth tokens)
- [ ] **SRVC-02**: Each registered service gets its own isolated task queue
- [ ] **SRVC-03**: Admin can deregister a service (drains queue, removes config)
- [ ] **SRVC-04**: Service configuration is persisted in Redis and survives gateway restarts

### Node Management

- [ ] **NODE-01**: Internal nodes can reverse-poll the gateway via gRPC to pick up tasks for their service
- [ ] **NODE-02**: Internal nodes can reverse-poll the gateway via HTTPS to pick up tasks for their service
- [ ] **NODE-03**: Nodes authenticate with pre-shared tokens scoped to their service
- [ ] **NODE-04**: Nodes report task completion (success or failure) with result payload back to the gateway
- [ ] **NODE-05**: Gateway tracks node health via heartbeat (last poll time, stale detection)
- [ ] **NODE-06**: Nodes can signal graceful drain — gateway stops assigning new tasks, waits for in-flight completion

### Task Lifecycle

- [ ] **LIFE-01**: Tasks follow a state machine: pending → assigned → running → completed/failed
- [ ] **LIFE-02**: Gateway uses reliable queue pattern (atomic move to processing list) to prevent task loss
- [ ] **LIFE-03**: Background reaper detects timed-out tasks (node died) and re-queues them
- [ ] **LIFE-04**: Failed tasks retry with configurable max retries and exponential backoff
- [ ] **LIFE-05**: Tasks exhausting retries move to a per-service dead letter queue

### Authentication

- [ ] **AUTH-01**: HTTPS clients authenticate via API key (bearer token)
- [ ] **AUTH-02**: gRPC clients authenticate via mTLS (mutual TLS certificates)
- [ ] **AUTH-03**: Internal nodes authenticate via pre-shared tokens validated on each poll

### Observability

- [ ] **OBSV-01**: Gateway emits structured JSON logs with task ID, service, and node context in every log line
- [ ] **OBSV-02**: Gateway exposes Prometheus metrics endpoint (queue depth, task latency, node counts, error rates)
- [ ] **OBSV-03**: Node health dashboard data available via admin API (active nodes, last seen, in-flight tasks)

### Infrastructure

- [ ] **INFR-01**: Gateway connects to Redis/Valkey for all persistent state (queues, results, config)
- [ ] **INFR-02**: Gateway is configurable via environment variables with optional TOML config file override
- [ ] **INFR-03**: Gateway builds as a single static binary (musl target)
- [ ] **INFR-04**: Gateway ships as a Docker image
- [ ] **INFR-05**: Gateway supports TLS termination for HTTPS and gRPC
- [ ] **INFR-06**: Gateway configures HTTP/2 keepalive pings to prevent silent connection death through NAT/LB

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
| TASK-01 | — | Pending |
| TASK-02 | — | Pending |
| TASK-03 | — | Pending |
| TASK-04 | — | Pending |
| RSLT-01 | — | Pending |
| RSLT-02 | — | Pending |
| RSLT-03 | — | Pending |
| RSLT-04 | — | Pending |
| RSLT-05 | — | Pending |
| SRVC-01 | — | Pending |
| SRVC-02 | — | Pending |
| SRVC-03 | — | Pending |
| SRVC-04 | — | Pending |
| NODE-01 | — | Pending |
| NODE-02 | — | Pending |
| NODE-03 | — | Pending |
| NODE-04 | — | Pending |
| NODE-05 | — | Pending |
| NODE-06 | — | Pending |
| LIFE-01 | — | Pending |
| LIFE-02 | — | Pending |
| LIFE-03 | — | Pending |
| LIFE-04 | — | Pending |
| LIFE-05 | — | Pending |
| AUTH-01 | — | Pending |
| AUTH-02 | — | Pending |
| AUTH-03 | — | Pending |
| OBSV-01 | — | Pending |
| OBSV-02 | — | Pending |
| OBSV-03 | — | Pending |
| INFR-01 | — | Pending |
| INFR-02 | — | Pending |
| INFR-03 | — | Pending |
| INFR-04 | — | Pending |
| INFR-05 | — | Pending |
| INFR-06 | — | Pending |

**Coverage:**
- v1 requirements: 36 total
- Mapped to phases: 0
- Unmapped: 36 ⚠️

---
*Requirements defined: 2026-03-21*
*Last updated: 2026-03-21 after initial definition*
