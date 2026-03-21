# Feature Research

**Domain:** Pull-model task gateway / distributed job queue
**Researched:** 2026-03-21
**Confidence:** HIGH

## Feature Landscape

### Table Stakes (Users Expect These)

Features users assume exist in any task queue / job gateway system. Missing these and operators will choose Temporal, Hatchet, Celery, or BullMQ instead.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| Task submission (gRPC + HTTPS) | Dual protocol is the stated requirement; gRPC for internal/high-perf, HTTPS for broad compatibility | MEDIUM | tonic for gRPC, axum for HTTP, shared handler layer underneath |
| Async task lifecycle (submit -> ID -> poll/callback) | Every queue system returns a handle; blocking callers on multi-second AI tasks is unacceptable | MEDIUM | State machine: pending -> assigned -> running -> completed/failed |
| Task status polling | Callers need to check "is it done yet?" without callbacks | LOW | Simple Redis GET by task ID, expose via both protocols |
| Callback delivery | Many callers prefer webhooks over polling; Celery, Cloud Tasks, and Hatchet all support this | MEDIUM | HTTP POST with configurable retry on delivery failure; must not block main queue processing |
| Service-scoped queues | Multi-tenant gateway needs isolation; each registered service gets its own queue | LOW | Redis list per service, simple namespace pattern |
| Node authentication | Nodes pulling work must prove identity; every production queue system requires this | LOW | Pre-shared token per service, validated on each poll |
| Client authentication | API keys for HTTPS, mTLS for gRPC; standard for any internet-facing API | MEDIUM | mTLS adds complexity (cert management, rotation) but is expected for gRPC |
| Task timeout detection | If a node takes work and dies, the task must not be lost forever | MEDIUM | Assign deadline at dispatch, background reaper moves expired tasks back to queue |
| Retry with backoff | Transient failures are the norm in distributed systems; auto-retry is expected | MEDIUM | Configurable max retries + exponential backoff; Temporal defaults to 1s initial, 2x coefficient, 100s cap |
| Dead letter queue (DLQ) | Tasks that exhaust retries need a place to go for manual inspection | LOW | Separate Redis list per service; tasks moved here after max retries |
| Node health tracking | Gateway must know which nodes are alive to avoid dispatching to dead nodes | MEDIUM | Heartbeat-based: nodes report liveness on each poll; mark stale after configurable interval |
| Graceful shutdown / drain | Nodes need to finish in-flight work before exiting; every production queue supports this | LOW | Node signals "draining", gateway stops assigning new tasks, waits for in-flight completion |
| Task result storage | Callers need to retrieve results after completion; results must survive gateway restarts | LOW | Store in Redis with configurable TTL; return via poll or callback |
| Structured logging | Operators need to debug task flows; JSON structured logs are baseline expectation | LOW | tracing crate with JSON formatter; include task ID, service, node in every log line |
| Configuration via env vars / config file | Standard deployment practice; 12-factor app compliance | LOW | Use environment variables with optional TOML/YAML config file override |

### Differentiators (Competitive Advantage)

