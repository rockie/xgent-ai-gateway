# Phase 15: Async-API Execution - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Agent handles two-phase async APIs: submit a job via HTTP, poll for completion at a configurable interval, detect success/failure via key-path conditions, and extract the final result into a response body template. Implements `AsyncApiExecutor` following the `Executor` trait pattern from Phase 13. Also refactors the shared `response` config section to support separate success/failed body templates with optional headers — a cross-cutting improvement that applies to all three execution modes. Examples and end-to-end validation are Phase 16.

</domain>

<decisions>
## Implementation Decisions

### Config layout
- **D-01:** Nested `submit` and `poll` sub-sections within `async_api`. Each sub-section mirrors sync-api's flat field pattern: `url`, `method`, `headers`, `body`. The `poll` sub-section adds `interval_secs`.
- **D-02:** Shared fields at the `async_api` level: `timeout_secs` (default 300s) and `tls_skip_verify` (default false). These apply to both submit and poll requests.
- **D-03:** `submit.method` defaults to `POST`. `poll.method` defaults to `GET`. Same default mechanism as sync-api.
- **D-04:** `completed_when` and `failed_when` are condition blocks at the `async_api` level, each with `path`, `operator`, and `value` fields.
- **D-05:** Validation at config load: if `mode` is `async-api`, `async_api` section must be present with `submit`, `poll`, and `completed_when` sub-sections. `failed_when` is optional.

### Polling strategy
- **D-06:** Fixed interval polling. `poll.interval_secs` with default 5 seconds. No exponential backoff.
- **D-07:** Default `timeout_secs` is 300 (5 minutes), consistent with CLI mode's default. Covers the total duration of submit + all poll iterations.
- **D-08:** `tokio::time::timeout` wraps the entire submit + poll loop. If timeout fires during a mid-flight poll HTTP request, that request gets cancelled. Timeout means exactly `timeout_secs`, not a soft limit.

### Completion and failure conditions
- **D-09:** `completed_when` is required. `failed_when` is optional. If `failed_when` is omitted, the only failure paths are timeout and HTTP errors.
- **D-10:** Evaluation order each poll: check `completed_when` first — if match, succeed and extract result. Then check `failed_when` — if match, fail task immediately. If neither matches, sleep `interval_secs` and poll again.
- **D-11:** Supported operators: `equal`, `not_equal`, `in`, `not_in`. The `in` and `not_in` operators take an array value (e.g., `value: ["failed", "error", "cancelled"]`). The `equal` and `not_equal` operators take a single string value.
- **D-12:** Condition `path` uses the same dot-notation key-path extraction as sync-api (`extract_json_value()`). Values are compared as strings after extraction.

### Error handling
- **D-13:** Submit request failures: one connection retry (same as sync-api D-13, `is_connect()` check only). If retry also fails, task fails immediately with descriptive error.
- **D-14:** Poll request failures: one connection retry per failed poll attempt. If retry also fails, task fails immediately. Does not silently continue polling.
- **D-15:** Non-2xx HTTP status on submit or poll fails the task with status code and response body in the error message (consistent with sync-api D-12).
- **D-16:** Non-JSON poll responses fail the task with a clear error. The condition check requires JSON parsing to extract the key-path value.

### Response section refactor (cross-cutting, all modes)
- **D-17:** The shared `response` config section is restructured with `success` and `failed` sub-sections, each with its own `body` template and optional `header` field. `max_bytes` stays at the `response` level.
- **D-18:** `response.success.body` — template applied on success path (replaces current `response.body`). `response.failed.body` — template applied on failure path. If `failed.body` is omitted, failure results remain empty (backwards-compatible with current behavior).
- **D-19:** `response.success.header` and `response.failed.header` — optional JSON string of extra headers to attach as result metadata, e.g., `'{"Content-Type": "application/json"}'`. These are passed through in the `ExecutionResult` for clients that need to know the result format.
- **D-20:** On CLI failure (non-zero exit code), `<stdout>`, `<stderr>`, and a new `<exit_code>` placeholder are available in `failed.body`. On sync-api/async-api failure (non-2xx or `failed_when`), `<response.path>` placeholders extract from the error/poll response.
- **D-21:** `ExecutionResult` struct gains a `headers: HashMap<String, String>` field (default empty) to carry the parsed header metadata back to the gateway. Existing success=true/false, result, error_message fields unchanged.
- **D-22:** Existing CLI and sync-api executors are updated to use the new response section structure. This is a refactor — their behavior changes only in that failures now produce structured results instead of empty `Vec::new()`.

### Response extraction (async-api specific)
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

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — AAPI-01 through AAPI-06 (6 requirements for this phase)

### Prior phase context
- `.planning/phases/13-config-placeholders-and-cli-execution/13-CONTEXT.md` — Phase 13 decisions on config format, placeholder engine, executor trait, response template (D-01 through D-19). Phase 15 extends these patterns.
- `.planning/phases/14-sync-api-execution/14-CONTEXT.md` — Phase 14 decisions on HTTP dispatch, response extraction, error handling, reqwest client setup (D-01 through D-17). Phase 15 reuses most of these patterns.

