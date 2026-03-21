# Phase 4: Task Reliability and Callbacks - Research

**Researched:** 2026-03-22
**Domain:** Background task reaping (Redis Streams XPENDING), HTTP callback delivery (reqwest), Rust async background tasks (tokio::spawn)
**Confidence:** HIGH

## Summary

Phase 4 adds two independent background capabilities to the gateway: (1) a reaper that detects timed-out tasks via Redis Streams XPENDING and marks them as failed, and (2) a callback delivery system that POSTs minimal notifications to client-provided URLs when tasks reach terminal state. Both are greenfield additions that integrate cleanly with existing patterns -- `tokio::spawn` for background work, `redis::pipe()` for atomic Redis operations, and `Arc<AppState>` for shared state access.

The scope is deliberately narrow thanks to descoping decisions in CONTEXT.md. No automatic retries, no dead letter queue, no task requeue. The reaper marks timed-out tasks as `failed` (terminal). The callback system delivers `{task_id, state}` and retries with exponential backoff on HTTP failure. reqwest is already in `Cargo.toml` -- no new dependencies needed.

**Primary recommendation:** Implement the reaper and callback delivery as two independent `tokio::spawn` background tasks in `main.rs`. The reaper scans XPENDING per service every 30 seconds. Callback delivery is triggered from `report_result` and the reaper's mark-failed path, spawning per-callback async tasks with retry logic.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** XPENDING scan per service -- reaper calls `XPENDING <stream> workers - + <count>` per service, checks idle time against service's `task_timeout_secs`
- **D-02:** Fixed 30-second reaper interval via `tokio::spawn` background loop
- **D-03:** Single background task that cycles through all registered services sequentially (no per-service tasks)
- **D-04:** Reaper does not requeue timed-out tasks and does not modify node health. Timed-out tasks are marked as `failed` (terminal). Clients resubmit if needed. Node health remains event-driven per Phase 3 pattern.
- **D-05:** No automatic retries in v1. When a node reports failure or a task times out, the task moves to `failed` (terminal). Clients are responsible for resubmitting.
- **D-06:** `max_retries` field in `ServiceConfig` remains for forward compatibility but the gateway ignores it in v1.
- **D-07:** No backoff logic, no retry state, no state machine changes needed.
- **D-08:** No dedicated DLQ. `failed` state on the task hash is terminal. Clients see failure via polling.
- **D-09:** LIFE-05 (dead letter queue) descoped from v1.
- **D-10:** Admin endpoint or metrics should expose failed task counts per service (not individual task listings). Exact mechanism is Claude's discretion (counter in Redis or computed from XPENDING).
- **D-11:** No special TTL for failed tasks -- same `result_ttl_secs` as all tasks.
- **D-12:** Reaper timeout error message: `"task timed out: node did not report result within {N}s"` where N is the service's `task_timeout_secs`.
- **D-13:** Callback URL is HTTP-only. Set as a default on the API key (`POST /v1/admin/api-keys` gains optional `callback_url` field in `ClientMetadata`). Individual HTTP task submissions can override with a per-task `callback_url` field.
- **D-14:** URL format validation at submission and key creation -- parse as valid URL, reject malformed. No reachability check.
- **D-15:** Callback fires on any terminal state (`completed` or `failed`).
- **D-16:** Callback POST body is minimal -- `task_id` and `state` only. Client fetches full result via poll if needed.
- **D-17:** `PATCH /v1/admin/api-keys/{key_hash}` endpoint to update callback URL on existing keys.
- **D-18:** Configurable max callback retries with exponential backoff. Configurable per gateway config (e.g., `callback.max_retries`, `callback.initial_delay_ms`). Default: 3 retries, 1s/2s/4s.
- **D-19:** Callback failure is log-only. No client-visible indication -- client should poll as fallback.

### Claude's Discretion
- reqwest HTTP client setup for callback delivery (connection pooling, timeouts)
- Callback config section structure in `GatewayConfig`
- How to track failed task counts per service (Redis counter vs computed)
- Proto changes needed for callback support (if any -- callback is HTTP-only)
- XPENDING batch size and pagination for services with many pending tasks
- Whether `callback_url` field is stored in the task hash or resolved from ClientMetadata at delivery time
- Background task lifecycle management (graceful shutdown of reaper)

