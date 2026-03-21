# Pitfalls Research

**Domain:** Pull-model task gateway / distributed job queue (Rust, gRPC, Redis)
**Researched:** 2026-03-21
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Task Loss on Worker Crash (The "Pop and Pray" Problem)

**What goes wrong:**
A node polls a task from the queue (destructive read), begins processing, then crashes before reporting the result. The task is gone from the queue and never completed. The client polls forever or times out with no result.

**Why it happens:**
Developers use a simple LPUSH/BRPOP pattern because it is straightforward. The pop operation removes the task from the queue atomically -- if the worker dies after the pop, the task vanishes. This is the single most common failure mode in Redis-backed queues.

**How to avoid:**
Use Redis Streams with consumer groups (XREADGROUP/XACK) instead of list-based BRPOP. Streams keep a Pending Entries List (PEL) that tracks delivered-but-unacknowledged messages. If a worker crashes, XAUTOCLAIM reassigns the task after an idle timeout. If you must use lists, use BRPOPLPUSH to atomically move the task to a per-worker processing list, then LREM on completion. A reaper process monitors the processing lists for stale entries. Either way, every task needs a state machine: `pending -> assigned -> running -> completed/failed`, persisted in Redis with TTLs.

**Warning signs:**
- Tasks that clients submit but never get results for (orphaned tasks)
- Task count in the system decreases without corresponding completions
- No monitoring for the "assigned but not completed" state

**Phase to address:**
Phase 1 (Core Queue). This must be correct from the start. Retrofitting reliable delivery onto a naive queue requires rewriting the entire task lifecycle.

---

### Pitfall 2: Lease Timeout / Duplicate Execution Race

**What goes wrong:**
A node takes longer than the lease timeout to complete a task (AI inference can be unpredictable). The gateway assumes the node is dead and reassigns the task to another node. Now two nodes are processing the same task. Both report results -- the client gets the second result, or worse, side effects happen twice (duplicate webhook callbacks, duplicate API calls in agent jobs).

**Why it happens:**
Fixed lease timeouts cannot accommodate variable-duration workloads. AI inference can take 2 seconds or 120 seconds depending on model, input size, and GPU load. A timeout safe for the fast case is deadly for the slow case, and vice versa.

**How to avoid:**
Implement heartbeat-based leases instead of fixed timeouts. Nodes must send periodic heartbeats while processing. The lease extends on each heartbeat. Only when heartbeats stop for N consecutive intervals does the gateway reassign. Additionally, every task assignment must carry a unique assignment token (a "fencing token" or monotonically increasing generation ID). When reporting results, the node includes its assignment token. The gateway rejects results from stale assignments. Require all downstream task handlers to be idempotent -- callbacks should include an idempotency key.

**Warning signs:**
- Duplicate results appearing for the same task ID
- Clients receiving results after already receiving a timeout error
- Nodes reporting "task not found" when submitting results (because another node already completed it)

**Phase to address:**
Phase 1 (Core Queue) for heartbeat leases. Phase 2 (Node Management) for fencing tokens. Idempotency guidance in client SDK/docs from the start.

---

### Pitfall 3: Thundering Herd on Queue Notification

**What goes wrong:**
When a new task arrives, all idle nodes for that service wake up and poll simultaneously. With 50 idle nodes, 50 Redis operations fire at once. 49 get nothing. Under burst load, this amplifies into thousands of wasted polls per second, overwhelming Redis and the gateway.

**Why it happens:**
The naive approach to pull-model is: "nodes poll on a fixed interval" or "nodes long-poll and all get notified when a task arrives." Both create thundering herd effects. Fixed-interval polling with synchronized timers is equally bad -- all nodes reset their timers on startup and poll in lockstep.

**How to avoid:**
Use Redis Streams with XREADGROUP and a COUNT of 1. Only one consumer in the group receives each message -- no thundering herd. If using list-based queues, use BRPOP which blocks until an item appears and only one client gets it. Add jitter to polling intervals (base interval + random(0, base/2)). Never use pub/sub to notify all nodes of new work -- use it only as a "wake-up hint" where nodes then compete to claim work via atomic operations. For the gRPC long-poll endpoint: hold the connection server-side and only respond when work is actually available for that specific node, not when work arrives for the service generally.

**Warning signs:**
- Redis CPU spikes on task submission
- Gateway CPU/memory spikes that correlate with task submission bursts rather than task volume
- High ratio of empty poll responses to successful task claims

**Phase to address:**
Phase 1 (Core Queue). The polling mechanism is foundational and extremely painful to change later.

