# Phase 19: JSON Payload Format (Remove Base64 Requirement) - Research

**Researched:** 2026-03-25
**Domain:** Payload encoding format across gRPC, HTTP, Redis storage, agent execution, and Node.js clients
**Confidence:** HIGH

## Summary

This phase replaces the current base64-encoded `bytes` payload model with a JSON value payload model across the entire stack: proto definition, HTTP API, Redis storage, agent execution, Node.js client examples, and documentation. The current architecture requires clients to base64-encode payloads before submission; the gateway decodes them to bytes, stores bytes as base64 in Redis, and re-encodes results as base64 on retrieval. This creates unnecessary friction for JSON-native clients (which are the primary use case).

The change is architecturally clean because the gateway already treats payloads as opaque -- it never inspects payload content. Switching from `Vec<u8>` (base64-encoded bytes) to `serde_json::Value` (or `String` containing JSON) maintains this opacity while removing the encoding/decoding ceremony. The gRPC proto changes from `bytes payload` to `string payload` (JSON-encoded string), and Redis stores the raw JSON string directly instead of base64-encoding bytes.

**Primary recommendation:** Change payload and result types from `Vec<u8>` to `String` throughout the Rust codebase, store JSON strings directly in Redis (no base64 wrapping), accept any valid JSON value in the HTTP `payload` field via `serde_json::Value`, and serialize it to a JSON string for Redis/gRPC transport. The proto `bytes` fields become `string` fields.

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| EXMP-04 | Node.js client example demonstrating full client -> gateway -> agent -> result flow | All three Node.js clients updated to send JSON object payloads (not plain strings), receive JSON results without base64 decoding; tested end-to-end |
</phase_requirements>

## Architecture Patterns

### Current Data Flow (Base64 Bytes)

```
Client (HTTP)                     Gateway                          Redis                    Agent
  |                                  |                               |                       |
  | POST payload: "aGVsbG8=" -----> | decode base64 -> Vec<u8>      |                       |
  |                                  | encode base64 -> String ------| HSET payload "aGVsbG8="|
  |                                  |                               |                       |
  |                                  | <-- poll task --------------  | HGET -> "aGVsbG8="    |
  |                                  | decode base64 -> Vec<u8>      |                       |
  |                                  | gRPC bytes field -----------> | payload = Vec<u8>     |
  |                                  |                               |                       |
  |                                  | <-- report result (bytes) --- |                       |
  |                                  | encode base64 -> String ------| HSET result "..."     |
  | GET result: "..." (base64) <--- | encode base64 from Vec<u8>   |                       |
```

### Target Data Flow (JSON String)

```
Client (HTTP)                     Gateway                          Redis                    Agent
  |                                  |                               |                       |
  | POST payload: {"key":"val"} --> | serde_json::Value -> String   |                       |
  |                                  | store JSON string directly ---| HSET payload '{"key":"val"}'|
  |                                  |                               |                       |
  |                                  | <-- poll task --------------  | HGET -> '{"key":"val"}'|
  |                                  | String -> gRPC string field ->| payload = String      |
  |                                  |                               |                       |
  |                                  | <-- report result (string) -- |                       |
  |                                  | store JSON string directly ---| HSET result "..."     |
  | GET result: {...} (JSON) <----- | parse String as JSON Value    |                       |
```

### Key Type Changes

