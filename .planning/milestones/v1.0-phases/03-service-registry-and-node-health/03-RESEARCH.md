# Phase 3: Service Registry and Node Health - Research

**Researched:** 2026-03-21
**Domain:** Redis-backed service registry, node health tracking, gRPC RPC extensions, graceful drain
**Confidence:** HIGH

## Summary

Phase 3 transforms services from implicit (lazy consumer group creation) to explicit managed entities with persisted configuration, and adds node health tracking and graceful drain. The codebase already has strong patterns for Redis hash storage (API keys, node tokens), admin HTTP endpoints, and gRPC service implementations that this phase extends directly.

The primary technical challenges are: (1) designing a Redis key schema for service config that supports both individual lookups and enumeration, (2) implementing service deregistration cleanup that safely deletes all associated Redis keys without blocking, (3) adding `last_seen` timestamp tracking to the existing poll loop without impacting XREADGROUP latency, and (4) extending the proto file and runner agent for Heartbeat/DrainNode RPCs plus SIGTERM handling.

**Primary recommendation:** Use Redis hashes for service config (one hash per service at `service:{name}`), a Redis set for service enumeration (`services:index`), per-node hashes for health state (`node:{service}:{node_id}`), and SCAN+UNLINK for deregistration cleanup of task hashes.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Services carry: `name`, `description`, `created_at`, `task_timeout_secs`, `max_retries`, `max_nodes` (optional cap), `node_stale_after_secs`, `drain_timeout_secs`
- **D-02:** `task_timeout_secs` and `max_retries` are defined now in the service config; Phase 4 reads them when implementing timeout/retry logic
- **D-03:** Gateway rejects task submissions for unregistered services -- submit_task checks the service registry in Redis before enqueuing
- **D-04:** Service registration does NOT create node tokens -- token management remains separate (create service first, then add tokens via existing admin endpoints)
- **D-05:** Admin endpoints: `POST /v1/admin/services` (register), `DELETE /v1/admin/services/{name}` (deregister), `GET /v1/admin/services` (list), `GET /v1/admin/services/{name}` (detail)
- **D-06:** Deregister is asynchronous -- endpoint returns immediately (202 Accepted), cleanup happens in a background `tokio::spawn` task
- **D-07:** All pending/queued tasks are marked as failed with error "service deregistered" immediately
- **D-08:** Nodes currently processing tasks can still report results via ReportResult RPC
- **D-09:** All node tokens for the service are auto-revoked (delete all `node_tokens:{service_name}:*` keys)
- **D-10:** All task result hashes (`task:{id}`) belonging to the service are deleted immediately -- clean break
- **D-11:** The service's Redis Stream (`tasks:{service_name}`) and consumer group are deleted
- **D-12:** Both passive and active health tracking -- poll timestamps update `last_seen` on every XREADGROUP cycle, plus a new `Heartbeat` unary RPC for nodes to signal liveness during task execution
- **D-13:** Staleness threshold is configurable per-service via `node_stale_after_secs` in service config, with a global default from gateway config
- **D-14:** Three node health states: `healthy` (recently seen), `unhealthy` (stale but stream may reconnect), `disconnected` (stream closed, node gone)
- **D-15:** No background reaper for health -- health state is derived in real-time from `last_seen` timestamps as poll/heartbeat events occur. Admin endpoint computes current status on-demand.
- **D-16:** Node registry tracks: `node_id`, `service_name`, `last_seen`, `health_state`, `in_flight_tasks`, `draining`
- **D-17:** New `DrainNode` unary RPC on `NodeService` -- node calls it to signal drain intent
- **D-18:** After drain signal, the gRPC poll stream stays open but gateway stops sending new tasks to that node
- **D-19:** Stream acts as liveness signal during drain -- node reports results via separate ReportResult RPC and disconnects when done
- **D-20:** Per-service drain timeout (`drain_timeout_secs` in service config) -- after timeout, gateway marks node as disconnected. In-flight task recovery deferred to Phase 4's reaper
- **D-21:** Runner agent SIGTERM handling is in scope: SIGTERM -> call DrainNode -> wait for in-flight tasks to complete -> exit cleanly