### Deferred Ideas (OUT OF SCOPE)
- LIFE-04 (automatic retries with backoff) -- descoped from v1. Clients resubmit on failure. May revisit in v2.
- LIFE-05 (dead letter queue) -- descoped from v1. `failed` state is sufficient. May revisit in v2.
- gRPC callback support -- v1 is HTTP-only. Could add gRPC streaming notifications in v2.
- Callback authentication -- v1 does not sign or authenticate callback POSTs. Could add HMAC signatures in v2.
- Task requeue on timeout -- v1 marks as failed only. Automatic requeue could be a v2 feature.
- Per-task callback retry configuration -- v1 uses gateway-global config only.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| LIFE-03 | Background reaper detects timed-out tasks (node died) and marks them failed | XPENDING with IDLE filter per service stream, 30s interval background loop, pipeline HSET state=failed + XACK |
| RSLT-03 | Client can optionally provide a callback URL at submission for result delivery | `callback_url` field on `ClientMetadata` (per-key default) and `SubmitTaskRequest` (per-task override), URL validation at submission |
| RSLT-04 | Gateway delivers results to callback URL with exponential backoff retries on failure | reqwest POST with `{task_id, state}` body, configurable retry with 1s/2s/4s backoff, fire-and-forget spawned tasks |
</phase_requirements>

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| redis-rs | 1.0.x (resolved 1.0.4) | XPENDING scan, pipeline HSET+XACK for reaper | Already used throughout. XPENDING is native Redis Streams command. |
| reqwest | 0.12.x (resolved 0.12.28) | HTTP POST for callback delivery | Already in Cargo.toml. Built on hyper/tokio. Connection pooling built-in. |
| tokio | 1.50+ | Background task spawning, sleep for intervals/backoff | Already the async runtime. `tokio::spawn`, `tokio::time::sleep`, `tokio::time::interval`. |
| serde / serde_json | 1.0 | Callback POST body serialization, config deserialization | Already used everywhere. |
| url (new) | 2.5.x | URL parsing and validation for callback URLs (D-14) | Standard Rust URL parser. `Url::parse()` validates format without reachability check. |

### No New Dependencies Needed (except url)
reqwest is already in `Cargo.toml`. The `url` crate should be added for proper URL validation (reqwest depends on it internally, but explicit dependency ensures it is available for validation at submission time without bringing in reqwest types in non-HTTP modules).

**Installation:**
```bash
cargo add url
```

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| `url` crate | Manual regex URL validation | url crate is battle-tested, handles edge cases (IDN, percent-encoding). Regex is fragile. |
| `reqwest` for callbacks | `hyper` client directly | reqwest handles redirects, connection pooling, timeouts out of the box. hyper is lower-level with more boilerplate. reqwest already in deps. |
| `tokio::time::interval` for reaper | `tokio_cron_scheduler` | Overkill for a fixed 30s loop. `interval` is simpler and part of tokio. |

## Architecture Patterns

### Recommended Module Structure
```
gateway/src/
  reaper/
    mod.rs           # Background reaper: XPENDING scan + mark-failed
  callback/
    mod.rs           # Callback delivery: reqwest POST with retry
  config.rs          # Add CallbackConfig section
  queue/redis.rs     # Add reaper_scan_service() method
  auth/api_key.rs    # Add callback_url to ClientMetadata
  http/submit.rs     # Add callback_url to SubmitTaskRequest
  http/admin.rs      # Add PATCH api-keys endpoint, callback_url in create
  main.rs            # Spawn reaper + callback background tasks
  lib.rs             # Add pub mod reaper; pub mod callback;
```

### Pattern 1: Background Reaper Loop
**What:** A single `tokio::spawn` task that runs every 30 seconds, iterating all registered services and scanning XPENDING for timed-out entries.
**When to use:** Always -- this is the core of LIFE-03.
**Example:**
```rust
// Reaper background task
pub async fn run_reaper(state: Arc<AppState>) {
    let mut interval = tokio::time::interval(Duration::from_secs(30));
    loop {
        interval.tick().await;
        if let Err(e) = reap_timed_out_tasks(&state).await {
            tracing::error!(error=%e, "reaper cycle failed");
        }
    }
}

async fn reap_timed_out_tasks(state: &AppState) -> Result<(), GatewayError> {
    let services = list_services(&mut state.auth_conn.clone()).await?;
    for svc in &services {
        reap_service(state, svc).await?;
    }
    Ok(())
}
```

