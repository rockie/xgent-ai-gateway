# Phase 15: Async-API Execution - Research

**Researched:** 2026-03-24
**Domain:** Rust async HTTP polling, config refactoring, executor trait implementation
**Confidence:** HIGH

## Summary

Phase 15 implements the `AsyncApiExecutor` -- a two-phase HTTP executor that submits a job and polls for completion. It also refactors the shared `ResponseSection` to support separate success/failed body templates with optional headers, a cross-cutting change affecting all three execution modes.

The codebase already has mature patterns from Phase 13 (CLI executor, placeholder engine, config loading) and Phase 14 (sync-api HTTP dispatch, JSON extraction, reqwest client, connection retry). Phase 15 follows these established patterns closely. The new code is primarily: (1) config structs for `async_api` section with `submit`/`poll`/`completed_when`/`failed_when` sub-sections, (2) a poll loop wrapped in `tokio::time::timeout`, (3) condition evaluation using the existing `extract_json_value()`, and (4) the response section restructure.

**Primary recommendation:** Follow the SyncApiExecutor as a structural template. The `send_request()` retry pattern, placeholder resolution flow, and response extraction logic are directly reusable. Move `extract_json_value()` and a generalized `find_prefixed_placeholders()` to a shared utility module since both sync-api and async-api executors need them.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Nested `submit` and `poll` sub-sections within `async_api`. Each sub-section mirrors sync-api's flat field pattern: `url`, `method`, `headers`, `body`. The `poll` sub-section adds `interval_secs`.
- **D-02:** Shared fields at the `async_api` level: `timeout_secs` (default 300s) and `tls_skip_verify` (default false). These apply to both submit and poll requests.
- **D-03:** `submit.method` defaults to `POST`. `poll.method` defaults to `GET`. Same default mechanism as sync-api.
- **D-04:** `completed_when` and `failed_when` are condition blocks at the `async_api` level, each with `path`, `operator`, and `value` fields.
- **D-05:** Validation at config load: if `mode` is `async-api`, `async_api` section must be present with `submit`, `poll`, and `completed_when` sub-sections. `failed_when` is optional.
- **D-06:** Fixed interval polling. `poll.interval_secs` with default 5 seconds. No exponential backoff.
- **D-07:** Default `timeout_secs` is 300 (5 minutes), consistent with CLI mode's default. Covers the total duration of submit + all poll iterations.
- **D-08:** `tokio::time::timeout` wraps the entire submit + poll loop. If timeout fires during a mid-flight poll HTTP request, that request gets cancelled. Timeout means exactly `timeout_secs`, not a soft limit.
- **D-09:** `completed_when` is required. `failed_when` is optional. If `failed_when` is omitted, the only failure paths are timeout and HTTP errors.
- **D-10:** Evaluation order each poll: check `completed_when` first -- if match, succeed and extract result. Then check `failed_when` -- if match, fail task immediately. If neither matches, sleep `interval_secs` and poll again.
- **D-11:** Supported operators: `equal`, `not_equal`, `in`, `not_in`. The `in` and `not_in` operators take an array value (e.g., `value: ["failed", "error", "cancelled"]`). The `equal` and `not_equal` operators take a single string value.
- **D-12:** Condition `path` uses the same dot-notation key-path extraction as sync-api (`extract_json_value()`). Values are compared as strings after extraction.
- **D-13:** Submit request failures: one connection retry (same as sync-api D-13, `is_connect()` check only). If retry also fails, task fails immediately with descriptive error.
- **D-14:** Poll request failures: one connection retry per failed poll attempt. If retry also fails, task fails immediately. Does not silently continue polling.
- **D-15:** Non-2xx HTTP status on submit or poll fails the task with status code and response body in the error message (consistent with sync-api D-12).
- **D-16:** Non-JSON poll responses fail the task with a clear error. The condition check requires JSON parsing to extract the key-path value.
- **D-17:** The shared `response` config section is restructured with `success` and `failed` sub-sections, each with its own `body` template and optional `header` field. `max_bytes` stays at the `response` level.
- **D-18:** `response.success.body` -- template applied on success path (replaces current `response.body`). `response.failed.body` -- template applied on failure path. If `failed.body` is omitted, failure results remain empty (backwards-compatible with current behavior).
- **D-19:** `response.success.header` and `response.failed.header` -- optional JSON string of extra headers to attach as result metadata, e.g., `'{"Content-Type": "application/json"}'`. These are passed through in the `ExecutionResult` for clients that need to know the result format.
- **D-20:** On CLI failure (non-zero exit code), `<stdout>`, `<stderr>`, and a new `<exit_code>` placeholder are available in `failed.body`. On sync-api/async-api failure (non-2xx or `failed_when`), `<response.path>` placeholders extract from the error/poll response.
- **D-21:** `ExecutionResult` struct gains a `headers: HashMap<String, String>` field (default empty) to carry the parsed header metadata back to the gateway. Existing success=true/false, result, error_message fields unchanged.
- **D-22:** Existing CLI and sync-api executors are updated to use the new response section structure. This is a refactor -- their behavior changes only in that failures now produce structured results instead of empty `Vec::new()`.
- **D-23:** `<poll_response.path>` placeholders in `response.success.body` extract values from the final successful poll response JSON. Uses the same `extract_json_value()` and `find_response_placeholders()` functions from sync-api.
- **D-24:** `<submit_response.path>` placeholders are used in the poll URL and body templates to inject values extracted from the submit response (e.g., job ID).
- **D-25:** When `failed_when` matches, `<poll_response.path>` placeholders in `response.failed.body` extract values from the failing poll response (e.g., error message, error code).

