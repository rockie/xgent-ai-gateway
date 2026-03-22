# Phase 3: Service Registry and Node Health - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

Admins can register and manage services, and the gateway tracks node health per service so it knows which nodes are alive and can gracefully handle node departures. Services become explicit entities with persisted configuration. Nodes gain health tracking via poll timestamps and heartbeat RPCs, plus a graceful drain flow for clean shutdown.

</domain>

<decisions>
## Implementation Decisions

### Service configuration and lifecycle
- **D-01:** Services carry: `name`, `description`, `created_at`, `task_timeout_secs`, `max_retries`, `max_nodes` (optional cap), `node_stale_after_secs`, `drain_timeout_secs`
- **D-02:** `task_timeout_secs` and `max_retries` are defined now in the service config; Phase 4 reads them when implementing timeout/retry logic
- **D-03:** Gateway rejects task submissions for unregistered services — submit_task checks the service registry in Redis before enqueuing
- **D-04:** Service registration does NOT create node tokens — token management remains separate (create service first, then add tokens via existing admin endpoints)
- **D-05:** Admin endpoints: `POST /v1/admin/services` (register), `DELETE /v1/admin/services/{name}` (deregister), `GET /v1/admin/services` (list), `GET /v1/admin/services/{name}` (detail)

### Service deregistration and cleanup
- **D-06:** Deregister is asynchronous — endpoint returns immediately (202 Accepted), cleanup happens in a background `tokio::spawn` task
- **D-07:** All pending/queued tasks are marked as failed with error "service deregistered" immediately
- **D-08:** Nodes currently processing tasks can still report results via ReportResult RPC
- **D-09:** All node tokens for the service are auto-revoked (delete all `node_tokens:{service_name}:*` keys)
- **D-10:** All task result hashes (`task:{id}`) belonging to the service are deleted immediately — clean break
- **D-11:** The service's Redis Stream (`tasks:{service_name}`) and consumer group are deleted

### Node health detection
- **D-12:** Both passive and active health tracking — poll timestamps update `last_seen` on every XREADGROUP cycle, plus a new `Heartbeat` unary RPC for nodes to signal liveness during task execution
- **D-13:** Staleness threshold is configurable per-service via `node_stale_after_secs` in service config, with a global default from gateway config
- **D-14:** Three node health states: `healthy` (recently seen), `unhealthy` (stale but stream may reconnect), `disconnected` (stream closed, node gone)
- **D-15:** No background reaper for health — health state is derived in real-time from `last_seen` timestamps as poll/heartbeat events occur. Admin endpoint computes current status on-demand.
- **D-16:** Node registry tracks: `node_id`, `service_name`, `last_seen`, `health_state`, `in_flight_tasks`, `draining`

### Graceful node drain
- **D-17:** New `DrainNode` unary RPC on `NodeService` — node calls it to signal drain intent
- **D-18:** After drain signal, the gRPC poll stream stays open but gateway stops sending new tasks to that node
- **D-19:** Stream acts as liveness signal during drain — node reports results via separate ReportResult RPC and disconnects when done
- **D-20:** Per-service drain timeout (`drain_timeout_secs` in service config) — after timeout, gateway marks node as disconnected. In-flight task recovery deferred to Phase 4's reaper
- **D-21:** Runner agent SIGTERM handling is in scope: SIGTERM -> call DrainNode -> wait for in-flight tasks to complete -> exit cleanly

### Claude's Discretion
- Redis key structure for service config storage (hash vs JSON string)
- Node registry Redis schema (per-node keys vs per-service hash)
- Proto message definitions for new RPCs (Heartbeat, DrainNode, service admin messages)
- Exact health state transition logic and edge cases
- Admin endpoint response shapes and error codes
- How to enumerate and delete task hashes belonging to a service during deregistration (scan pattern or index)
- Global default values for `node_stale_after_secs` and `drain_timeout_secs`
- Admin endpoint authentication (deferred from Phase 2 — decide whether to implement now or continue deferring)

</decisions>

<specifics>
## Specific Ideas

