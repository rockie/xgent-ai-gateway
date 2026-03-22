# Phase 4: Task Reliability and Callbacks - Context

**Gathered:** 2026-03-22
**Status:** Ready for planning

<domain>
## Phase Boundary

Tasks that fail or time out are detected by a background reaper and marked as failed. Clients can optionally receive results via callback URL instead of polling. No automatic retries and no dead letter queue in v1 — failed tasks are terminal, clients resubmit if needed.

**Descoped from original roadmap:**
- LIFE-04 (retry with backoff) — clients resubmit instead
- LIFE-05 (dead letter queue) — `failed` state on task hash is sufficient

**In scope:**
- LIFE-03 (background reaper detects timed-out tasks, marks them failed)
- RSLT-03 (callback URL at submission)
- RSLT-04 (callback delivery with exponential backoff retries on failure)

</domain>

<decisions>
## Implementation Decisions

### Timeout detection (background reaper)
- **D-01:** XPENDING scan per service — reaper calls `XPENDING <stream> workers - + <count>` per service, checks idle time against service's `task_timeout_secs`
- **D-02:** Fixed 30-second reaper interval via `tokio::spawn` background loop
- **D-03:** Single background task that cycles through all registered services sequentially (no per-service tasks)
- **D-04:** Reaper does not requeue timed-out tasks and does not modify node health. Timed-out tasks are marked as `failed` (terminal). Clients resubmit if needed. Node health remains event-driven per Phase 3 pattern.

### Retry policy
- **D-05:** No automatic retries in v1. When a node reports failure or a task times out, the task moves to `failed` (terminal). Clients are responsible for resubmitting.
- **D-06:** `max_retries` field in `ServiceConfig` remains for forward compatibility but the gateway ignores it in v1.
- **D-07:** No backoff logic, no retry state, no state machine changes needed.

### Dead letter queue
- **D-08:** No dedicated DLQ. `failed` state on the task hash is terminal. Clients see failure via polling.
- **D-09:** LIFE-05 (dead letter queue) descoped from v1.
- **D-10:** Admin endpoint or metrics should expose failed task counts per service (not individual task listings). Exact mechanism is Claude's discretion (counter in Redis or computed from XPENDING).
- **D-11:** No special TTL for failed tasks — same `result_ttl_secs` as all tasks.
- **D-12:** Reaper timeout error message: `"task timed out: node did not report result within {N}s"` where N is the service's `task_timeout_secs`.

### Callback delivery
- **D-13:** Callback URL is HTTP-only. Set as a default on the API key (`POST /v1/admin/api-keys` gains optional `callback_url` field in `ClientMetadata`). Individual HTTP task submissions can override with a per-task `callback_url` field.
- **D-14:** URL format validation at submission and key creation — parse as valid URL, reject malformed. No reachability check.
- **D-15:** Callback fires on any terminal state (`completed` or `failed`).
- **D-16:** Callback POST body is minimal — `task_id` and `state` only. Client fetches full result via poll if needed.
- **D-17:** `PATCH /v1/admin/api-keys/{key_hash}` endpoint to update callback URL on existing keys.
- **D-18:** Configurable max callback retries with exponential backoff. Configurable per gateway config (e.g., `callback.max_retries`, `callback.initial_delay_ms`). Default: 3 retries, 1s/2s/4s.
- **D-19:** Callback failure is log-only. No client-visible indication — client should poll as fallback.

### Claude's Discretion
- reqwest HTTP client setup for callback delivery (connection pooling, timeouts)
- Callback config section structure in `GatewayConfig`
- How to track failed task counts per service (Redis counter vs computed)
- Proto changes needed for callback support (if any — callback is HTTP-only)
- XPENDING batch size and pagination for services with many pending tasks
- Whether `callback_url` field is stored in the task hash or resolved from ClientMetadata at delivery time
- Background task lifecycle management (graceful shutdown of reaper)

</decisions>

<specifics>
## Specific Ideas

- XPENDING is purpose-built for detecting claimed-but-unacknowledged stream entries — maps directly to "task assigned to node but never completed"
- Per-key callback URL with per-task override mirrors how webhook systems work (Stripe, GitHub) — a sensible default with escape hatch
- Minimal callback body (`task_id` + `state`) avoids leaking result payloads to potentially less-secure callback endpoints — client uses authenticated poll to fetch actual results
- The reaper + callback delivery are the first two background tasks in the gateway (alongside the dual-port listeners) — establishes the pattern for future background work