Features that set xgent-ai-gateway apart from generic job queues. These align with the core value proposition: reliable cross-network-boundary task brokering for AI/agent workloads.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| Pull-model with NAT traversal | The core differentiator: nodes behind firewalls/NAT pull work without inbound connectivity. Celery, BullMQ, Sidekiq all require workers to reach the broker directly. This gateway IS the broker on the public internet. | HIGH | Long-poll or server-sent gRPC streams from nodes; gateway must handle thousands of idle connections efficiently |
| Protocol-native dual interface (gRPC + HTTP) | Not a sidecar or adapter -- native gRPC and HTTP on the same binary. Most queue systems are single-protocol or require separate gateway layers. | MEDIUM | tonic + axum sharing the same tokio runtime; protobuf definitions shared |
| Single static binary deployment | Unlike Temporal (requires server cluster), Hatchet (requires Postgres), or Celery (requires Python runtime) -- one binary, one Redis. Drastically simpler ops. | LOW | Rust compiles to static binary by default; this is a natural advantage |
| Service-level node pool management | Register/deregister nodes per service, view pool health. Goes beyond simple worker discovery -- it's a managed fleet view per service. | MEDIUM | Service registry in Redis with node metadata (last seen, capacity, version) |
| Language-agnostic worker protocol | Any language that speaks gRPC or HTTP can be a node. Celery = Python, Sidekiq = Ruby, BullMQ = Node.js. This gateway is polyglot by design. | LOW | Well-defined protobuf contract for node <-> gateway communication |
| Task-level OpenTelemetry tracing | Inject trace context at submission, propagate through assignment and execution, emit spans for full task lifecycle. Most queue systems bolt this on; building it in from day one is a differentiator. | MEDIUM | opentelemetry-rust crate; propagate W3C trace context through task metadata |
| Redis/Valkey-only dependency | No Postgres, no Kafka, no RabbitMQ. Single external dependency reduces operational burden significantly. Hatchet requires Postgres; Temporal requires its own cluster. | LOW | Already a design constraint; promote it as a feature |
| Task metadata / labels | Attach arbitrary key-value metadata to tasks for filtering, routing, and observability. Enables "send this task to a node with GPU" without priority queues. | LOW | Store as hash fields alongside task; nodes can filter on poll |
| Bulk task submission | Submit batches of tasks in a single RPC call. Important for CI pipelines and batch AI inference. | LOW | Array wrapper around single-task submission; amortize network round-trips |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem valuable but add disproportionate complexity, contradict the architecture, or dilute focus for v1.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| Priority queues | "Some tasks are more urgent." | Adds queue complexity, starvation risk for low-priority tasks, harder to reason about fairness. Hatchet supports it but it's a common source of bugs. | Use separate services (queues) for different priority tiers. A "fast-inference" service and a "batch-inference" service. Service-level isolation is simpler and more predictable. |
| Task scheduling (cron/delayed) | "Run this every hour" or "run in 5 minutes." | Requires persistent scheduler state, clock drift handling, timezone complexity. This is a different product (cron service). | Defer to external schedulers (Kubernetes CronJobs, systemd timers) that submit tasks to the gateway. Keep the gateway stateless w.r.t. time. |
| Workflow orchestration / DAGs | "Chain tasks A -> B -> C." | Turns a simple queue into a workflow engine (Temporal territory). Massive complexity increase for state management, partial failure, fan-out/fan-in. | Callers orchestrate: submit A, get result, submit B. Or build a thin orchestrator client that uses the gateway as transport. The gateway routes tasks, not workflows. |
| Web UI dashboard | "I want to see tasks visually." | Adds frontend stack, auth for UI, ongoing maintenance. Distracts from core gateway quality. | Expose metrics (Prometheus) and structured logs. Grafana dashboards provide better observability than a bespoke UI. CLI tool for task inspection. |
| Multi-region federation | "Tasks should route to the nearest region." | Requires consensus protocols, cross-region Redis replication, conflict resolution. Enormous complexity. | Run independent gateway instances per region. Clients choose their region's endpoint. Cross-region coordination is the caller's problem. |
| Rate limiting per client | "Prevent one client from flooding the system." | Adds token bucket state, complicates queue semantics, edge cases with retries. | Defer to an API gateway (nginx, Envoy) in front of the task gateway. Rate limiting is an infrastructure concern, not a task queue concern. |
| Task data encryption at rest | "Encrypt task payloads in Redis." | Redis already supports TLS in transit; at-rest encryption in Redis requires external solutions. Task payloads are opaque bytes -- callers can encrypt before submission. | Document that callers should encrypt sensitive payloads before submission. Support Redis TLS for in-transit encryption. |
| Streaming/WebSocket result delivery | "Push results to me in real-time." | Requires persistent connection management, reconnection logic, and fundamentally different delivery semantics from poll/callback. | Poll endpoint with ETag/long-poll for near-real-time. Callback webhook for push. These two patterns cover all practical use cases. |
| Plugin/middleware system | "Let users extend the gateway with custom logic." | Plugin APIs ossify quickly, security concerns with arbitrary code execution, testing matrix explodes. | Well-defined protobuf contracts let callers build middleware externally. Gateway stays focused on routing. |

## Feature Dependencies

