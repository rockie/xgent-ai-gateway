# Architecture Research

**Domain:** Pull-model task gateway (Rust, gRPC/HTTPS, Redis-backed)
**Researched:** 2026-03-21
**Confidence:** HIGH

## Standard Architecture

### System Overview

```
                    PUBLIC INTERNET                         PRIVATE NETWORK
                                                            (behind NAT)
  ┌──────────┐                                            ┌──────────────┐
  │ Client A │──┐                                    ┌────│   Node 1     │
  │ (HTTPS)  │  │                                    │    │ (service: llm)│
  └──────────┘  │    ┌───────────────────────────┐   │    └──────────────┘
                │    │      GATEWAY PROCESS       │   │
  ┌──────────┐  │    │                           │   │    ┌──────────────┐
  │ Client B │──┼───▶│  ┌─────────────────────┐  │◀──┼────│   Node 2     │
  │ (gRPC)   │  │    │  │  Protocol Layer      │  │   │    │ (service: llm)│
  └──────────┘  │    │  │  (Axum + Tonic)      │  │   │    └──────────────┘
                │    │  └─────────┬───────────┘  │   │
  ┌──────────┐  │    │            │              │   │    ┌──────────────┐
  │ Client C │──┘    │  ┌─────────▼───────────┐  │   └────│   Node 3     │
  │ (gRPC+   │       │  │  Core Engine         │  │        │ (service: ci)│
  │  mTLS)   │       │  │  - Auth              │  │        └──────────────┘
  └──────────┘       │  │  - Task Router       │  │
                     │  │  - Queue Manager     │  │
                     │  │  - Node Registry     │  │
                     │  │  - Result Dispatcher │  │
                     │  └─────────┬───────────┘  │
                     │            │              │
                     │  ┌─────────▼───────────┐  │
                     │  │  Storage Layer       │  │
                     │  │  (Redis/Valkey)      │  │
                     │  └─────────────────────┘  │
                     └───────────────────────────┘
```

### Component Responsibilities

| Component | Responsibility | Typical Implementation |
|-----------|----------------|------------------------|
| **Protocol Layer** | Accept gRPC and HTTPS on the same port, route to handlers | Axum + Tonic multiplexed via `content-type` header detection on a shared Hyper server |
| **Auth Middleware** | Validate API keys (HTTP), mTLS certs (gRPC), pre-shared tokens (nodes) | Tower middleware layers; rustls for mTLS; API key lookup against Redis/config |
| **Task Router** | Accept task submissions, assign task IDs, enqueue to correct service queue | Thin handler that validates payload, generates nanoid/ulid, pushes to Redis list |
| **Queue Manager** | Maintain per-service FIFO queues, handle reliable dequeue, timeout reaping | Redis lists with BRPOPLPUSH for atomic dequeue into processing list; background reaper task |
| **Node Registry** | Track which nodes exist per service, their health, last heartbeat | Redis hash per service mapping node_id to metadata; TTL-based liveness |
| **Task Lifecycle Tracker** | Manage state machine: pending -> assigned -> running -> completed/failed | Redis hash per task_id storing state, timestamps, assigned_node, retry count |
| **Result Dispatcher** | Deliver results to polling clients and fire optional HTTP callbacks | Read from task hash on poll; background tokio task for callback POST with retries |
| **Node Poll Handler** | Handle long-poll or streaming connections from internal nodes requesting work | gRPC server-streaming or unary with blocking dequeue from Redis; Tokio channel bridge |

## Recommended Project Structure