</specifics>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Project specifications
- `.planning/PROJECT.md` — Core constraints: Rust, dual protocol, Redis, auth model, static binary
- `.planning/REQUIREMENTS.md` — Phase 4 covers LIFE-03, RSLT-03, RSLT-04. LIFE-04 and LIFE-05 descoped.
- `.planning/ROADMAP.md` — Phase 4 success criteria (4 items — criteria 2 and 3 are descoped, update roadmap accordingly)

### Technology stack
- `CLAUDE.md` §Technology Stack — redis-rs 1.0 MultiplexedConnection, tokio for background tasks
- `CLAUDE.md` §Supporting Libraries — reqwest not in current stack; will need adding for HTTP callback delivery

### Prior phase context
- `.planning/phases/01-core-queue-loop/01-CONTEXT.md` — Redis Streams strategy (D-01..D-04), XREADGROUP/XACK pattern, stream_id tracking
- `.planning/phases/02-authentication-and-tls/02-CONTEXT.md` — API key storage (D-01..D-04), ClientMetadata structure, admin endpoints
- `.planning/phases/03-service-registry-and-node-health/03-CONTEXT.md` — ServiceConfig with `task_timeout_secs`/`max_retries` (D-01..D-02), node health event-driven (D-15), reaper deferred to Phase 4

### Existing code (critical integration points)
- `gateway/src/queue/redis.rs` — `report_result` (line 216) transitions to Failed; reaper needs similar state update. XPENDING scan happens on the same streams.
- `gateway/src/queue/redis.rs` — `poll_task` (line 301) stores `stream_id` in task hash — reaper uses this for XACK after marking timeout.
- `gateway/src/registry/service.rs` — `list_services` (line 101) for reaper to iterate all services. `get_service` (line 81) for `task_timeout_secs`.
- `gateway/src/registry/node_health.rs` — `ServiceConfig` struct (line 9) has `task_timeout_secs` field.
- `gateway/src/auth/api_key.rs` — `ClientMetadata` needs `callback_url` field. Key CRUD needs `callback_url` support.
- `gateway/src/http/submit.rs` — `SubmitTaskRequest` needs optional `callback_url` field for per-task override.
- `gateway/src/http/admin.rs` — Needs PATCH endpoint for updating API key callback URL.
- `gateway/src/types.rs` — `TaskState` state machine unchanged (no new states needed).
- `gateway/src/config.rs` — Needs `CallbackConfig` section (max_retries, initial_delay_ms).
- `proto/src/gateway.proto` — No changes needed (callback is HTTP-only per D-13).

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `registry/service.rs:list_services` — Reaper iterates this to find all services for XPENDING scan
- `registry/service.rs:get_service` — Reaper reads `task_timeout_secs` per service to compare against XPENDING idle time
- `queue/redis.rs:report_result` — Pattern for marking tasks as failed (HSET state + XACK) reusable for reaper's timeout marking
- `auth/api_key.rs:ClientMetadata` — Extend with `callback_url` field; existing HSET/HGETALL pattern for persistence
- `http/admin.rs` — Admin endpoint pattern (State extractor, JSON, error mapping) reusable for PATCH key endpoint

### Established Patterns
- `tokio::spawn` for background work (dual-port listeners, async deregistration cleanup) — same pattern for reaper and callback delivery
- Redis hash storage for structured data (task hashes, API keys, service configs) — consistent across all modules
- `Arc<AppState>` shared state — reaper and callback tasks access queue and config through same pattern
- Redis pipeline for atomic multi-command operations — reaper's mark-failed + XACK should use pipeline

### Integration Points
- `queue/redis.rs` — New method for XPENDING scan + timeout detection
- `queue/redis.rs:report_result` — After marking terminal state, check for callback URL and trigger delivery
- `auth/api_key.rs` — Add `callback_url` to `ClientMetadata`, update store/retrieve
- `http/submit.rs` — Add optional `callback_url` to `SubmitTaskRequest`, store in task hash
- `http/admin.rs` — Add PATCH endpoint for API key callback URL update
- `config.rs` — Add `CallbackConfig` section
- `main.rs` — Spawn reaper background task alongside server listeners

</code_context>

<deferred>
## Deferred Ideas

- LIFE-04 (automatic retries with backoff) — descoped from v1. Clients resubmit on failure. May revisit in v2.
- LIFE-05 (dead letter queue) — descoped from v1. `failed` state is sufficient. May revisit in v2.
- gRPC callback support — v1 is HTTP-only. Could add gRPC streaming notifications in v2.
- Callback authentication — v1 does not sign or authenticate callback POSTs. Could add HMAC signatures in v2.
- Task requeue on timeout — v1 marks as failed only. Automatic requeue could be a v2 feature.
- Per-task callback retry configuration — v1 uses gateway-global config only.

</deferred>

---

*Phase: 04-task-reliability-and-callbacks*
*Context gathered: 2026-03-22*