```
[Service Registration]
    |
    +--requires--> [Service-Scoped Queues]
    |                   |
    |                   +--requires--> [Task Submission (gRPC + HTTP)]
    |                   |
    |                   +--requires--> [Task Result Storage]
    |                                       |
    |                                       +--enables--> [Status Polling]
    |                                       +--enables--> [Callback Delivery]
    |
    +--requires--> [Node Pool Management]
                        |
                        +--requires--> [Node Authentication]
                        +--requires--> [Node Health Tracking]
                        +--enables--> [Task Assignment via Pull]

[Task Timeout Detection]
    +--requires--> [Task Lifecycle State Machine]
    +--enables--> [Retry with Backoff]
                      +--enables--> [Dead Letter Queue]

[Client Authentication]
    +--independent-- (needed before any client-facing endpoint goes live)

[OpenTelemetry Tracing]
    +--enhances--> [Task Lifecycle State Machine]
    +--enhances--> [Node Health Tracking]

[Task Metadata / Labels]
    +--enhances--> [Task Assignment via Pull] (node-side filtering)
```

### Dependency Notes

- **Task Submission requires Service-Scoped Queues:** Tasks are always submitted to a specific service's queue. Services must be registered first.
- **Status Polling and Callback require Task Result Storage:** Cannot return results without storing them.
- **Retry with Backoff requires Task Timeout Detection:** Retries are triggered when tasks time out or nodes report failure. Timeout detection is the primary trigger.
- **Dead Letter Queue requires Retry:** DLQ is where tasks go after exhausting retries. Without retry logic, there is no DLQ trigger.
- **Node Health Tracking requires Node Authentication:** Cannot track health of unauthenticated nodes.
- **OpenTelemetry is independent but enhances everything:** Can be wired in at any phase, but benefits compound when added early.

## MVP Definition

### Launch With (v1)

Minimum viable product -- what's needed to validate the pull-model gateway concept.

- [ ] Service registration (register a named service with its queue) -- foundation for everything
- [ ] Task submission via gRPC -- primary high-performance interface
- [ ] Task submission via HTTPS -- broad compatibility interface
- [ ] Async task lifecycle (pending -> assigned -> running -> completed/failed) -- core state machine
- [ ] Task status polling (gRPC + HTTP) -- callers need to get results
- [ ] Node authentication (pre-shared tokens) -- security baseline
- [ ] Client authentication (API keys for HTTP, mTLS for gRPC) -- security baseline
- [ ] Node reverse-polling to pick up tasks -- the core differentiator
- [ ] Node result reporting -- tasks must complete the round trip
- [ ] Task result storage in Redis with TTL -- durability across restarts
- [ ] Task timeout detection and reassignment -- handle node failures
- [ ] Retry with exponential backoff -- handle transient failures
- [ ] Dead letter queue -- capture permanently failed tasks
- [ ] Node health tracking via heartbeat -- know which nodes are alive
- [ ] Structured JSON logging -- operational baseline
- [ ] Prometheus metrics endpoint -- operational baseline

### Add After Validation (v1.x)

Features to add once the core loop is proven and real workloads are running.

- [ ] Callback/webhook delivery -- add when polling proves insufficient for latency-sensitive callers
- [ ] OpenTelemetry distributed tracing -- add when debugging cross-service task flows becomes painful
- [ ] Task metadata and labels -- add when users need routing beyond round-robin
- [ ] Bulk task submission -- add when batch workloads (CI, batch inference) are onboarded
- [ ] Graceful node drain -- add when production deployments need zero-downtime node updates
- [ ] Service-level node pool dashboard (CLI) -- add when operators need fleet visibility beyond metrics
- [ ] Task cancellation -- add when long-running tasks need to be aborted

### Future Consideration (v2+)

Features to defer until the gateway has proven its value and operational patterns are understood.

- [ ] Priority queues -- defer because service-level separation handles most priority cases; add only if clear demand emerges
- [ ] Rate limiting per client -- defer to API gateway layer; add only if built-in rate limiting proves necessary
- [ ] Task scheduling (delayed/cron) -- defer because external schedulers handle this; add only if "submit with delay" is heavily requested
- [ ] Multi-region federation -- defer because single-region covers v1 scale targets; add only if cross-region routing is needed
- [ ] Web UI -- defer because Grafana + CLI covers observability; add only if non-technical operators need access

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority |
|---------|------------|---------------------|----------|
| Task submission (gRPC + HTTP) | HIGH | MEDIUM | P1 |
| Async task lifecycle | HIGH | MEDIUM | P1 |
| Node reverse-polling (pull model) | HIGH | HIGH | P1 |
| Task status polling | HIGH | LOW | P1 |
| Node authentication | HIGH | LOW | P1 |
| Client authentication | HIGH | MEDIUM | P1 |
| Service-scoped queues | HIGH | LOW | P1 |
| Task timeout + reassignment | HIGH | MEDIUM | P1 |
| Retry with backoff | HIGH | MEDIUM | P1 |
| Dead letter queue | MEDIUM | LOW | P1 |
| Task result storage (Redis) | HIGH | LOW | P1 |
| Node health tracking | HIGH | MEDIUM | P1 |
| Structured logging | MEDIUM | LOW | P1 |
| Prometheus metrics | MEDIUM | LOW | P1 |
| Callback delivery | MEDIUM | MEDIUM | P2 |
| OpenTelemetry tracing | MEDIUM | MEDIUM | P2 |
| Task metadata / labels | MEDIUM | LOW | P2 |
| Bulk task submission | MEDIUM | LOW | P2 |
| Graceful node drain | MEDIUM | LOW | P2 |
| Task cancellation | MEDIUM | MEDIUM | P2 |
| CLI for task/node inspection | MEDIUM | MEDIUM | P2 |
| Priority queues | LOW | HIGH | P3 |
| Task scheduling | LOW | HIGH | P3 |
| Web UI | LOW | HIGH | P3 |
| Multi-region federation | LOW | HIGH | P3 |

