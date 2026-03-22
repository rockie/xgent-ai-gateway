# Phase 1: Core Queue Loop - Context

**Gathered:** 2026-03-21
**Status:** Ready for planning

<domain>
## Phase Boundary

A client can submit a task via gRPC or HTTPS, an internal node can poll and claim that task via gRPC server-streaming, execute it, report the result via unary RPC, and the client can retrieve the result by polling -- all backed by Redis Streams with reliable queue semantics. Includes a lightweight node-side proxy (runner agent) that connects to the gateway via gRPC stream and dispatches tasks locally. No authentication in this phase.

</domain>

<decisions>
## Implementation Decisions

### Redis queue strategy
- **D-01:** Use Redis Streams (XADD/XREADGROUP/XACK) instead of list-based BLMOVE
- **D-02:** Each registered service gets its own stream (e.g., `tasks:{service_name}`)
- **D-03:** Each service has one consumer group; each node is a consumer in that group
- **D-04:** Valkey fully supports Streams -- no compatibility constraints

### Protocol hosting
- **D-05:** Dual port -- separate listeners for gRPC and HTTP
- **D-06:** Each port is independently configurable (enable/disable via config)
- **D-07:** Two `tokio::spawn` calls, one per listener

### Project structure
- **D-08:** 2-crate Cargo workspace: `proto/` (tonic-build codegen) + `gateway/` (binary, all business logic)
- **D-09:** Shared types (TaskId, TaskState, ServiceName) live as modules inside `gateway/`

### Node polling model
- **D-10:** Nodes connect via gRPC server-streaming only -- no HTTP polling endpoint for nodes
- **D-11:** Node-side deploys a lightweight proxy (runner agent) that maintains the gRPC stream to gateway and dispatches tasks locally (similar to CI runner agents)
- **D-12:** One task per stream push -- gateway sends next task to whichever node's stream is ready, not batched
- **D-13:** NODE-02 (HTTPS node polling) is deferred -- the proxy unifies the node-side protocol to gRPC

### Result reporting
- **D-14:** Nodes report results via a separate unary RPC, not on the task stream
- **D-15:** Rationale: avoids head-of-line blocking from large result payloads on the task dispatch stream; HTTP/2 multiplexing reuses the same TCP connection

### Disconnect handling
- **D-16:** Phase 1 includes basic stream disconnect detection and reconnection logic in the proxy
- **D-17:** Tasks assigned to a disconnected node need a recovery path (at minimum: detect, log, allow manual re-queue; full reaper is Phase 4)

### Claude's Discretion
- Redis key naming conventions and stream trimming strategy
- Proto message field types and naming
- Gateway module organization within `gateway/` crate
- Config file format details (TOML structure, env var naming)
- Proxy's local dispatch mechanism (how it calls the actual compute service)
- Error types and error propagation strategy

</decisions>

<specifics>
## Specific Ideas

- Runner agent model inspired by CI systems (GitHub Actions runners, GitLab runners) -- lightweight proxy on the node that maintains a persistent connection to the gateway and invokes local services
- Task stream is one-directional for dispatch; results flow back on a separate RPC -- clean separation of concerns

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/PROJECT.md` -- Core value, constraints (Rust, dual protocol, Redis, auth model, static binary)
- `.planning/REQUIREMENTS.md` -- Full requirement list with IDs; Phase 1 covers TASK-01..04, RSLT-01/02/05, NODE-01/04, LIFE-01/02, SRVC-02, INFR-01/02
- `.planning/ROADMAP.md` -- Phase 1 success criteria (5 items)

### Technology stack
- `CLAUDE.md` §Technology Stack -- Recommended versions, compatibility matrix, co-hosting patterns, stack patterns by variant
- `CLAUDE.md` §Alternatives Considered -- What NOT to use and why
- `CLAUDE.md` §Version Compatibility -- Tonic/Prost/Axum/redis-rs version pinning

### Blockers from STATE.md
- `.planning/STATE.md` §Blockers/Concerns -- redis-rs MultiplexedConnection under load, static musl + rustls edge cases

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- None -- greenfield project, no existing code

### Established Patterns
- None yet -- Phase 1 establishes all foundational patterns

### Integration Points
- Redis/Valkey instance (external dependency, must be running for development)
- Node-side compute services (the proxy dispatches to these, interface TBD by proxy design)

</code_context>

<deferred>
## Deferred Ideas

- NODE-02 (HTTP node polling) -- replaced by proxy model, may revisit if lightweight HTTP-only nodes are needed
- gRPC bidirectional streaming for combined task dispatch + result reporting -- deferred in favor of clean separation (unary RPC for results)
- Single-port co-hosting (content-type based routing) -- may revisit in Phase 2 when TLS configs are set up
- Node concurrency declaration ("I can handle N tasks simultaneously") -- future enhancement

</deferred>

---

*Phase: 01-core-queue-loop*
*Context gathered: 2026-03-21*