---

### Pitfall 4: gRPC Long-Poll Connections Killed by Intermediaries

**What goes wrong:**
Nodes maintain gRPC long-poll connections to the gateway. Load balancers, reverse proxies (nginx, envoy, cloud LBs), and NAT gateways silently close idle connections after 60-120 seconds. The node thinks it is connected and waiting for work. The gateway thinks the connection is alive. Neither side knows it is dead until the next task assignment fails.

**Why it happens:**
HTTP/2 connections (which gRPC uses) can appear idle when no frames are being exchanged during a long poll. Network intermediaries have idle timeout policies. Cloud load balancers (AWS ALB: 60s default, GCP: 600s) and NAT gateways (AWS NAT Gateway: 350s for TCP) aggressively reclaim idle connections. This is especially insidious because the gateway is on the public internet and nodes connect from behind NAT -- there are always intermediaries.

**How to avoid:**
Enable HTTP/2 keepalive pings on both sides. In tonic, configure `http2_keepalive_interval` (e.g., 30 seconds) and `http2_keepalive_timeout` (e.g., 10 seconds) on both server and client. These send HTTP/2 PING frames that keep the connection alive through intermediaries and detect dead connections. Additionally, implement application-level heartbeats as a defense-in-depth measure -- do not rely solely on TCP/HTTP/2 keepalives. On the server side, track last-seen timestamps per node and mark nodes as unhealthy if silent for too long.

**Warning signs:**
- Nodes appearing as "connected" in the gateway but never receiving tasks
- Periodic connection resets every ~60 seconds in node logs
- Tasks assigned to nodes that never acknowledge them

**Phase to address:**
Phase 1 (gRPC server setup). Configure keepalives from day one. This is a single configuration knob but discovering it in production is painful.

---

### Pitfall 5: Redis as Single Point of Failure Without Durability Guarantees

**What goes wrong:**
Redis crashes or runs out of memory. All in-flight task state is lost -- pending tasks, running task assignments, result data. If AOF persistence is not enabled (or is set to `everysec`), the last second of writes is lost. If Redis is the only store, there is no recovery path. Alternatively, Redis evicts keys under memory pressure, silently dropping task data.

**Why it happens:**
Redis is treated as a database when it is an in-memory cache with optional persistence. Developers trust `maxmemory-policy noeviction` to prevent data loss but forget that this causes Redis to reject writes instead, which manifests as gateway errors under load. Redis Sentinel failover can also lose data -- the new primary may be behind the old one by some writes.

**How to avoid:**
Configure `appendonly yes` with `appendfsync everysec` (acceptable for this use case -- losing 1 second of tasks on crash is tolerable given retry logic). Set `maxmemory-policy noeviction` and monitor memory usage with alerts at 70% and 90%. Implement result expiration -- completed task results should have a TTL (e.g., 1 hour) so they do not accumulate forever. Keep task payloads out of Redis if they are large -- store a reference to an object store instead. For production, use Redis Sentinel or Valkey with replication for failover, but understand that failover can lose the most recent writes. The gateway must handle Redis unavailability gracefully: reject new submissions with a clear error rather than silently dropping them.

**Warning signs:**
- Redis memory usage climbing monotonically (no TTLs on results)
- No AOF file configured
- `maxmemory-policy` set to `allkeys-lru` (task data being evicted silently)
- Gateway errors during Redis failover with no retry logic

**Phase to address:**
Phase 1 (Infrastructure). Redis configuration is a deployment concern but the data model (TTLs, key structure, payload sizes) must be designed in Phase 1 code.

---

### Pitfall 6: Node Authentication Bypass Through Replay or Token Theft

**What goes wrong:**
Pre-shared tokens for node authentication are static secrets. If a token leaks (logs, config files, container images), an attacker can impersonate a node, claim tasks, and either steal sensitive payloads (AI inference inputs may contain PII) or return malicious results. Since the gateway sits on the public internet, the attack surface is real.

**Why it happens:**
Pre-shared tokens are chosen for simplicity. Unlike mTLS, they do not require certificate infrastructure. But static secrets have no expiration, no revocation mechanism, and no cryptographic binding to the node identity. Developers also tend to log request headers during debugging, inadvertently persisting tokens.

