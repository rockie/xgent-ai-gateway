# Feature Research: Flexible Agent Execution Engine (v1.2)

**Domain:** Configurable task execution engine for pull-model gateway agent
**Researched:** 2026-03-24
**Confidence:** HIGH (patterns are well-established; implementation is Rust-specific but concepts are mature)

## Current State

The runner agent (`gateway/src/bin/agent.rs`) currently:
- Polls the gateway via gRPC server-streaming (`PollTasks`)
- Receives `TaskAssignment { task_id, payload (bytes), metadata (map) }`
- Hardcodes HTTP POST to a single `--dispatch-url` endpoint
- Forwards payload as body, metadata as `X-Meta-*` headers
- Reports result back via `ReportResult` RPC (success: response bytes, failure: error message)
- All config via CLI args (`--gateway-addr`, `--service-name`, `--dispatch-url`, etc.)

The v1.2 milestone replaces the hardcoded HTTP POST dispatch with a configurable execution engine supporting three modes: `cli`, `sync-api`, `async-api`.

## Feature Landscape

### Table Stakes (Users Expect These)

Features that any configurable task execution agent must have. Without these, the agent is not meaningfully more useful than the current hardcoded dispatch.

| Feature | Why Expected | Complexity | Notes |
|---------|--------------|------------|-------|
| **Per-service TOML config file** | Users need to define execution behavior per service without recompiling. A single `agent.toml` with `[[services]]` sections is the minimum viable config surface. | MEDIUM | Replaces CLI `--dispatch-url`. Must coexist with existing CLI args for gateway connection. Use the `config` crate (already in stack) for layered config. |
| **CLI execution mode: argument-based** | The most common pattern for invoking scripts/binaries. Command template with `<payload>` placeholder replaced by the task payload. Example: `python predict.py --input <payload>` | MEDIUM | Use `tokio::process::Command` with argument array (NOT shell). Placeholder replacement is simple string substitution. Must NOT invoke via shell to prevent injection. |
| **CLI execution mode: stdin pipe** | Many CLI tools (jq, python scripts, ML inference wrappers) expect input on stdin. Pattern: pipe payload bytes to stdin, capture stdout/stderr. | MEDIUM | Use `Command::stdin(Stdio::piped())`, write payload, drop stdin handle, `wait_with_output()`. Simpler and safer than arg-based for binary/large payloads. |
| **CLI execution timeout** | Long-running CLI processes must not block the agent forever. Configurable per-service timeout with process kill on expiry. | LOW | `tokio::time::timeout` wrapping `child.wait_with_output()`. On timeout, `child.kill().await`. Use `kill_on_drop(true)` as safety net. |
| **CLI exit code handling** | Exit code 0 = success, non-zero = failure. This is the universal contract. Agent must map exit codes to success/failure in `ReportResultRequest`. | LOW | `output.status.success()` for the boolean, `output.status.code()` for the value. Include exit code in error message on failure. |
| **CLI stdout/stderr capture** | Agent must capture stdout (result payload) and stderr (error detail) separately. Both needed for response mapping. | LOW | `Command::stdout(Stdio::piped()).stderr(Stdio::piped())`. `wait_with_output()` returns both. |
| **CLI response body template** | Configurable mapping of stdout/stderr into the result payload using `<stdout>` and `<stderr>` placeholders. Example: `{"status": "ok", "data": <stdout>}` or just raw `<stdout>`. | MEDIUM | Simple string replacement. Must handle case where stdout/stderr contain JSON (embed raw) vs plain text (quote as string). See "placeholder system" below. |
| **sync-api execution mode** | HTTP request to a synchronous API endpoint. Configurable URL, method, headers, body template. Response body = task result. This replaces the current hardcoded POST. | MEDIUM | Use existing `reqwest::Client`. Template the URL, headers, and body with `<payload>` and `${ENV_VAR}` substitutions. Return response body as result bytes. |
| **sync-api configurable method/headers** | Different APIs require different HTTP methods (POST, PUT) and auth headers (`Authorization: Bearer ${API_KEY}`). | LOW | Method enum in config. Headers as key-value pairs with env var interpolation. |
| **sync-api response mapping** | Extract specific fields from the API response body to construct the task result. JSON key-path extraction like `<response.data.output>`. | MEDIUM | Use `serde_json::Value::pointer` (RFC 6901 JSON Pointer) for extraction. Simpler and more predictable than full JSONPath. Already available via serde_json (no new dependency). |
| **async-api execution mode: submit phase** | POST to an async API, receive a job ID in the response. This is the standard async request-reply pattern (Microsoft Azure, AWS, most AI/ML APIs). | MEDIUM | Same HTTP machinery as sync-api for the submit call. Extract job ID from response using key-path (e.g., `<response.output.task_id>`). |
| **async-api execution mode: poll phase** | Poll a status endpoint at configurable interval until completion condition is met. URL template includes values extracted from submit response. | HIGH | Polling loop with interval, timeout, and completion condition evaluation. This is the most complex single feature. |
| **async-api completion condition** | Configurable condition to determine when async job is done. Check a key-path value against expected values (equal, not_equal, in, not_in). | MEDIUM | Extract value via JSON pointer, compare against configured target. Support `equal`, `not_equal`, `in` (value is one of a set), `not_in`. |
| **async-api timeout** | Overall timeout for the submit + poll cycle. Prevents infinite polling on stuck remote APIs. | LOW | Wrap entire async-api flow in `tokio::time::timeout`. |
| **Environment variable interpolation** | `${ENV_VAR}` in URLs, headers, and body templates resolved at task execution time. Essential for API keys and secrets that should not be in config files. | LOW | Simple regex or manual `${...}` scanning with `std::env::var()` lookup. Fail loudly if env var is missing. |
| **Placeholder system for `<payload>`** | The task payload (opaque bytes from the gateway) must be injectable into CLI args, HTTP body templates, and URL templates. `<payload>` is the universal placeholder. | LOW | String replacement. For CLI arg mode, the payload must be valid UTF-8 (or base64 encode). For HTTP body, embed raw. |
| **Error propagation to gateway** | All three modes must map failures to `ReportResultRequest { success: false, error_message, ... }`. Include meaningful context: HTTP status codes, CLI exit codes, timeout details, poll failures. | LOW | Already exists in current agent for HTTP dispatch errors. Extend the pattern to CLI and async-api modes. |

