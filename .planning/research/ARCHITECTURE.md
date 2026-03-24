# Architecture Research: Flexible Agent Execution Engine

**Domain:** Configurable execution modes for pull-model task agent
**Researched:** 2026-03-24
**Confidence:** HIGH

## Current State

The `xgent-agent` binary (`gateway/src/bin/agent.rs`) is a ~300-line file that:

1. Parses CLI args via clap (gateway addr, service name, node ID, dispatch URL, token, TLS)
2. Enters an outer reconnect loop with exponential backoff
3. Inner `run_poll_loop` opens a gRPC streaming connection (`PollTasks`) to the gateway
4. On each `TaskAssignment`, calls `dispatch_task` -- a hardcoded HTTP POST to `--dispatch-url`
5. Reports result back via `ReportResult` unary RPC
6. Handles SIGTERM graceful drain via `DrainNode` RPC

**The entire execution logic is the `dispatch_task` function** -- 20 lines that POST the payload and return the response bytes. This is the single point where the new execution engine plugs in.

The agent is a `[[bin]]` inside the `gateway` crate (not a separate workspace member). It imports `xgent_proto` types and uses `reqwest` + `tonic` as its only runtime dependencies.

## System Overview: After Integration

```
                          agent.toml
                              |
                              v
                    +-----------------+
                    | AgentConfig     |
                    | (TOML parsing)  |
                    +--------+--------+
                             |
                             v
+--------+          +--------+--------+          +-----------+
| Gateway | <-gRPC->| Agent Main Loop |          | Executors |
| (poll   |         | (reconnect +    +--------->| (trait     |
|  stream)|         |  select! loop)  |          |  objects)  |
+--------+          +--------+--------+          +-----+-----+
                             |                         |
                             v                   +-----+-----+
                    +--------+--------+          |           |
                    | ReportResult    |     +----+---+ +-----+----+ +--------+
                    | (gRPC unary)    |     | CliExec| | SyncApi  | |AsyncApi|
                    +-----------------+     +--------+ +----------+ +--------+
                                                |          |             |
                                           child proc  reqwest     reqwest +
                                           (stdout/    (single     poll loop
                                            stderr)     POST)
```

### Component Responsibilities

| Component | Responsibility | New vs Modified |
|-----------|----------------|-----------------|
| `AgentConfig` | Parse `agent.toml`, validate service configs, resolve env vars | **NEW** -- replaces clap-only config |
| `Executor` trait | Unified interface: `execute(TaskAssignment) -> ExecutionResult` | **NEW** -- core abstraction |
| `CliExecutor` | Spawn child process, pipe payload, capture stdout/stderr | **NEW** |
| `SyncApiExecutor` | HTTP request with templated URL/body/headers, return response | **NEW** (replaces hardcoded `dispatch_task`) |
| `AsyncApiExecutor` | Two-phase: submit HTTP, then poll for completion with timeout | **NEW** |
| `PlaceholderEngine` | Resolve `<payload>`, `<task_id>`, `<metadata.key>`, `<stdout>`, env vars | **NEW** |
| `ResponseMapper` | Extract result bytes from executor output using configured template | **NEW** |
| Agent main loop | Select between gRPC stream and shutdown; dispatch to executor | **MODIFIED** -- use executor instead of `dispatch_task` |
| CLI args | Retain `--config` path, gateway addr, token; remove `--dispatch-url` | **MODIFIED** |

## Recommended Project Structure

```
gateway/src/
  bin/
    agent.rs              # MODIFIED: main loop, config loading, executor dispatch
  agent/                  # NEW module tree
    mod.rs                # pub mod declarations
    config.rs             # AgentConfig, ServiceConfig, ExecutionMode enums
    executor/
      mod.rs              # Executor trait, ExecutionResult, executor_for_service()
      cli.rs              # CliExecutor
      sync_api.rs         # SyncApiExecutor (absorbs old dispatch_task)
      async_api.rs        # AsyncApiExecutor with poll loop
    placeholder.rs        # Template resolution engine
    response.rs           # Response body mapping
```

### Structure Rationale