**How to avoid:**
Treat pre-shared tokens as a Phase 1 expedient, not the final auth model. In Phase 1: scope tokens per-service (not global), store them hashed (bcrypt/argon2) in the gateway's config, never log them, and transmit them only over TLS. Add token rotation support early -- the gateway should accept both the current and previous token for a service during rotation. Plan mTLS for nodes in Phase 2 or 3. It solves identity, authentication, and transport security in one mechanism. For the public internet surface: rate-limit failed auth attempts per source IP.

**Warning signs:**
- Tokens appearing in log files or error messages
- No token rotation mechanism (tokens that have been the same since deployment)
- Single global token shared across all services
- No TLS on the node-to-gateway connection in development (habits carry to production)

**Phase to address:**
Phase 1 (basic token auth with per-service scoping), Phase 3 (mTLS for nodes, token rotation API).

---

### Pitfall 7: Callback Delivery Failures Silently Losing Results

**What goes wrong:**
A task completes successfully. The gateway attempts to deliver the result via the client's callback URL. The callback endpoint is down, returns a 5xx, or times out. The gateway does not retry or retries a fixed number of times and gives up. The result is lost -- the client never configured polling as a fallback and assumes the callback will arrive.

**Why it happens:**
Callback delivery is treated as a fire-and-forget side effect rather than a reliable delivery problem. Developers implement the happy path (POST result to URL, done) and underestimate how often external HTTP endpoints fail.

**How to avoid:**
Design callbacks as a best-effort notification, not the primary delivery mechanism. Results must always be retrievable via polling by task ID. For callbacks: implement exponential backoff retries (e.g., 1s, 5s, 30s, 2m, 10m) with a configurable max retry count. Store callback delivery status alongside the task. After max retries, mark the callback as failed but keep the result available for polling. Optionally implement a dead-letter log for failed callbacks. Sign callback payloads with HMAC so clients can verify the gateway sent them (prevents spoofing).

**Warning signs:**
- No retry logic in the callback delivery path
- No monitoring of callback success/failure rates
- Clients asking "where is my result?" when the task shows as completed