```
src/
├── main.rs                 # Entry point, server bootstrap, signal handling
├── config.rs               # Configuration from env/file (clap + config crate)
├── server/
│   ├── mod.rs              # Server builder, protocol multiplexing
│   ├── http.rs             # Axum router for HTTPS endpoints
│   └── grpc.rs             # Tonic service implementations
├── proto/                  # .proto files and generated code
│   ├── gateway.proto       # Client-facing service definition
│   └── worker.proto        # Node-facing service definition
├── auth/
│   ├── mod.rs              # Auth trait and dispatch
│   ├── api_key.rs          # API key validation (HTTPS clients)
│   ├── mtls.rs             # mTLS cert validation (gRPC clients)
│   └── token.rs            # Pre-shared token validation (nodes)
├── queue/
│   ├── mod.rs              # Queue trait
│   ├── redis.rs            # Redis-backed reliable queue implementation
│   └── task.rs             # Task struct, state machine, serialization
├── registry/
│   ├── mod.rs              # Service and node registry
│   └── health.rs           # Heartbeat checking, stale node reaping
├── dispatch/
│   ├── mod.rs              # Result delivery orchestration
│   ├── poll.rs             # Client poll handler
│   └── callback.rs         # HTTP callback delivery with retries
├── error.rs                # Unified error types
└── telemetry.rs            # Tracing, metrics setup
```

### Structure Rationale

- **server/:** Isolates protocol concerns. The multiplexing logic lives here, not in business logic. Axum routes and Tonic services are separate files because they have different middleware stacks.
- **proto/:** Separate .proto files for client-facing vs node-facing APIs. These are different trust boundaries with different auth requirements.
- **auth/:** Three distinct auth mechanisms warrant their own module. A common trait allows the middleware to dispatch without coupling to a specific scheme.
- **queue/:** The queue is the heart of the system. Trait-based design allows testing with an in-memory queue while production uses Redis. Task state machine logic lives here.
- **registry/:** Service and node management is distinct from queueing. A service exists independently of whether it has tasks or nodes.
- **dispatch/:** Result delivery is its own concern -- polling is synchronous, callbacks are async with retries, and they share nothing except reading from the task store.

## Architectural Patterns

### Pattern 1: Reliable Queue via BRPOPLPUSH (now BLMOVE)

**What:** When a node picks up a task, atomically move it from the pending queue to a processing list. The task is not deleted until the node confirms completion. If the node dies, a reaper notices the task in the processing list and re-enqueues it.

**When to use:** Always, for every task dequeue operation. This is the foundation of reliability.

**Trade-offs:** Slightly more complex than simple LPOP, but prevents task loss on node crashes. The reaper adds a background goroutine but is essential.

**Redis key layout:**
```
queue:{service_id}:pending       # LIST - tasks waiting for pickup
queue:{service_id}:processing    # LIST - tasks currently being worked on
task:{task_id}                   # HASH - task state, payload, result, timestamps
service:{service_id}:nodes       # HASH - node_id -> {last_heartbeat, status}
auth:apikeys                     # HASH - api_key -> client_id
```

**Example (pseudo-Rust):**
```rust
// Atomic dequeue: blocks until a task is available, then moves it
// from pending to processing in one atomic operation
let task_id: String = redis.blmove(
    &format!("queue:{}:pending", service_id),
    &format!("queue:{}:processing", service_id),
    Duration::from_secs(30), // long-poll timeout
).await?;

// Update task state
redis.hset(&format!("task:{}", task_id), &[
    ("state", "assigned"),
    ("assigned_node", node_id),
    ("assigned_at", &now_unix()),
]).await?;
```

### Pattern 2: Protocol Multiplexing (Axum + Tonic on one port)

**What:** Run both HTTP/1.1 (Axum) and gRPC/HTTP2 (Tonic) on the same TCP port by inspecting the `content-type` header. If it starts with `application/grpc`, route to Tonic; otherwise route to Axum.

**When to use:** Always. A single port simplifies deployment, load balancer config, and TLS termination.

**Trade-offs:** Slightly more complex server setup, but well-documented pattern with official Axum examples. Both frameworks share the Tower middleware ecosystem.

**Example:**
```rust
// Tonic can convert its services into Axum routes directly
let grpc_service = tonic::transport::Server::builder()
    .add_service(GatewayServiceServer::new(gateway))
    .add_service(WorkerServiceServer::new(worker))
    .into_router();

let app = Router::new()
    .route("/v1/tasks", post(submit_task))
    .route("/v1/tasks/:id", get(get_task))
    .route("/v1/tasks/:id/result", get(get_result))
    .merge(grpc_service);
```

### Pattern 3: Separate Proto Services for Clients vs Nodes