| Location | Current Type | New Type | Notes |
|----------|-------------|----------|-------|
| `gateway.proto` SubmitTaskRequest.payload | `bytes` | `string` | JSON-encoded string |
| `gateway.proto` TaskAssignment.payload | `bytes` | `string` | JSON-encoded string |
| `gateway.proto` GetTaskStatusResponse.result | `bytes` | `string` | JSON-encoded string |
| `gateway.proto` ReportResultRequest.result | `bytes` | `string` | JSON-encoded string |
| `TaskStatus.payload` (redis.rs) | `Vec<u8>` | `String` | Direct from Redis |
| `TaskStatus.result` (redis.rs) | `Vec<u8>` | `String` | Direct from Redis |
| `TaskAssignmentData.payload` (redis.rs) | `Vec<u8>` | `String` | Direct from Redis |
| `ExecutionResult.result` (executor.rs) | `Vec<u8>` | `String` | Already UTF-8 in practice |
| HTTP SubmitTaskRequest.payload | `String` (base64) | `serde_json::Value` | Accept any JSON |
| HTTP GetTaskResponse.result | `Option<String>` (base64) | `Option<serde_json::Value>` | Return raw JSON |
| Admin TaskDetailResponse.payload/result | `String` (base64) | `serde_json::Value` | Return raw JSON |

### Pattern: Accept Any JSON, Transport as String

The HTTP layer accepts `serde_json::Value` for maximum flexibility (objects, arrays, strings, numbers, booleans, null). For Redis storage and gRPC transport, this is serialized to a `String` via `serde_json::to_string()`. On retrieval, it is deserialized back to `serde_json::Value` for HTTP JSON responses, or passed as-is as a gRPC string.

```rust
// HTTP submit handler - accept any JSON value
#[derive(Debug, Deserialize)]
pub struct SubmitTaskRequest {
    pub service_name: String,
    pub payload: serde_json::Value, // any valid JSON
    #[serde(default)]
    pub metadata: HashMap<String, String>,
    pub callback_url: Option<String>,
}

// Convert to string for storage/transport
let payload_str = serde_json::to_string(&req.payload)
    .map_err(|e| GatewayError::InvalidRequest(format!("failed to serialize payload: {e}")))?;
```

```rust
// HTTP result handler - return parsed JSON
#[derive(Debug, Serialize)]
pub struct GetTaskResponse {
    pub task_id: String,
    pub state: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>, // raw JSON, not base64
    // ...
}

// Parse stored JSON string back to Value
let result = if status.result.is_empty() {
    None
} else {
    Some(serde_json::from_str(&status.result).unwrap_or(serde_json::Value::String(status.result.clone())))
};
```

### Anti-Patterns to Avoid

- **Double-serialization:** Do NOT `serde_json::to_string()` a value that is already a string. If the payload is `"hello"`, `to_string()` produces `"\"hello\""`. Use `Value::String(s)` wrapping or handle string payloads carefully.
- **Lossy fallback hiding bugs:** When deserializing results from Redis, a fallback to `Value::String(raw)` is acceptable for backward compatibility with pre-migration data, but log a warning so the fallback does not silently mask format errors in new data.
- **Breaking the agent placeholder system:** The agent's `build_task_variables` currently does `String::from_utf8_lossy(&assignment.payload)` to get a string for `<payload>` placeholder resolution. With the new `string` proto field, this becomes simply `assignment.payload.clone()`. The placeholder system already works with strings -- no changes needed to the resolution engine itself.

## Affected Files Inventory

### Proto (1 file)
| File | Change |
|------|--------|
| `proto/src/gateway.proto` | 4 fields: `bytes` -> `string` |

### Gateway Core (3 files)
| File | Change |
|------|--------|
| `gateway/src/queue/redis.rs` | Remove base64 encode/decode for payload and result; change `Vec<u8>` to `String`; store/retrieve JSON strings directly |
| `gateway/src/http/submit.rs` | Accept `serde_json::Value` payload; serialize to string for queue |
| `gateway/src/http/result.rs` | Return `serde_json::Value` result; parse string from queue |

### Gateway HTTP Admin (1 file)
| File | Change |
|------|--------|
| `gateway/src/http/admin.rs` | `TaskDetailResponse` payload/result fields: `String` (base64) -> `serde_json::Value`; remove base64 encoding |

### Gateway gRPC (2 files)
| File | Change |
|------|--------|
| `gateway/src/grpc/submit.rs` | `req.payload` is now `String` not `Vec<u8>`; pass directly to queue |
| `gateway/src/grpc/poll.rs` | `task_data.payload` is now `String`; `req.result` in report_result is now `String` |

