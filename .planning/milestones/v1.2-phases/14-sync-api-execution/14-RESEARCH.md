# Phase 14: Sync-API Execution - Research

**Researched:** 2026-03-24
**Domain:** HTTP client dispatch with templated requests and JSON response extraction
**Confidence:** HIGH

## Summary

Phase 14 implements `SyncApiExecutor` -- a new `Executor` trait implementation that dispatches tasks as HTTP requests to configurable endpoints and extracts values from JSON responses. This is a well-scoped extension of the Phase 13 architecture: it reuses the existing placeholder engine, response template system, and executor trait pattern. The primary new work is (1) a `SyncApiSection` config struct, (2) `reqwest::Client` setup with timeout/TLS/redirect configuration, (3) dot-notation JSON response traversal, and (4) one-retry logic for connection-level failures.

The project already has `reqwest 0.12.28` in `Cargo.toml` with the `json` feature enabled, and `serde_json 1.0` for JSON manipulation. No new crate dependencies are needed. The `CliExecutor` implementation (247 lines including tests) serves as the exact structural template -- `SyncApiExecutor` follows the same constructor pattern, variable map building, placeholder resolution, and response body assembly.

**Primary recommendation:** Follow the `CliExecutor` pattern exactly. Build `reqwest::Client` once in `SyncApiExecutor::new()`. Implement dot-notation traversal as a standalone function that walks `serde_json::Value`. Wire into the existing `ExecutionMode::SyncApi` match arm in `bin/agent.rs`.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Single `url` field with placeholder support: `url: "${API_BASE}/v1/run"`. Env vars resolve at config load time, task placeholders (`<payload>`, `<service_name>`, `<metadata.key>`) resolve at execution time.
- **D-02:** `method` field defaults to `POST` if omitted. Supports standard HTTP methods (GET, POST, PUT, PATCH, DELETE).
- **D-03:** `headers` field is a `HashMap<String, String>` with env var interpolation: `Authorization: "Bearer ${API_TOKEN}"`.
- **D-04:** `body` template supports dual mode -- `<payload>` as entire body for raw passthrough, or embedded in a JSON structure: `{"input": "<payload>", "model": "gpt-4"}`. Same placeholder engine as CLI mode.
- **D-05:** `timeout_secs` field with default 30s. On expiry, task fails with "HTTP request timed out after Ns".
- **D-06:** `tls_skip_verify` field (default false) for self-signed cert targets. Mirrors `gateway.tls_skip_verify` pattern.
- **D-07:** Validation at config load: if `mode` is `sync-api`, `sync_api` section must be present (same pattern as CLI validation).
- **D-08:** Dot notation for key-paths: `<response.result.text>`, `<response.data.0.id>`. Numeric path segments index into JSON arrays.
- **D-09:** Unresolved response key-path fails the task with an error listing the path and the actual response structure.
- **D-10:** Non-string values (numbers, booleans, objects, arrays) are JSON-serialized when extracted: `42` becomes `"42"`, objects become compact JSON strings.
- **D-11:** Response body template uses the existing `response` section shared across modes. `<response.path>` placeholders are added to the variables map alongside `<payload>`, `<service_name>`, etc.
- **D-12:** Non-2xx HTTP status fails the task. Error message includes status code and full response body: `"HTTP 422: {\"error\": \"invalid input\"}"`.
- **D-13:** Connection-level failures (DNS, connection refused, TLS error) retry once, then fail the task with the reqwest error message. One-time automatic retry for transient network issues only.
- **D-14:** Timeout fires via reqwest's built-in timeout. Task fails with descriptive message including configured duration.
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

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| SAPI-01 | Agent dispatches HTTP request with configurable URL, method, and headers | `SyncApiSection` config struct + `reqwest::Client` with method/URL/headers resolution via placeholder engine. D-01, D-02, D-03. |
| SAPI-02 | Body template supports `<payload>` as entire body or embedded in JSON structure | Reuse `placeholder::resolve_placeholders()` on body template. D-04. Same engine as CLI mode. |
| SAPI-03 | Response body template maps `<response.path>` key-paths into result shape | Dot-notation JSON traversal function extracts values into variable map, then `response::resolve_response_body()` assembles final output. D-08, D-09, D-10, D-11. |
| SAPI-04 | Non-2xx HTTP status maps to failure with status code and body in error | `response.status().is_success()` check. D-12 format. |
</phase_requirements>

