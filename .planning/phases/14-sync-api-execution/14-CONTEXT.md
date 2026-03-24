# Phase 14: Sync-API Execution - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Agent dispatches tasks to configurable HTTP endpoints with templated requests and response mapping. Implements `SyncApiExecutor` following the `Executor` trait pattern from Phase 13. Async-API (two-phase submit + poll) is Phase 15.

</domain>

<decisions>
## Implementation Decisions

### Config structure
- **D-01:** Single `url` field with placeholder support: `url: "${API_BASE}/v1/run"`. Env vars resolve at config load time, task placeholders (`<payload>`, `<service_name>`, `<metadata.key>`) resolve at execution time.
- **D-02:** `method` field defaults to `POST` if omitted. Supports standard HTTP methods (GET, POST, PUT, PATCH, DELETE).
- **D-03:** `headers` field is a `HashMap<String, String>` with env var interpolation: `Authorization: "Bearer ${API_TOKEN}"`.
- **D-04:** `body` template supports dual mode — `<payload>` as entire body for raw passthrough, or embedded in a JSON structure: `{"input": "<payload>", "model": "gpt-4"}`. Same placeholder engine as CLI mode.
- **D-05:** `timeout_secs` field with default 30s. On expiry, task fails with "HTTP request timed out after Ns".
- **D-06:** `tls_skip_verify` field (default false) for self-signed cert targets. Mirrors `gateway.tls_skip_verify` pattern.
- **D-07:** Validation at config load: if `mode` is `sync-api`, `sync_api` section must be present (same pattern as CLI validation).

### Response extraction
- **D-08:** Dot notation for key-paths: `<response.result.text>`, `<response.data.0.id>`. Numeric path segments index into JSON arrays.
- **D-09:** Unresolved response key-path fails the task with an error listing the path and the actual response structure — consistent with D-08 from Phase 13 (unresolved placeholders fail task).
- **D-10:** Non-string values (numbers, booleans, objects, arrays) are JSON-serialized when extracted: `42` becomes `"42"`, objects become compact JSON strings.
- **D-11:** Response body template uses the existing `response` section shared across modes. `<response.path>` placeholders are added to the variables map alongside `<payload>`, `<service_name>`, etc.

### Error handling
- **D-12:** Non-2xx HTTP status fails the task. Error message includes status code and full response body: `"HTTP 422: {\"error\": \"invalid input\"}"`.
- **D-13:** Connection-level failures (DNS, connection refused, TLS error) retry once, then fail the task with the reqwest error message. This is a one-time automatic retry for transient network issues only.
- **D-14:** Timeout fires via reqwest's built-in timeout. Task fails with descriptive message including configured duration.

### HTTP client setup
- **D-15:** `reqwest::Client` built once in `SyncApiExecutor::new()` with configured timeout and TLS settings, stored as field, reused for all requests. Connection pooling and TLS session reuse.
- **D-16:** Redirect policy: follow up to 5 redirects (capped from reqwest default of 10).
- **D-17:** Agent binary wires `SyncApiExecutor` into the existing `ExecutionMode::SyncApi` match arm, replacing the current `eprintln!` + `exit(1)`.

### Claude's Discretion
- Exact `SyncApiSection` struct field names and serde attributes
- How dot-notation path traversal is implemented (manual walk vs helper fn)
- How the one-retry logic is structured (loop vs explicit retry)
- Test strategy and fixture structure
- Error message exact formatting
- Whether URL also supports task placeholders (`<service_name>` in URL path)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — SAPI-01 through SAPI-04 (4 requirements for this phase)

### Prior phase context
- `.planning/phases/13-config-placeholders-and-cli-execution/13-CONTEXT.md` — Phase 13 decisions on config format, placeholder engine, executor trait, response template (D-01 through D-19). Phase 14 extends these patterns.

### Existing agent code
- `gateway/src/agent/executor.rs` — `Executor` trait and `ExecutionResult` struct. Phase 14 implements this trait.
- `gateway/src/agent/config.rs` — `AgentConfig`, `ExecutionMode::SyncApi`, config loading with env var interpolation. Phase 14 adds `SyncApiSection` and validation.
- `gateway/src/agent/cli_executor.rs` — Reference implementation of `Executor` trait. Pattern for constructor, execute(), placeholder variable building.
- `gateway/src/agent/placeholder.rs` — Single-pass `resolve_placeholders()` engine. Reused as-is for request body and URL templates.
- `gateway/src/agent/response.rs` — `resolve_response_body()` with max_bytes check. Phase 14 extends variable map with `<response.path>` entries.
- `gateway/src/bin/agent.rs` — Agent binary with `ExecutionMode::SyncApi` match arm (currently exits with error). Phase 14 wires in `SyncApiExecutor`.
- `gateway/src/agent/mod.rs` — Module declarations. Phase 14 adds `pub mod sync_api_executor;`.

### Project decisions
- `.planning/PROJECT.md` — Key Decisions table: `async_trait` for `Box<dyn Executor>`, stay on `reqwest 0.12`, no retry philosophy (D-07 — though sync-api has one connection retry per D-13 above)

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `placeholder::resolve_placeholders()` — Reuse directly for URL template, body template, and header value resolution
- `response::resolve_response_body()` — Reuse for final response body assembly with max_bytes check. Extend the variables map with `response.*` entries extracted from the HTTP response JSON.
- `CliExecutor` constructor pattern — Reference for how to build `SyncApiExecutor::new()` (takes config section + response section)
- `reqwest` crate — Already in `Cargo.toml` dependencies from old agent code

### Established Patterns
- Config validation at load time: `if mode == SyncApi && sync_api.is_none() { error }`
- Placeholder variable map building: `HashMap<String, String>` populated from task assignment fields
- `ExecutionResult { success, result, error_message }` return type
- Structured logging with `tracing::info!` / `tracing::error!` and field context

### Integration Points
- `AgentConfig` struct — Add `sync_api: Option<SyncApiSection>` field
- `agent/mod.rs` — Add `pub mod sync_api_executor;`
- `bin/agent.rs` match arm — Replace `eprintln!` with `Box::new(SyncApiExecutor::new(...))`
- `load_config_from_str()` validation — Add sync-api mode check

</code_context>

<specifics>
## Specific Ideas

- Config structure follows the same YAML pattern as CLI section — familiar for users configuring multiple services
- Dot notation chosen over JSON Pointer for key-paths — more readable and familiar to users: `<response.result.text>` vs `<response./result/text>`
- One connection retry is an intentional departure from the "no retries" project philosophy — justified because connection-level failures are transient infrastructure issues, not task-level failures

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 14-sync-api-execution*
*Context gathered: 2026-03-24*