### Claude's Discretion
- Redis key structure for service config storage (hash vs JSON string)
- Node registry Redis schema (per-node keys vs per-service hash)
- Proto message definitions for new RPCs (Heartbeat, DrainNode, service admin messages)
- Exact health state transition logic and edge cases
- Admin endpoint response shapes and error codes
- How to enumerate and delete task hashes belonging to a service during deregistration (scan pattern or index)
- Global default values for `node_stale_after_secs` and `drain_timeout_secs`
- Admin endpoint authentication (deferred from Phase 2 -- decide whether to implement now or continue deferring)

### Deferred Ideas (OUT OF SCOPE)
- Admin endpoint authentication -- Phase 2 created unauthenticated admin endpoints; securing them (admin tokens, separate auth) remains deferred unless addressed in this phase at Claude's discretion
- Node authentication via mTLS certificates instead of pre-shared tokens -- v2 requirement (EAUTH-01)
- Auto-scaling node pools based on queue depth -- not in v1 scope
- Service-level rate limiting -- deferred to v2 (OPS-03)
- Single-port co-hosting -- deferred from Phase 1 and Phase 2, still deferred
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SRVC-01 | Admin can register a new service with the gateway (name, config, node auth tokens) | Redis hash schema for service config, admin endpoint patterns from existing code, `ensure_consumer_group` integration |
| SRVC-03 | Admin can deregister a service (drains queue, removes config) | SCAN+UNLINK for task cleanup, XGROUP DESTROY for stream cleanup, background tokio::spawn pattern |
| SRVC-04 | Service configuration is persisted in Redis and survives gateway restarts | Redis hash storage pattern, no in-memory cache needed -- read from Redis on each request |
| NODE-03 | Nodes authenticate with pre-shared tokens scoped to their service | Already implemented in Phase 2 via `node_token::validate_node_token`; Phase 3 adds service registry check |
| NODE-05 | Gateway tracks node health via heartbeat (last poll time, stale detection) | Per-node Redis hash with `last_seen` timestamp, real-time health derivation, Heartbeat RPC |
| NODE-06 | Nodes can signal graceful drain -- gateway stops assigning new tasks, waits for in-flight completion | DrainNode RPC, drain-aware poll loop, SIGTERM handler in agent |
</phase_requirements>

## Standard Stack

No new dependencies required. Phase 3 uses the existing stack entirely:

### Core (already in Cargo.toml)
| Library | Version | Purpose | Phase 3 Use |
|---------|---------|---------|-------------|
| redis-rs | 1.0.x | Redis client | Service config hashes, node health hashes, SCAN for cleanup, XGROUP DESTROY |
| tonic | 0.14.x | gRPC server | New Heartbeat and DrainNode RPCs on NodeService |
| prost | 0.14.x | Protobuf codegen | New message types for Heartbeat, DrainNode, service admin |
| axum | 0.8.x | HTTP server | New admin endpoints for service CRUD |
| tokio | 1.50+ | Async runtime | `tokio::spawn` for async deregistration, `tokio::signal` for SIGTERM |
| chrono | 0.4.x | Timestamps | `last_seen` timestamps, `created_at` for services |
| serde/serde_json | 1.x | Serialization | Service config serialization for Redis storage, JSON responses |

### New Feature Flags Needed
| Crate | Feature | Purpose |
|-------|---------|---------|
| tokio | `signal` | SIGTERM handling in runner agent (already included via `features = ["full"]`) |

No `npm install` or `cargo add` needed -- all dependencies are already present.

## Architecture Patterns

### Recommended Redis Key Schema

```
# Service registry
service:{service_name}              # Hash: service config fields
services:index                      # Set: all registered service names

# Node health registry
node:{service_name}:{node_id}       # Hash: node_id, service_name, last_seen, draining, in_flight_tasks
nodes:{service_name}                # Set: all node IDs for a service

# Existing keys (unchanged)
tasks:{service_name}                # Stream: task queue
task:{task_id}                      # Hash: task data
node_tokens:{service_name}:{hash}   # Hash: node token metadata
api_key:{hash}                      # Hash: API key metadata
```