## Standard Stack

### Core (already in Cargo.toml -- no additions needed)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| reqwest | 0.12.28 | HTTP client | Already a dependency. Async, Tokio-native, built-in timeout, redirect policy, TLS via rustls. |
| serde_json | 1.0 | JSON parsing | Already a dependency. `serde_json::Value` for response body traversal. |
| async-trait | 0.1 | Trait async methods | Already used for `Executor` trait. |

### Supporting (already in Cargo.toml)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| serde | 1.0 | Deserialization | `SyncApiSection` struct derives `Deserialize`. |
| tracing | 0.1 | Structured logging | Log HTTP request/response details at info/error level. |

### No New Dependencies
This phase requires zero new crate additions. Everything needed is already in `gateway/Cargo.toml`.

## Architecture Patterns

### New Files
```
gateway/src/agent/
├── mod.rs                  # Add: pub mod sync_api_executor;
├── sync_api_executor.rs    # NEW: SyncApiExecutor implementation (~200 LOC + tests)
├── config.rs               # MODIFY: Add SyncApiSection, sync_api field, validation
├── executor.rs             # UNCHANGED
├── placeholder.rs          # UNCHANGED
├── response.rs             # UNCHANGED (max_bytes check generalizes to response body size)
└── cli_executor.rs         # UNCHANGED
```

### Pattern 1: SyncApiSection Config Struct
**What:** Deserialization target for the `sync_api:` YAML section
**When to use:** Config load time
**Example:**
```rust
// Follows CliSection pattern exactly
#[derive(Debug, Clone, Deserialize)]
pub struct SyncApiSection {
    pub url: String,
    #[serde(default = "default_http_method")]
    pub method: String,       // "GET", "POST", etc. -- validated at use time
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>, // Body template; None for GET requests
    #[serde(default = "default_sync_api_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub tls_skip_verify: bool,
}

fn default_http_method() -> String { "POST".to_string() }
fn default_sync_api_timeout() -> u64 { 30 }
```

### Pattern 2: reqwest::Client Construction
**What:** Build client once in constructor, reuse for all requests
**When to use:** `SyncApiExecutor::new()`
**Example:**
```rust
use reqwest::redirect::Policy;

pub struct SyncApiExecutor {
    service_name: String,
    sync_api: SyncApiSection,
    response: ResponseSection,
    client: reqwest::Client,
}

impl SyncApiExecutor {
    pub fn new(
        service_name: String,
        sync_api: SyncApiSection,
        response: ResponseSection,
    ) -> Result<Self, String> {
        let mut builder = reqwest::Client::builder()
            .timeout(Duration::from_secs(sync_api.timeout_secs))
            .redirect(Policy::limited(5));

        if sync_api.tls_skip_verify {
            builder = builder.danger_accept_invalid_certs(true);
        }

        let client = builder.build()
            .map_err(|e| format!("failed to build HTTP client: {}", e))?;

        Ok(Self { service_name, sync_api, response, client })
    }
}
```

**Key detail:** `SyncApiExecutor::new()` returns `Result` (unlike `CliExecutor::new()` which is infallible) because `reqwest::Client::builder().build()` can fail with invalid TLS config. The agent binary handles this error at startup.