### Existing agent code
- `gateway/src/agent/executor.rs` — `Executor` trait and `ExecutionResult` struct. Phase 15 implements this trait.
- `gateway/src/agent/config.rs` — `AgentConfig`, `ExecutionMode::AsyncApi`, config loading with env var interpolation, validation pattern. Phase 15 adds `AsyncApiSection` and validation.
- `gateway/src/agent/sync_api_executor.rs` — `extract_json_value()`, `find_response_placeholders()`, `SyncApiExecutor` implementation. Phase 15 reuses the JSON extraction functions and follows the same executor structure.
- `gateway/src/agent/placeholder.rs` — Single-pass `resolve_placeholders()` engine. Reused for submit body, poll URL, and poll body templates.
- `gateway/src/agent/response.rs` — `resolve_response_body()` with max_bytes check. Phase 15 refactors this to support success/failed body templates and extends variable map with `<poll_response.path>` entries.
- `gateway/src/agent/cli_executor.rs` — `CliExecutor` implementation. Phase 15 updates failure paths to use `failed.body` template.
- `gateway/src/bin/agent.rs` — Agent binary with `ExecutionMode::AsyncApi` match arm (currently exits with error). Phase 15 wires in `AsyncApiExecutor`.
- `gateway/src/agent/mod.rs` — Module declarations. Phase 15 adds `pub mod async_api_executor;`.

### Project decisions
- `.planning/PROJECT.md` — Key Decisions table: `async_trait` for `Box<dyn Executor>`, stay on `reqwest 0.12`, one-retry for connection errors

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `sync_api_executor::extract_json_value()` — Dot-notation JSON value extraction. Reuse directly for condition path evaluation and response extraction. Consider moving to a shared module since both sync-api and async-api need it.
- `sync_api_executor::find_response_placeholders()` — Scans templates for `<response.XXX>` placeholders. Needs adaptation for `<submit_response.XXX>` and `<poll_response.XXX>` prefixes.
- `placeholder::resolve_placeholders()` — Reuse for submit body, poll URL template, and poll body template resolution.
- `response::resolve_response_body()` — Refactored to accept success/failed body templates. Max_bytes check stays.
- `reqwest::Client` — Already in Cargo.toml. Build once with timeout and TLS settings, store as field.

### Established Patterns
- Config validation at load time: `if mode == AsyncApi && async_api.is_none() { error }`
- Placeholder variable map: `HashMap<String, String>` populated from task assignment fields
- `ExecutionResult { success, result, error_message }` return type — gains `headers: HashMap<String, String>` field
- One connection retry with `is_connect()` check for transient failures
- Structured logging with `tracing::info!` / `tracing::error!` with field context

### Integration Points
- `AgentConfig` struct — Add `async_api: Option<AsyncApiSection>` field
- `agent/mod.rs` — Add `pub mod async_api_executor;`
- `bin/agent.rs` match arm — Replace `eprintln!` with `Box::new(AsyncApiExecutor::new(...))`
- `load_config_from_str()` validation — Add async-api mode check
- `ResponseSection` struct — Restructure from flat `{ body, max_bytes }` to `{ success, failed, max_bytes }`
- `ExecutionResult` struct — Add `headers` field
- `CliExecutor` — Update failure paths to resolve `failed.body` template with `<stderr>`, `<stdout>`, `<exit_code>`
- `SyncApiExecutor` — Update failure paths to resolve `failed.body` template with `<response.path>` from error response
- `bin/agent.rs` — Pass parsed headers from `ExecutionResult` into `ReportResultRequest`

</code_context>

<specifics>
## Specific Ideas

- Config structure follows nested sub-sections (`submit`, `poll`) because async-api has two distinct HTTP calls, but each sub-section internally mirrors the flat sync-api field pattern for consistency
- `extract_json_value()` and `find_response_placeholders()` should likely be moved to a shared utility module since both sync-api and async-api executors need them — Claude's discretion on exact approach
- The `in`/`not_in` operators use YAML arrays, not comma-separated strings, for type safety and clarity
- Response section restructure example showing all three modes:

```yaml
# CLI mode
response:
  success:
    header: '{"Content-Type": "text/plain"}'
    body: '<stdout>'
  failed:
    body: '{"error": "<stderr>", "exit_code": "<exit_code>"}'
  max_bytes: 1048576

# Sync-API mode
response:
  success:
    header: '{"Content-Type": "application/json"}'
    body: '{"result": "<response.data.output>"}'
  failed:
    body: '{"error": "<response.error.message>", "status": "<response.status>"}'
  max_bytes: 1048576

# Async-API mode
response:
  success:
    body: '{"result": "<poll_response.result.output>"}'
  failed:
    body: '{"error": "<poll_response.error.message>"}'
  max_bytes: 1048576
```

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 15-async-api-execution*
*Context gathered: 2026-03-24*