### Claude's Discretion
- Exact `AsyncApiSection`, `SubmitSection`, `PollSection`, `CompletionCondition` struct names and serde attributes
- How the condition operator enum is implemented (serde rename_all kebab-case or lowercase)
- Internal structure of the poll loop (while loop vs loop with break)
- Test strategy and fixture structure
- Error message exact formatting
- Whether `submit_response.*` extraction reuses `find_response_placeholders()` with a different prefix or uses a separate scan
- Exact naming of the new `ResponseSection` sub-structs (`SuccessResponse`, `FailedResponse`, etc.)
- How header JSON string is parsed and stored (parse at config load vs pass-through)
- Whether `response.success` is required or falls back to legacy `response.body` for backwards compatibility during migration

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope.
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| AAPI-01 | Submit phase sends HTTP request and extracts values from response via key-path | Reuse SyncApiExecutor's send_request() retry pattern and extract_json_value(); store extracted values as `submit_response.*` in variables map |
| AAPI-02 | Poll phase sends HTTP request at configurable interval with submit response values in URL/body | `tokio::time::sleep(Duration::from_secs(interval_secs))` between polls; resolve `<submit_response.*>` placeholders in poll URL/body via existing placeholder engine |
| AAPI-03 | Completion condition checks key-path value with operators (equal, not_equal, in, not_in) | CompletionCondition struct with ConditionOperator enum; extract_json_value() for path, string comparison for operators |
| AAPI-04 | Failed_when condition short-circuits polling on detected failure state | Same condition evaluation as completed_when; checked second per D-10 evaluation order |
| AAPI-05 | Configurable timeout caps total submit + poll duration | `tokio::time::timeout(Duration::from_secs(timeout_secs), async { submit + poll_loop })` wrapping entire flow |
| AAPI-06 | Response body template maps poll response values into result shape | Generalized find_prefixed_placeholders() scanning for `poll_response.` prefix; extract from final poll JSON into variables map |
</phase_requirements>

## Standard Stack

### Core (already in Cargo.toml)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.12.x | HTTP client for submit/poll requests | Already used by SyncApiExecutor. Same client builder pattern. |
| tokio | 1.50+ | Async runtime, `time::timeout`, `time::sleep` | Project runtime. timeout wraps entire submit+poll. sleep between polls. |
| serde / serde_yaml_ng | 1.0 / 0.10 | Config deserialization | Existing config parsing. New structs derive Deserialize. |
| serde_json | 1.0 | Poll response JSON parsing | Already used for extract_json_value(). |
| async-trait | 0.1 | Executor trait impl | Existing pattern for dyn Executor dispatch. |
| tracing | 0.1 | Structured logging | Existing pattern. Log submit, each poll iteration, completion/failure. |

### No New Dependencies
This phase requires zero new crate additions. Everything needed is already in `gateway/Cargo.toml`.

## Architecture Patterns

