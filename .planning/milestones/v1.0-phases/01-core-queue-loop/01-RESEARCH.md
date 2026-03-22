# Phase 1: Core Queue Loop - Research

**Researched:** 2026-03-21
**Domain:** Rust async gRPC/HTTP gateway with Redis Streams task queue
**Confidence:** HIGH

## Summary

Phase 1 establishes the foundational pull-model task gateway: clients submit tasks via gRPC or HTTPS, internal nodes reverse-poll via gRPC server-streaming to claim tasks, report results via unary RPC, and clients retrieve results by polling. All state is backed by Redis Streams with consumer group semantics for reliable delivery.

The core technical challenges are: (1) setting up a 2-crate Cargo workspace with tonic-build codegen in the `proto` crate, (2) implementing Redis Streams consumer groups for per-service task queues with proper XREADGROUP/XACK lifecycle, (3) implementing gRPC server-streaming for node task dispatch with disconnect detection, and (4) running dual-port Axum + Tonic listeners. All libraries are mature, well-documented, and version-compatible per the project stack in CLAUDE.md.

**Primary recommendation:** Build bottom-up -- Redis queue layer first, then proto definitions + codegen, then gRPC service implementations, then HTTP REST layer mirroring gRPC, then the node-side runner agent proxy. Each layer is independently testable.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use Redis Streams (XADD/XREADGROUP/XACK) instead of list-based BLMOVE
- **D-02:** Each registered service gets its own stream (e.g., `tasks:{service_name}`)
- **D-03:** Each service has one consumer group; each node is a consumer in that group
- **D-04:** Valkey fully supports Streams -- no compatibility constraints
- **D-05:** Dual port -- separate listeners for gRPC and HTTP
- **D-06:** Each port is independently configurable (enable/disable via config)
- **D-07:** Two `tokio::spawn` calls, one per listener
- **D-08:** 2-crate Cargo workspace: `proto/` (tonic-build codegen) + `gateway/` (binary, all business logic)
- **D-09:** Shared types (TaskId, TaskState, ServiceName) live as modules inside `gateway/`
- **D-10:** Nodes connect via gRPC server-streaming only -- no HTTP polling endpoint for nodes
- **D-11:** Node-side deploys a lightweight proxy (runner agent) that maintains the gRPC stream to gateway and dispatches tasks locally
- **D-12:** One task per stream push -- gateway sends next task to whichever node's stream is ready, not batched
- **D-13:** NODE-02 (HTTPS node polling) is deferred -- the proxy unifies the node-side protocol to gRPC
- **D-14:** Nodes report results via a separate unary RPC, not on the task stream
- **D-15:** Rationale: avoids head-of-line blocking from large result payloads on the task dispatch stream
- **D-16:** Phase 1 includes basic stream disconnect detection and reconnection logic in the proxy
- **D-17:** Tasks assigned to a disconnected node need a recovery path (at minimum: detect, log, allow manual re-queue; full reaper is Phase 4)

### Claude's Discretion
- Redis key naming conventions and stream trimming strategy
- Proto message field types and naming
- Gateway module organization within `gateway/` crate
- Config file format details (TOML structure, env var naming)
- Proxy's local dispatch mechanism (how it calls the actual compute service)
- Error types and error propagation strategy