### Pattern 2: XPENDING Scan with IDLE Filter
**What:** Use `XPENDING <stream> workers IDLE <timeout_ms> - + <batch_size>` to find entries that have been pending longer than the service's `task_timeout_secs`.
**When to use:** Inside the reaper for each service.
**Critical detail:** XPENDING IDLE filter is in milliseconds. The service config stores `task_timeout_secs`, so multiply by 1000.
**Example:**
```rust
// XPENDING with IDLE filter -- returns entries idle longer than threshold
let timeout_ms = svc.task_timeout_secs * 1000;
let result: Vec<redis::streams::StreamPendingData> = redis::cmd("XPENDING")
    .arg(&stream_key)          // "tasks:{service_name}"
    .arg("workers")            // consumer group name
    .arg("IDLE")
    .arg(timeout_ms)
    .arg("-")                  // range start
    .arg("+")                  // range end
    .arg(100)                  // batch size
    .query_async(&mut conn)
    .await?;
// Each entry contains: stream_id, consumer_name, idle_time_ms, delivery_count
```

### Pattern 3: Reaper Mark-Failed Pipeline
**What:** For each timed-out entry found by XPENDING, extract task_id from the stream entry, then pipeline HSET state=failed + error_message + completed_at, and XACK.
**When to use:** After XPENDING returns timed-out entries.
**Critical detail:** The stream entry contains `task_id` as a field. The reaper must XRANGE to read the entry data, then use the task_id to update the hash. Alternatively, the XPENDING result gives us the stream_id, and we can look up the task hash by scanning. However, a simpler approach: XPENDING gives stream IDs, then for each ID do `XRANGE stream_key <id> <id>` to get the task_id field, then pipeline the state update.
**Example:**
```rust
// For each timed-out stream entry from XPENDING:
// 1. Read the stream entry to get task_id
let entries: redis::streams::StreamRangeReply = redis::cmd("XRANGE")
    .arg(&stream_key)
    .arg(&stream_id)
    .arg(&stream_id)
    .query_async(&mut conn)
    .await?;
// 2. Extract task_id, update hash, XACK
let error_msg = format!(
    "task timed out: node did not report result within {}s",
    svc.task_timeout_secs
);
redis::pipe()
    .cmd("HSET").arg(&hash_key)
        .arg("state").arg("failed")
        .arg("error_message").arg(&error_msg)
        .arg("completed_at").arg(&now)
        .ignore()
    .cmd("XACK").arg(&stream_key).arg("workers").arg(&stream_id)
        .ignore()
    .query_async(&mut conn)
    .await?;
```

### Pattern 4: Callback Delivery with Retry
**What:** When a task reaches terminal state, if a callback URL is configured, spawn an async task that POSTs `{task_id, state}` with exponential backoff retries.
**When to use:** After `report_result` completes and after reaper marks a task failed.
**Example:**
```rust
pub async fn deliver_callback(
    client: reqwest::Client,
    url: String,
    task_id: String,
    state: String,
    max_retries: u32,
    initial_delay_ms: u64,
) {
    let body = serde_json::json!({
        "task_id": task_id,
        "state": state,
    });

    for attempt in 0..=max_retries {
        if attempt > 0 {
            let delay = initial_delay_ms * 2u64.pow(attempt - 1);
            tokio::time::sleep(Duration::from_millis(delay)).await;
        }
        match client.post(&url).json(&body).send().await {
            Ok(resp) if resp.status().is_success() => {
                tracing::info!(task_id=%task_id, url=%url, "callback delivered");
                return;
            }
            Ok(resp) => {
                tracing::warn!(task_id=%task_id, url=%url, status=%resp.status(),
                    attempt=attempt+1, "callback delivery failed, retrying");
            }
            Err(e) => {
                tracing::warn!(task_id=%task_id, url=%url, error=%e,
                    attempt=attempt+1, "callback delivery error, retrying");
            }
        }
    }
    tracing::error!(task_id=%task_id, url=%url, "callback delivery exhausted all retries");
}
```

### Pattern 5: Callback URL Resolution
**What:** Determine the callback URL for a task by checking: (1) per-task `callback_url` stored in task hash, (2) fall back to per-key `callback_url` from `ClientMetadata`.
**Recommendation:** Store the resolved `callback_url` in the task hash at submission time. This avoids needing to look up the API key at delivery time (the key_hash is not stored in the task hash today, and adding it creates coupling). At submission, resolve: per-task override > per-key default > none. Store the final URL (or empty string) in the task hash.
**Why this is better than resolving at delivery time:**
- The task hash is self-contained -- reaper does not need to know about API keys
- If the API key's callback_url changes after submission, the task uses the URL that was active at submission time (more predictable)
- The `report_result` path already reads the task hash -- no additional Redis lookup needed