**Rationale for per-node keys (not per-service hash):**
- Per-node keys allow atomic field updates (HSET on individual fields) without read-modify-write on a shared hash
- TTL can be set per-node (auto-cleanup if gateway restarts and node never reconnects)
- SCAN by prefix `node:{service_name}:*` enumerates nodes for a service
- A companion set `nodes:{service_name}` provides O(1) enumeration without SCAN

### Pattern 1: Service Config as Redis Hash

**What:** Store each service config field as a separate hash field, enabling atomic field-level reads/updates.
**When to use:** Always for service config -- fields are read individually (e.g., `node_stale_after_secs` during health check) and updated rarely.

```rust
// Service registration: store config as Redis hash
pub async fn register_service(
    conn: &mut MultiplexedConnection,
    name: &str,
    config: &ServiceConfig,
) -> Result<(), GatewayError> {
    let key = format!("service:{}", name);
    let now = chrono::Utc::now().to_rfc3339();

    // Check if already exists
    let exists: bool = redis::cmd("EXISTS")
        .arg(&key)
        .query_async(conn)
        .await
        .map_err(GatewayError::Redis)?;
    if exists {
        return Err(GatewayError::ServiceAlreadyExists(name.to_string()));
    }

    // Store config + add to index set atomically via pipeline
    redis::pipe()
        .cmd("HSET").arg(&key)
            .arg("name").arg(name)
            .arg("description").arg(&config.description)
            .arg("created_at").arg(&now)
            .arg("task_timeout_secs").arg(config.task_timeout_secs)
            .arg("max_retries").arg(config.max_retries)
            .arg("max_nodes").arg(config.max_nodes.unwrap_or(0))
            .arg("node_stale_after_secs").arg(config.node_stale_after_secs)
            .arg("drain_timeout_secs").arg(config.drain_timeout_secs)
        .ignore()
        .cmd("SADD").arg("services:index").arg(name)
        .ignore()
        .query_async(conn)
        .await
        .map_err(GatewayError::Redis)?;

    Ok(())
}
```

### Pattern 2: Real-Time Health Derivation (No Background Reaper)

**What:** Node health state is computed on-demand from `last_seen` timestamp, not stored as a cached enum.
**When to use:** Per D-15 -- health state is derived, not persisted as an authoritative field.

```rust
/// Derive node health from last_seen timestamp and service config.
pub fn derive_health_state(
    last_seen: &str,        // RFC3339 timestamp
    stale_after_secs: u64,  // From service config
    is_disconnected: bool,  // Stream closed flag
) -> NodeHealthState {
    if is_disconnected {
        return NodeHealthState::Disconnected;
    }
    if let Ok(ts) = chrono::DateTime::parse_from_rfc3339(last_seen) {
        let elapsed = chrono::Utc::now().signed_duration_since(ts);
        if elapsed.num_seconds() <= stale_after_secs as i64 {
            NodeHealthState::Healthy
        } else {
            NodeHealthState::Unhealthy
        }
    } else {
        NodeHealthState::Unhealthy
    }
}
```

### Pattern 3: Drain-Aware Poll Loop

**What:** The existing poll loop in `grpc/poll.rs` checks a node's drain flag before dispatching tasks.
**When to use:** After DrainNode RPC sets the `draining` flag on the node's Redis hash.

```rust
// Inside the poll loop (modified from current code):
loop {
    tokio::select! {
        _ = tx.closed() => {
            // Mark node as disconnected in Redis
            update_node_disconnected(&mut conn, &service, &node_id).await;
            break;
        }
        result = queue.poll_task(&service, &node_id) => {
            // Check drain state before sending task
            let draining = is_node_draining(&mut conn, &service, &node_id).await;
            if draining {
                // Don't send task, just keep stream alive
                // Re-queue the task if one was claimed (or skip poll entirely)
                continue;
            }
            // ... existing task dispatch logic ...
        }
    }

    // Update last_seen on each cycle (passive health tracking)
    update_node_last_seen(&mut conn, &service, &node_id).await;
}
```