- Service registration replaces the current lazy consumer group creation — `ensure_consumer_group` becomes part of service registration, not submit_task
- The `DrainNode` RPC maps naturally to the runner agent's SIGTERM handler — agent catches signal, calls drain, waits, exits
- Per-service config fields (`task_timeout_secs`, `max_retries`) are write-now-read-later — Phase 4 consumes them without schema changes
- Node health is event-driven, not poll-driven — no background scanning loop, just timestamp updates on each poll/heartbeat event

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/PROJECT.md` — Core constraints: Rust, dual protocol, Redis, auth model, static binary
- `.planning/REQUIREMENTS.md` — Phase 3 covers SRVC-01, SRVC-03, SRVC-04, NODE-03, NODE-05, NODE-06
- `.planning/ROADMAP.md` — Phase 3 success criteria (5 items), depends on Phase 2

### Technology stack
- `CLAUDE.md` §Technology Stack — redis-rs 1.0 MultiplexedConnection, tonic 0.14, axum 0.8, tower middleware
- `CLAUDE.md` §Version Compatibility — tonic/prost/axum version pinning

### Prior phase context
- `.planning/phases/01-core-queue-loop/01-CONTEXT.md` — Redis Streams strategy (D-01..D-04), dual-port pattern (D-05..D-07), project structure (D-08..D-09), node polling model (D-10..D-13)
- `.planning/phases/02-authentication-and-tls/02-CONTEXT.md` — API key storage/transport (D-01..D-06), provisioning (D-07..D-09), admin endpoints unauthenticated (deferred)

### Existing code (critical integration points)
- `gateway/src/queue/redis.rs` — `ensure_consumer_group` (line 70), `submit_task` (line 92), `poll_task` (line 296) — all need service registry checks
- `gateway/src/state.rs` — `AppState` needs service registry state
- `gateway/src/auth/node_token.rs` — Token CRUD functions, `node_tokens:{service_name}:{token_hash}` key pattern
- `gateway/src/http/admin.rs` — Existing admin endpoints for API key and node token CRUD
- `gateway/src/grpc/poll.rs` — Node poll stream, needs drain-aware task dispatch
- `proto/src/gateway.proto` — Needs new RPCs: Heartbeat, DrainNode; needs service admin messages if exposing via gRPC

### Blockers from STATE.md
- `.planning/STATE.md` §Blockers/Concerns — redis-rs MultiplexedConnection under load (relevant for health tracking writes)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `gateway/src/auth/node_token.rs` — Token generate/store/validate/revoke pattern reusable for service registry CRUD
- `gateway/src/http/admin.rs` — Admin endpoint pattern (State extractor, JSON request/response, error mapping) reusable for service endpoints
- `gateway/src/queue/redis.rs:ensure_consumer_group` — Stream/group creation logic moves into service registration
- `gateway/src/types.rs` — `ServiceName` validation type already exists, can enforce registered-service checks
- `gateway/src/error.rs` — `GatewayError` enum extensible for new error variants (ServiceNotFound, ServiceAlreadyExists, NodeNotFound, DrainTimeout)

### Established Patterns
- Redis hash storage for structured data (API keys, node tokens) — same pattern for service config
- SHA-256 hash-based key lookup (API keys, node tokens) — consistent auth pattern
- `Arc<AppState>` shared state — service registry and node health state accessible from both gRPC and HTTP handlers
- Admin endpoints at `/v1/admin/*` with JSON request/response — extend for service CRUD
- `tokio::spawn` for background work (dual-port listeners) — reuse for async deregistration cleanup

### Integration Points
- `queue/redis.rs:submit_task` — Add service registry check before enqueuing
- `grpc/poll.rs` — Record `last_seen` on each poll cycle, check drain state before sending tasks
- `proto/gateway.proto` — Add `Heartbeat` and `DrainNode` RPCs to `NodeService`
- `bin/agent.rs` — Add SIGTERM handler that calls DrainNode and waits for in-flight completion
- `http/admin.rs` — Add service CRUD endpoints alongside existing key/token endpoints
- `state.rs` — Extend AppState with service registry access

</code_context>

<deferred>
## Deferred Ideas

- Admin endpoint authentication — Phase 2 created unauthenticated admin endpoints; securing them (admin tokens, separate auth) remains deferred unless addressed in this phase at Claude's discretion
- Node authentication via mTLS certificates instead of pre-shared tokens — v2 requirement (EAUTH-01)
- Auto-scaling node pools based on queue depth — not in v1 scope
- Service-level rate limiting — deferred to v2 (OPS-03)
- Single-port co-hosting — deferred from Phase 1 and Phase 2, still deferred

</deferred>

---

*Phase: 03-service-registry-and-node-health*
*Context gathered: 2026-03-21*