### Agent (4 files)
| File | Change |
|------|--------|
| `gateway/src/agent/executor.rs` | `ExecutionResult.result`: `Vec<u8>` -> `String` |
| `gateway/src/agent/placeholder.rs` | `build_task_variables`: `assignment.payload` is now `String`, remove `from_utf8_lossy` |
| `gateway/src/agent/cli_executor.rs` | `result: bytes` -> `result: String` in all `ExecutionResult` returns |
| `gateway/src/agent/response.rs` | `resolve_response_body` returns `String` instead of `Vec<u8>` (trivial: remove `.into_bytes()`) |

### Agent HTTP Executors (2 files)
| File | Change |
|------|--------|
| `gateway/src/agent/sync_api_executor.rs` | Result is String not Vec<u8> |
| `gateway/src/agent/async_api_executor.rs` | Result is String not Vec<u8> |

### Agent Binary (1 file)
| File | Change |
|------|--------|
| `gateway/src/bin/agent.rs` | `ReportResultRequest.result` is now `String` (proto changed) |

### Node.js Clients (3 files)
| File | Change |
|------|--------|
| `examples/nodejs-client/cli-client.js` | Send JSON object payload; parse result as JSON (no Buffer.from base64) |
| `examples/nodejs-client/sync-api-client.js` | Same changes |
| `examples/nodejs-client/async-api-client.js` | Same changes |

### Tests (3+ files)
| File | Change |
|------|--------|
| `gateway/tests/integration_test.rs` | Remove base64 encoding of payloads in test submissions |
| `gateway/tests/auth_integration_test.rs` | Remove base64 encoding in ~12 test payload constructions |
| `gateway/src/queue/redis.rs` (tests) | Update `submit_task` calls: `b"hello".to_vec()` -> `"\"hello\""` or JSON string |
| `gateway/src/agent/cli_executor.rs` (tests) | Update `make_assignment` to use String payload |

### Documentation (2 files)
| File | Change |
|------|--------|
| `README.md` | Update API example, remove "base64-encoded opaque bytes" language |
| `examples/nodejs-client/README.md` | Update usage examples |

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON validation | Custom JSON parser | `serde_json::Value` deserialization | serde_json handles all valid JSON values (objects, arrays, strings, numbers, booleans, null) correctly |
| JSON string escaping | Manual escaping | `serde_json::to_string()` | Handles unicode escaping, nested quotes, special characters |
| Proto string encoding | Manual UTF-8 handling | Prost's native String handling | Proto3 `string` fields are UTF-8 by definition; Prost maps them to Rust `String` |

## Common Pitfalls

### Pitfall 1: Double-Serialized Strings
**What goes wrong:** A client sends `payload: "hello"` (a JSON string). `serde_json::Value` deserializes this to `Value::String("hello")`. Then `serde_json::to_string(&value)` produces `"\"hello\""` (a quoted string). When the agent reads this from gRPC, the payload variable becomes `"hello"` (with quotes), breaking CLI arg substitution.
**Why it happens:** JSON strings are valid JSON values, so they get double-wrapped when serialized for storage.
**How to avoid:** Use `serde_json::to_string()` consistently for storage, and document that `<payload>` in agent templates will contain the JSON-serialized form. For simple string payloads, clients should send `payload: {"text": "hello"}` (an object) rather than `payload: "hello"` (a bare string). Alternatively, the agent can detect if the payload is a JSON string value and unwrap it for the `<payload>` variable.
**Warning signs:** CLI echo tests returning `"hello"` (with quotes) instead of `hello`.

### Pitfall 2: Backward Compatibility with Existing Redis Data
**What goes wrong:** Tasks submitted before the migration have base64-encoded payloads in Redis. After deployment, `get_task_status` tries to parse them as JSON strings and fails.
**Why it happens:** Rolling deployment or existing tasks in the queue during upgrade.
**How to avoid:** Add a fallback in `get_task_status`: if the stored payload string does not look like it was stored in the new format, attempt base64 decode and convert. Or accept that this is a breaking change and document it -- tasks in-flight during upgrade may show garbled payloads. Given the TTL on tasks (typically hours), this is a brief window.
**Warning signs:** 500 errors on GET /v1/tasks/{id} after deployment.