- **`agent/` as a module inside `gateway` crate** -- the agent binary already lives here as a `[[bin]]`. A separate workspace crate is premature since it shares `xgent_proto` types and the `reqwest`/`tonic` dependencies are already in `gateway/Cargo.toml`. If the agent grows beyond ~1500 LOC, extract to a workspace member later.
- **`executor/` sub-module** -- each execution mode has distinct enough logic (process spawning vs HTTP vs HTTP+poll) to warrant separate files, but they share the trait and result type from `mod.rs`.
- **`placeholder.rs` separate from executors** -- placeholder resolution is used by all three modes (CLI args, HTTP body templates, response mapping). Keeping it standalone avoids duplication.
- **`response.rs` separate from placeholders** -- response mapping takes an `ExecutionResult` plus a template and produces final bytes. It depends on placeholders but is a distinct concern (when to apply, what context to inject post-execution).

## Architectural Patterns

### Pattern 1: Trait-Based Executor Dispatch

**What:** Define an `Executor` trait with a single async method. Each execution mode implements it. The agent main loop holds a `Box<dyn Executor>` per service and dispatches without knowing the mode.

**When to use:** Always -- this is the core abstraction.

**Trade-offs:** Dynamic dispatch via `Box<dyn Executor>` adds one vtable indirection per task execution (negligible cost given executors do I/O). The alternative -- an enum with match arms -- is simpler but forces all executor logic into one file and makes adding modes require touching the enum. Trait objects win here because modes are configured at startup and never change at runtime.

```rust
use async_trait::async_trait;
use xgent_proto::TaskAssignment;

/// Result of executing a task -- success with bytes or failure with error message.
pub struct ExecutionResult {
    pub success: bool,
    pub result: Vec<u8>,
    pub error_message: String,
}

#[async_trait]
pub trait Executor: Send + Sync {
    /// Execute a task and return the result. Must not panic.
    async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult;
}
```