**What:** Define two separate gRPC service definitions: `GatewayService` (client-facing) and `WorkerService` (node-facing). They share the `Task` message type but have different RPCs and different auth requirements.

**When to use:** Always. Clients and nodes are different trust boundaries with different capabilities.

**Trade-offs:** Two .proto files to maintain, but clean separation of concerns. A node should never be able to call client APIs and vice versa.

```protobuf
// gateway.proto - Client-facing
service GatewayService {
  rpc SubmitTask(SubmitTaskRequest) returns (SubmitTaskResponse);
  rpc GetTaskStatus(GetTaskStatusRequest) returns (TaskStatus);
  rpc GetTaskResult(GetTaskResultRequest) returns (TaskResult);
}

// worker.proto - Node-facing
service WorkerService {
  rpc PollTask(PollTaskRequest) returns (PollTaskResponse);       // Unary long-poll
  rpc StreamTasks(StreamTasksRequest) returns (stream Task);      // Server-streaming alternative
  rpc ReportResult(ReportResultRequest) returns (ReportResultResponse);
  rpc Heartbeat(HeartbeatRequest) returns (HeartbeatResponse);
}
```

### Pattern 4: Node Task Pickup via gRPC Server-Streaming

**What:** Nodes open a persistent gRPC server-streaming connection. The gateway sends tasks down the stream as they become available. This is more efficient than repeated long-poll requests.

**When to use:** Preferred for gRPC nodes. Offer unary long-poll as a fallback for HTTP-only nodes.

**Trade-offs:** Server-streaming keeps connections open, consuming server resources per connected node. At the v1 target of ~100 nodes, this is fine. The gateway must handle stream lifecycle (reconnection, backpressure). Simpler alternative: unary `PollTask` with a 30-second blocking wait (long-poll), which is stateless on the server side.

**Recommendation:** Implement unary long-poll first (simpler, stateless). Add server-streaming as an optimization in a later phase.

## Data Flow

### Task Submission Flow

```
Client                    Gateway                         Redis
  │                         │                               │
  │── POST /v1/tasks ──────▶│                               │
  │   (or SubmitTask RPC)   │── validate auth ──────────────│
  │                         │── generate task_id            │
  │                         │── HSET task:{id} state=pending│
  │                         │── LPUSH queue:{svc}:pending   │
  │◀── 202 {task_id} ──────│                               │
```

### Node Task Pickup Flow

```
Node                      Gateway                         Redis
  │                         │                               │
  │── PollTask(service) ───▶│                               │
  │   (or long-poll HTTP)   │── validate node token         │
  │                         │── BLMOVE pending -> processing│
  │                         │   (blocks up to 30s)          │
  │                         │── HSET task:{id} state=assigned│
  │◀── Task payload ───────│                               │
  │                         │                               │
  │   ... node executes ... │                               │
  │                         │                               │
  │── ReportResult ────────▶│                               │
  │                         │── HSET task:{id} result=...   │
  │                         │── HSET task:{id} state=complete│
  │                         │── LREM processing task_id     │
  │                         │── fire callback (if set) ────▶│ (external URL)
  │◀── ACK ────────────────│                               │
```

### Result Retrieval Flow

```
Client                    Gateway                         Redis
  │                         │                               │
  │── GET /v1/tasks/{id} ──▶│                               │
  │                         │── HGETALL task:{id}           │
  │◀── {state, result} ────│                               │
```

### Key Data Flows

1. **Task submission:** Client -> Auth middleware -> Task Router -> Redis (task hash + queue push) -> 202 response with task_id
2. **Node pickup:** Node -> Auth middleware -> Queue Manager -> Redis BLMOVE (blocks until task available) -> Task payload to node
3. **Result delivery:** Node -> Auth middleware -> Task Lifecycle Tracker -> Redis (update hash, remove from processing) -> Optional callback dispatch
4. **Client polling:** Client -> Auth middleware -> Redis HGETALL -> State/result response
5. **Health monitoring:** Background reaper task -> scan processing lists -> check task timeouts -> re-enqueue expired tasks -> check node heartbeat TTLs -> mark stale nodes

## Scaling Considerations