### Pitfall 3: Proto `bytes` to `string` Wire Compatibility
**What goes wrong:** A gRPC client compiled against the old proto (with `bytes payload`) sends a message to a gateway compiled with the new proto (`string payload`). Proto3 wire format for `bytes` and `string` is identical (length-delimited), so the message will decode, but the semantic interpretation changes.
**Why it happens:** Proto3 uses the same wire type (2) for both `bytes` and `string`.
**How to avoid:** This is actually a non-issue for wire compatibility -- old clients sending base64-encoded bytes will have their payload arrive as a string containing base64. The gateway can detect and handle this during a transition period, or the proto change can be treated as a breaking API version change.
**Warning signs:** None -- wire format is compatible.

### Pitfall 4: Agent stdin Mode with JSON Payloads
**What goes wrong:** In CLI stdin mode, the agent pipes `assignment.payload` to the child process's stdin. Currently this is raw bytes. With the change, it becomes a JSON string. If the payload is `{"key": "value"}`, stdin receives `{"key": "value"}` which is correct. But if the payload was `"hello"`, stdin receives `"hello"` (with quotes), which may surprise the child process.
**Why it happens:** JSON serialization of string values includes quotes.
**How to avoid:** Document that stdin mode receives the JSON-serialized payload. For plain text use cases, wrap in an object: `{"text": "hello"}`. The child process can parse JSON from stdin.
**Warning signs:** CLI services receiving unexpected quote characters.

## Code Examples

### Proto Change
```protobuf
// Before
message SubmitTaskRequest {
  string service_name = 1;
  bytes payload = 2;         // was bytes
  map<string, string> metadata = 3;
  string callback_url = 4;
}

// After
message SubmitTaskRequest {
  string service_name = 1;
  string payload = 2;        // now string (JSON-encoded)
  map<string, string> metadata = 3;
  string callback_url = 4;
}
```

### Redis Storage (no more base64)
```rust
// Before (redis.rs submit_task)
let payload_b64 = base64::Engine::encode(
    &base64::engine::general_purpose::STANDARD,
    &payload,
);
// ... HSET ... .arg("payload").arg(&payload_b64)

// After
// payload is already a String (JSON-encoded)
// ... HSET ... .arg("payload").arg(&payload)
```

### HTTP Submit (accept any JSON)
```rust
// Before
#[derive(Debug, Deserialize)]
pub struct SubmitTaskRequest {
    pub service_name: String,
    pub payload: String, // base64
    // ...
}
let payload_bytes = base64::Engine::decode(..., &req.payload)?;

// After
#[derive(Debug, Deserialize)]
pub struct SubmitTaskRequest {
    pub service_name: String,
    pub payload: serde_json::Value, // any valid JSON
    // ...
}
let payload_json = serde_json::to_string(&req.payload)?;
```

### Node.js Client (no base64)
```javascript
// Before
const submitRes = await fetch(`${GATEWAY_URL}/v1/tasks`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${API_KEY}` },
  body: JSON.stringify({
    service_name: 'cli-echo',
    payload: payload, // was plain string, gateway expected base64
  }),
});
// ... result decoding:
console.log(Buffer.from(task.result, 'base64').toString());

// After
const submitRes = await fetch(`${GATEWAY_URL}/v1/tasks`, {
  method: 'POST',
  headers: { 'Content-Type': 'application/json', 'Authorization': `Bearer ${API_KEY}` },
  body: JSON.stringify({
    service_name: 'cli-echo',
    payload: { message: payload }, // JSON object
  }),
});
// ... result is already JSON:
console.log(JSON.stringify(task.result, null, 2));
```

### Agent Placeholder Change
```rust
// Before (placeholder.rs)
pub fn build_task_variables(assignment: &TaskAssignment, service_name: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("payload".to_string(), String::from_utf8_lossy(&assignment.payload).to_string());
    // ...
}