### Pattern 3: Dot-Notation JSON Traversal
**What:** Walk a `serde_json::Value` using dot-separated path segments
**When to use:** Extracting response values for `<response.path>` placeholders
**Example:**
```rust
fn extract_json_value(root: &serde_json::Value, path: &str) -> Result<String, String> {
    let segments: Vec<&str> = path.split('.').collect();
    let mut current = root;

    for segment in &segments {
        if let Ok(index) = segment.parse::<usize>() {
            // Array index
            current = current.get(index).ok_or_else(|| {
                format!("array index {} out of bounds at path '{}'", index, path)
            })?;
        } else {
            // Object key
            current = current.get(*segment).ok_or_else(|| {
                format!("key '{}' not found at path '{}'; response: {}",
                    segment, path, serde_json::to_string(root).unwrap_or_default())
            })?;
        }
    }

    // D-10: Convert to string representation
    match current {
        serde_json::Value::String(s) => Ok(s.clone()),
        serde_json::Value::Null => Ok("null".to_string()),
        other => Ok(serde_json::to_string(other).unwrap_or_default()),
    }
}
```

### Pattern 4: Response Variable Map Population
**What:** Parse HTTP response body as JSON, extract all `response.*` placeholders from the response template, add to variable map
**When to use:** After successful HTTP response, before response body template resolution
**Example:**
```rust
// Scan response template for <response.XXX> placeholders
// For each found, extract from JSON and add "response.XXX" -> value to variables map
// Then call resolve_response_body() as normal
```

**Important design choice:** Scan the response body template to discover which `response.*` paths are needed, extract each from the JSON, insert into variables map, then let `resolve_response_body()` handle final assembly. This keeps the placeholder engine untouched.

### Pattern 5: One-Retry for Connection Failures
**What:** Retry once on connection-level errors, not on HTTP status errors
**When to use:** In `execute()` method
**Example:**
```rust
let response = match self.send_request(&url, &body, &variables).await {
    Ok(resp) => resp,
    Err(e) if e.is_connect() || e.is_timeout() => {
        // D-13: One retry for transient connection failures
        tracing::warn!(error = %e, "connection failed, retrying once");
        self.send_request(&url, &body, &variables).await
            .map_err(|e| format!("HTTP request failed after retry: {}", e))?
    }
    Err(e) => return /* failure ExecutionResult */,
};
```

**Distinguishing connection vs. HTTP errors:** `reqwest::Error::is_connect()` returns true for DNS failures, connection refused, and TLS handshake failures. `reqwest::Error::is_timeout()` covers request timeouts. HTTP 4xx/5xx responses are NOT errors at the reqwest level -- they return `Ok(Response)` and are checked via `response.status().is_success()`.

### Anti-Patterns to Avoid
- **Do NOT parse response body template at config load time:** The response template is shared across modes and may contain `<stdout>` or other mode-specific placeholders. Parse/scan it at execution time.
- **Do NOT use `response.json::<serde_json::Value>()` eagerly:** Read the body as text first (`response.text().await`), then parse with `serde_json::from_str()`. This preserves the raw body for error messages (D-12).
- **Do NOT add `reqwest::Error` to the `Executor` trait:** The trait returns `ExecutionResult` with string error messages. Convert reqwest errors to strings inside the executor.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| HTTP client | Raw hyper request builder | `reqwest::Client` | Connection pooling, redirect following, TLS, timeout -- all built in |
| JSON path traversal | Full JSONPath spec | Simple dot-notation walk on `serde_json::Value` | D-08 specifies dot notation only. Full JSONPath (serde_json_path crate) is overkill for `a.b.0.c` paths |
| Request timeout | Manual `tokio::time::timeout` wrapper | `reqwest::Client::builder().timeout()` | reqwest handles timeout at the connection+transfer level, not just wall-clock |
| TLS skip verify | Custom TLS config | `reqwest::Client::builder().danger_accept_invalid_certs(true)` | Single method call, well-tested |
| HTTP method dispatch | Match on method string manually | `reqwest::Client::request(Method::from_bytes(...))` | reqwest's `Method` type handles validation |

## Common Pitfalls

### Pitfall 1: Consuming Response Body Twice
**What goes wrong:** Calling `response.text()` consumes the body. If you need the body for both JSON parsing and error messages, you must read it once and reuse.
**Why it happens:** reqwest's `Response` body is a stream -- read once then gone.
**How to avoid:** Always `let body_text = response.text().await?;` first, then `serde_json::from_str(&body_text)` for parsing, and include `body_text` in error messages.
**Warning signs:** Compiler error about moved value, or empty error messages.