### Pattern 4: Async Deregistration Cleanup

**What:** Service deregistration returns 202 immediately; cleanup runs in background.
**When to use:** Per D-06 -- avoid blocking the HTTP response on potentially slow cleanup.

```rust
// Deregistration endpoint handler
pub async fn deregister_service(
    State(state): State<Arc<AppState>>,
    Path(name): Path<String>,
) -> Result<StatusCode, GatewayError> {
    // Verify service exists
    let key = format!("service:{}", name);
    let exists: bool = /* check Redis */;
    if !exists {
        return Err(GatewayError::ServiceNotFound(name));
    }

    // Spawn background cleanup
    let conn = state.auth_conn.clone();
    let queue_conn = state.queue.conn.clone();
    tokio::spawn(async move {
        if let Err(e) = cleanup_service(&mut conn.clone(), &mut queue_conn.clone(), &name).await {
            tracing::error!(service=%name, error=%e, "service deregistration cleanup failed");
        }
    });

    Ok(StatusCode::ACCEPTED) // 202
}
```

### Pattern 5: SIGTERM Handler in Runner Agent

**What:** Agent catches SIGTERM, calls DrainNode RPC, waits for in-flight task, exits.
**When to use:** Per D-21 -- runner agent graceful shutdown.

```rust
// In agent main loop, wrap the stream processing with signal handling
use tokio::signal::unix::{signal, SignalKind};

let mut sigterm = signal(SignalKind::terminate())?;

tokio::select! {
    // Normal task processing
    result = process_stream(&mut stream, &http_client, &report_client, &cli) => {
        result?;
    }
    // SIGTERM received
    _ = sigterm.recv() => {
        tracing::info!("SIGTERM received, initiating graceful drain");
        // Call DrainNode RPC
        let drain_req = DrainNodeRequest {
            service_name: cli.service_name.clone(),
            node_id: cli.node_id.clone(),
        };
        client.drain_node(drain_req).await?;
        // Wait for in-flight task to complete (stream will close naturally)
        // ... or set a timeout
    }
}
```

### Recommended Project Structure Changes

```
gateway/src/
  registry/              # NEW: service registry module
    mod.rs               # Public API
    service.rs           # Service CRUD (register, deregister, get, list)
    node_health.rs       # Node health tracking (last_seen, derive health, drain state)
    cleanup.rs           # Deregistration cleanup logic (SCAN, UNLINK, XGROUP DESTROY)
  http/
    admin.rs             # MODIFIED: add service CRUD endpoints
  grpc/
    poll.rs              # MODIFIED: add last_seen tracking, drain checks
  error.rs              # MODIFIED: add ServiceAlreadyExists variant
  config.rs             # MODIFIED: add global defaults for node_stale_after_secs, drain_timeout_secs
  state.rs              # MODIFIED: no new fields needed (registry reads from Redis directly)
  bin/
    agent.rs            # MODIFIED: add SIGTERM handler, DrainNode RPC call
proto/src/
  gateway.proto         # MODIFIED: add Heartbeat, DrainNode RPCs and messages
```

### Anti-Patterns to Avoid
- **In-memory service registry cache:** Don't cache service configs in `AppState` -- read from Redis each time. Multiple gateway instances would have stale caches. Redis hashes are fast enough for config lookups.
- **Background health reaper thread:** Per D-15, don't run a periodic background scan. Derive health on-demand from timestamps. This avoids timer management and race conditions.
- **Blocking deregistration:** Don't perform cleanup synchronously in the HTTP handler. SCAN over potentially thousands of task keys could take seconds. Use `tokio::spawn`.
- **KEYS command for cleanup:** Never use `KEYS node_tokens:{service}:*` in production. Use SCAN with cursor iteration and batch UNLINK.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Task hash enumeration for cleanup | Custom index tracking every task ID per service | Redis SCAN with pattern `task:*` + HGET service field | Adding a per-service task index adds write overhead to every submit_task. SCAN on deregistration (rare event) is acceptable. |
| Signal handling | Raw libc signal handlers | `tokio::signal::unix::signal(SignalKind::terminate())` | Tokio's signal handling is async-safe and integrates with the event loop. Raw signals in async Rust cause UB. |
| Health state machine | Complex state machine with transitions | Simple timestamp comparison in `derive_health_state()` | Per D-15, health is derived not stored. A state machine adds unnecessary complexity. |
| Redis connection pooling | Custom connection pool | `MultiplexedConnection::clone()` | redis-rs 1.0 MultiplexedConnection is clone-safe and handles multiplexing internally. |