### Recommended Module Structure
```
gateway/src/agent/
  mod.rs                    # Add: pub mod async_api_executor; pub mod http_common;
  config.rs                 # Add: AsyncApiSection, SubmitSection, PollSection,
                            #       CompletionCondition, ConditionOperator,
                            #       Refactor: ResponseSection -> success/failed sub-sections
  executor.rs               # Modify: ExecutionResult gains headers field
  http_common.rs            # NEW: extract_json_value(), find_prefixed_placeholders(),
                            #       send_with_retry() shared between sync/async executors
  async_api_executor.rs     # NEW: AsyncApiExecutor implementing Executor trait
  sync_api_executor.rs      # Refactor: use http_common, use new ResponseSection
  cli_executor.rs           # Refactor: use new ResponseSection, add failed.body support
  response.rs               # Refactor: resolve_response_body updated for new structure
  placeholder.rs            # Unchanged
```

### Pattern 1: Shared HTTP Utilities (http_common.rs)

**What:** Move `extract_json_value()` and a generalized placeholder scanner out of `sync_api_executor.rs` into a shared module.

**When to use:** Both sync-api and async-api executors need JSON path extraction and response placeholder scanning.

**Example:**
```rust
// gateway/src/agent/http_common.rs

/// Extract a value from a JSON object using dot-notation path.
/// (Moved from sync_api_executor.rs -- identical implementation)
pub fn extract_json_value(root: &serde_json::Value, path: &str) -> Result<String, String> {
    // ... existing implementation ...
}

/// Scan a template string for `<{prefix}.XXX>` placeholders and return the paths.
/// Generalizes the old find_response_placeholders() to support any prefix:
/// "response", "poll_response", "submit_response".
pub fn find_prefixed_placeholders(template: &str, prefix: &str) -> Vec<String> {
    let prefix_dot = format!("{}.", prefix);
    let mut paths = Vec::new();
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            let mut token = String::new();
            let mut found_close = false;
            for c2 in chars.by_ref() {
                if c2 == '>' { found_close = true; break; }
                token.push(c2);
            }
            if found_close {
                if let Some(rest) = token.strip_prefix(&prefix_dot) {
                    paths.push(rest.to_string());
                }
            }
        }
    }
    paths
}
```

### Pattern 2: AsyncApiExecutor Poll Loop

**What:** The core submit-then-poll pattern wrapped in tokio::time::timeout.

**Example:**
```rust
// Simplified structure -- Claude's discretion on exact implementation
pub struct AsyncApiExecutor {
    service_name: String,
    async_api: AsyncApiSection,
    response: ResponseSection, // New restructured version
    client: reqwest::Client,
}

#[async_trait]
impl Executor for AsyncApiExecutor {
    async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult {
        let timeout_dur = Duration::from_secs(self.async_api.timeout_secs);

        match tokio::time::timeout(timeout_dur, self.run_submit_poll(assignment)).await {
            Ok(result) => result,
            Err(_) => ExecutionResult {
                success: false,
                result: Vec::new(),
                error_message: format!(
                    "async-api timed out after {}s",
                    self.async_api.timeout_secs
                ),
                headers: HashMap::new(),
            },
        }
    }
}

impl AsyncApiExecutor {
    async fn run_submit_poll(&self, assignment: &TaskAssignment) -> ExecutionResult {
        // 1. Build variables, resolve submit URL/body/headers
        // 2. Send submit request (with retry)
        // 3. Parse submit response JSON
        // 4. Extract submit_response.* values into variables
        // 5. Loop:
        //    a. Sleep interval_secs
        //    b. Resolve poll URL/body with submit_response values
        //    c. Send poll request (with retry)
        //    d. Parse poll response JSON
        //    e. Check completed_when -> if match, extract result, return success
        //    f. Check failed_when -> if match, return failure
        //    g. Continue loop
        todo!()
    }
}
```

### Pattern 3: Condition Evaluation

**What:** Check a JSON path value against an operator and expected value(s).

**Example:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct CompletionCondition {
    pub path: String,
    pub operator: ConditionOperator,
    pub value: ConditionValue,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConditionOperator {
    Equal,
    NotEqual,
    In,
    NotIn,
}

/// Value can be a single string or an array of strings.
/// serde untagged handles both YAML forms.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ConditionValue {
    Single(String),
    Multiple(Vec<String>),
}