### Deferred Ideas (OUT OF SCOPE)
- NODE-02 (HTTP node polling) -- replaced by proxy model
- gRPC bidirectional streaming for combined task dispatch + result reporting
- Single-port co-hosting (content-type based routing)
- Node concurrency declaration ("I can handle N tasks simultaneously")
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TASK-01 | Client submits task via gRPC with opaque payload, receives task ID | Tonic unary RPC + proto definitions; Redis XADD to service stream |
| TASK-02 | Client submits task via HTTPS REST with opaque payload, receives task ID | Axum POST handler mirroring gRPC logic; shared service layer |
| TASK-03 | Client attaches arbitrary key-value metadata/labels at submission | Proto map<string, string> field; stored as Redis stream entry fields |
| TASK-04 | Task payloads are opaque bytes -- gateway does not interpret content | Proto `bytes` type; stored as-is in Redis stream |
| RSLT-01 | Client polls task status and result by task ID via gRPC | Tonic unary RPC; Redis HGET on task hash |
| RSLT-02 | Client polls task status and result by task ID via HTTPS REST | Axum GET handler; shared service layer |
| RSLT-05 | Task results stored in Redis with configurable TTL | Redis HSET + EXPIRE on task result hash |
| NODE-01 | Internal nodes reverse-poll gateway via gRPC for tasks | Tonic server-streaming RPC with mpsc channel pattern |
| NODE-02 | Internal nodes reverse-poll via HTTPS | DEFERRED per D-13 -- proxy unifies to gRPC |
| NODE-04 | Nodes report task completion with result payload | Tonic unary RPC; Redis XACK + task hash update |
| LIFE-01 | Task state machine: pending -> assigned -> running -> completed/failed | Enum in gateway types; tracked in Redis task hash |
| LIFE-02 | Reliable queue pattern -- atomic move to processing, no task loss on restart | Redis Streams consumer groups with PEL provide this natively |
| SRVC-02 | Each registered service gets its own isolated task queue | Per-service stream `tasks:{service_name}` with dedicated consumer group |
| INFR-01 | Gateway connects to Redis/Valkey for all persistent state | redis-rs 1.0 MultiplexedConnection with streams feature |
| INFR-02 | Gateway configurable via env vars with optional TOML config override | `config` crate with layered sources; `clap` for CLI args |
</phase_requirements>

## Standard Stack

### Core (Phase 1 subset)

| Library | Version | Purpose | Verified |
|---------|---------|---------|----------|
| tokio | 1.50.0 | Async runtime | crates.io 2026-03-03 |
| tonic | 0.14.5 | gRPC server + client (runner agent) | crates.io 2026-02-19 |
| tonic-build | 0.14.5 | Proto codegen in build.rs | crates.io 2026-02-19 |
| prost | 0.14.3 | Protobuf types | crates.io 2026-01-10 |
| axum | 0.8.8 | HTTP/REST server | crates.io 2025-12-20 |
| redis | 1.0.5 | Redis/Valkey client (with `streams` + `tokio-comp` + `aio` features) | crates.io 2026-03-08 |
| tower | 0.5.3 | Shared middleware | crates.io 2026-01-12 |

### Supporting (Phase 1 subset)

| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| tokio-stream | 0.1.18 | `ReceiverStream` for server-streaming gRPC | Wrapping mpsc::Receiver for tonic responses |
| serde | 1.0.228 | Serialization for config + JSON responses | HTTP request/response bodies, config parsing |
| serde_json | 1.0.149 | JSON serialization | HTTP REST API bodies |
| uuid | 1.22.0 | Task ID generation (v7 feature for time-sortable) | Every task submission |
| chrono | 0.4.44 | Timestamps on task lifecycle events | Task created/assigned/completed timestamps |
| clap | 4.6.0 | CLI argument parsing (derive API) | Gateway binary entry point |
| config | 0.15.22 | Layered configuration (TOML + env vars) | Gateway startup configuration |
| tracing | 0.1.44 | Structured logging | All operational logging |
| tracing-subscriber | 0.3.x | Log output formatting | Gateway startup init |

### Dev Dependencies

| Library | Purpose |
|---------|---------|
| tonic-build 0.14.5 | build.rs in proto crate |
| protoc (system) | Required by tonic-build; install via `brew install protobuf` |

## Architecture Patterns

### Recommended Project Structure