## Common Pitfalls

### Pitfall 1: SCAN During Deregistration Missing Keys
**What goes wrong:** SCAN cursor iteration can miss keys if keys are created/deleted during the scan.
**Why it happens:** Redis SCAN provides eventual consistency, not point-in-time snapshot.
**How to avoid:** Accept this as a known limitation for deregistration. New tasks will be rejected (D-03 service registry check) so no new task hashes are created during cleanup. The real danger is orphaned task hashes -- which have TTL already set (from submit_task) and will auto-expire.
**Warning signs:** Task hashes lingering after deregistration -- harmless due to TTL.

### Pitfall 2: Race Between DrainNode and Poll Task Dispatch
**What goes wrong:** Node calls DrainNode, but the poll loop has already claimed a task from XREADGROUP and is about to send it.
**Why it happens:** XREADGROUP claim and drain flag check are not atomic.
**How to avoid:** Check drain flag BEFORE calling `poll_task()` (which does the XREADGROUP). If drain is detected after a task is claimed but before sending, the task stays in the consumer's PEL and will be reclaimed by Phase 4's reaper. This is acceptable -- drain is best-effort for new task prevention.
**Warning signs:** Node receives one extra task after calling DrainNode.

### Pitfall 3: Node ID Collisions
**What goes wrong:** Two agent instances with the same `node_id` overwrite each other's health state.
**Why it happens:** Default node_id uses UUID v7 but if set via env var could collide.
**How to avoid:** The existing agent code generates a UUID v7 by default (`default_value_t = uuid::Uuid::now_v7().to_string()`). Document that manual node_id must be unique per service.
**Warning signs:** Phantom "healthy" node that actually disconnected, because another node keeps updating `last_seen`.

### Pitfall 4: Deregistration While Nodes Are Connected
**What goes wrong:** Connected nodes continue polling a deleted stream, getting errors.
**Why it happens:** D-11 deletes the stream, but connected nodes have active XREADGROUP calls.
**How to avoid:** The XREADGROUP will return an error when the stream/group is destroyed. The poll loop already handles Redis errors with a 1-second retry delay. The node will get repeated errors and eventually reconnect or notice the service is gone. Consider having deregistration also set a "deregistered" flag that the poll loop checks, returning a gRPC error status to the node.
**Warning signs:** Node agent logs showing repeated Redis errors after service deregistration.

### Pitfall 5: `last_seen` Update Adding Latency to Poll Loop
**What goes wrong:** Adding an HSET for `last_seen` on every poll cycle adds a Redis round-trip.
**Why it happens:** The poll loop is latency-sensitive -- it's the hot path for task dispatch.
**How to avoid:** Pipeline the `last_seen` HSET into the existing poll_task write (which already does HSET for task state). Or use a fire-and-forget approach with `tokio::spawn` for the `last_seen` update. Since `last_seen` is best-effort (health is derived, not authoritative), a slightly stale value is acceptable.
**Warning signs:** Increased poll-to-dispatch latency measurable in task assignment timestamps.

### Pitfall 6: SIGTERM Handler on Non-Unix Platforms
**What goes wrong:** `tokio::signal::unix::signal` is not available on Windows.
**Why it happens:** SIGTERM is a Unix concept.
**How to avoid:** Use conditional compilation: `#[cfg(unix)]` for SIGTERM handler, `tokio::signal::ctrl_c()` as fallback. The agent is primarily deployed on Linux but should compile on macOS for development.
**Warning signs:** Compilation failure on non-Unix targets.

## Code Examples

### Service Config Struct