**Phase to address:**
Phase 2 (Callback Delivery). Polling must work in Phase 1. Callbacks are Phase 2 with reliability built in from the start of that phase.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| BRPOP lists instead of Redis Streams | Simpler implementation, fewer Redis concepts | No built-in PEL, manual crash recovery, no message history | Never for production -- Streams are the right primitive for this use case |
| Single Redis key namespace (no prefixes) | Less code for key construction | Key collisions between services, impossible to shard later, cannot reason about data ownership | Never -- use `svc:{service_id}:tasks:pending` style keys from day one |
| Storing full task payloads in Redis | No external storage dependency | Redis memory bloat, slow persistence, expensive replication | Only if payloads are guaranteed small (<10KB). For AI workloads with images/prompts, use object storage references |
| Global pre-shared token | One config value, quick setup | Cannot revoke one compromised service, no audit trail, lateral movement risk | Only in local development |
| Synchronous callback delivery in the task completion path | Simpler code flow | Task completion blocked by external HTTP call, cascading timeouts | Never -- always deliver callbacks asynchronously |
| No task result TTL | Results always available | Redis memory grows unbounded, OOM crash | Never -- set TTLs from day one (1 hour default, configurable per service) |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| Redis/Valkey | Using `MULTI/EXEC` transactions expecting rollback semantics | Redis transactions are not ACID -- all commands execute even if one fails. Use Lua scripts for atomic check-and-set operations (e.g., claim a task only if it is still in `pending` state) |
| Redis/Valkey | Blocking commands (BRPOP) in connection pools | Blocking commands tie up a connection indefinitely. Use a dedicated connection (or connection pool) for blocking operations, separate from the pool used for regular commands |
| tonic (gRPC) | Default message size limits (4MB decode) | AI inference results (images, long text) can exceed 4MB. Configure `max_decoding_message_size` and `max_encoding_message_size` explicitly. Better: stream large results via chunked responses or external storage references |
| tonic (gRPC) | Creating new streams per message in bidirectional streaming | Use mpsc channels bridged to the gRPC stream. Pass the stream to the handler once and send messages through the channel |
| axum (HTTP) | Sharing state without Arc/Clone-friendly types | axum extractors require `Clone` on shared state. Use `Arc<AppState>` from the start. Refactoring state management later touches every handler |
| Cloud Load Balancers | Assuming HTTP/2 end-to-end | Many cloud LBs (AWS ALB) terminate HTTP/2 and re-establish HTTP/1.1 to the backend, breaking gRPC. Use NLB (TCP passthrough) or a gRPC-aware LB for the gRPC port |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| Polling interval too short | Redis CPU saturated, gateway CPU high, most polls return empty | Adaptive polling: start with long interval, shorten when tasks are available, back off when idle. Or use blocking reads (XREADGROUP BLOCK) | >50 nodes polling at <1s intervals |
| Serializing task payloads through the gateway | Gateway becomes a bandwidth bottleneck, memory spikes on large payloads | For large payloads (>100KB), store in object storage (S3/MinIO), pass only the reference through the queue | ~100 nodes with image/model payloads (MBs each) |
| Single Redis instance for all services | All services share one Redis, one slow service's large payloads evict another service's small tasks | Use Redis key prefixes + monitor per-service memory. For true isolation, use separate Redis databases or instances per service group | >10 services with mixed workload sizes |
| No connection pooling for Redis | New TCP connection per operation, connection setup overhead dominates | Use a connection pool (e.g., `bb8` or `deadpool-redis` in Rust) with a size matched to expected concurrency | >100 concurrent operations |
| Unbounded in-memory queues in the gateway | Gateway buffers tasks in memory before writing to Redis, OOM under burst | Apply backpressure: reject or queue-limit submissions when Redis write latency exceeds threshold. Use bounded channels (tokio mpsc) internally | Burst of >1000 tasks/second |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| API keys transmitted over plain HTTP | Credential theft via network sniffing | Enforce TLS on all endpoints. Reject non-TLS connections. HSTS headers on HTTP API |
| Task payloads visible to all nodes for a service | A compromised node in service A can claim work for service B if tokens are shared | Scope tokens strictly per-service. Validate that the claiming node's token matches the task's service |
| No rate limiting on task submission | DoS via flooding the queue, exhausting Redis memory | Implement per-API-key rate limits. Even basic token bucket (e.g., 100 tasks/min per key) prevents abuse |
| Callback URLs not validated | SSRF -- an attacker submits a task with a callback URL pointing to internal infrastructure (169.254.169.254, internal services) | Validate callback URLs against an allowlist of schemes (https only) and block private IP ranges. Resolve DNS and check the IP before connecting |
| Node identity not verified beyond token | A stolen token allows full impersonation with no forensic trail | Log node connection metadata (source IP, TLS fingerprint). In later phases, move to mTLS where identity is cryptographically bound |
| Task results not encrypted at rest in Redis | Anyone with Redis access reads all results, which may contain PII/sensitive AI outputs | Encrypt sensitive fields before storing in Redis. Or accept the risk for v1 and document it as a known limitation with a plan to address |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Task status only shows "running" with no progress indication | Clients poll repeatedly not knowing if the task is stuck or progressing | Include a `last_heartbeat` timestamp in status responses so clients can distinguish "running and active" from "running but possibly stuck" |
| Opaque error messages on task failure | Clients get `{"status": "failed"}` with no actionable information | Include error category (timeout, node_crash, application_error), a human-readable message, and whether the task will be retried |
| No way to cancel a submitted task | Client submits wrong task, wastes compute for minutes | Implement task cancellation that propagates to the assigned node. Even if the node cannot stop immediately, mark the result as "cancelled" so it is not delivered |
| Callback registration only at submission time | Client wants to add/change callback URL after submission | Allow updating callback URL on a pending/running task. Common when debugging or when the callback endpoint changes |
| No task listing or search | Clients lose track of task IDs, cannot audit what they submitted | Provide a list endpoint filtered by API key, service, status, and time range. Even a simple "my recent tasks" endpoint prevents support burden |

## "Looks Done But Isn't" Checklist