```
xgent-ai-gateway/
├── Cargo.toml              # Workspace root
├── proto/
│   ├── Cargo.toml          # lib crate, depends on tonic + prost
│   ├── build.rs            # tonic_build::compile_protos()
│   └── src/
│       └── lib.rs          # tonic::include_proto!("xgent.gateway.v1");
├── gateway/
│   ├── Cargo.toml          # bin crate, depends on proto + redis + axum + tonic + etc.
│   └── src/
│       ├── main.rs         # CLI parsing, config loading, dual-port server startup
│       ├── config.rs       # Config struct, TOML + env layering
│       ├── types.rs        # TaskId, TaskState, ServiceName newtypes
│       ├── error.rs        # Error enum, Into<tonic::Status>, Into<axum response>
│       ├── queue/
│       │   ├── mod.rs
│       │   └── redis.rs    # RedisQueue: XADD, XREADGROUP, XACK, task hash ops
│       ├── grpc/
│       │   ├── mod.rs
│       │   ├── submit.rs   # TaskSubmission service impl
│       │   ├── poll.rs     # NodePoll server-streaming impl
│       │   └── result.rs   # ResultReport unary impl + client status query
│       └── http/
│           ├── mod.rs
│           ├── submit.rs   # POST /v1/tasks
│           └── result.rs   # GET /v1/tasks/:id
└── agent/                  # Optional: runner agent proxy (could be separate binary in gateway crate)
    └── (or as a feature/binary target in gateway/)
```

**Runner agent note:** The runner agent (lightweight node-side proxy) can be a second binary target in the `gateway` crate (`[[bin]]` in Cargo.toml) or a separate crate. A second binary target is simpler for Phase 1 since it shares the proto types.

### Pattern 1: Redis Streams Consumer Group Queue

**What:** Each service has a dedicated Redis stream. Tasks are added via XADD, nodes consume via XREADGROUP with consumer groups.

**Key design:**
- Stream key: `tasks:{service_name}` (e.g., `tasks:image-resize`)
- Consumer group: `workers` (one per service)
- Consumer name: node ID (e.g., `node-abc123`)
- Task detail hash: `task:{task_id}` (stores full task state, payload, result)

**Flow:**
1. Client submits task -> gateway generates UUID v7 task ID
2. Gateway stores task details in hash `task:{task_id}` with state=pending
3. Gateway adds minimal entry to stream `tasks:{service}` via XADD: `{task_id: <id>}`
4. Node calls XREADGROUP with BLOCK to wait for tasks
5. Gateway detects available task, pushes to node's server-stream
6. Task state updated to assigned/running in hash
7. Node completes, calls ReportResult RPC
8. Gateway XACKs the stream entry, updates hash to completed/failed with result
9. Gateway sets TTL on the task hash (RSLT-05)

**Why stream entry is minimal:** Store only the task_id in the stream entry, with full details in a separate hash. This keeps stream entries small (better XREADGROUP performance), allows updating task state without stream manipulation, and makes result storage clean.

```rust
// Redis Streams usage with redis-rs 1.0
use redis::AsyncCommands;
use redis::streams::{StreamReadOptions, StreamReadReply};

// Create consumer group (idempotent on startup)
let _: () = conn.xgroup_create_mkstream(
    "tasks:my-service", "workers", "0"
).await.unwrap_or(()); // Ignore BUSYGROUP error if already exists

// Submit task: XADD + HSET
let stream_id: String = conn.xadd(
    "tasks:my-service", "*", &[("task_id", task_id.to_string())]
).await?;

// Store task details
let _: () = redis::pipe()
    .hset_multiple(
        format!("task:{task_id}"),
        &[
            ("state", "pending"),
            ("service", "my-service"),
            ("payload", payload_b64),
            ("created_at", timestamp),
            ("stream_id", &stream_id),
        ],
    )
    .expire(format!("task:{task_id}"), result_ttl_secs)
    .query_async(&mut conn)
    .await?;

// Node reads task (blocking)
let opts = StreamReadOptions::default()
    .group("workers", "node-abc123")
    .count(1)
    .block(5000); // block 5 seconds

let result: StreamReadReply = conn.xread_options(
    &["tasks:my-service"], &[">"], &opts
).await?;

// ACK after processing
let _: () = conn.xack("tasks:my-service", "workers", &[&entry_id]).await?;
```

### Pattern 2: gRPC Server-Streaming for Node Task Dispatch

**What:** Nodes connect via a server-streaming RPC. The gateway holds the stream open and pushes tasks one at a time as they become available.

**Key design:**
- Gateway spawns a per-node task that polls Redis via XREADGROUP
- When a task arrives, it sends it through the mpsc channel to the gRPC stream
- Disconnect detection via `tx.closed()` in tokio::select!