```rust
/// Service configuration stored in Redis.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub task_timeout_secs: u64,
    pub max_retries: u32,
    pub max_nodes: Option<u32>,       // None = unlimited
    pub node_stale_after_secs: u64,
    pub drain_timeout_secs: u64,
}
```

### Proto Additions for Heartbeat and DrainNode

```protobuf
// Add to NodeService in gateway.proto
service NodeService {
  rpc PollTasks(PollTasksRequest) returns (stream TaskAssignment);
  rpc ReportResult(ReportResultRequest) returns (ReportResultResponse);
  rpc Heartbeat(HeartbeatRequest) returns (HeartbeatResponse);       // NEW
  rpc DrainNode(DrainNodeRequest) returns (DrainNodeResponse);       // NEW
}

message HeartbeatRequest {
  string service_name = 1;
  string node_id = 2;
}

message HeartbeatResponse {
  bool acknowledged = 1;
}

message DrainNodeRequest {
  string service_name = 1;
  string node_id = 2;
}

message DrainNodeResponse {
  bool acknowledged = 1;
  uint64 drain_timeout_secs = 2;  // Echoes back the timeout so node knows
}

// Add node health enum
enum NodeHealthState {
  NODE_HEALTH_UNSPECIFIED = 0;
  NODE_HEALTH_HEALTHY = 1;
  NODE_HEALTH_UNHEALTHY = 2;
  NODE_HEALTH_DISCONNECTED = 3;
}
```

### Deregistration Cleanup with SCAN + UNLINK

```rust
/// Clean up all Redis state for a deregistered service.
async fn cleanup_service(
    conn: &mut MultiplexedConnection,
    service_name: &str,
) -> Result<(), GatewayError> {
    // 1. Delete service config hash
    let _: () = redis::cmd("DEL")
        .arg(format!("service:{}", service_name))
        .query_async(conn).await?;

    // 2. Remove from index set
    let _: () = redis::cmd("SREM")
        .arg("services:index")
        .arg(service_name)
        .query_async(conn).await?;

    // 3. Delete node tokens: SCAN for node_tokens:{service_name}:*
    scan_and_unlink(conn, &format!("node_tokens:{}:*", service_name)).await?;

    // 4. Fail all pending tasks in the stream's PEL, then delete stream
    //    Read all pending entries, mark their task hashes as failed
    let stream_key = format!("tasks:{}", service_name);

    // 5. Destroy consumer group + delete stream
    let _: Result<(), _> = redis::cmd("XGROUP")
        .arg("DESTROY").arg(&stream_key).arg("workers")
        .query_async(conn).await;
    let _: () = redis::cmd("DEL")
        .arg(&stream_key)
        .query_async(conn).await?;

    // 6. Delete node health entries
    scan_and_unlink(conn, &format!("node:{}:*", service_name)).await?;
    let _: () = redis::cmd("DEL")
        .arg(format!("nodes:{}", service_name))
        .query_async(conn).await?;

    // 7. Delete task hashes belonging to this service
    //    SCAN for task:* and check service field -- or use indexed approach
    scan_and_delete_service_tasks(conn, service_name).await?;

    tracing::info!(service=%service_name, "service deregistration cleanup complete");
    Ok(())
}

/// SCAN for keys matching pattern and UNLINK them in batches.
async fn scan_and_unlink(
    conn: &mut MultiplexedConnection,
    pattern: &str,
) -> Result<(), GatewayError> {
    let mut cursor: u64 = 0;
    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH").arg(pattern)
            .arg("COUNT").arg(100)
            .query_async(conn).await
            .map_err(GatewayError::Redis)?;

        if !keys.is_empty() {
            let mut cmd = redis::cmd("UNLINK");
            for key in &keys {
                cmd.arg(key);
            }
            let _: () = cmd.query_async(conn).await.map_err(GatewayError::Redis)?;
        }

        cursor = next_cursor;
        if cursor == 0 { break; }
    }
    Ok(())
}
```

### Admin Endpoint Response Shapes