### Anti-Patterns to Avoid
- **Per-service reaper tasks:** D-03 specifies a single background task cycling through all services. Do not spawn one reaper per service.
- **Blocking the reaper on callback delivery:** The reaper should spawn callback delivery as a separate async task. Never block the reaper loop waiting for HTTP responses.
- **Using `try_transition` for reaper's Assigned->Failed:** The current `TaskState::try_transition` does not allow Assigned->Failed. The reaper should either: (a) add Assigned->Failed to the state machine, or (b) bypass `try_transition` and directly set the state like `report_result` does. Recommendation: add Assigned->Failed to `try_transition` since it is a legitimate transition when a node times out. This is a minor state machine update, not a "new state."
- **Creating a new reqwest::Client per callback:** Create one `reqwest::Client` at startup (stored in `AppState`) and clone it for each delivery. reqwest::Client uses connection pooling internally.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| URL validation | Custom regex or string parsing | `url::Url::parse()` | Handles IDN, percent-encoding, missing scheme, port validation. Edge cases are surprising. |
| HTTP client with retries | Manual hyper + retry loop | `reqwest::Client` + simple retry loop | reqwest handles connection pooling, redirects, timeouts. Retry logic is simple enough to hand-write (3 attempts). |
| Exponential backoff timing | Custom timer math | `initial_delay * 2^attempt` with `tokio::time::sleep` | Simple formula. No need for a backoff crate for 3 retries. |
| XPENDING parsing | Manual Redis response parsing | redis-rs StreamPendingData types | redis-rs has built-in stream types, though XPENDING extended form may need raw parsing (see pitfall below). |

## Common Pitfalls

### Pitfall 1: XPENDING IDLE Filter Returns Stream IDs, Not Task Data
**What goes wrong:** XPENDING returns (stream_id, consumer_name, idle_ms, delivery_count) -- NOT the task_id field from the stream entry. You cannot directly update the task hash from XPENDING output alone.
**Why it happens:** XPENDING is a metadata command about pending entries, not a data retrieval command.
**How to avoid:** After XPENDING identifies timed-out stream IDs, use `XRANGE <key> <id> <id>` for each (or batch) to retrieve the `task_id` field from the actual stream entry. Then use the task_id to update the task hash.
**Warning signs:** Task hashes not being updated despite XPENDING finding entries.

### Pitfall 2: redis-rs XPENDING Extended Form Parsing
**What goes wrong:** redis-rs `StreamPendingCountReply` / `StreamPendingData` types may not directly support the IDLE filter syntax. The raw Redis response for XPENDING extended form is an array of arrays.
**Why it happens:** redis-rs stream types are designed for common patterns; XPENDING with IDLE may need manual parsing.
**How to avoid:** Use `redis::cmd("XPENDING")...query_async::<Vec<(String, String, u64, u64)>>()` or parse from `redis::Value` manually. Test the exact deserialization format with a real Redis instance.
**Warning signs:** Deserialization errors or empty results despite pending entries.

### Pitfall 3: State Machine Mismatch -- Assigned to Failed
**What goes wrong:** The reaper needs to mark tasks as `failed` that are in `assigned` state (node claimed but never reported). The current `TaskState::try_transition` only allows `Running -> Failed`, not `Assigned -> Failed`.
**Why it happens:** Phase 1 state machine assumed only running tasks could fail. Timeout of assigned-but-never-started tasks was not considered.
**How to avoid:** Add `(TaskState::Assigned, TaskState::Failed)` to the valid transitions in `try_transition`. This is correct -- if a node claims a task but dies before reporting anything, the task should transition directly to failed.
**Warning signs:** Reaper failing with InvalidStateTransition errors.

### Pitfall 4: Callback URL Stored in Wrong Place
**What goes wrong:** If callback_url is only stored on the API key (ClientMetadata) and not in the task hash, the reaper has no way to find it -- the task hash does not store which API key submitted it.
**Why it happens:** The existing task hash has no `key_hash` or `callback_url` field.
**How to avoid:** Resolve the callback URL at submission time and store it in the task hash. Both `report_result` and the reaper can then read it from the same place.
**Warning signs:** Callbacks working for `report_result` path but not for reaper-detected timeouts.