```rust
// Proto definition
// service NodeService {
//   rpc PollTasks(PollTasksRequest) returns (stream TaskAssignment);
//   rpc ReportResult(ReportResultRequest) returns (ReportResultResponse);
// }

// Server implementation
async fn poll_tasks(
    &self,
    request: Request<PollTasksRequest>,
) -> Result<Response<Self::PollTasksStream>, Status> {
    let req = request.into_inner();
    let service_name = req.service_name;
    let node_id = req.node_id;

    let (tx, rx) = tokio::sync::mpsc::channel(1); // backpressure: 1 task at a time
    let redis_conn = self.redis.clone();

    tokio::spawn(async move {
        let opts = StreamReadOptions::default()
            .group("workers", &node_id)
            .count(1)
            .block(5000);

        loop {
            tokio::select! {
                _ = tx.closed() => {
                    tracing::info!(node_id, "node disconnected");
                    // D-17: log disconnect, task remains in PEL for recovery
                    break;
                }
                result = read_next_task(&redis_conn, &service_name, &opts) => {
                    match result {
                        Ok(Some(task)) => {
                            if tx.send(Ok(task)).await.is_err() {
                                break; // client gone
                            }
                        }
                        Ok(None) => continue, // timeout, retry
                        Err(e) => {
                            tracing::error!(?e, "redis read error");
                            // brief backoff then retry
                        }
                    }
                }
            }
        }
    });

    let stream = tokio_stream::wrappers::ReceiverStream::new(rx);
    Ok(Response::new(stream))
}
```

### Pattern 3: Dual-Port Listeners

**What:** Two separate `tokio::spawn` calls, one for Axum HTTP and one for Tonic gRPC, each on their own port.

```rust
// main.rs
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let config = load_config()?;
    let redis_conn = connect_redis(&config).await?;

    // Shared state
    let app_state = Arc::new(AppState { redis: redis_conn, config: config.clone() });

    let mut handles = Vec::new();

    // gRPC listener
    if config.grpc.enabled {
        let state = app_state.clone();
        handles.push(tokio::spawn(async move {
            let addr = config.grpc.listen_addr.parse().unwrap();
            tonic::transport::Server::builder()
                .add_service(TaskServiceServer::new(GrpcTaskService::new(state.clone())))
                .add_service(NodeServiceServer::new(GrpcNodeService::new(state)))
                .serve(addr)
                .await
        }));
    }

    // HTTP listener
    if config.http.enabled {
        let state = app_state.clone();
        handles.push(tokio::spawn(async move {
            let app = axum::Router::new()
                .route("/v1/tasks", axum::routing::post(http::submit::submit_task))
                .route("/v1/tasks/:task_id", axum::routing::get(http::result::get_task))
                .with_state(state);
            let listener = tokio::net::TcpListener::bind(&config.http.listen_addr).await.unwrap();
            axum::serve(listener, app).await
        }));
    }

    // Wait for both
    futures::future::try_join_all(handles).await?;
    Ok(())
}
```

### Pattern 4: Task State Machine

**What:** Task lifecycle tracked in a Redis hash with atomic state transitions.

```
pending -> assigned -> running -> completed
                    \          -> failed
```

- **pending:** Task submitted, entry in stream, waiting for a node
- **assigned:** XREADGROUP delivered to a node, stream entry in PEL
- **running:** Node acknowledged receipt (implicit when stream delivers)
- **completed/failed:** Node called ReportResult RPC

State stored in `task:{id}` hash field `state`. Transitions use Redis pipelines for atomicity.

### Pattern 5: Shared Service Layer

**What:** Both gRPC and HTTP handlers call the same business logic functions, avoiding duplication.

```rust
// queue/mod.rs -- protocol-agnostic business logic
pub async fn submit_task(
    redis: &MultiplexedConnection,
    service: &str,
    payload: Vec<u8>,
    metadata: HashMap<String, String>,
) -> Result<TaskId, QueueError> { /* ... */ }

pub async fn get_task_status(
    redis: &MultiplexedConnection,
    task_id: &TaskId,
) -> Result<TaskStatus, QueueError> { /* ... */ }
```

Both `grpc::submit` and `http::submit` call the same `queue::submit_task()`.