- [ ] **Task lifecycle:** Often missing the `assigned -> timed_out -> re-queued` transition -- verify that stuck tasks are automatically recovered
- [ ] **Node deregistration:** Often missing cleanup of tasks assigned to a deregistered node -- verify those tasks are re-queued
- [ ] **Graceful shutdown:** Often missing drain logic -- verify that the gateway stops accepting new tasks, waits for in-flight callbacks, and gives nodes time to complete current work before exiting
- [ ] **Redis reconnection:** Often missing automatic reconnect with backoff after Redis restarts -- verify the gateway recovers without manual intervention
- [ ] **Clock skew handling:** Often missing -- verify that timeout calculations use monotonic clocks or server-side timestamps, not wall clocks that can jump
- [ ] **Service deletion:** Often missing cleanup of queued tasks and registered nodes for a deleted service -- verify no orphaned data remains
- [ ] **Large result handling:** Often missing -- verify what happens when a node returns a 50MB result (reject? stream? store externally?)
- [ ] **Concurrent task limit per node:** Often missing -- verify that a node can declare its capacity (e.g., 4 concurrent tasks for 4 GPUs) and the gateway respects it

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Task loss from naive BRPOP | HIGH | Redesign queue to use Redis Streams. Migrate existing tasks (likely lost). Add PEL-based recovery. Requires client-side idempotency retrofit |
| Duplicate execution from fixed timeouts | MEDIUM | Add fencing tokens to the protocol (breaking API change). Clients must handle duplicate callbacks (idempotency keys) |
| Thundering herd | MEDIUM | Switch polling mechanism. If using pub/sub notification, move to competing-consumer pattern. May require protocol changes |
| Silent connection death | LOW | Add keepalive configuration. Deploy and monitor. No protocol changes needed |
| Redis data loss | HIGH | If AOF was not enabled, data is gone. Enable AOF, restore from any available backups, re-submit lost tasks (if clients have records). Implement proper persistence config before next incident |
| Token compromise | MEDIUM | Rotate the compromised token immediately (requires all nodes for that service to update). Audit task results submitted during the compromise window. Move to mTLS to prevent recurrence |
| Callback delivery failure | LOW | Results are still available via polling (if designed correctly). Replay failed callbacks from the dead-letter log. If no log exists, clients must poll for any tasks submitted during the failure window |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Task loss on worker crash | Phase 1 (Core Queue) | Integration test: kill a node mid-task, verify task is re-queued and completed by another node |
| Duplicate execution race | Phase 1 (Core Queue) + Phase 2 (Node Mgmt) | Test: slow-process a task past timeout, verify only one result is accepted via fencing token |
| Thundering herd | Phase 1 (Core Queue) | Load test: submit 1 task with 50 idle nodes, measure Redis operations (should be ~1, not ~50) |
| Connection death through intermediaries | Phase 1 (gRPC Setup) | Deploy behind a load balancer, idle for 5 minutes, verify node still receives tasks |
| Redis as SPOF | Phase 1 (Infrastructure) | Chaos test: restart Redis, verify gateway recovers and no tasks are lost |
| Token-based auth weaknesses | Phase 1 (Auth) basic, Phase 3 (mTLS) full | Security review: verify tokens are hashed at rest, scoped per-service, rotatable |
| Callback delivery failures | Phase 2 (Callbacks) | Test: submit task with callback to unreachable URL, verify retries and eventual dead-letter |
| SSRF via callback URLs | Phase 2 (Callbacks) | Test: submit callback URL pointing to 169.254.169.254, verify it is rejected |

## Sources

- [Redis Streams documentation](https://redis.io/docs/latest/develop/data-types/streams/) - Consumer groups, PEL, XAUTOCLAIM
- [Redis reliable queue patterns (BRPOPLPUSH)](https://redis.io/docs/latest/commands/rpoplpush/) - Reliable queue pattern documentation
- [How to Build a Reliable Queue in Redis (Svix)](https://www.svix.com/resources/redis/reliable-queue/) - Practical reliable queue implementation
- [gRPC Performance Best Practices](https://grpc.io/docs/guides/performance/) - Keepalive, connection management
- [Microsoft gRPC Performance Best Practices](https://learn.microsoft.com/en-us/aspnet/core/grpc/performance) - Channel pooling, message sizes
- [tonic GitHub - streaming issues](https://github.com/hyperium/tonic/issues/2228) - Long-lived stream pitfalls
- [System Design: Distributed Job Scheduler](https://www.systemdesignhandbook.com/guides/design-a-distributed-job-scheduler/) - Lease-based race conditions, idempotency
- [Temporal: Reliable Data Processing](https://temporal.io/blog/reliable-data-processing-queues-workflows) - Queue reliability patterns
- [Multi-process task queue using Redis Streams (Charles Leifer)](https://charlesleifer.com/blog/multi-process-task-queue-using-redis-streams/) - Practical Streams-based queue
- [AWS mTLS considerations](https://aws.amazon.com/blogs/containers/three-things-to-consider-when-implementing-mutual-tls-with-aws-app-mesh/) - Certificate rotation, trust anchors
- [Exactly Once Operations: Idempotency in the Business Layer](https://equatorops.com/resources/blog/idempotency-business-layer) - Why exactly-once requires application-level idempotency
- [DBOS: Durable Queues](https://www.dbos.dev/blog/durable-queues) - 15 years of queue design lessons

---
*Pitfalls research for: Pull-model task gateway / distributed job queue*
*Researched: 2026-03-21*