```rust
// POST /v1/admin/services
#[derive(Debug, Deserialize)]
pub struct RegisterServiceRequest {
    pub name: String,
    pub description: Option<String>,
    pub task_timeout_secs: Option<u64>,    // default: 300
    pub max_retries: Option<u32>,          // default: 3
    pub max_nodes: Option<u32>,            // default: None (unlimited)
    pub node_stale_after_secs: Option<u64>, // default: from gateway config
    pub drain_timeout_secs: Option<u64>,   // default: from gateway config
}

#[derive(Debug, Serialize)]
pub struct ServiceResponse {
    pub name: String,
    pub description: String,
    pub created_at: String,
    pub task_timeout_secs: u64,
    pub max_retries: u32,
    pub max_nodes: Option<u32>,
    pub node_stale_after_secs: u64,
    pub drain_timeout_secs: u64,
}

// GET /v1/admin/services
#[derive(Debug, Serialize)]
pub struct ListServicesResponse {
    pub services: Vec<ServiceResponse>,
}

// GET /v1/admin/services/{name} -- includes live node health
#[derive(Debug, Serialize)]
pub struct ServiceDetailResponse {
    #[serde(flatten)]
    pub service: ServiceResponse,
    pub nodes: Vec<NodeStatusResponse>,
}

#[derive(Debug, Serialize)]
pub struct NodeStatusResponse {
    pub node_id: String,
    pub health: String,       // "healthy", "unhealthy", "disconnected"
    pub last_seen: String,
    pub in_flight_tasks: u32,
    pub draining: bool,
}
```

## Discretion Recommendations

### Redis Key Structure: Use Hashes (Recommended)
Redis hashes for service config allow atomic field-level reads (HGET for individual fields like `node_stale_after_secs`) without deserializing the entire config blob. The existing codebase already uses this pattern for API keys and node tokens. Consistency wins.

### Node Registry: Per-Node Keys + Service Set (Recommended)
Use `node:{service}:{node_id}` hashes for per-node state, plus `nodes:{service}` sets for enumeration. This matches the existing `node_tokens:{service}:{hash}` pattern and allows TTL on individual nodes.

### Task Hash Cleanup During Deregistration: SCAN with Service Filter (Recommended)
Rather than maintaining a per-service task index (which adds write overhead to every submit_task), SCAN for `task:*` keys and check the `service` field via HGET. This is O(n) over all tasks but deregistration is rare. Alternatively, add a `tasks_index:{service}` set on registration and add task IDs to it during submit -- this trades write overhead for faster cleanup. **Recommendation: Start with SCAN, add index only if deregistration is too slow.**

### Global Defaults
- `node_stale_after_secs`: 60 (1 minute -- nodes poll every 5 seconds, so 12 missed polls = stale)
- `drain_timeout_secs`: 300 (5 minutes -- generous timeout for in-flight tasks)