### Anti-Patterns to Avoid

- **Storing full payloads in stream entries:** Stream entries should be lightweight (just task_id). Full payload in a separate hash. Streams are append-only and entries remain until trimmed -- large entries waste memory.
- **Using XREAD instead of XREADGROUP:** Without consumer groups, every node would receive every task. XREADGROUP ensures each task goes to exactly one consumer.
- **Blocking the main connection with XREADGROUP BLOCK:** Use a dedicated connection or spawn per-node tasks. The MultiplexedConnection handles concurrent non-blocking commands well, but long BLOCK calls add latency to other operations sharing the same multiplexed connection. For Phase 1, spawning a separate connection per active XREADGROUP loop is safest.
- **Mixing task dispatch and result reporting on the same stream:** D-14/D-15 correctly separates these. A large result payload on a streaming RPC would block dispatch of subsequent tasks to that node.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Reliable task queue | Custom file-based queue or in-memory queue | Redis Streams + consumer groups | At-least-once delivery, PEL tracking, consumer failure detection built in |
| gRPC codegen | Manual protobuf parsing | tonic-build + prost | Type-safe, keeps proto and Rust in sync automatically |
| Server-streaming backpressure | Custom TCP stream management | mpsc channel + ReceiverStream | Tokio channel handles backpressure; tonic integrates via ReceiverStream |
| Configuration layering | Custom env/file parsing | `config` crate | Handles defaults -> TOML -> env vars -> CLI args out of the box |
| Task ID generation | Custom timestamp + random | uuid v7 | Time-sortable, collision-resistant, widely understood format |
| Connection multiplexing | Manual connection pooling | redis-rs MultiplexedConnection | Clone-safe, cancellation-safe, built into redis-rs 1.0 |

## Common Pitfalls

### Pitfall 1: BUSYGROUP Error on Consumer Group Creation

**What goes wrong:** Calling `xgroup_create` when the consumer group already exists returns a BUSYGROUP error.
**Why it happens:** Gateway restarts or multiple instances try to create the same group.
**How to avoid:** Use `xgroup_create_mkstream` and catch/ignore the BUSYGROUP error:
```rust
let result: RedisResult<()> = conn.xgroup_create_mkstream(key, group, "0").await;
match result {
    Ok(()) => {}
    Err(e) if e.to_string().contains("BUSYGROUP") => {} // already exists
    Err(e) => return Err(e.into()),
}
```
**Warning signs:** Gateway crashes on restart with Redis error.

### Pitfall 2: XREADGROUP BLOCK on MultiplexedConnection

**What goes wrong:** Long-blocking XREADGROUP calls on a shared MultiplexedConnection add latency to all other Redis operations on that connection.
**Why it happens:** MultiplexedConnection is a single TCP connection multiplexing multiple commands. A BLOCK 5000 call doesn't block other commands, but under load it can increase response times.
**How to avoid:** Use a separate `MultiplexedConnection` (or even a standard connection) for each XREADGROUP blocking loop. The main connection handles non-blocking operations (XADD, HSET, HGET).
**Warning signs:** Increased latency on task submission when many nodes are connected.

### Pitfall 3: Forgetting to XACK

**What goes wrong:** Tasks remain in the Pending Entries List (PEL) forever, appearing as "stuck" tasks.
**Why it happens:** Node processes a task but the XACK call fails or is forgotten in the code path.
**How to avoid:** Always XACK after processing, even on failure. The ReportResult RPC handler must XACK regardless of success/failure status. Track PEL size in monitoring.
**Warning signs:** `XPENDING` shows growing count of entries that never get acknowledged.

### Pitfall 4: Proto Package Naming Conflicts

**What goes wrong:** tonic-build generates conflicting module names or code that doesn't compile.
**Why it happens:** Proto package names that collide with Rust keywords or module names.
**How to avoid:** Use a namespaced package like `xgent.gateway.v1` and reference the generated code via `tonic::include_proto!("xgent.gateway.v1")`.
**Warning signs:** Compilation errors in generated code after adding new proto files.

### Pitfall 5: Server-Streaming Task Leak on Node Disconnect