**Priority key:**
- P1: Must have for launch -- validates the pull-model gateway concept
- P2: Should have, add after core is proven with real workloads
- P3: Nice to have, future consideration based on demand

## Competitor Feature Analysis

| Feature | Temporal | Hatchet | Celery | BullMQ | xgent-ai-gateway |
|---------|----------|---------|--------|--------|-------------------|
| Pull-model (NAT traversal) | Workers connect to Temporal server (pull) | Workers via gRPC (pull) | Workers connect to broker (pull) | Workers connect to Redis (pull) | Pull with public internet gateway -- nodes behind NAT need zero inbound ports |
| Protocol | gRPC | gRPC + REST | AMQP/Redis | Redis | gRPC + HTTPS native |
| Language support | Go, Java, Python, TypeScript, .NET | Python, TypeScript, Go | Python only | Node.js only | Any language (gRPC/HTTP) |
| Deployment complexity | Temporal server cluster + DB | Server + Postgres | Broker (RabbitMQ/Redis) + app | Redis + app | Single binary + Redis |
| Workflow orchestration | Full DAG/saga support | DAG support | Chaining/chords | Limited chaining | Not built in (by design) |
| Priority queues | Task queue routing | FIFO, LIFO, Round Robin, Priority | Yes (queue-based) | Yes | No (use separate services) |
| DLQ | Via workflow failure handling | Yes | Yes | Yes | Yes |
| Observability | Built-in event history + OTel | Web dashboard + metrics | OTel instrumentation available | Bull Board UI | Prometheus + OTel + structured logs |
| Retry policies | Configurable with backoff | Configurable | Configurable | Configurable | Configurable with exponential backoff |
| Scheduling | Cron workflows | Cron triggers | Celery Beat | Repeatable jobs | Not in scope (use external) |
| Rate limiting | Per task queue | Built-in | Via celery-throttle | Built-in | Not in scope (use API gateway) |

## Sources

- [Temporal Task Queues documentation](https://docs.temporal.io/task-queue)
- [Temporal Retry Policies](https://docs.temporal.io/encyclopedia/retry-policies)
- [Hatchet - distributed task queue](https://github.com/hatchet-dev/hatchet)
- [Hatchet features](https://hatchet.run/)
- [Bull vs Celery vs Sidekiq comparison](https://www.index.dev/skill-vs-skill/backend-sidekiq-vs-celery-vs-bull)
- [Modern Queueing Architectures - Celery, RabbitMQ, Redis, Temporal](https://medium.com/@pranavprakash4777/modern-queueing-architectures-celery-rabbitmq-redis-or-temporal-f93ea7c526ec)
- [BullMQ retrying failing jobs](https://docs.bullmq.io/guide/retrying-failing-jobs)
- [Google Cloud Tasks queue configuration](https://docs.cloud.google.com/tasks/docs/configuring-queues)
- [gRPC Health Checking Protocol](https://grpc.io/docs/guides/health-checking/)
- [QCon: Queue observability with OpenTelemetry](https://www.infoq.com/news/2026/03/queue-otel-observability/)
- [Task Queues resource list](https://taskqueues.com/)
- [Distributed Job Scheduler system design](https://blog.algomaster.io/p/design-a-distributed-job-scheduler)

---
*Feature research for: Pull-model task gateway / distributed job queue*
*Researched: 2026-03-21*