### Pitfall 2: Timeout Error vs. Connection Error Distinction
**What goes wrong:** `reqwest::Error::is_timeout()` returns true for reqwest's built-in timeout. But the retry logic (D-13) should NOT retry on timeout -- timeout means the server was too slow, retrying will likely timeout again. Only retry on `is_connect()`.
**Why it happens:** D-13 says "connection-level failures (DNS, connection refused, TLS error)" -- these are `is_connect()`. Timeout is D-14 separately.
**How to avoid:** Retry condition: `e.is_connect()` only. Timeout produces its own distinct error message per D-14.
**Warning signs:** Tasks that timeout once then timeout again after the retry, doubling wait time.

### Pitfall 3: max_bytes Check for Response Mode
**What goes wrong:** `resolve_response_body()` checks `stdout` + `stderr` combined size. For sync-api mode, there is no stdout/stderr -- the check should apply to the HTTP response body instead.
**Why it happens:** The max_bytes check was written for CLI mode.
**How to avoid:** Insert the HTTP response body text as a variable (e.g., under a key that the max_bytes check recognizes), OR perform a separate size check before calling `resolve_response_body()`. The simplest approach: check `body_text.len() > max_bytes` before JSON parsing, and do NOT insert `stdout`/`stderr` keys (they won't exist), so `resolve_response_body()`'s built-in check passes trivially (0 + 0 = 0 <= max_bytes).
**Warning signs:** Extremely large API responses consuming excessive memory.

### Pitfall 4: Method String Case Sensitivity
**What goes wrong:** User writes `method: post` in YAML, but `reqwest::Method::from_bytes(b"post")` returns an error because HTTP methods are uppercase by convention in reqwest.
**Why it happens:** YAML values are case-sensitive but users expect case-insensitivity.
**How to avoid:** `.to_uppercase()` the method string before converting to `reqwest::Method`.
**Warning signs:** "invalid HTTP method" errors for lowercase methods in config.

### Pitfall 5: URL Placeholder Resolution Order
**What goes wrong:** URL contains both env vars (`${API_BASE}`) and task placeholders (`<service_name>`). Env vars are already resolved at config load time. Task placeholders must be resolved at execution time.
**Why it happens:** Two separate resolution passes by design (D-01).
**How to avoid:** Just call `placeholder::resolve_placeholders(&self.sync_api.url, &variables)` at execution time. The `${...}` tokens are already gone from config load. Only `<...>` tokens remain.
**Warning signs:** None -- this just works if you follow the pattern.

## Code Examples

### Complete execute() Flow (Pseudocode)
```rust
#[async_trait]
impl Executor for SyncApiExecutor {
    async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult {
        // 1. Build task variables
        let mut variables = placeholder::build_task_variables(assignment, &self.service_name);

        // 2. Resolve URL template
        let url = match placeholder::resolve_placeholders(&self.sync_api.url, &variables) {
            Ok(u) => u,
            Err(e) => return ExecutionResult { success: false, result: Vec::new(),
                error_message: format!("failed to resolve URL placeholder: {}", e) },
        };

        // 3. Resolve body template (if present)
        let body = if let Some(ref body_template) = self.sync_api.body {
            match placeholder::resolve_placeholders(body_template, &variables) {
                Ok(b) => Some(b),
                Err(e) => return ExecutionResult { success: false, result: Vec::new(),
                    error_message: format!("failed to resolve body placeholder: {}", e) },
            }
        } else {
            None
        };

        // 4. Resolve header values
        let mut headers = reqwest::header::HeaderMap::new();
        for (name, value_template) in &self.sync_api.headers {
            let resolved = match placeholder::resolve_placeholders(value_template, &variables) {
                Ok(v) => v,
                Err(e) => return ExecutionResult { success: false, result: Vec::new(),
                    error_message: format!("failed to resolve header placeholder: {}", e) },
            };
            // HeaderName/HeaderValue parsing...
            // headers.insert(name, resolved);
        }

        // 5. Send HTTP request (with one retry on connection failure)
        let method = reqwest::Method::from_bytes(
            self.sync_api.method.to_uppercase().as_bytes()
        ).unwrap_or(reqwest::Method::POST);

        let send = |client: &reqwest::Client| {
            let mut req = client.request(method.clone(), &url).headers(headers.clone());
            if let Some(ref b) = body { req = req.body(b.clone()); }
            req.send()
        };

        let response = match send(&self.client).await {
            Ok(r) => r,
            Err(e) if e.is_connect() => {
                tracing::warn!(error = %e, "connection failed, retrying once");
                match send(&self.client).await {
                    Ok(r) => r,
                    Err(e) => return ExecutionResult { success: false, result: Vec::new(),
                        error_message: format!("HTTP request failed after retry: {}", e) },
                }
            }
            Err(e) if e.is_timeout() => return ExecutionResult { success: false,
                result: Vec::new(),
                error_message: format!("HTTP request timed out after {}s", self.sync_api.timeout_secs) },
            Err(e) => return ExecutionResult { success: false, result: Vec::new(),
                error_message: format!("HTTP request failed: {}", e) },
        };

        // 6. Check HTTP status
        let status = response.status();
        let body_text = match response.text().await {
            Ok(t) => t,
            Err(e) => return ExecutionResult { success: false, result: Vec::new(),
                error_message: format!("failed to read response body: {}", e) },
        };

        if !status.is_success() {
            return ExecutionResult {
                success: false,
                result: Vec::new(),
                error_message: format!("HTTP {}: {}", status.as_u16(), body_text),
            };
        }

        // 7. Parse JSON and extract response.* values
        // (scan template for <response.XXX>, extract each, add to variables)
        // ... see dot-notation traversal pattern above ...

        // 8. Resolve response body template
        match response::resolve_response_body(&self.response.body, &variables, self.response.max_bytes) {
            Ok(bytes) => ExecutionResult { success: true, result: bytes, error_message: String::new() },
            Err(e) => ExecutionResult { success: false, result: Vec::new(), error_message: e },
        }
    }
}
```

### Config Validation Addition
```rust
// In load_config_from_str(), add after CLI validation:
if config.service.mode == ExecutionMode::SyncApi && config.sync_api.is_none() {
    return Err("mode is 'sync-api' but [sync_api] section is missing".to_string());
}
```

### Agent Binary Wiring
```rust
// In bin/agent.rs, replace the SyncApi match arm:
ExecutionMode::SyncApi => {
    let sync_api_section = config
        .sync_api
        .clone()
        .expect("sync_api section required for sync-api mode");
    match SyncApiExecutor::new(
        config.service.name.clone(),
        sync_api_section,
        config.response.clone(),
    ) {
        Ok(executor) => Box::new(executor),
        Err(e) => {
            eprintln!("failed to initialize sync-api executor: {}", e);
            std::process::exit(1);
        }
    }
}
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | None -- standard Rust test infrastructure |
| Quick run command | `cargo test -p xgent-gateway --lib agent::sync_api_executor -- --nocapture` |
| Full suite command | `cargo test -p xgent-gateway` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| SAPI-01 | HTTP dispatch with URL, method, headers | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::sends_request_with_configured_method -- -x` | Wave 0 |
| SAPI-01 | Header placeholder resolution | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::headers_resolve_placeholders -- -x` | Wave 0 |
| SAPI-02 | Body template with embedded payload | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::body_template_embeds_payload -- -x` | Wave 0 |
| SAPI-02 | Body template as raw payload | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::body_raw_payload_passthrough -- -x` | Wave 0 |
| SAPI-03 | Response dot-notation extraction | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::response_dot_notation_extracts_values -- -x` | Wave 0 |
| SAPI-03 | Array index in dot path | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::response_array_index -- -x` | Wave 0 |
| SAPI-03 | Missing key-path fails task | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::missing_key_path_fails -- -x` | Wave 0 |
| SAPI-04 | Non-2xx returns failure with status and body | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::non_2xx_fails_with_status_and_body -- -x` | Wave 0 |
| D-07 | Config validation for sync-api mode | unit | `cargo test -p xgent-gateway --lib agent::config::tests::sync_api_mode_without_section_fails -- -x` | Wave 0 |
| D-10 | Non-string JSON values serialize correctly | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::non_string_values_serialize -- -x` | Wave 0 |
| D-13 | Connection retry logic | unit | `cargo test -p xgent-gateway --lib agent::sync_api_executor::tests::retries_on_connection_failure -- -x` | Wave 0 |

### Testing Strategy
**Unit tests for dot-notation traversal:** Test the `extract_json_value()` function in isolation with various JSON structures -- nested objects, arrays, missing keys, non-string values.

**Integration tests with mock HTTP server:** For the full `execute()` flow, tests need an HTTP server. Two options:
1. **`mockito` crate** (dev-dependency): Lightweight HTTP mock server. Start per-test, configure response status/body. Simplest approach.
2. **In-process axum server**: Use axum (already a dependency) to spin up a test server with `tokio::spawn`. More control, no new dependency.

**Recommendation (Claude's discretion):** Use an in-process axum test server. Axum is already a dependency, avoids adding `mockito`. Spin up a tiny axum app that returns configurable responses, bind to `127.0.0.1:0` for random port assignment.

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway --lib agent::sync_api_executor`
- **Per wave merge:** `cargo test -p xgent-gateway`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `gateway/src/agent/sync_api_executor.rs` -- all test functions listed above
- [ ] Config validation test in `gateway/src/agent/config.rs` -- `sync_api_mode_without_section_fails`

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| `reqwest 0.11` default TLS | `reqwest 0.12` with rustls default | reqwest 0.12 (2024) | No OpenSSL dependency needed; `danger_accept_invalid_certs()` API unchanged |
| `hyper::Method` directly | `reqwest::Method` (re-export) | N/A | Same type, just access path difference |

**Nothing deprecated or outdated** in the dependencies relevant to this phase.

## Open Questions

1. **URL task placeholder support**
   - What we know: D-01 says URL supports env vars at config load and task placeholders at execution time. The CONTEXT.md Claude's Discretion section asks "Whether URL also supports task placeholders (`<service_name>` in URL path)."
   - What's unclear: This is listed as Claude's discretion.
   - Recommendation: YES, support task placeholders in URL. It's free -- just call `resolve_placeholders()` on the URL string the same way body is resolved. Use case: `url: "https://api.example.com/v1/<service_name>/run"`.

2. **max_bytes applicability for sync-api**
   - What we know: `resolve_response_body()` checks `stdout` + `stderr` combined. Sync-api has neither.
   - What's unclear: Should max_bytes apply to the HTTP response body size?
   - Recommendation: Check HTTP response body size against max_bytes before JSON parsing. This prevents unbounded memory consumption from large API responses. The existing `resolve_response_body()` check passes trivially (0 + 0 = 0) so add a separate pre-check.

## Sources

### Primary (HIGH confidence)
- **Project codebase** -- `gateway/src/agent/` module files read directly. All patterns, types, and conventions verified from source.
- **Cargo.toml** -- `reqwest 0.12.28` with `json` feature confirmed via `cargo tree`.
- **Phase 13 CONTEXT.md** -- Executor trait pattern, placeholder engine design, response template system.
- **Phase 14 CONTEXT.md** -- All 17 locked decisions (D-01 through D-17).

### Secondary (MEDIUM confidence)
- **reqwest API** -- `Client::builder()`, `danger_accept_invalid_certs()`, `redirect::Policy::limited()`, `Error::is_connect()`, `Error::is_timeout()` -- well-known stable API, verified against reqwest 0.12 docs.
- **serde_json::Value traversal** -- `.get()` method for both object keys and array indices -- standard serde_json API.

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all dependencies already in Cargo.toml, versions verified
- Architecture: HIGH -- follows established CliExecutor pattern exactly, code read from source
- Pitfalls: HIGH -- identified from direct code analysis (response body consumption, max_bytes check, method case sensitivity)

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable -- no version changes expected)