**What goes wrong:** Node disconnects mid-task. The task was delivered via XREADGROUP but never ACKed or reported.
**Why it happens:** The gRPC stream drops, the spawned task exits, but the task entry remains in the PEL unacknowledged.
**How to avoid:** In Phase 1, detect disconnect (via `tx.closed()`), log the orphaned task ID. The PEL naturally tracks these -- `XPENDING` will show them. Phase 4 adds a reaper to reclaim them. For Phase 1, ensure the disconnect handler at minimum logs the pending task details.
**Warning signs:** Growing PEL entries for disconnected consumers visible via `XPENDING`.

### Pitfall 6: Axum State Extraction Mismatches

**What goes wrong:** Axum handlers fail to compile because state type doesn't match.
**Why it happens:** Axum 0.8 requires `State<T>` where T matches what was passed to `.with_state()`. Mismatched types cause cryptic compiler errors.
**How to avoid:** Define a single `AppState` struct, wrap in `Arc`, pass to `.with_state()`, and extract as `State<Arc<AppState>>` in handlers.
**Warning signs:** Long trait-bound compiler errors mentioning `FromRef` or `FromRequestParts`.

## Code Examples

### Proto Definition (recommended)

```protobuf
// proto/src/gateway.proto
syntax = "proto3";
package xgent.gateway.v1;

// Client-facing service
service TaskService {
  rpc SubmitTask(SubmitTaskRequest) returns (SubmitTaskResponse);
  rpc GetTaskStatus(GetTaskStatusRequest) returns (GetTaskStatusResponse);
}

// Node-facing service
service NodeService {
  rpc PollTasks(PollTasksRequest) returns (stream TaskAssignment);
  rpc ReportResult(ReportResultRequest) returns (ReportResultResponse);
}

message SubmitTaskRequest {
  string service_name = 1;
  bytes payload = 2;
  map<string, string> metadata = 3;
}

message SubmitTaskResponse {
  string task_id = 1;
}

message GetTaskStatusRequest {
  string task_id = 1;
}

message GetTaskStatusResponse {
  string task_id = 1;
  TaskState state = 2;
  bytes result = 3;        // empty if not completed
  string error_message = 4; // populated on failure
  string created_at = 5;
  string completed_at = 6;
  map<string, string> metadata = 7;
}

enum TaskState {
  TASK_STATE_UNSPECIFIED = 0;
  TASK_STATE_PENDING = 1;
  TASK_STATE_ASSIGNED = 2;
  TASK_STATE_RUNNING = 3;
  TASK_STATE_COMPLETED = 4;
  TASK_STATE_FAILED = 5;
}

message PollTasksRequest {
  string service_name = 1;
  string node_id = 2;
}

message TaskAssignment {
  string task_id = 1;
  bytes payload = 2;
  map<string, string> metadata = 3;
}

message ReportResultRequest {
  string task_id = 1;
  bool success = 2;
  bytes result = 3;
  string error_message = 4;
}

message ReportResultResponse {
  bool acknowledged = 1;
}
```

### Redis Key Naming Convention (Claude's Discretion recommendation)

```
tasks:{service_name}           # Stream -- task dispatch queue
task:{task_id}                 # Hash   -- task state, payload, result, metadata
service:{service_name}:group   # Consumer group name: "workers" (convention, not a key)
```

**Stream trimming strategy:** Use approximate MAXLEN trimming on XADD to cap acknowledged entries:
```rust
// Trim stream to ~10000 entries (approximate, efficient)
let _: String = conn.xadd_maxlen(
    stream_key,
    redis::streams::StreamMaxlen::Approx(10000),
    "*",
    &[("task_id", &task_id)],
).await?;
```
This prevents unbounded stream growth. Approximate trimming (`~`) is much more efficient than exact. Tune the limit based on expected throughput.

### Config Structure (Claude's Discretion recommendation)

```toml
# gateway.toml
[grpc]
enabled = true
listen_addr = "0.0.0.0:50051"

[http]
enabled = true
listen_addr = "0.0.0.0:8080"

[redis]
url = "redis://127.0.0.1:6379"
result_ttl_secs = 86400  # 24 hours

[queue]
stream_maxlen = 10000
block_timeout_ms = 5000
```