### Differentiators (Competitive Advantage)

Features that make this agent notably more useful than a simple webhook forwarder or custom scripts. Not required for v1.2 launch but high value-to-effort ratio.

| Feature | Value Proposition | Complexity | Notes |
|---------|-------------------|------------|-------|
| **Working directory per service** | CLI commands often need to run in a specific directory (e.g., where the Python virtual env lives). `cwd` config field per service. | LOW | `Command::current_dir()`. Single config field, trivial to implement. |
| **Environment variables per service** | Inject service-specific env vars into CLI processes beyond just template interpolation. Useful for `PYTHONPATH`, `CUDA_VISIBLE_DEVICES`, model paths. | LOW | `Command::env()` or `Command::envs()`. Map in config. |
| **Metadata-based placeholders** | Beyond `<payload>`, allow `<meta.key_name>` placeholders that resolve to task metadata values. Enables routing/parameterization based on submission metadata. | LOW | Task metadata is already `map<string, string>` in the proto. Simple lookup during template expansion. |
| **Config hot-reload** | Watch `agent.toml` for changes and reload service configs without restarting the agent. Useful in development and for adding new services. | MEDIUM | `notify` crate for filesystem watching. Swap `Arc<Config>` atomically. Must not affect in-flight tasks. |
| **Health check per service** | Before marking a service as ready, optionally run a health check command or HTTP probe. Prevents the agent from pulling tasks before its backends are ready. | MEDIUM | Optional `healthcheck` config field (URL or command). Run on startup and periodically. Stop polling if unhealthy. |
| **Retry on transient failure** | Retry CLI execution or HTTP calls on specific transient errors (connection refused, 502/503/504) before reporting failure to gateway. Configurable max retries and delay. | MEDIUM | The gateway itself does NOT retry (design decision D-07). But the agent retrying local execution before reporting failure is different -- it is retrying the local dispatch, not the task. This is a local concern. |
| **Dry-run mode** | Run the agent with `--dry-run` to validate config, print the resolved commands/URLs for a test payload, and exit. Useful for config debugging. | LOW | Parse config, run template expansion with a dummy payload, print results. No network or process execution. |
| **Multi-service support** | Single agent process serving multiple services simultaneously. Each service has its own execution config in `agent.toml`. | MEDIUM | Spawn separate poll loops per `[[services]]` entry. Each gets its own `tokio::spawn`. Already somewhat implied by the per-service config model. |
| **Structured logging with task context** | Include `task_id`, `service_name`, `execution_mode`, and `duration_ms` in every log line during task execution. Enables log correlation. | LOW | Already using `tracing`. Add span fields per task execution. |
| **Response body size limit** | Cap the result payload size before reporting back to the gateway. Prevents a runaway CLI dumping gigabytes of output into the gRPC response. | LOW | Truncate stdout/response body at configurable max bytes. Include `[truncated]` marker. |