| Scale | Architecture Adjustments |
|-------|--------------------------|
| 1-100 nodes, 1K tasks/hr | Single gateway process, single Redis instance. This is v1 target. No special optimizations needed. |
| 100-1K nodes, 100K tasks/hr | Redis Cluster for queue sharding by service_id. Multiple gateway replicas behind a load balancer (stateless design enables this). Connection pooling for Redis. |
| 1K+ nodes, 1M+ tasks/hr | Shard services across gateway clusters. Consider dedicated Redis instances per high-volume service. Add metrics-driven autoscaling. At this point, evaluate whether to add a proper message broker (NATS, Kafka) behind the gateway. |

### Scaling Priorities

1. **First bottleneck: Redis connections.** Each blocking BLMOVE holds a connection for up to 30 seconds. With 100 nodes polling, that is 100 concurrent Redis connections just for task pickup. Use a dedicated Redis connection pool for blocking operations, separate from the general pool. Size it to node_count + buffer.
2. **Second bottleneck: Callback delivery.** If many tasks complete simultaneously and all have callbacks, the gateway must fire many outbound HTTP requests. Use a bounded Tokio task pool for callbacks with backpressure, and retry with exponential backoff. Do not block task completion on callback success.
3. **Third bottleneck: Task payload size.** If tasks carry large payloads (e.g., images for AI inference), storing them in Redis is inefficient. For v1, set a max payload size (e.g., 1MB). For v2, add an object store (S3/MinIO) and store only references in Redis.

## Anti-Patterns

### Anti-Pattern 1: Pushing Tasks Directly to Nodes

**What people do:** Gateway maintains open connections to nodes and pushes tasks when they arrive, like a traditional load balancer.
**Why it is wrong:** Nodes are behind NAT and cannot receive inbound connections. Even if they could, push requires the gateway to track node capacity, handle connection failures, and implement circuit breaking -- all of which the pull model avoids.
**Do this instead:** Nodes poll the gateway. The gateway only needs to manage queues. Nodes self-regulate their concurrency by controlling how many poll requests they issue.

### Anti-Pattern 2: Using Redis Pub/Sub for Task Dispatch

**What people do:** Publish tasks to a Redis channel and have nodes subscribe.
**Why it is wrong:** Redis Pub/Sub is fire-and-forget. If no node is listening when a task is published, the task is lost. If multiple nodes receive the same message, you get duplicate execution. Pub/Sub has no persistence, no acknowledgment, no retry.
**Do this instead:** Use Redis lists with BLMOVE for reliable, exactly-once task delivery. Pub/Sub can be used as a notification mechanism ("new task available, go poll") but never as the delivery mechanism.

### Anti-Pattern 3: Storing Task State Only in the Queue

**What people do:** Put the entire task (payload + state) in the queue list. When it moves between states, they serialize/deserialize the whole thing.
**Why it is wrong:** Expensive for large payloads, makes state queries slow (you must scan lists), and complicates the processing list pattern.
**Do this instead:** Store task state in a Redis hash keyed by task_id. The queue lists contain only task_id strings. This separates the queue mechanism from the task data, making both simpler and faster.

### Anti-Pattern 4: Single Auth Mechanism for All Callers

**What people do:** Use the same API key scheme for clients and nodes.
**Why it is wrong:** Clients and nodes have fundamentally different trust levels and access patterns. A client API key should grant "submit tasks" and "read results." A node token should grant "pick up tasks for service X" and "report results." Mixing them risks privilege escalation.
**Do this instead:** Separate auth schemes with separate middleware. Client auth (API key or mTLS) and node auth (pre-shared token scoped to a service) should be validated by different code paths with different permission sets.

### Anti-Pattern 5: Blocking the Event Loop on Redis Operations

**What people do:** Use synchronous Redis calls or forget to use the async Redis driver.
**Why it is wrong:** A blocking BLMOVE call will block the entire Tokio runtime thread, starving all other connections on that thread.
**Do this instead:** Use the `redis` crate with the `tokio-comp` feature for fully async operations. For blocking operations like BLMOVE, use a dedicated connection (not from the shared pool) so the block does not affect other operations.

## Integration Points

### External Services