### Pitfall 5: Reaper Scanning Deleted or Empty Streams
**What goes wrong:** If a service is deregistered while the reaper is iterating, the stream may not exist. XPENDING on a non-existent stream or non-existent consumer group returns an error.
**Why it happens:** Race condition between service cleanup and reaper scan.
**How to avoid:** Catch and log Redis errors per-service in the reaper loop. Do not let one service's error abort the entire reaper cycle. Use `continue` on error.
**Warning signs:** Reaper cycle stopping partway through services.

### Pitfall 6: reqwest Client Without Timeout
**What goes wrong:** Callback POST to an unresponsive endpoint hangs indefinitely, consuming a spawned task forever.
**Why it happens:** reqwest default has no request timeout.
**How to avoid:** Configure `reqwest::Client::builder().timeout(Duration::from_secs(10))` at creation time. Also set `.connect_timeout(Duration::from_secs(5))`.
**Warning signs:** Growing number of spawned tasks that never complete.

## Code Examples

### CallbackConfig for GatewayConfig
```rust
// Source: Design based on D-18 locked decisions
#[derive(Debug, Deserialize, Clone)]
pub struct CallbackConfig {
    #[serde(default = "default_callback_max_retries")]
    pub max_retries: u32,
    #[serde(default = "default_callback_initial_delay_ms")]
    pub initial_delay_ms: u64,
    #[serde(default = "default_callback_timeout_secs")]
    pub timeout_secs: u64,
}

impl Default for CallbackConfig {
    fn default() -> Self {
        Self {
            max_retries: default_callback_max_retries(),
            initial_delay_ms: default_callback_initial_delay_ms(),
            timeout_secs: default_callback_timeout_secs(),
        }
    }
}

fn default_callback_max_retries() -> u32 { 3 }
fn default_callback_initial_delay_ms() -> u64 { 1000 }
fn default_callback_timeout_secs() -> u64 { 10 }
```

### ClientMetadata with callback_url
```rust
// Source: Extending existing auth/api_key.rs
#[derive(Debug, Clone, serde::Serialize)]
pub struct ClientMetadata {
    pub key_hash: String,
    pub service_names: Vec<String>,
    pub created_at: String,
    pub callback_url: Option<String>,  // NEW: per-key default callback URL
}
```

### SubmitTaskRequest with callback_url
```rust
// Source: Extending existing http/submit.rs
#[derive(Debug, Deserialize)]
pub struct SubmitTaskRequest {
    pub service_name: String,
    pub payload: String,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    pub callback_url: Option<String>,  // NEW: per-task override
}
```

### XPENDING Scan in Reaper
```rust
// Source: Redis XPENDING docs (https://redis.io/docs/latest/commands/xpending/)
async fn reap_service(state: &AppState, svc: &ServiceConfig) -> Result<(), GatewayError> {
    let stream_key = format!("tasks:{}", svc.name);
    let timeout_ms = svc.task_timeout_secs * 1000;
    let mut conn = state.queue.conn().clone();

    // Scan for entries idle longer than task_timeout_secs
    // Returns: Vec of (stream_id, consumer_name, idle_ms, delivery_count)
    let pending: Vec<redis::Value> = redis::cmd("XPENDING")
        .arg(&stream_key)
        .arg("workers")
        .arg("IDLE")
        .arg(timeout_ms)
        .arg("-")
        .arg("+")
        .arg(100_u64)  // batch size
        .query_async(&mut conn)
        .await
        .map_err(GatewayError::Redis)?;

    // Parse and process each timed-out entry...
    // For each: XRANGE to get task_id, then HSET failed + XACK
}
```