### Anti-Features (Commonly Requested, Often Problematic)

Features that seem useful but should NOT be built for the v1.2 execution engine.

| Feature | Why Requested | Why Problematic | Alternative |
|---------|---------------|-----------------|-------------|
| **Shell execution mode** | "I want to run `cmd1 \| cmd2 > output.txt`" with shell pipes and redirects | Shell execution (invoking `/bin/sh -c "..."`) is the number one cause of command injection vulnerabilities. The OWASP command injection cheat sheet explicitly warns against this. Payloads from external clients flow into these commands -- any shell metacharacter (`;`, `\|`, `` ` ``, `$()`) becomes an attack vector. | Use argument array mode (safe, no shell) or stdin pipe mode (no injection risk). If users need shell pipes, they can wrap their pipeline in a script file and invoke that script via the CLI mode. |
| **Dynamic plugin loading** | "Let me write a Rust plugin that runs inside the agent" | PROJECT.md explicitly descopes dynamic loading (.so/.dylib). Adds unsafe code, ABI compatibility nightmares, and security risks. The agent becomes an attack surface. | The three execution modes (cli, sync-api, async-api) cover all practical dispatch patterns. Users write their logic in any language and expose it as a CLI tool or HTTP service. |
| **Full Jinja2/Handlebars template engine** | "I need conditionals, loops, and filters in my templates" | Massive dependency for what is fundamentally string replacement. Tera/MiniJinja/Handlebars add 50-200KB to the binary and introduce a DSL users must learn. The template system here needs `<payload>`, `<stdout>`, `<stderr>`, `<meta.key>`, `<response.path.to.value>`, and `${ENV_VAR}`. That is 6 placeholder types -- string replacement, not a template engine. | Simple custom placeholder expansion. Regex-free, predictable, zero dependencies. If a user needs conditional logic, they put it in their CLI script or API handler. |
| **Parallel task execution per service** | "Let one agent process multiple tasks concurrently for the same service" | The gateway's streaming model sends one task at a time per poll stream -- the agent processes and reports before receiving the next. Parallelism is achieved by running multiple agent instances (horizontal scaling), which is simpler and gives isolation between tasks. Concurrent execution within one agent adds shared state, resource contention, and complex error handling. | Run multiple agent instances. Each polls independently and gets its own tasks from the gateway's queue. This is the intended scaling model. |
| **Custom result transformers in Lua/WASM** | "I need to transform the result with custom code before reporting" | Embedding a scripting runtime adds massive complexity, security concerns, and a learning curve for users. Response body templates with JSON key-path extraction handle 95% of transformation needs. | Use response body template with `<stdout>`, `<response.key.path>`, and `<stderr>` placeholders. For complex transformations, add a post-processing step in the CLI script or API handler. |
| **Payload file staging** | "Write the payload to a temp file and pass the file path to the CLI" | Adds temp file lifecycle management, cleanup on failure, disk space concerns, and path escaping issues. Most CLIs that need file input also accept stdin. | Use stdin pipe mode. If the CLI truly requires a file, the user's wrapper script can write stdin to a temp file itself. |
| **Webhook callback from agent** | "Let the agent call a webhook when a task completes" | The gateway already has a callback mechanism (optional `callback_url` on task submission with exponential backoff). The agent reports to the gateway, and the gateway handles callbacks. Duplicating this in the agent creates two callback paths. | Use the gateway's built-in callback feature. The agent's job ends at `ReportResult`. |

## Feature Dependencies

```
[Per-service TOML config]
    |
    +-- enables --> [CLI execution mode]
    |                   |
    |                   +-- requires --> [Placeholder system (<payload>)]
    |                   +-- requires --> [CLI timeout]
    |                   +-- requires --> [CLI exit code handling]
    |                   +-- requires --> [CLI stdout/stderr capture]
    |                   +-- enables  --> [CLI response body template]
    |
    +-- enables --> [sync-api execution mode]
    |                   |
    |                   +-- requires --> [Placeholder system (<payload>)]
    |                   +-- requires --> [Env var interpolation (${ENV})]
    |                   +-- requires --> [sync-api method/headers config]
    |                   +-- enables  --> [sync-api response mapping]
    |
    +-- enables --> [async-api execution mode]
                        |
                        +-- requires --> [sync-api execution mode] (submit phase reuses HTTP machinery)
                        +-- requires --> [JSON key-path extraction] (extract job ID from submit response)
                        +-- requires --> [async-api completion condition]
                        +-- requires --> [async-api timeout]

[Placeholder system]
    +-- <payload>   -- base, all modes
    +-- <stdout>    -- CLI mode only
    +-- <stderr>    -- CLI mode only
    +-- <meta.key>  -- all modes (differentiator)
    +-- <response.path> -- sync-api and async-api response mapping
    +-- ${ENV_VAR}  -- all modes, resolved from environment

[JSON key-path extraction]
    +-- used by --> [sync-api response mapping]
    +-- used by --> [async-api job ID extraction]
    +-- used by --> [async-api completion condition]
    +-- used by --> [async-api poll URL template]
```

### Dependency Notes

- **async-api depends on sync-api HTTP machinery:** The submit phase of async-api is identical to a sync-api call. Build sync-api first, then extend with poll loop for async-api. Do not build them in parallel.
- **JSON key-path extraction is shared:** Used by sync-api response mapping AND async-api job ID/status extraction. Build it as a standalone utility function, not embedded in either mode. Use `serde_json::Value::pointer()` (RFC 6901 JSON Pointer notation like `/output/task_id`).
- **Placeholder system is foundational:** All three modes depend on it. Build and test it first. It is simple string substitution -- do not over-engineer it.
- **CLI response template conflicts with raw stdout:** If response template is set, apply it. If not, return raw stdout as result bytes. The "no template = passthrough" default is important for simple CLI tools.

## MVP Definition

### Launch With (v1.2)

The minimum set to make the agent a genuinely configurable execution engine.

- [ ] **Per-service TOML config** -- `agent.toml` with `[[services]]` array, each entry specifying `name`, `type` (cli/sync-api/async-api), and mode-specific `[services.settings]`
- [ ] **Placeholder system** -- `<payload>`, `<stdout>`, `<stderr>`, `${ENV_VAR}` substitution in all string fields
- [ ] **CLI execution: arg mode** -- Split command string on whitespace, replace `<payload>` in args, exec via `tokio::process::Command` (no shell)
- [ ] **CLI execution: stdin pipe** -- Pipe payload bytes to child stdin, capture stdout/stderr
- [ ] **CLI timeout + exit code** -- Per-service configurable timeout, kill on expiry, exit code to success/failure mapping
- [ ] **CLI response body template** -- `success.body` and `failed.body` templates with `<stdout>` and `<stderr>` placeholders
- [ ] **sync-api mode** -- Configurable URL, method, headers (with `${ENV}` interpolation), body template with `<payload>`
- [ ] **sync-api response mapping** -- JSON key-path extraction for constructing result payload from response
- [ ] **async-api mode** -- Submit phase (POST, extract job ID from response), poll phase (GET at interval, check completion condition)
- [ ] **async-api completion condition** -- Key-path value comparison (equal, not_equal, in, not_in) and overall timeout
- [ ] **async-api response mapping** -- Construct result payload from final poll response using key-path extraction
- [ ] **Error propagation** -- All modes map failures to `ReportResultRequest` with meaningful error messages
- [ ] **Example service configs** -- One example per mode (cli echo, sync-api echo, async-api mock)
- [ ] **Node.js client example** -- End-to-end: submit task via HTTP, agent executes via each mode, poll result

### Add After Validation (v1.2.x)

Features to add once the core execution engine is in daily use.

- [ ] **Multi-service support** -- Single agent process polling multiple services concurrently
- [ ] **Working directory per service** -- `cwd` config field
- [ ] **Environment variables per service** -- `env` map in config, injected into CLI processes
- [ ] **Metadata placeholders** -- `<meta.key>` expansion in templates
- [ ] **Dry-run mode** -- `--dry-run` flag for config validation
- [ ] **Response body size limit** -- Truncate large outputs before reporting

### Future Consideration (v2+)

- [ ] **Config hot-reload** -- Watch `agent.toml` for changes without restart
- [ ] **Health check per service** -- Pre-flight readiness probe before pulling tasks
- [ ] **Retry on transient failure** -- Local retry before reporting failure to gateway
- [ ] **Structured execution metrics** -- Per-service execution duration, success rate, reported via tracing

## Feature Prioritization Matrix

| Feature | User Value | Implementation Cost | Priority | Dependencies |
|---------|------------|---------------------|----------|-------------|
| Per-service TOML config | HIGH | MEDIUM | P1 | None (foundational) |
| Placeholder system | HIGH | LOW | P1 | None (foundational) |
| CLI arg mode | HIGH | MEDIUM | P1 | Placeholder system |
| CLI stdin pipe | HIGH | MEDIUM | P1 | Placeholder system |
| CLI timeout + exit code | HIGH | LOW | P1 | CLI modes |
| CLI response template | HIGH | LOW | P1 | Placeholder system, CLI modes |
| sync-api mode | HIGH | MEDIUM | P1 | Placeholder system, env var interpolation |
| sync-api response mapping | MEDIUM | MEDIUM | P1 | JSON key-path extraction |
| async-api mode | HIGH | HIGH | P1 | sync-api mode, JSON key-path extraction |
| async-api completion condition | HIGH | MEDIUM | P1 | JSON key-path extraction |
| Env var interpolation | HIGH | LOW | P1 | None |
| JSON key-path extraction | HIGH | LOW | P1 | None (use serde_json::Value::pointer) |
| Example configs | MEDIUM | LOW | P1 | All execution modes |
| Node.js client example | MEDIUM | LOW | P1 | All execution modes |
| Multi-service support | MEDIUM | MEDIUM | P2 | Per-service config |
| Working dir per service | MEDIUM | LOW | P2 | CLI mode |
| Env vars per service | MEDIUM | LOW | P2 | CLI mode |
| Metadata placeholders | MEDIUM | LOW | P2 | Placeholder system |
| Dry-run mode | MEDIUM | LOW | P2 | Config parsing |
| Response size limit | LOW | LOW | P2 | All modes |
| Config hot-reload | LOW | MEDIUM | P3 | Per-service config |
| Health check | LOW | MEDIUM | P3 | Per-service config |
| Retry on transient failure | LOW | MEDIUM | P3 | All modes |

**Priority key:**
- P1: Must have for v1.2 launch
- P2: Should have, add in v1.2.x patches
- P3: Nice to have, future consideration

## Config Schema Design (Reference)

This is the target config structure based on the DRAFT.md reference and feature requirements:

```toml
# agent.toml

# Gateway connection (existing CLI args move here, CLI args still override)
[gateway]
addr = "localhost:50051"
# token via XGENT_NODE_TOKEN env var (not in file)
# ca_cert, tls_skip_verify from CLI args

[[services]]
name = "text-processor"
node_id = "auto"  # "auto" = generate UUID v7

type = "cli"
[services.settings]
cmd = "python process.py --input <payload>"
# OR for stdin mode:
# cmd = "python process.py"
# stdin = true
timeout_secs = 30

[services.settings.success]
body = "<stdout>"

[services.settings.failed]
body = '{"error": "<stderr>", "exit_code": "<exit_code>"}'


[[services]]
name = "image-api"

type = "sync-api"
[services.settings]
url = "http://localhost:8090/api/generate"
method = "POST"
body = "<payload>"
timeout_secs = 60

[services.settings.headers]
Authorization = "Bearer ${IMAGE_API_KEY}"
Content-Type = "application/json"

[services.settings.success]
body = "<response.data.output>"

[services.settings.failed]
body = "<response.error.message>"


[[services]]
name = "video-synthesis"

type = "async-api"
[services.settings.submit]
url = "http://localhost:8080/api/v1/video/generate"
method = "POST"
body = "<payload>"

[services.settings.submit.headers]
Authorization = "Bearer ${VIDEO_API_KEY}"

[services.settings.poll]
url = "http://localhost:8080/api/v1/tasks/<submit_response.output.task_id>"
method = "GET"
interval_secs = 2
timeout_secs = 300

[services.settings.poll.headers]
Authorization = "Bearer ${VIDEO_API_KEY}"

[services.settings.poll.completed_when]
key = "<poll_response.output.task_status>"
condition = "equal"  # equal, not_equal, in, not_in
value = "SUCCEEDED"
# For "in"/"not_in": values = ["SUCCEEDED", "FAILED"]

[services.settings.poll.failed_when]
key = "<poll_response.output.task_status>"
condition = "in"
values = ["FAILED", "ERROR", "CANCELLED"]

[services.settings.success]
body = "<poll_response.output>"

[services.settings.failed]
body = "<poll_response.error>"
```

## Placeholder System Design

Six placeholder types, all resolved by simple string scanning and replacement:

| Placeholder | Scope | Resolved From | Example |
|-------------|-------|---------------|---------|
| `<payload>` | All modes | `TaskAssignment.payload` bytes (UTF-8 decoded) | `cmd = "echo <payload>"` |
| `<stdout>` | CLI only | Child process stdout capture | `body = "<stdout>"` |
| `<stderr>` | CLI only | Child process stderr capture | `body = '{"error": "<stderr>"}'` |
| `<exit_code>` | CLI only | Child process exit code | `body = '{"code": <exit_code>}'` |
| `<response.path.to.value>` | sync-api, async-api | JSON Pointer on HTTP response body | `body = "<response.data.url>"` |
| `<submit_response.path>` | async-api poll config | JSON Pointer on submit phase response | `url = ".../tasks/<submit_response.id>"` |
| `<poll_response.path>` | async-api result | JSON Pointer on final poll response | `body = "<poll_response.output>"` |
| `<meta.key_name>` | All modes (P2) | `TaskAssignment.metadata` map lookup | `url = "http://host/<meta.model>"` |
| `${ENV_VAR}` | All string fields | `std::env::var("ENV_VAR")` | `Authorization = "Bearer ${API_KEY}"` |

**Implementation approach:** Do NOT use a template engine library. Scan for `<...>` and `${...}` patterns, look up the value, replace. This is ~50 lines of Rust code. A full template engine (Tera, MiniJinja, Handlebars) would add 50-200KB to the binary for functionality we do not need.

**JSON Pointer mapping:** `<response.data.output>` maps to serde_json's `Value::pointer("/data/output")`. Convert dot notation to slash notation internally: split on `.`, skip the prefix (`response`, `submit_response`, `poll_response`), join with `/`, prepend `/`.

## Competitor/Reference Analysis

| Feature | GA4GH TES API | n8n Task Runners | Temporal Workers | Our Approach |
|---------|---------------|-------------------|------------------|--------------|
| Execution modes | Container-based only (Docker image + command) | JavaScript sandbox with Node.js runner | Language-specific SDK (Go, Java, Python, etc.) | CLI process + HTTP API (language-agnostic, no container requirement) |
| Command specification | Array of executors with `command[]`, `stdin`, `stdout`, `stderr` paths | Predefined node types with JSON config | Activity functions registered in code | TOML config with placeholder templates |
| Input passing | File staging to container filesystem | JSON data through task broker | Protobuf/JSON via SDK | Opaque payload bytes via `<payload>` placeholder |
| Output extraction | File output paths + exit code | Return value from JS execution | Return value from activity function | stdout/stderr capture (CLI) or JSON key-path (API) |
| Async support | Inherent (all tasks are async with status polling) | Built-in task queue with status tracking | Built-in workflow state machine | async-api mode with configurable submit/poll/completion |
| Config format | OpenAPI/JSON task definitions | Visual workflow editor | Code-defined workflows | TOML per-service config |
| Security model | Container isolation | Sandboxed JS runtime | mTLS between worker and server | Process isolation (CLI) + env var secrets (API) |

**Key takeaway:** The TES API's sequential executor model with stdin/stdout/stderr mapping validates our CLI mode design. The approach of opaque payload + configurable extraction is the right level of abstraction -- more flexible than TES (which requires containers), simpler than Temporal (which requires SDK integration).

## Sources

- [Tokio process module documentation](https://docs.rs/tokio/latest/tokio/process/index.html) -- Command, Child, stdin/stdout/stderr piping
- [Tokio process::Command documentation](https://docs.rs/tokio/latest/tokio/process/struct.Command.html) -- kill_on_drop, env, current_dir
- [OWASP Command Injection Defense](https://cheatsheetseries.owasp.org/cheatsheets/OS_Command_Injection_Defense_Cheat_Sheet.html) -- argument array over shell, input validation
- [Shell Injection (matklad)](https://matklad.github.io/2021/07/30/shell-injection.html) -- why shell execution is dangerous, argument array pattern
- [Microsoft Asynchronous Request-Reply Pattern](https://learn.microsoft.com/en-us/azure/architecture/patterns/asynchronous-request-reply) -- async API polling pattern reference
- [Zuplo: Async Operations in REST APIs](https://zuplo.com/learning-center/asynchronous-operations-in-rest-apis-managing-long-running-tasks) -- submit/poll/completion patterns
- [serde_json_path on crates.io](https://crates.io/crates/serde_json_path) -- RFC 9535 JSONPath (considered, decided against -- serde_json::Value::pointer is simpler)
- [serde_json::Value::pointer](https://docs.rs/serde_json/latest/serde_json/) -- RFC 6901 JSON Pointer built into serde_json
- [GA4GH Task Execution Service](https://ga4gh.github.io/task-execution-schemas/docs/) -- executor specification with command array, stdin/stdout mapping
- [command_timeout crate](https://crates.io/crates/command_timeout) -- advanced timeout with inactivity detection (reference, not recommended for use)
- [Rust template engine list on lib.rs](https://lib.rs/template-engine) -- Tera, MiniJinja, Handlebars comparison (decided against full template engines)
- [Tokio forum: spawn process with timeout](https://users.rust-lang.org/t/spawn-process-with-timeout-and-capture-output-in-tokio/128305) -- community patterns for process timeout
- DRAFT.md (project reference) -- Owner's vision for CLI and async-api config structure

---
*Feature research for: Flexible Agent Execution Engine (v1.2 milestone)*
*Researched: 2026-03-24*