Environment variable override pattern: `GATEWAY__GRPC__LISTEN_ADDR=0.0.0.0:50051` (double underscore separator, standard for `config` crate).

### Error Type Strategy (Claude's Discretion recommendation)

```rust
#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("redis error: {0}")]
    Redis(#[from] redis::RedisError),
    #[error("task not found: {0}")]
    TaskNotFound(TaskId),
    #[error("service not found: {0}")]
    ServiceNotFound(String),
    #[error("invalid request: {0}")]
    InvalidRequest(String),
}

impl From<GatewayError> for tonic::Status {
    fn from(err: GatewayError) -> Self {
        match err {
            GatewayError::TaskNotFound(_) => Status::not_found(err.to_string()),
            GatewayError::ServiceNotFound(_) => Status::not_found(err.to_string()),
            GatewayError::InvalidRequest(_) => Status::invalid_argument(err.to_string()),
            GatewayError::Redis(_) => Status::internal("internal error"),
        }
    }
}

// For Axum, implement IntoResponse similarly
```

Note: `thiserror` is not in the CLAUDE.md stack table but is ubiquitous in Rust. Add as a supporting dependency.

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Redis lists + BLMOVE | Redis Streams + consumer groups | Redis 5.0+ (2018), now mature | Built-in consumer tracking, PEL, no custom reliable queue logic needed |
| deadpool-redis for connection pooling | redis-rs 1.0 MultiplexedConnection | redis-rs 1.0 (2024) | No external pool crate needed; clone-safe, cancellation-safe |
| Custom protobuf parsing | tonic-build codegen | Stable since tonic 0.1 | Type-safe, keeps proto and Rust types in sync |
| Axum 0.7 with tower-http 0.5 | Axum 0.8 with tower-http 0.6 | Jan 2025 | Breaking changes in extractors; use 0.8 patterns |
| Warp / Actix for HTTP | Axum | 2023+ | Axum is Tokio-native, shares Tower with Tonic |

## Validation Architecture

### Test Framework

| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (`cargo test`) + tokio test runtime |
| Config file | None -- Wave 0 |
| Quick run command | `cargo test -p gateway --lib` |
| Full suite command | `cargo test --workspace` |

### Phase Requirements to Test Map

| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| TASK-01 | Submit task via gRPC returns task ID | integration | `cargo test -p gateway --test grpc_submit` | Wave 0 |
| TASK-02 | Submit task via HTTP returns task ID | integration | `cargo test -p gateway --test http_submit` | Wave 0 |
| TASK-03 | Metadata attached at submission | unit | `cargo test -p gateway --lib queue::tests::metadata` | Wave 0 |
| TASK-04 | Opaque payload stored as-is | unit | `cargo test -p gateway --lib queue::tests::opaque_payload` | Wave 0 |
| RSLT-01 | Poll status via gRPC | integration | `cargo test -p gateway --test grpc_status` | Wave 0 |
| RSLT-02 | Poll status via HTTP | integration | `cargo test -p gateway --test http_status` | Wave 0 |
| RSLT-05 | Result TTL in Redis | unit | `cargo test -p gateway --lib queue::tests::result_ttl` | Wave 0 |
| NODE-01 | Node reverse-polls via gRPC stream | integration | `cargo test -p gateway --test node_poll` | Wave 0 |
| NODE-04 | Node reports result | integration | `cargo test -p gateway --test node_report` | Wave 0 |
| LIFE-01 | State machine transitions | unit | `cargo test -p gateway --lib types::tests::state_transitions` | Wave 0 |
| LIFE-02 | Reliable queue -- no task loss | integration | `cargo test -p gateway --test reliable_queue` | Wave 0 |
| SRVC-02 | Per-service isolated queue | unit | `cargo test -p gateway --lib queue::tests::service_isolation` | Wave 0 |
| INFR-01 | Redis connection established | integration | `cargo test -p gateway --test redis_connect` | Wave 0 |
| INFR-02 | Config from env + TOML | unit | `cargo test -p gateway --lib config::tests` | Wave 0 |