### Admin Auth: Continue Deferring (Recommended)
Phase 2 already added `AdminConfig.token` field to config. The infrastructure is there. However, the admin auth middleware is not yet implemented. **Recommendation: Defer to Phase 5 (observability/ops) rather than adding it in Phase 3.** Reason: Phase 3 is already substantial with service registry + node health + drain + agent SIGTERM. Keep scope tight.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) + #[ignore] gated integration tests |
| Config file | None (Rust built-in test framework) |
| Quick run command | `cargo test -p xgent-gateway --lib` |
| Full suite command | `cargo test -p xgent-gateway -- --ignored` (requires Redis) |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SRVC-01 | Register service creates Redis hash + stream | integration | `cargo test -p xgent-gateway --test integration_test test_register_service -- --ignored` | No -- Wave 0 |
| SRVC-03 | Deregister service cleans up all Redis state | integration | `cargo test -p xgent-gateway --test integration_test test_deregister_service -- --ignored` | No -- Wave 0 |
| SRVC-04 | Service config persists across gateway restart | integration | `cargo test -p xgent-gateway --test integration_test test_service_persistence -- --ignored` | No -- Wave 0 |
| NODE-03 | Submit_task rejects unregistered service | integration | `cargo test -p xgent-gateway --test integration_test test_submit_unregistered_rejected -- --ignored` | No -- Wave 0 |
| NODE-05 | Health state derived from last_seen | unit | `cargo test -p xgent-gateway --lib test_derive_health_state` | No -- Wave 0 |
| NODE-06 | DrainNode stops task dispatch to node | integration | `cargo test -p xgent-gateway --test integration_test test_drain_node -- --ignored` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway --lib`
- **Per wave merge:** `cargo test -p xgent-gateway -- --ignored` (requires Redis)
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `gateway/src/registry/mod.rs` -- unit tests for `derive_health_state`, service config validation
- [ ] New integration test functions in `gateway/tests/integration_test.rs` -- service CRUD lifecycle, deregistration cleanup, drain behavior
- [ ] No new test fixtures needed -- existing `test_queue()` helper and Redis cleanup pattern are reusable

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Lazy consumer group creation in submit_task | Explicit creation during service registration | Phase 3 | submit_task no longer calls ensure_consumer_group; registration does |
| Implicit services (any name accepted) | Explicit service registry with validation | Phase 3 | submit_task rejects unregistered services |
| No node tracking | Per-node Redis hashes with health derivation | Phase 3 | Gateway knows which nodes are alive |
| No graceful shutdown | DrainNode RPC + SIGTERM handler | Phase 3 | Clean node departures without task loss |

## Open Questions

1. **Task hash enumeration efficiency for deregistration (D-10)**
   - What we know: SCAN with service field check works but is O(n) over all tasks globally
   - What's unclear: Whether a per-service task index set is worth the write overhead on every submit_task
   - Recommendation: Start with SCAN approach. If deregistration of services with many tasks is slow, add `tasks_index:{service}` set as optimization in a later phase.

2. **Drain flag storage: Redis vs in-memory**
   - What we know: Redis hash field is the simplest and works across gateway restarts
   - What's unclear: Whether the extra Redis read on every poll cycle is acceptable latency
   - Recommendation: Store in Redis for durability. The read can be pipelined with `last_seen` write. If latency is a concern, cache the drain flag in the poll loop's local state and refresh periodically.

3. **Node registration lifecycle**
   - What we know: Nodes appear when they first poll. There is no explicit "register node" step.
   - What's unclear: Should there be an explicit node registration, or is auto-registration on first poll sufficient?
   - Recommendation: Auto-register on first poll (create `node:{service}:{node_id}` hash + SADD to `nodes:{service}`). This matches the pull model -- nodes announce themselves by polling.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `gateway/src/queue/redis.rs`, `gateway/src/auth/node_token.rs`, `gateway/src/http/admin.rs`, `gateway/src/grpc/poll.rs` -- established patterns for Redis hash storage, admin endpoints, gRPC services
- [Redis SCAN documentation](https://redis.io/docs/latest/commands/scan/) -- cursor-based iteration for key cleanup
- [Redis XGROUP DESTROY documentation](https://redis.io/docs/latest/commands/xgroup-destroy/) -- consumer group deletion
- [Tokio graceful shutdown guide](https://tokio.rs/tokio/topics/shutdown) -- SIGTERM handling patterns

### Secondary (MEDIUM confidence)
- [Redis hash vs JSON storage patterns](https://redis.io/docs/latest/develop/ai/redisvl/user_guide/hash_vs_json/) -- hash recommended for per-field access
- [Redis massive key deletion best practices](https://redis.io/faq/doc/12ei3i4gvo/how-can-i-perform-massive-key-deletion-in-redis-without-impacting-performance) -- SCAN+UNLINK pattern

### Tertiary (LOW confidence)
- None -- all findings verified against official docs or existing codebase.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies, extending existing patterns
- Architecture: HIGH -- Redis key schemas follow established codebase patterns, well-understood Redis commands
- Pitfalls: HIGH -- race conditions and SCAN limitations are well-documented Redis behaviors
- Proto extensions: HIGH -- adding unary RPCs to existing service is straightforward tonic pattern

**Research date:** 2026-03-21
**Valid until:** 2026-04-21 (stable domain, no fast-moving dependencies)