**Why `async_trait` and not native async traits:** As of Rust stable 1.85+, async fn in traits is stable but does not support `dyn` dispatch (no `dyn AsyncTrait`). The `async_trait` crate desugars to `Pin<Box<dyn Future>>` which enables `Box<dyn Executor>`. Once `dyn async Trait` stabilizes (tracking rust-lang/rust#133119), migrate away from the proc macro.

### Pattern 2: Config-Driven Executor Construction (Factory)

**What:** A factory function reads the `ServiceConfig` and returns the correct executor instance, fully configured. The agent main loop never parses mode-specific config.

**When to use:** At agent startup, after TOML is loaded and validated.

**Trade-offs:** Validation happens once at startup. If the config is invalid, the agent fails fast with a clear error. The downside is config changes require a restart (acceptable -- the agent is a long-running daemon, not a hot-reloadable service).

```rust
pub fn executor_for_service(
    service_cfg: &ServiceConfig,
    http_client: &reqwest::Client,
) -> Result<Box<dyn Executor>, AgentConfigError> {
    match service_cfg.mode.as_str() {
        "cli" => {
            let cli_cfg = service_cfg.cli.as_ref()
                .ok_or(AgentConfigError::MissingSection("cli"))?;
            Ok(Box::new(CliExecutor::new(cli_cfg.clone())))
        }
        "sync-api" => {
            let api_cfg = service_cfg.sync_api.as_ref()
                .ok_or(AgentConfigError::MissingSection("sync_api"))?;
            Ok(Box::new(SyncApiExecutor::new(api_cfg.clone(), http_client.clone())))
        }
        "async-api" => {
            let async_cfg = service_cfg.async_api.as_ref()
                .ok_or(AgentConfigError::MissingSection("async_api"))?;
            Ok(Box::new(AsyncApiExecutor::new(async_cfg.clone(), http_client.clone())))
        }
        other => Err(AgentConfigError::UnknownMode(other.to_string())),
    }
}
```

### Pattern 3: Placeholder Resolution as a Standalone Pass

**What:** Template strings like `<payload>`, `<task_id>`, `<metadata.key>`, `${ENV_VAR}` are resolved in a single pass before being handed to the executor. Output placeholders like `<stdout>`, `<stderr>` are resolved after execution.

**When to use:** Two phases -- input placeholders before execution, output placeholders after.

**Trade-offs:** A single-pass resolver is simple and debuggable. The alternative (lazy resolution) adds complexity for no benefit since all values are known at resolution time. Unknown placeholders produce hard errors at resolution time rather than silently passing through.

```rust
/// Resolve placeholders in a template string.
/// Input phase: <payload>, <payload_json>, <task_id>, <metadata.KEY>, ${ENV_VAR}
/// Output phase: <stdout>, <stderr>, <exit_code>
pub fn resolve(
    template: &str,
    context: &PlaceholderContext,
) -> Result<String, PlaceholderError> {
    // Simple approach: no regex needed.
    // 1. Scan for ${...} -- env var interpolation
    // 2. Scan for <...> -- placeholder resolution
    // Use iterative str::find + str::replace approach
    // Unknown placeholders are errors (fail fast)
}

pub struct PlaceholderContext {
    pub task_id: String,
    pub payload: Vec<u8>,
    pub payload_utf8: Option<String>,
    pub metadata: HashMap<String, String>,
    // Post-execution fields (None before execution)
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub exit_code: Option<i32>,
    pub response_body: Option<Vec<u8>>,
    pub job_id: Option<String>,
}
```

**Implementation note:** Avoid the `regex` crate for this. The placeholder syntax is simple enough (`<word>` and `${WORD}`) that iterative `str::find` with manual parsing is faster, has no dependency, and is easier to debug. Only add regex if placeholder syntax grows complex.

## Data Flow

### Task Execution Flow (All Modes)

```
Gateway gRPC Stream
    |
    v
TaskAssignment { task_id, payload, metadata }
    |
    v
Build PlaceholderContext (input phase: payload, task_id, metadata, env vars)
    |
    v
executor.execute(&assignment) --> internally uses PlaceholderContext
    |                              to resolve templates
    v
ExecutionResult { success, result, error_message }
    |
    v
(Optional) ResponseMapper applies response_template to shape result bytes
    |
    v
ReportResultRequest --> gRPC ReportResult RPC --> Gateway
```

### CLI Mode Data Flow

```
PlaceholderContext resolves args: ["--input", "<payload>"] --> ["--input", "actual data"]
    |
    v
tokio::process::Command::new(program)
    .args(resolved_args)
    .stdin(if stdin_pipe: payload bytes)
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    |
    v
Wait for exit (with configurable timeout)
    |
    v
Capture stdout, stderr, exit_code
    |
    v
Resolve response_template with <stdout>, <stderr>, <exit_code>
    |
    v
ExecutionResult { success: exit_code == 0, result: resolved_template_bytes }
```

### Sync API Data Flow

```
PlaceholderContext resolves:
  - url: "http://localhost:8090/process/<task_id>"
  - body: "{\"data\": \"<payload>\"}"
  - headers: {"X-Custom": "<metadata.priority>"}
    |
    v
reqwest::Client
    .request(method, resolved_url)
    .headers(resolved_headers)
    .body(resolved_body)
    .timeout(config.timeout)
    .send()
    |
    v
HTTP Response (status + body bytes)
    |
    v
ExecutionResult { success: status.is_success(), result: response_body }
```

### Async API Data Flow

```
Phase 1: Submit
    PlaceholderContext resolves submit URL, body, headers
        |
        v
    reqwest POST/PUT --> HTTP Response
        |
        v
    Extract job_id from response using job_id_path (JSON pointer)
        (serde_json::Value pointer navigation)

Phase 2: Poll
    Loop (bounded by interval + max_attempts + timeout):
        |
        v
    PlaceholderContext resolves poll URL with <job_id>
        |
        v
    reqwest GET --> HTTP Response
        |
        v
    Check completion_condition against response body
        (key_path "status" == expected value like "complete")
        |
        v
    If complete: extract result from configured result_path
    If still pending: sleep(poll_interval), continue loop
    If timeout/max_attempts exceeded: ExecutionResult { success: false }
```

### Async Polling vs gRPC Stream: No Conflict

The async-api poll loop runs **inside** `executor.execute()`, which is an async function called from within the gRPC stream's `select!` branch. The outer loop structure remains:

```rust
loop {
    tokio::select! {
        _ = &mut shutdown => { /* drain */ }
        msg = stream.message() => {
            // TaskAssignment received
            let result = executor.execute(&assignment).await;  // may poll internally
            report_client.report_result(result).await;
        }
    }
}
```

The async-api executor's internal poll loop is a series of `reqwest` calls with `tokio::time::sleep` between them. Because the agent processes **one task at a time** (sequential within the stream), there is no concurrency conflict. The gRPC stream is server-side flow-controlled -- the gateway sends the next task only after the node reports the previous result via `ReportResult`.

**Key insight:** The agent does NOT need to poll the gRPC stream and the async-api target simultaneously. The gRPC stream blocks (no new assignment) while the executor runs. The `select!` only matters for shutdown signals during execution.

**Shutdown during long-running async-api polls:** The current `select!` structure does not cancel the executor if shutdown fires while `execute()` is running. This is intentional -- the graceful drain waits for the in-flight task to complete (via `in_flight_done.notified()`). For v1.2, this is acceptable. If cancellation during execution is needed later, wrap `execute()` in a `tokio::select!` with a cancellation token.

If future requirements demand concurrent task execution, the executor call moves into a `tokio::spawn` with a semaphore for concurrency control. That is out of scope for v1.2.

## TOML Config Structure

### Agent Config Design

```toml
# agent.toml

[gateway]
addr = "localhost:50051"
token = "${XGENT_NODE_TOKEN}"    # env var interpolation
node_id = "node-001"             # optional, auto-generated UUIDv7 if absent
ca_cert = "/path/to/ca.pem"     # optional TLS
tls_skip_verify = false
max_reconnect_delay_secs = 30

[service]
name = "echo"
mode = "sync-api"                # "cli" | "sync-api" | "async-api"

# --- Mode-specific sections (only the one matching `mode` is required) ---

[service.sync_api]
url = "http://localhost:8090/execute"
method = "POST"
timeout_secs = 30
[service.sync_api.headers]
Content-Type = "application/octet-stream"
X-Task-Id = "<task_id>"

[service.cli]
program = "/usr/local/bin/convert"
args = ["-resize", "50%", "-"]
stdin_pipe = true
timeout_secs = 60
response_template = "<stdout>"
[service.cli.env]
MAGICK_HOME = "/usr/local"

[service.async_api]
submit_url = "http://ml-service:5000/predict"
submit_method = "POST"
submit_body = "{\"input\": \"<payload>\"}"
job_id_path = "job_id"
poll_url = "http://ml-service:5000/status/<job_id>"
poll_method = "GET"
poll_interval_secs = 2
poll_timeout_secs = 300
poll_max_attempts = 150
completion_path = "status"
completion_value = "complete"
result_path = "output"
[service.async_api.submit_headers]
Content-Type = "application/json"
Authorization = "Bearer ${ML_API_KEY}"
```

### Why `[service]` (singular) Not `[services.*]` (map)

The current agent connects as one node for one service. The gRPC `PollTasksRequest` takes a single `service_name`. The reconnect loop, drain logic, and in-flight tracking are all single-service.

Using `[service]` (singular) keeps the v1.2 scope tight:
- No need for multi-stream management
- No need for per-service reconnect state
- Config is simpler to validate (one mode, one executor)
- Users who need multiple services run multiple agent processes (standard practice for daemon-per-service)

The TOML structure can evolve to `[services.X]` in a future version if multi-service agents become necessary. The executor abstraction does not change -- only the config parsing and main loop grow.

### Rust Config Structs

```rust
#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub gateway: GatewayConnectionConfig,
    pub service: ServiceConfig,
}

#[derive(Debug, Deserialize)]
pub struct GatewayConnectionConfig {
    pub addr: String,
    pub token: String,       // supports ${ENV_VAR} interpolation
    pub node_id: Option<String>,
    pub ca_cert: Option<String>,
    #[serde(default)]
    pub tls_skip_verify: bool,
    #[serde(default = "default_max_reconnect_delay")]
    pub max_reconnect_delay_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct ServiceConfig {
    pub name: String,
    pub mode: String,  // "cli" | "sync-api" | "async-api"
    pub cli: Option<CliConfig>,
    pub sync_api: Option<SyncApiConfig>,
    pub async_api: Option<AsyncApiConfig>,
    pub response_template: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct CliConfig {
    pub program: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub stdin_pipe: bool,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    #[serde(default)]
    pub env: HashMap<String, String>,
    pub working_dir: Option<String>,
    pub response_template: Option<String>,
}

#[derive(Debug, Deserialize, Clone)]
pub struct SyncApiConfig {
    pub url: String,
    #[serde(default = "default_post")]
    pub method: String,
    #[serde(default)]
    pub headers: HashMap<String, String>,
    pub body: Option<String>,       // template string; if None, raw payload forwarded
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
}

#[derive(Debug, Deserialize, Clone)]
pub struct AsyncApiConfig {
    pub submit_url: String,
    #[serde(default = "default_post")]
    pub submit_method: String,
    #[serde(default)]
    pub submit_headers: HashMap<String, String>,
    pub submit_body: Option<String>,
    pub job_id_path: String,          // dot-separated path into JSON response
    pub poll_url: String,             // must contain <job_id> placeholder
    #[serde(default = "default_get")]
    pub poll_method: String,
    #[serde(default)]
    pub poll_headers: HashMap<String, String>,
    #[serde(default = "default_poll_interval")]
    pub poll_interval_secs: u64,
    #[serde(default = "default_poll_timeout")]
    pub poll_timeout_secs: u64,
    pub poll_max_attempts: Option<u64>,
    pub completion_path: String,      // dot-separated path to status field
    pub completion_value: String,     // value indicating completion
    pub result_path: Option<String>,  // dot-separated path to extract result
}
```

### Config Validation Rules (Fail-Fast at Startup)

1. `mode` must be one of `"cli"`, `"sync-api"`, `"async-api"`
2. The corresponding config section (`cli`, `sync_api`, `async_api`) must be present for the declared mode
3. `program` in CLI mode must be a non-empty string (PATH resolution happens at execution time)
4. `url` / `submit_url` / `poll_url` must be parseable as URLs after env var interpolation
5. `poll_url` in async-api mode must contain the `<job_id>` placeholder
6. `token` (or `${ENV_VAR}` resolving to a non-empty string) must be present
7. `timeout_secs` / `poll_timeout_secs` must be > 0

### Env Var Interpolation in Config

The `config` crate does not natively resolve `${ENV_VAR}` inside string values. Two approaches:

**Recommended: Post-parse resolution.** After deserializing the TOML, walk all string fields and resolve `${VAR}` patterns. This is explicit, testable, and does not require a custom deserializer.

```rust
fn resolve_env_vars(s: &str) -> Result<String, AgentConfigError> {
    // Find ${VAR_NAME} patterns, replace with std::env::var(VAR_NAME)
    // Error if env var not set (fail-fast, no silent empty strings)
}
```

**Alternative: Use the `config` crate's environment source.** Map TOML keys to `AGENT__GATEWAY__TOKEN` env vars. This works for top-level overrides but does not handle inline interpolation like `"Bearer ${ML_API_KEY}"` in header values.

**Use both:** The `config` crate's env source for full-key overrides (e.g., `AGENT__GATEWAY__ADDR=host:port`) and post-parse `${VAR}` resolution for inline interpolation in templates and headers.

## Integration Points

### What Changes in Existing Code

| File | Change | Scope |
|------|--------|-------|
| `gateway/src/bin/agent.rs` | Replace clap-only config with TOML loading via `--config` arg; replace `dispatch_task()` call with `executor.execute()`; retain reconnect/drain logic | **Major rewrite** of main + `run_poll_loop`; ~50% of lines change |
| `gateway/Cargo.toml` | Add `async-trait`; add `tokio` `process` feature; move `toml` from dev to normal deps | **Minor addition** |
| `gateway/src/lib.rs` | Add `pub mod agent;` | **One line** |

### What Does NOT Change

- Proto definitions -- `TaskAssignment`, `ReportResultRequest` unchanged
- Gateway-side code -- zero modifications needed
- The gRPC streaming contract -- agent still calls `PollTasks` and `ReportResult` identically
- TLS/auth handling -- token still goes in gRPC metadata
- Reconnect/backoff logic -- same outer loop pattern
- Graceful drain -- same `DrainNode` RPC + in-flight wait

### New Dependencies

| Dependency | Purpose | Already in Cargo.toml? |
|------------|---------|----------------------|
| `async-trait` | `dyn Executor` dispatch with async | No -- add |
| `toml` | Parse agent.toml | Yes (dev-dependencies) -- move to `[dependencies]` |
| `tokio` `process` feature | `Command` for CLI mode | No -- add feature flag to existing tokio dep |

**No `regex` needed.** The placeholder syntax (`<name>` and `${VAR}`) is simple enough for iterative `str::find`-based parsing. Adding regex for this would be overkill.

**`serde_json` already present** for JSON pointer navigation in async-api mode (extracting `job_id` and checking `completion_path` from response bodies).

## Anti-Patterns

### Anti-Pattern 1: Executor Holding gRPC Client Reference

**What people do:** Pass the `report_client` into the executor so it can report intermediate status updates.
**Why it is wrong:** Executors should be pure computation -- take input, return output. Mixing transport concerns makes testing require a gRPC mock. The gateway's task state machine expects a single terminal `ReportResult`, not intermediate updates.
**Do this instead:** Executor returns `ExecutionResult`. Main loop maps it to `ReportResultRequest` and sends via gRPC. If progress reporting is needed later, add it as a callback/channel parameter, not a gRPC client injection.

### Anti-Pattern 2: Generic Executor Over Task Types

**What people do:** Make the executor generic over the task payload type (e.g., `Executor<ImageTask>`, `Executor<TextTask>`).
**Why it is wrong:** The gateway treats payloads as opaque bytes. The agent should too. Typed executors create coupling between the agent binary and specific payload schemas, defeating the gateway's protocol-agnostic design.
**Do this instead:** All executors work with `&TaskAssignment` (opaque `Vec<u8>` payload + string metadata map). Payload interpretation is handled entirely by the placeholder templates and the target service.

### Anti-Pattern 3: Spawning Async-API Poll Loop as Background Task

**What people do:** For async-api mode, spawn the poll loop as a detached `tokio::spawn` and try to correlate results back later via a channel.
**Why it is wrong:** The agent processes one task at a time. A detached poll loop requires a result channel, complicates shutdown, and adds concurrency the gateway does not expect (gateway will not send a new task until `ReportResult` is called for the current one).
**Do this instead:** The async-api poll loop runs inline within `execute()`. It is an async function that happens to contain a loop with sleep. From the caller's perspective, it is just a slow `execute()` call.

### Anti-Pattern 4: Hot-Reloading Config

**What people do:** Watch the TOML file with `notify` crate and rebuild executors on change.
**Why it is wrong:** The agent registers as a node for a specific service. Config changes that affect which service is polled require a new gRPC stream and re-registration. Hot-reload adds file-watcher complexity and edge cases (partially-valid config, executor mid-execution) for near-zero operational benefit.
**Do this instead:** Restart the agent on config change. Use systemd/supervisord for automatic restart. Document this.

### Anti-Pattern 5: Using an Enum Instead of Trait for Executors

**What people do:** Define `enum ExecutorKind { Cli(CliExecutor), SyncApi(SyncApiExecutor), AsyncApi(AsyncApiExecutor) }` and match on every call.
**Why it is wrong for this case:** While enum dispatch avoids heap allocation, it couples all modes together. Adding a fourth mode requires modifying the enum and every match site. The performance difference (stack vs heap dispatch) is irrelevant when the executor does I/O (process spawn, HTTP requests).
**Do this instead:** `Box<dyn Executor>`. Constructed once at startup, dispatched many times. The trait boundary makes each mode independently testable and extensible.

## Suggested Build Order

Dependencies flow downward -- each step produces independently testable output.

| Step | Component | Depends On | Test Strategy |
|------|-----------|------------|---------------|
| 1 | `agent/placeholder.rs` -- template resolution engine | Nothing | Unit tests: resolve known placeholders, reject unknown, env var substitution, edge cases (empty payload, missing metadata key) |
| 2 | `agent/config.rs` -- TOML parsing + validation + env var interpolation | Step 1 (for resolving `${VAR}` in config values) | Unit tests: parse valid TOML, reject invalid mode, reject missing section, env var resolution |
| 3 | `agent/executor/mod.rs` -- trait + `ExecutionResult` + factory function | Nothing (type definitions only) | Compiles; factory tested in step 7 |
| 4 | `agent/executor/sync_api.rs` -- synchronous HTTP executor | Steps 1, 3 | Integration test: spin up a mock HTTP server (axum on random port), verify URL/header/body template resolution, success and error status handling |
| 5 | `agent/executor/cli.rs` -- CLI process executor | Steps 1, 3 | Integration test: execute `echo "hello"`, verify stdout capture; test stdin pipe with `cat`; test timeout with `sleep 999`; test non-zero exit code |
| 6 | `agent/executor/async_api.rs` -- async two-phase executor | Steps 1, 3 | Integration test: mock server with `/submit` (returns job_id) + `/status/<id>` (returns pending then complete); verify poll loop, timeout, max_attempts |
| 7 | `agent/response.rs` -- response body mapping | Step 1 | Unit tests: apply output template with `<stdout>`, `<stderr>`, `<exit_code>` context |
| 8 | `bin/agent.rs` rewrite -- TOML config + executor wiring | Steps 1-7 | End-to-end: start gateway + agent with agent.toml + mock service, submit task, verify result flows back |
| 9 | Example configs + documentation | Step 8 | Manual verification with example `agent.toml` files for each mode |

**Rationale for ordering:**
- Placeholder engine is a leaf dependency with zero I/O -- build and test it first with pure unit tests.
- Config depends on placeholder engine for env var resolution.
- Sync-api executor comes before CLI because it is closest to the existing `dispatch_task` (minimal conceptual risk, validates the trait pattern).
- CLI comes next because it adds process spawning (new capability, needs the `process` tokio feature).
- Async-api is last among executors because it has the most moving parts (two-phase, polling, timeout, JSON pointer extraction).
- The agent binary rewrite is last because it integrates everything and requires an end-to-end test environment (gateway + Redis + agent + mock target).

## Scaling Considerations

| Concern | Current (v1.2) | Future (if needed) |
|---------|-----------------|---------------------|
| Tasks per second per agent | Sequential (1 at a time) -- sufficient for most workloads | Add `concurrency` config option, use `tokio::spawn` + semaphore per executor call |
| Long-running async-api tasks | Blocks the agent from taking new tasks during polling | Acceptable for v1.2; future: spawn poll loop, accept new tasks on separate stream |
| Multiple services per agent | Run multiple agent processes | Future: `[services.*]` map, one `tokio::spawn` per service with own poll loop |
| Config complexity | Single TOML file | Future: TOML includes, or split per-service config files |

## Sources

- Existing codebase: `gateway/src/bin/agent.rs` -- current 317-line agent implementation
- Existing codebase: `gateway/src/config.rs` -- config loading pattern (same `config` crate, same layered approach)
- Existing codebase: `proto/src/gateway.proto` -- gRPC contract (unchanged by this work)
- Existing codebase: `gateway/Cargo.toml` -- dependency list, confirms `reqwest`, `tonic`, `serde_json` already available
- Rust `async-trait` crate -- standard pattern for dyn-dispatchable async traits until native `dyn async Trait` stabilizes
- Rust `tokio::process::Command` -- async child process execution, part of tokio with `process` feature flag
- Rust `serde_json::Value::pointer` -- JSON pointer navigation for extracting values from API responses

---
*Architecture research for: xgent-agent flexible execution engine*
*Researched: 2026-03-24*