### Sampling Rate

- **Per task commit:** `cargo test -p gateway --lib`
- **Per wave merge:** `cargo test --workspace`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps

- [ ] `gateway/tests/` directory -- all integration test files
- [ ] Test fixtures: Redis test container or mock (recommend `testcontainers` crate for integration tests with real Redis)
- [ ] `proto/` crate must compile before any tests run
- [ ] `thiserror` crate -- add to dependencies (not in CLAUDE.md stack but standard for Rust error handling)
- [ ] `tokio-stream` crate -- add to dependencies for `ReceiverStream`
- [ ] `futures` crate -- for `try_join_all` in main

## Open Questions

1. **Runner agent binary location**
   - What we know: D-11 specifies a lightweight proxy on the node side
   - What's unclear: Whether it should be a second `[[bin]]` target in the gateway crate or a third workspace member crate
   - Recommendation: Start as a second binary target in `gateway/` crate (`gateway/src/bin/agent.rs`). It shares proto types directly. If it grows complex, extract to its own crate later.

2. **Redis connection strategy for XREADGROUP BLOCK**
   - What we know: MultiplexedConnection works but BLOCK calls can add latency
   - What's unclear: Whether one dedicated connection per node or a shared "blocking" connection is better
   - Recommendation: One dedicated `MultiplexedConnection` clone per spawned node task for XREADGROUP. The clone shares the underlying TCP connection but the multiplexer handles concurrent blocking calls. If latency becomes an issue, open a second raw connection dedicated to blocking operations. Test early.

3. **Service registration in Phase 1**
   - What we know: SRVC-02 requires per-service queues, but SRVC-01 (admin registration) is Phase 3
   - What's unclear: How services are created in Phase 1 without admin APIs
   - Recommendation: Auto-create stream and consumer group on first task submission for a service name. This is the simplest approach and defers admin CRUD to Phase 3. The gateway lazily creates `tasks:{service_name}` stream and `workers` consumer group when first referenced.

4. **Proxy dispatch mechanism**
   - What we know: Runner agent dispatches tasks to local compute services
   - What's unclear: What protocol the agent uses to call local services (HTTP? Unix socket? Subprocess?)
   - Recommendation: Start with HTTP POST to a configurable local URL (e.g., `http://localhost:8090/execute`). This is the simplest, most universal interface. The agent sends the task payload as the request body and receives the result as the response body.

## Sources

### Primary (HIGH confidence)
- [redis-rs streams module](https://docs.rs/redis/latest/redis/streams/index.html) - StreamCommands trait, stream types
- [Redis XREADGROUP docs](https://redis.io/docs/latest/commands/xreadgroup/) - Full command documentation with consumer group semantics
- [tonic streaming example](https://github.com/hyperium/tonic/blob/master/examples/src/streaming/server.rs) - Server-streaming with mpsc + ReceiverStream
- [tonic disconnect detection](https://github.com/hyperium/tonic/discussions/1190) - tx.closed() pattern for detecting client disconnect
- crates.io version checks (2026-03-21) - All versions verified against registry

### Secondary (MEDIUM confidence)
- [Axum + Tonic co-hosting](https://github.com/sunsided/http-grpc-cohosting) - Pattern reference for dual-port and same-port approaches
- [redis-rs Commands trait](https://docs.rs/redis/latest/redis/trait.Commands.html) - Full stream method signatures

### Tertiary (LOW confidence)
- Runner agent dispatch mechanism - No authoritative source; recommendation based on common patterns in CI runner systems (GitHub Actions, GitLab)

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - All versions verified on crates.io; compatibility confirmed by CLAUDE.md version matrix
- Architecture: HIGH - Redis Streams consumer groups are well-documented; tonic server-streaming is a documented pattern with official examples
- Pitfalls: HIGH - Drawn from official Redis docs (BUSYGROUP, PEL behavior) and tonic discussions (disconnect detection)
- Redis Streams specifics: HIGH - XREADGROUP/XACK/consumer group semantics thoroughly documented by Redis
- Runner agent design: MEDIUM - Based on common patterns, not verified against a specific reference implementation

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable ecosystem, 30 days)