impl CompletionCondition {
    pub fn evaluate(&self, json: &serde_json::Value) -> Result<bool, String> {
        let actual = extract_json_value(json, &self.path)?;
        match (&self.operator, &self.value) {
            (ConditionOperator::Equal, ConditionValue::Single(expected)) => {
                Ok(&actual == expected)
            }
            (ConditionOperator::NotEqual, ConditionValue::Single(expected)) => {
                Ok(&actual != expected)
            }
            (ConditionOperator::In, ConditionValue::Multiple(values)) => {
                Ok(values.iter().any(|v| v == &actual))
            }
            (ConditionOperator::NotIn, ConditionValue::Multiple(values)) => {
                Ok(!values.iter().any(|v| v == &actual))
            }
            _ => Err(format!(
                "operator {:?} requires {} value",
                self.operator,
                match self.operator {
                    ConditionOperator::In | ConditionOperator::NotIn => "an array",
                    _ => "a single string",
                }
            )),
        }
    }
}
```

### Pattern 4: ResponseSection Restructure

**What:** The shared response config changes from flat `{ body, max_bytes }` to `{ success: { body, header }, failed: { body, header }, max_bytes }`.

**Example:**
```rust
#[derive(Debug, Clone, Deserialize)]
pub struct ResponseSection {
    pub success: SuccessResponseConfig,
    #[serde(default)]
    pub failed: Option<FailedResponseConfig>,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SuccessResponseConfig {
    pub body: String,
    #[serde(default)]
    pub header: Option<String>, // JSON string parsed at use time
}

#[derive(Debug, Clone, Deserialize)]
pub struct FailedResponseConfig {
    pub body: String,
    #[serde(default)]
    pub header: Option<String>,
}
```

### Anti-Patterns to Avoid
- **Polling without timeout wrapper:** Never let the poll loop run unbounded. Always wrap in `tokio::time::timeout`.
- **Sleeping before first poll:** The submit response itself might already indicate completion. However, per the CONTEXT decisions, the flow is submit -> extract -> loop(sleep -> poll -> check). The sleep comes before each poll, not after.
- **Sharing reqwest::Client between submit and poll with different timeouts:** Use per-request timeout override if submit and poll need different request-level timeouts, or use the single client with no per-request timeout since the overall `tokio::time::timeout` handles the total duration.
- **Silently swallowing poll errors:** Per D-14, poll HTTP failures immediately fail the task after one retry. Do not continue polling on error.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP request + retry | Custom HTTP client | Reuse SyncApiExecutor's `send_request()` pattern (extract to shared) | Tested, handles timeout vs connect distinction |
| JSON path extraction | Custom JSON walker | Existing `extract_json_value()` | Already handles nested objects, arrays, type coercion |
| Placeholder resolution | Custom template engine | Existing `resolve_placeholders()` | Single-pass, injection-safe, tested |
| Timeout enforcement | Manual timer tracking | `tokio::time::timeout` | Cancellation-safe, handles mid-flight request cancellation |
| YAML array deserialization for `in`/`not_in` | Custom parser | `serde(untagged)` enum with `String` and `Vec<String>` variants | serde handles both YAML scalar and sequence forms |

## Common Pitfalls

### Pitfall 1: Submit Response Values Not Available in Poll Templates
**What goes wrong:** Poll URL/body templates contain `<submit_response.job_id>` but the submit response values were never extracted into the variables map.
**Why it happens:** Forgetting to scan the poll URL and poll body templates for `submit_response.*` prefixed placeholders and populate them from the submit response JSON.
**How to avoid:** After parsing the submit response JSON, scan BOTH `poll.url` and `poll.body` templates for `<submit_response.*>` placeholders, extract each path from the submit JSON, and insert into the variables map before resolving poll templates.
**Warning signs:** "unresolved placeholder" errors mentioning `submit_response.*`.

### Pitfall 2: ResponseSection Backwards Compatibility
**What goes wrong:** Existing CLI and sync-api YAML configs break because `response.body` moved to `response.success.body`.
**Why it happens:** The restructure changes the config schema. All existing test YAML fixtures and any real config files must be updated.
**How to avoid:** Update ALL test fixtures in config.rs, cli_executor.rs, sync_api_executor.rs, and response.rs simultaneously. Consider whether to support legacy `response.body` as fallback (Claude's discretion area).
**Warning signs:** Deserialization errors in existing tests after the restructure.

### Pitfall 3: Timeout Fires During HTTP Request
**What goes wrong:** `tokio::time::timeout` cancels a mid-flight reqwest request. The future is dropped, reqwest cleans up the connection.
**Why it happens:** This is expected behavior per D-08. The concern is that error handling doesn't distinguish timeout from other errors.
**How to avoid:** The outer timeout handler produces a clear "timed out after Xs" message. Inner send_request errors should NOT mention timeout since the outer timeout is the authority.
**Warning signs:** Confusing error messages mixing inner request timeout with outer poll timeout.

### Pitfall 4: ConditionValue Serde Untagged Ambiguity
**What goes wrong:** `serde(untagged)` with `Single(String)` and `Multiple(Vec<String>)` -- a YAML single string could match either variant.
**Why it happens:** serde untagged tries variants in order. `["foo"]` (single-element array) should match `Multiple`, not `Single`.
**How to avoid:** Order the enum variants correctly: `Multiple(Vec<String>)` first, then `Single(String)`. serde untagged tries in declaration order. Actually, a single string like `"foo"` would try to deserialize as `Vec<String>` first and fail, then succeed as `String`. So `Single` first, `Multiple` second is correct.
**Warning signs:** Test with both `value: "completed"` and `value: ["failed", "error"]` to verify deserialization.

### Pitfall 5: reqwest Client Timeout vs tokio Timeout Interaction
**What goes wrong:** If the reqwest client is built with a per-request timeout (from sync-api pattern), it might fire before the outer tokio timeout, producing confusing errors.
**Why it happens:** SyncApiExecutor sets `Client::builder().timeout()` from its config. AsyncApiExecutor should NOT set a per-request timeout on the client because the outer `tokio::time::timeout` governs total duration.
**How to avoid:** Build the reqwest client WITHOUT a per-request timeout for async-api mode. The outer tokio timeout handles cancellation. Individual poll requests don't need their own timeout since a hung request will be cancelled by the outer timeout.
**Warning signs:** "request timed out" errors when the outer timeout hasn't expired yet.

### Pitfall 6: ExecutionResult headers Field Breaks Existing Code
**What goes wrong:** Adding `headers: HashMap<String, String>` to `ExecutionResult` requires updating every place that constructs an `ExecutionResult`.
**Why it happens:** Rust struct construction requires all fields. No default.
**How to avoid:** Add `headers` field, then update all construction sites in cli_executor.rs, sync_api_executor.rs, the new async_api_executor.rs, and executor.rs tests. Use `HashMap::new()` for all existing paths initially.
**Warning signs:** Compilation errors across multiple files.

## Code Examples

### Verified: tokio::time::timeout wrapping async block
```rust
// Source: tokio docs, verified pattern from existing codebase (cli_executor.rs line 135)
use tokio::time::{timeout, Duration};

let result = timeout(
    Duration::from_secs(300),
    async {
        // submit + poll loop here
        // If timeout fires, this entire future is cancelled
        Ok::<ExecutionResult, ExecutionResult>(exec_result)
    }
).await;

match result {
    Ok(Ok(exec_result)) => exec_result,      // Completed within timeout
    Ok(Err(exec_result)) => exec_result,      // Failed within timeout
    Err(_elapsed) => ExecutionResult { ... },  // Timeout expired
}
```

### Verified: tokio::time::sleep for poll interval
```rust
// Source: tokio docs
use tokio::time::{sleep, Duration};

loop {
    sleep(Duration::from_secs(self.async_api.poll.interval_secs)).await;
    // send poll request...
}
```

### Verified: serde untagged enum for condition value
```rust
// Source: serde docs, standard pattern
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ConditionValue {
    Single(String),
    Multiple(Vec<String>),
}

// YAML: value: "completed"     -> Single("completed")
// YAML: value: ["a", "b"]     -> Multiple(["a", "b"])
```

### Verified: Header JSON string parsing
```rust
// Source: serde_json standard pattern
fn parse_header_string(header_json: &str) -> Result<HashMap<String, String>, String> {
    serde_json::from_str::<HashMap<String, String>>(header_json)
        .map_err(|e| format!("invalid header JSON '{}': {}", header_json, e))
}
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in Rust test framework) |
| Config file | None -- uses `#[cfg(test)] mod tests` inline |
| Quick run command | `cargo test -p xgent-gateway --lib agent::async_api_executor -- --nocapture` |
| Full suite command | `cargo test -p xgent-gateway` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| AAPI-01 | Submit sends HTTP, extracts values | integration | `cargo test -p xgent-gateway --lib agent::async_api_executor::tests::submit_extracts_job_id -x` | Wave 0 |
| AAPI-02 | Poll at interval with submit values in URL | integration | `cargo test -p xgent-gateway --lib agent::async_api_executor::tests::poll_uses_submit_values -x` | Wave 0 |
| AAPI-03 | Completion condition operators | unit | `cargo test -p xgent-gateway --lib agent::async_api_executor::tests::condition_operators -x` | Wave 0 |
| AAPI-04 | Failed_when short-circuits | integration | `cargo test -p xgent-gateway --lib agent::async_api_executor::tests::failed_when_shortcircuits -x` | Wave 0 |
| AAPI-05 | Timeout caps total duration | integration | `cargo test -p xgent-gateway --lib agent::async_api_executor::tests::timeout_cancels_polling -x` | Wave 0 |
| AAPI-06 | Response template maps poll values | integration | `cargo test -p xgent-gateway --lib agent::async_api_executor::tests::response_maps_poll_values -x` | Wave 0 |
| D-17/22 | ResponseSection restructure, existing tests pass | unit+integration | `cargo test -p xgent-gateway --lib agent -x` | Existing (must update) |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway --lib agent`
- **Per wave merge:** `cargo test -p xgent-gateway`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `gateway/src/agent/async_api_executor.rs` -- new file with all async-api tests
- [ ] `gateway/src/agent/http_common.rs` -- shared utilities with moved tests from sync_api_executor
- [ ] Update all existing test YAML fixtures in config.rs, cli_executor.rs, sync_api_executor.rs, response.rs for new ResponseSection structure

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Flat `response: { body, max_bytes }` | Nested `response: { success: { body, header }, failed: { body, header }, max_bytes }` | This phase | All existing YAML fixtures and config files need updating |
| `ExecutionResult { success, result, error_message }` | Gains `headers: HashMap<String, String>` | This phase | All ExecutionResult construction sites need updating |
| `find_response_placeholders()` in sync_api_executor | `find_prefixed_placeholders(prefix)` in http_common | This phase | Generalized for response, poll_response, submit_response prefixes |
| Failure returns empty `Vec::new()` result | Failure resolves `failed.body` template | This phase | CLI and sync-api failures now produce structured results |

## Open Questions

1. **Legacy response.body fallback**
   - What we know: D-17/D-18 specify the new structure. Claude's discretion includes whether to support legacy format.
   - What's unclear: Whether there are real config files in production that would break.
   - Recommendation: Since this is pre-v1.2 and no production deployments yet, do a clean break. No legacy fallback. Update all fixtures. Simpler code.

2. **reqwest Client timeout for async-api**
   - What we know: SyncApiExecutor sets per-request timeout via `Client::builder().timeout()`. For async-api, the outer `tokio::time::timeout` governs total duration.
   - What's unclear: Should individual poll requests have their own timeout to prevent a single hung request from consuming the entire timeout budget?
   - Recommendation: Do NOT set per-request timeout on the reqwest client. The outer tokio timeout handles cancellation. A hung request gets cancelled when the outer timeout fires. This is simpler and matches D-08's intent. If needed later, add a per-poll-request timeout as a config option.

3. **send_with_retry reuse between executors**
   - What we know: SyncApiExecutor has `send_request()` with retry logic baked in as a method.
   - What's unclear: Whether to extract it as a free function in http_common or keep it as a method.
   - Recommendation: Extract to http_common as a free function taking `&reqwest::Client` and request parameters. Both executors call it. This reduces duplication without changing behavior.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `gateway/src/agent/sync_api_executor.rs` -- verified HTTP dispatch, retry, JSON extraction patterns
- Existing codebase: `gateway/src/agent/config.rs` -- verified config deserialization, validation, default patterns
- Existing codebase: `gateway/src/agent/executor.rs` -- verified Executor trait and ExecutionResult struct
- Existing codebase: `gateway/src/agent/placeholder.rs` -- verified placeholder resolution engine
- Existing codebase: `gateway/src/agent/response.rs` -- verified response body resolution
- Existing codebase: `gateway/src/bin/agent.rs` -- verified agent binary with AsyncApi match arm stub
- tokio docs: `tokio::time::timeout` and `tokio::time::sleep` -- standard async patterns

### Secondary (MEDIUM confidence)
- serde docs: `#[serde(untagged)]` enum deserialization for ConditionValue -- standard pattern, well-documented

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- zero new dependencies, all patterns proven in Phase 13/14
- Architecture: HIGH -- direct extension of existing executor pattern, all code reviewed
- Pitfalls: HIGH -- derived from actual code review, not theoretical concerns

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable -- no external dependency changes)