// After
pub fn build_task_variables(assignment: &TaskAssignment, service_name: &str) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("payload".to_string(), assignment.payload.clone());
    // ...
}
```

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (built-in) |
| Config file | Cargo.toml (workspace) |
| Quick run command | `cargo test -p xgent-gateway --lib` |
| Full suite command | `cargo test -p xgent-gateway` |

### Phase Requirements -> Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| EXMP-04-a | HTTP submit accepts JSON value payload | integration | `cargo test -p xgent-gateway --test integration_test -- --ignored test_submit_task_http` | Exists (needs update) |
| EXMP-04-b | gRPC payload field is string | integration | `cargo test -p xgent-gateway --test integration_test -- --ignored test_submit_task_grpc` | Exists (needs update) |
| EXMP-04-c | GET task returns JSON result (not base64) | integration | `cargo test -p xgent-gateway --test integration_test -- --ignored test_full_lifecycle` | Exists (needs update) |
| EXMP-04-d | Agent deserializes JSON payload | unit | `cargo test -p xgent-gateway --lib placeholder::tests` | Exists (needs update) |
| EXMP-04-e | Node.js clients work end-to-end | manual | `node examples/nodejs-client/cli-client.js` | Exists (needs update) |
| EXMP-04-f | README documents JSON contract | manual | Visual inspection | Exists (needs update) |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway --lib`
- **Per wave merge:** `cargo test -p xgent-gateway`
- **Phase gate:** Full suite green + manual Node.js client test

### Wave 0 Gaps
None -- existing test infrastructure covers all phase requirements after updating test expectations.

## Open Questions

1. **Backward compatibility strategy**
   - What we know: Proto3 `bytes` and `string` share the same wire type, so old clients can still communicate. Redis data has a TTL so old format data expires naturally.
   - What's unclear: Whether the project needs a graceful migration period or can do a clean break.
   - Recommendation: Clean break -- the project is pre-1.0, no production users are mentioned, and task TTLs ensure old data expires within hours. Document the breaking change.

2. **Agent payload variable format for string values**
   - What we know: If a client sends `payload: "hello"`, `serde_json::to_string()` produces `"\"hello\""`. The `<payload>` placeholder in CLI commands will include JSON quotes.
   - What's unclear: Whether this is acceptable for CLI arg/stdin mode.
   - Recommendation: Accept this behavior and document that `<payload>` contains the JSON-serialized value. For CLI services expecting plain text, clients should send `payload: {"text": "hello"}` and the command template should parse accordingly. Alternatively, the agent could strip outer quotes for bare string payloads.

## Sources

### Primary (HIGH confidence)
- Codebase analysis: `gateway.proto`, `redis.rs`, `submit.rs`, `result.rs`, `admin.rs`, `placeholder.rs`, `executor.rs`, `cli_executor.rs`, `agent.rs` -- all files read and analyzed
- Node.js client examples: `cli-client.js`, `sync-api-client.js`, `async-api-client.js` -- read and analyzed
- Proto3 specification: `bytes` and `string` share wire type 2 (length-delimited) -- verified from protobuf language guide

### Secondary (MEDIUM confidence)
- Proto3 wire compatibility between bytes and string -- consistent with proto3 specification but not tested with actual cross-compiled clients in this codebase

## Metadata

**Confidence breakdown:**
- Architecture: HIGH - Direct codebase analysis, clear type transformation path
- Affected files: HIGH - Exhaustive grep of all base64 usage in the codebase
- Pitfalls: HIGH - Based on actual code patterns observed
- Proto compatibility: MEDIUM - Based on proto3 spec knowledge, not tested cross-version

**Research date:** 2026-03-25
**Valid until:** 2026-04-25 (stable domain, internal refactor)