### reqwest Client Setup
```rust
// Source: reqwest docs (https://docs.rs/reqwest/latest/reqwest/struct.Client.html)
// Create once at startup, store in AppState
let http_client = reqwest::Client::builder()
    .timeout(Duration::from_secs(config.callback.timeout_secs))
    .connect_timeout(Duration::from_secs(5))
    .pool_max_idle_per_host(10)
    .pool_idle_timeout(Duration::from_secs(90))
    .build()
    .expect("Failed to build HTTP client");
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| redis-rs < 1.0 custom parsing | redis-rs 1.0 stream types | 2024 | StreamPendingData, StreamRangeReply available but XPENDING extended form with IDLE may still need manual parsing |
| reqwest 0.11 | reqwest 0.12 (hyper 1.x) | 2024 | Same API, but built on hyper 1.x for better connection handling |

## Open Questions

1. **XPENDING Pagination for Large Backlogs**
   - What we know: D-01 uses batch size (e.g., 100). XPENDING supports exclusive range start for pagination (`(last_id`).
   - What's unclear: Should the reaper paginate through ALL pending entries in a single cycle, or process one batch per cycle? For v1 with no retries, a single batch of 100 is likely sufficient.
   - Recommendation: Start with a single batch of 100 per service per cycle. If services accumulate more than 100 timed-out entries between 30s cycles, something is very wrong and logging a warning is appropriate. Add pagination in v2 if needed.

2. **Failed Task Counter (D-10)**
   - What we know: Admin needs failed task counts per service. Options: (a) Redis INCR counter `failed_count:{service}` incremented by reaper/report_result, (b) computed from scanning task hashes.
   - Recommendation: Use a Redis INCR counter. It is O(1) and trivially added to the reaper and report_result pipelines. Scanning task hashes would be O(N) and impractical. The counter can be reset on service deregistration.

3. **Graceful Shutdown of Reaper**
   - What we know: The reaper runs in a `tokio::spawn` loop. Current main.rs uses `join_all` on server handles. Reaper should stop when the gateway shuts down.
   - Recommendation: Use `tokio_util::sync::CancellationToken` or a simple `tokio::sync::watch` channel. The reaper checks the cancellation signal on each loop iteration. This is a pattern decision for Claude's discretion.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in (`cargo test`) + Redis integration tests |
| Config file | None (standard `cargo test`) |
| Quick run command | `cargo test -p xgent-gateway -- --lib` |
| Full suite command | `cargo test -p xgent-gateway -- --include-ignored` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| LIFE-03 | Reaper detects timed-out tasks via XPENDING and marks them failed | integration (needs Redis) | `cargo test -p xgent-gateway reaper -- --ignored` | No -- Wave 0 |
| LIFE-03 | Reaper reads `task_timeout_secs` per service | unit | `cargo test -p xgent-gateway reaper` | No -- Wave 0 |
| RSLT-03 | Callback URL stored in task hash at submission | integration (needs Redis) | `cargo test -p xgent-gateway callback -- --ignored` | No -- Wave 0 |
| RSLT-03 | URL validation rejects malformed URLs | unit | `cargo test -p xgent-gateway callback` | No -- Wave 0 |
| RSLT-03 | Per-key default with per-task override resolution | unit | `cargo test -p xgent-gateway callback` | No -- Wave 0 |
| RSLT-04 | Callback delivery POSTs to URL on terminal state | integration (needs HTTP server mock) | `cargo test -p xgent-gateway callback -- --ignored` | No -- Wave 0 |
| RSLT-04 | Exponential backoff retries on callback failure | unit (mock client) | `cargo test -p xgent-gateway callback` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway -- --lib`
- **Per wave merge:** `cargo test -p xgent-gateway -- --include-ignored`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `gateway/src/reaper/mod.rs` -- new module with unit tests for timeout detection logic
- [ ] `gateway/src/callback/mod.rs` -- new module with unit tests for URL resolution and retry logic
- [ ] Integration tests in respective modules (gated with `#[ignore]`) for Redis and HTTP interactions

## Sources

### Primary (HIGH confidence)
- [Redis XPENDING docs](https://redis.io/docs/latest/commands/xpending/) -- IDLE filter syntax, return format, time complexity
- [reqwest Client docs](https://docs.rs/reqwest/latest/reqwest/struct.Client.html) -- builder API, timeout, connection pooling
- Existing codebase: `gateway/src/queue/redis.rs`, `gateway/src/auth/api_key.rs`, `gateway/src/config.rs`, `gateway/src/main.rs` -- established patterns

### Secondary (MEDIUM confidence)
- [reqwest HTTP client guide](https://oneuptime.com/blog/post/2026-01-26-rust-reqwest-http-client/view) -- connection pooling best practices
- [redis-rs streams support](https://crates.io/crates/redis) -- stream types available in redis-rs 1.0

### Tertiary (LOW confidence)
- redis-rs XPENDING extended form parsing -- needs validation with actual redis-rs 1.0 deserialization against a running Redis instance. The exact Rust types for XPENDING with IDLE filter are uncertain.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- no new dependencies except `url` crate; reqwest already in Cargo.toml; XPENDING is well-documented Redis command
- Architecture: HIGH -- patterns directly extend existing codebase (tokio::spawn, redis::pipe, Arc<AppState>)
- Pitfalls: HIGH -- identified through code analysis of existing state machine, task hash structure, and Redis Streams behavior

**Research date:** 2026-03-22
**Valid until:** 2026-04-22 (stable domain, no fast-moving dependencies)