| Service | Integration Pattern | Notes |
|---------|---------------------|-------|
| Redis/Valkey | `redis` crate with `tokio-comp` and `connection-manager` features | Use connection pooling (`deadpool-redis` or `bb8-redis`). Separate pools for blocking vs non-blocking ops. |
| Callback URLs | `reqwest` with timeout and retry | Fire-and-forget with retry queue. Do not block task completion on callback success. Store callback status in task hash. |
| TLS/mTLS | `rustls` via `tonic`'s built-in TLS support | Configure `ServerTlsConfig` with CA cert for client verification. Use `rcgen` for dev cert generation. |

### Internal Boundaries

| Boundary | Communication | Notes |
|----------|---------------|-------|
| Protocol Layer <-> Core Engine | Direct Rust function calls via shared state (Arc) | No serialization overhead. Axum handlers and Tonic service impls call the same core functions. |
| Core Engine <-> Redis | Async Redis commands via connection pool | Abstract behind a `Store` trait for testability. All Redis logic in `queue/redis.rs`. |
| Auth <-> Core Engine | Tower middleware layers | Auth runs before handlers. Extracts identity into request extensions. Core reads identity from extensions. |
| Background Tasks <-> Core Engine | Tokio tasks with shared Arc state | Reaper, heartbeat checker, and callback dispatcher run as background Tokio tasks spawned at startup. |

## Build Order (Dependencies Between Components)

The following order respects dependencies -- each layer builds on the previous:

1. **Proto definitions + basic types** -- Everything depends on the shared message types. Define `gateway.proto`, `worker.proto`, `Task`, `TaskState` enum, error types.
2. **Redis storage layer** -- Queue and task state operations with the `Store` trait. This is the data foundation. Test with real Redis (use testcontainers).
3. **Core engine** -- Task router, queue manager, task lifecycle. Pure business logic calling the Store trait. Testable with mock store.
4. **Protocol layer (HTTP + gRPC)** -- Axum routes + Tonic services, multiplexed on one port. Wires protocol handling to core engine.
5. **Auth middleware** -- API key, mTLS, node tokens. Add as Tower layers. Can be built in parallel with protocol layer but wired in after.
6. **Node registry + health** -- Service/node management, heartbeat processing, stale node reaping. Depends on storage layer.
7. **Result dispatch** -- Client polling (simple, just reads Redis) and callback delivery (needs background task, retry logic).
8. **Background tasks** -- Timeout reaper, heartbeat monitor. These are the "operational reliability" layer.

**Phase grouping recommendation:**
- Phase 1: Items 1-4 (core loop: submit task, pick up task, report result -- no auth)
- Phase 2: Items 5-6 (auth + node management -- production hardening)
- Phase 3: Items 7-8 (result delivery + operational reliability)

## Sources

- [Axum + Tonic multiplexing example](https://github.com/sunsided/http-grpc-cohosting) -- Reference implementation for running both on one port
- [Axum gRPC multiplex discussion](https://github.com/tokio-rs/axum/discussions/1840) -- Community patterns for protocol multiplexing
- [Conductor Architecture](https://orkes.io/content/conductor-architecture) -- Pull-based worker polling architecture reference
- [Conductor Task Lifecycle](https://conductor-oss.github.io/conductor/devguide/architecture/tasklifecycle.html) -- Task state machine with timeouts and retries
- [Redis BRPOPLPUSH/BLMOVE](https://redis.io/docs/latest/commands/rpoplpush/) -- Reliable queue pattern documentation
- [Resc - Rust task orchestrator](https://github.com/Canop/resc) -- Rust implementation of Redis-based task orchestration with BRPOPLPUSH
- [Tonic mTLS discussion](https://github.com/hyperium/tonic/issues/511) -- mTLS authorization patterns with tonic
- [Rust mTLS example](https://github.com/camelop/rust-mtls-example) -- Complete mTLS client/server example
- [Tonic gRPC streaming](https://docs.rs/tonic/latest/tonic/struct.Streaming.html) -- Server-side streaming API reference

---
*Architecture research for: Rust pull-model task gateway*
*Researched: 2026-03-21*
