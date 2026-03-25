# Phase 13: Config, Placeholders, and CLI Execution - Research

**Researched:** 2026-03-24
**Domain:** Rust agent config parsing (YAML), placeholder template resolution, child process management (tokio::process)
**Confidence:** HIGH

## Summary

This phase replaces the hardcoded HTTP POST dispatch in `agent.rs` with a configurable execution engine driven by a YAML config file (`agent.yaml`). The scope covers three interconnected systems: (1) YAML config loading with `${ENV_VAR}` interpolation at startup, (2) a placeholder template engine that resolves `<payload>`, `<stdout>`, `<stderr>`, `<metadata.key>`, and `<service_name>` tokens in command and response templates, and (3) a CLI executor that runs child processes in `arg` or `stdin` mode with timeout enforcement and output size limits.

The existing agent code (317 LOC in `bin/agent.rs`) has a clean separation point: `dispatch_task()` is the only function that needs replacement. The poll loop, reconnection logic, graceful drain, and result reporting all remain untouched. The refactoring moves agent logic into `gateway/src/agent/` modules while `bin/agent.rs` stays as the thin entrypoint.

**Primary recommendation:** Use `serde_yaml_ng` (maintained fork of deprecated `serde_yaml`), `async-trait 0.1.89` for the `Executor` trait, and `tokio::process::Command` with concurrent `tokio::spawn` tasks for stdin/stdout/stderr I/O to prevent pipe deadlocks.

<user_constraints>
## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Use YAML (`agent.yaml`) instead of TOML -- tree-structured dispatch config is more readable in YAML. Breaks naming convention with `gateway.toml` but dispatch config benefits significantly.
- **D-02:** Add `serde_yaml` dependency for YAML parsing.
- **D-03:** Top-level `mode: cli` field determines execution mode. Only the active mode's section is required.
- **D-04:** CLI args dropped -- only `--config <path>` and `--dry-run` flags. All config in YAML. Env vars work via `${ENV_VAR}` in YAML values.
- **D-05:** Three core task variables aligned with the API contract: `<service_name>`, `<payload>`, `<metadata.key>` (dot-access into task metadata map).
- **D-06:** Execution-specific variables: `<stdout>`, `<stderr>` (CLI mode); `<response.path>` (sync-api, Phase 14); `<submit_response.path>`, `<poll_response.path>` (async-api, Phase 15).
- **D-07:** Environment variable interpolation uses `${ENV_VAR}` syntax, resolved once at config load time. Missing env var = agent fails to start.
- **D-08:** Unresolved task placeholders (e.g., `<metadata.missing_key>`) fail the task with a clear error listing the unresolved token and available keys.
- **D-09:** Single-pass resolution only -- no recursive/nested placeholder expansion. Resolved values containing `<token>` syntax are NOT re-resolved. This prevents injection from untrusted payload/stdout data.
- **D-10:** Two CLI input modes: `arg` (payload substituted into command template) and `stdin` (payload piped to process stdin).
- **D-11:** Stdin mode uses concurrent tokio tasks: one writes payload to stdin (then closes), one reads stdout, one reads stderr. Prevents deadlock when pipe buffers fill.
- **D-12:** Timeout enforcement: SIGKILL immediately on expiry via `child.kill()`. No SIGTERM grace period. Simple and reliable.
- **D-13:** Output exceeding `max_bytes` fails the task (not truncated). Error includes actual size and configured limit.
- **D-14:** Exit code 0 = success, non-zero = failure with exit code in error message.
- **D-15:** Refactor `agent.rs` into modules: `gateway/src/agent/{mod.rs, config.rs, executor.rs, cli_executor.rs, placeholder.rs, response.rs}`. `bin/agent.rs` remains as the main entrypoint.
- **D-16:** `Executor` trait using `async_trait` for `Box<dyn Executor>` (per STATE.md decision -- re-check native dyn async traits at implementation time).
- **D-17:** Phase 14 adds `sync_api_executor.rs`, Phase 15 adds `async_api_executor.rs` to the same module structure.
- **D-18:** Shared `response` section across all modes with `body` template and `max_bytes` limit.
- **D-19:** `body` template is a string with placeholder tokens. Not a structured YAML object -- the template itself produces the result shape.

### Claude's Discretion
- Exact YAML config struct field names and serde attributes
- Placeholder regex pattern implementation details
- Error message formatting
- Test strategy and test fixture structure
- Whether to use `tokio::process::Command` directly or wrap it

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope
</user_constraints>

<phase_requirements>
## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CFG-01 | Agent reads per-service execution config from agent.yaml file | serde_yaml_ng for YAML parsing, custom env var interpolation pre-parse |
| CFG-02 | Placeholder system resolves `<payload>`, `<stdout>`, `<stderr>`, `<response.path>`, `<submit_response.path>`, `<poll_response.path>` tokens in templates | Simple regex-based single-pass string replacement; only `<payload>`, `<stdout>`, `<stderr>` for this phase |
| CFG-03 | Environment variable interpolation resolves `${ENV_VAR}` in URLs, headers, and body templates | Regex `\$\{([^}]+)\}` applied once at config load time; fail-fast on missing vars |
| CFG-04 | Metadata placeholders resolve `<metadata.key>` to task metadata values | Dot-access pattern parsed from `<metadata.(.+?)>` regex, looked up in `TaskAssignment.metadata` HashMap |
| CFG-05 | Per-service working directory (cwd) config for CLI processes | `tokio::process::Command::current_dir()` |
| CFG-06 | Per-service environment variables injected into CLI processes | `tokio::process::Command::env()` / `envs()` |
| CLI-01 | Agent executes CLI commands in arg mode with `<payload>` replaced in command template | Resolve placeholders in command string, split into program + args, spawn process |
| CLI-02 | Agent executes CLI commands in stdin mode, piping payload to process stdin | `Stdio::piped()` for stdin, concurrent tokio tasks for write/read |
| CLI-03 | Configurable per-service timeout kills process on expiry (kill_on_drop safety) | `tokio::time::timeout` wrapping process execution, `child.kill()` on timeout |
| CLI-04 | Exit code 0 maps to success, non-zero maps to failure with exit code in error | `child.wait()` status code inspection |
| CLI-05 | Response body template maps `<stdout>` and `<stderr>` into configurable result shape | Placeholder resolution applied to response body template post-execution |
| SAFE-01 | Response body size limit caps result payload to prevent runaway output | Check `stdout.len() + stderr.len()` against `max_bytes` before template resolution |
</phase_requirements>

## Standard Stack

### Core (New Dependencies)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| serde_yaml_ng | 0.10.0 | YAML parsing for agent.yaml | Maintained fork of deprecated serde_yaml (archived March 2024). Community consensus replacement. Drop-in compatible API. |
| async-trait | 0.1.89 | `Executor` trait with async methods | Native `async fn in dyn Trait` still unstable (rust-lang/rust#133119). async-trait is the standard solution and will remain so for the foreseeable future. |

### Already in Cargo.toml (Reused)
| Library | Version | Purpose | Notes |
|---------|---------|---------|-------|
| tokio | 1.50 | Process spawning, timeout, I/O | `tokio::process::Command`, `tokio::time::timeout` -- already a dependency |
| serde | 1.0 | Config struct derive | Already in Cargo.toml with `derive` feature |
| clap | 4.6 | Minimal `--config` and `--dry-run` flags | Already a dependency; agent CLI struct gets simplified |
| tracing | 0.1 | Structured logging | Already in use throughout agent code |

### Critical Note: serde_yaml is Deprecated
D-02 in CONTEXT.md says "Add `serde_yaml` dependency" but `serde_yaml 0.9.34` was deprecated and archived in March 2024. **Use `serde_yaml_ng 0.10.0` instead** -- it is the community-recommended maintained fork with an identical API. The planner MUST use `serde_yaml_ng`, not `serde_yaml`.

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| serde_yaml_ng | serde_yml 0.0.12 | Community warns against serde_yml (quality concerns). serde_yaml_ng is the consensus choice. |
| serde_yaml_ng | yaml_serde 0.10.4 | Maintained by "The YAML Organization" -- less community adoption than serde_yaml_ng. |
| async-trait | Native dyn async traits | Still unstable behind `#![feature(async_fn_in_dyn_trait)]`. Not usable on stable Rust. |

**Installation:**
```toml
# Add to gateway/Cargo.toml [dependencies]
serde_yaml_ng = "0.10"
async-trait = "0.1"
```

## Architecture Patterns

### Recommended Module Structure
```
gateway/src/
├── agent/
│   ├── mod.rs              # Re-exports, Executor trait definition
│   ├── config.rs           # AgentConfig struct, YAML loading, env var interpolation
│   ├── executor.rs         # Executor trait, ExecutionResult type
│   ├── cli_executor.rs     # CliExecutor: arg mode, stdin mode, timeout, output limit
│   ├── placeholder.rs      # Placeholder resolution engine (single-pass, regex-based)
│   └── response.rs         # Response body template resolution
├── bin/
│   └── agent.rs            # Thin entrypoint: parse --config, load YAML, build executor, run poll loop
└── lib.rs                  # Add `pub mod agent;`
```

### Pattern 1: Executor Trait with async_trait
**What:** Trait object for polymorphic execution modes (CLI now, sync-api Phase 14, async-api Phase 15)
**When to use:** Always -- the trait allows `run_poll_loop` to be mode-agnostic
**Example:**
```rust
use async_trait::async_trait;

pub struct ExecutionResult {
    pub success: bool,
    pub result: Vec<u8>,
    pub error_message: String,
}

#[async_trait]
pub trait Executor: Send + Sync {
    async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult;
}
```

### Pattern 2: Env Var Interpolation at Config Load Time
**What:** Resolve `${ENV_VAR}` in the raw YAML string before parsing into structs
**When to use:** Config loading -- D-07 requires resolution once at startup
**Example:**
```rust
use std::env;

fn interpolate_env_vars(raw: &str) -> Result<String, String> {
    let re = regex::Regex::new(r"\$\{([^}]+)\}").unwrap();
    let mut result = raw.to_string();
    for cap in re.captures_iter(raw) {
        let var_name = &cap[1];
        let value = env::var(var_name)
            .map_err(|_| format!("missing environment variable: ${{{}}}", var_name))?;
        result = result.replace(&cap[0], &value);
    }
    Ok(result)
}
```
**Note on regex crate:** The project does not currently depend on `regex`. For this simple pattern, consider using manual string scanning (`find`/`replace`) to avoid adding a dependency. Alternatively, `regex` is a reasonable addition (~200KB compile overhead) if preferred for clarity. Claude's discretion per CONTEXT.md.

### Pattern 3: Concurrent stdin/stdout/stderr with tokio::spawn
**What:** Prevent pipe deadlock by reading stdout/stderr concurrently while writing stdin
**When to use:** CLI stdin mode (D-11)
**Example:**
```rust
use tokio::process::Command;
use tokio::io::AsyncWriteExt;
use std::process::Stdio;

async fn execute_stdin_mode(
    program: &str,
    args: &[String],
    payload: &[u8],
    timeout_secs: u64,
    max_bytes: usize,
) -> Result<(String, String, i32), String> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true)  // Safety: SIGKILL if executor is dropped
        .spawn()
        .map_err(|e| format!("failed to spawn process: {}", e))?;

    // Take ownership of stdio handles
    let mut stdin = child.stdin.take().expect("stdin was piped");
    let mut stdout = child.stdout.take().expect("stdout was piped");
    let mut stderr = child.stderr.take().expect("stderr was piped");

    let payload = payload.to_vec();

    // Concurrent I/O tasks
    let stdin_task = tokio::spawn(async move {
        stdin.write_all(&payload).await?;
        stdin.shutdown().await?;
        Ok::<(), std::io::Error>(())
    });

    let stdout_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut stdout, &mut buf).await?;
        Ok::<Vec<u8>, std::io::Error>(buf)
    });

    let stderr_task = tokio::spawn(async move {
        let mut buf = Vec::new();
        tokio::io::AsyncReadExt::read_to_end(&mut stderr, &mut buf).await?;
        Ok::<Vec<u8>, std::io::Error>(buf)
    });

    // Wait for process with timeout
    let status = tokio::time::timeout(
        std::time::Duration::from_secs(timeout_secs),
        child.wait(),
    ).await
    .map_err(|_| {
        // Timeout expired -- child is killed via kill_on_drop
        "process timed out".to_string()
    })?
    .map_err(|e| format!("failed to wait for process: {}", e))?;

    // Collect I/O results
    let _ = stdin_task.await;
    let stdout_bytes = stdout_task.await
        .map_err(|e| format!("stdout task panicked: {}", e))?
        .map_err(|e| format!("stdout read error: {}", e))?;
    let stderr_bytes = stderr_task.await
        .map_err(|e| format!("stderr task panicked: {}", e))?
        .map_err(|e| format!("stderr read error: {}", e))?;

    // Check output size
    let total_size = stdout_bytes.len() + stderr_bytes.len();
    if total_size > max_bytes {
        return Err(format!(
            "output size {} bytes exceeds limit of {} bytes",
            total_size, max_bytes
        ));
    }

    let stdout_str = String::from_utf8_lossy(&stdout_bytes).to_string();
    let stderr_str = String::from_utf8_lossy(&stderr_bytes).to_string();
    let exit_code = status.code().unwrap_or(-1);

    Ok((stdout_str, stderr_str, exit_code))
}
```

### Pattern 4: Single-Pass Placeholder Resolution
**What:** Replace `<token>` patterns in a template string without re-resolving substituted values
**When to use:** All template resolution (command templates, response body templates)
**Example:**
```rust
use std::collections::HashMap;

fn resolve_placeholders(
    template: &str,
    variables: &HashMap<String, String>,
) -> Result<String, String> {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            // Scan for closing >
            let mut token = String::new();
            let mut found_close = false;
            for c2 in chars.by_ref() {
                if c2 == '>' {
                    found_close = true;
                    break;
                }
                token.push(c2);
            }
            if found_close {
                match variables.get(&token) {
                    Some(value) => result.push_str(value),
                    None => {
                        return Err(format!(
                            "unresolved placeholder <{}>; available: [{}]",
                            token,
                            variables.keys().cloned().collect::<Vec<_>>().join(", ")
                        ));
                    }
                }
            } else {
                result.push('<');
                result.push_str(&token);
            }
        } else {
            result.push(c);
        }
    }

    Ok(result)
}
```
**Key insight:** This char-scanning approach naturally provides single-pass behavior (D-09). Substituted values are pushed into the result buffer and never re-scanned. This prevents injection attacks where a payload contains `<stdout>` or similar tokens.

### Anti-Patterns to Avoid
- **Shell execution (`sh -c "command"`):** Explicitly out of scope per REQUIREMENTS.md. External client payloads flow into commands -- shell injection risk is severe.
- **Recursive placeholder resolution:** D-09 forbids it. Never call `resolve_placeholders` on its own output.
- **Blocking I/O in async context:** Never use `std::process::Command` -- always use `tokio::process::Command` to avoid blocking the Tokio runtime.
- **Reading all of stdout before stderr (or vice versa):** Sequential reads cause deadlock when pipe buffers (typically 64KB on Linux) fill. Always read concurrently (D-11).

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| YAML parsing | Custom YAML parser | serde_yaml_ng | YAML spec is complex; anchors, multiline strings, type coercion all have edge cases |
| Async trait objects | Manual Pin<Box<Future>> wrappers | async-trait crate | Correct implementation requires careful lifetime handling; crate handles it |
| Process timeout | Manual timer + signal handling | `tokio::time::timeout` + `kill_on_drop(true)` | Tokio's timeout is cancellation-safe; kill_on_drop ensures cleanup on any exit path |

**Key insight:** The placeholder resolution is simple enough to hand-roll (it is literally angle-bracket-delimited string replacement), but the YAML parsing and async trait machinery are not.

## Common Pitfalls

### Pitfall 1: Pipe Deadlock in stdin Mode
**What goes wrong:** Writing a large payload to stdin blocks because the OS pipe buffer fills up (typically 64KB on Linux, 16KB on macOS). The child process is trying to write to stdout, which nobody is reading, so it also blocks. Both processes deadlock.
**Why it happens:** Sequential I/O: write all stdin, then read stdout, then read stderr.
**How to avoid:** Concurrent tokio tasks for stdin write, stdout read, and stderr read (D-11). Take ownership of stdio handles before spawning tasks.
**Warning signs:** Agent hangs when processing tasks with payloads > 64KB.

### Pitfall 2: kill_on_drop vs explicit kill on timeout
**What goes wrong:** Using `tokio::time::timeout` around `child.wait()` -- when timeout fires, the `child` variable may still be alive and the process keeps running.
**Why it happens:** `tokio::time::timeout` cancels the future but does not kill the child process.
**How to avoid:** Set `kill_on_drop(true)` on the `Command` builder AND explicitly call `child.kill().await` when timeout is detected. The `kill_on_drop` is a safety net for any code path that drops the `Child` value.
**Warning signs:** Zombie processes accumulating on the host.

### Pitfall 3: serde_yaml (deprecated) vs serde_yaml_ng
**What goes wrong:** Adding `serde_yaml = "0.9"` to Cargo.toml -- it compiles but is unmaintained and will not receive security fixes.
**Why it happens:** D-02 in CONTEXT.md says "Add `serde_yaml` dependency" -- this was decided before checking deprecation status.
**How to avoid:** Use `serde_yaml_ng = "0.10"` instead. API is identical (drop-in replacement). Import as `use serde_yaml_ng as serde_yaml;` if desired.
**Warning signs:** `cargo audit` warnings, `+deprecated` in version string.

### Pitfall 4: Env Var Interpolation Timing
**What goes wrong:** Resolving `${ENV_VAR}` after YAML parsing -- serde has already interpreted the string, potentially changing its structure.
**Why it happens:** Thinking of env vars as post-parse transforms.
**How to avoid:** Read the file as a raw string, resolve `${ENV_VAR}` first, then pass to serde_yaml_ng. This matches D-07: "resolved once at config load time."
**Warning signs:** YAML values containing `$` being misinterpreted.

### Pitfall 5: Command Splitting for Arg Mode
**What goes wrong:** Naive `split_whitespace()` to split command strings breaks on arguments containing spaces (e.g., `echo "hello world"`).
**Why it happens:** Shell-style quoting is not handled.
**How to avoid:** Since shell execution is explicitly out of scope, define the command as a list in YAML: `command: ["python", "script.py", "--input"]`. The first element is the program, the rest are args. Do not accept a single command string that needs shell-style splitting.
**Warning signs:** Arguments with spaces getting split incorrectly.

### Pitfall 6: Output Size Check Location
**What goes wrong:** Checking size after template resolution -- the template may inflate the size beyond max_bytes.
**Why it happens:** Checking at the wrong stage.
**How to avoid:** Check raw stdout + stderr size against max_bytes before template resolution (SAFE-01). The max_bytes limit protects against runaway process output, not template expansion.
**Warning signs:** Large outputs passing the check but producing oversized results.

## Code Examples

### YAML Config Structure (agent.yaml)
```yaml
# Agent configuration
gateway:
  address: "localhost:50051"
  token: "${XGENT_NODE_TOKEN}"
  node_id: "node-01"       # Optional, auto-generated UUID v7 if omitted
  ca_cert: null             # Path to CA cert for TLS, null for plaintext
  tls_skip_verify: false
  max_reconnect_delay_secs: 30

service:
  name: "my-service"
  mode: cli

cli:
  command: ["python", "process.py"]
  input_mode: arg           # "arg" or "stdin"
  timeout_secs: 60
  cwd: "/opt/scripts"
  env:
    PYTHONPATH: "/opt/lib"
    API_KEY: "${MY_API_KEY}"

response:
  body: |
    {"output": "<stdout>", "errors": "<stderr>"}
  max_bytes: 1048576        # 1MB
```

### Config Structs (config.rs)
```rust
use serde::Deserialize;
use std::collections::HashMap;

#[derive(Debug, Deserialize)]
pub struct AgentConfig {
    pub gateway: GatewaySection,
    pub service: ServiceSection,
    #[serde(default)]
    pub cli: Option<CliSection>,
    pub response: ResponseSection,
}

#[derive(Debug, Deserialize)]
pub struct GatewaySection {
    pub address: String,
    pub token: String,
    #[serde(default = "default_node_id")]
    pub node_id: String,
    pub ca_cert: Option<String>,
    #[serde(default)]
    pub tls_skip_verify: bool,
    #[serde(default = "default_max_reconnect")]
    pub max_reconnect_delay_secs: u64,
}

#[derive(Debug, Deserialize)]
pub struct ServiceSection {
    pub name: String,
    pub mode: ExecutionMode,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "kebab-case")]
pub enum ExecutionMode {
    Cli,
    SyncApi,
    AsyncApi,
}

#[derive(Debug, Deserialize)]
pub struct CliSection {
    pub command: Vec<String>,
    #[serde(default = "default_arg")]
    pub input_mode: CliInputMode,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u64,
    pub cwd: Option<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

#[derive(Debug, Deserialize, Clone, Copy, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum CliInputMode {
    Arg,
    Stdin,
}

#[derive(Debug, Deserialize)]
pub struct ResponseSection {
    pub body: String,
    #[serde(default = "default_max_bytes")]
    pub max_bytes: usize,
}

fn default_node_id() -> String {
    uuid::Uuid::now_v7().to_string()
}
fn default_max_reconnect() -> u64 { 30 }
fn default_arg() -> CliInputMode { CliInputMode::Arg }
fn default_timeout() -> u64 { 300 }
fn default_max_bytes() -> usize { 1_048_576 } // 1MB
```

### Placeholder Variable Map Construction
```rust
use std::collections::HashMap;
use xgent_proto::TaskAssignment;

fn build_task_variables(
    assignment: &TaskAssignment,
    service_name: &str,
) -> HashMap<String, String> {
    let mut vars = HashMap::new();
    vars.insert("payload".to_string(), String::from_utf8_lossy(&assignment.payload).to_string());
    vars.insert("service_name".to_string(), service_name.to_string());

    // Metadata placeholders: <metadata.key> for each key in metadata map
    for (key, value) in &assignment.metadata {
        vars.insert(format!("metadata.{}", key), value.clone());
    }

    vars
}

fn add_cli_output_variables(
    vars: &mut HashMap<String, String>,
    stdout: String,
    stderr: String,
) {
    vars.insert("stdout".to_string(), stdout);
    vars.insert("stderr".to_string(), stderr);
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| serde_yaml 0.9 | serde_yaml_ng 0.10 | March 2024 (deprecation) | Must use _ng variant; identical API, maintained fork |
| async-trait crate | Still async-trait | Ongoing (2026) | Native dyn async traits still unstable; async-trait remains correct choice |
| std::process::Command in async | tokio::process::Command | Tokio 1.0 (2021) | Always use tokio variant in async contexts; kill_on_drop available since tokio 1.17 |

**Deprecated/outdated:**
- `serde_yaml` 0.9.34+deprecated: Archived by dtolnay March 2024. Use `serde_yaml_ng`.
- `shell_words` crate for command splitting: Unnecessary when command is defined as a YAML list.

## Open Questions

1. **Placeholder syntax for `<payload>` in arg mode command template**
   - What we know: In arg mode, `<payload>` is substituted into the command template. But the command is a YAML list `["python", "script.py", "--data", "<payload>"]`.
   - What's unclear: Should `<payload>` be resolved within individual list elements, or can it replace an entire element?
   - Recommendation: Resolve within each string element of the command list. This allows `["python", "script.py", "--data=<payload>"]` as well as `["python", "script.py", "--data", "<payload>"]`.

2. **Binary payload handling**
   - What we know: `TaskAssignment.payload` is `bytes` (protobuf). `String::from_utf8_lossy` converts to string for placeholder substitution.
   - What's unclear: What happens with non-UTF8 binary payloads in arg mode?
   - Recommendation: Use lossy conversion for arg mode (replacement character for invalid bytes). For stdin mode, pipe raw bytes directly (no conversion needed). Document that arg mode is text-only.

3. **Regex dependency**
   - What we know: The project does not currently depend on `regex`. Env var interpolation needs `${VAR}` pattern matching.
   - What's unclear: Whether to add regex or use manual string scanning.
   - Recommendation: Manual string scanning is sufficient for the two simple patterns (`${...}` and `<...>`). Avoids adding a dependency for two patterns. Claude's discretion per CONTEXT.md.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | Rust built-in test framework (`#[cfg(test)]` + `#[tokio::test]`) |
| Config file | None needed -- uses `#[cfg(test)] mod tests` inline + integration tests in `gateway/tests/` |
| Quick run command | `cargo test -p xgent-gateway --lib agent` |
| Full suite command | `cargo test -p xgent-gateway` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CFG-01 | YAML config loading | unit | `cargo test -p xgent-gateway agent::config::tests` | Wave 0 |
| CFG-02 | Placeholder resolution | unit | `cargo test -p xgent-gateway agent::placeholder::tests` | Wave 0 |
| CFG-03 | Env var interpolation | unit | `cargo test -p xgent-gateway agent::config::tests::env_var` | Wave 0 |
| CFG-04 | Metadata placeholder resolution | unit | `cargo test -p xgent-gateway agent::placeholder::tests::metadata` | Wave 0 |
| CFG-05 | CLI cwd config | unit | `cargo test -p xgent-gateway agent::cli_executor::tests::cwd` | Wave 0 |
| CFG-06 | CLI env vars | unit | `cargo test -p xgent-gateway agent::cli_executor::tests::env` | Wave 0 |
| CLI-01 | Arg mode execution | integration | `cargo test -p xgent-gateway agent::cli_executor::tests::arg_mode` | Wave 0 |
| CLI-02 | Stdin mode execution | integration | `cargo test -p xgent-gateway agent::cli_executor::tests::stdin_mode` | Wave 0 |
| CLI-03 | Timeout kills process | integration | `cargo test -p xgent-gateway agent::cli_executor::tests::timeout` | Wave 0 |
| CLI-04 | Exit code mapping | unit | `cargo test -p xgent-gateway agent::cli_executor::tests::exit_code` | Wave 0 |
| CLI-05 | Response body template | unit | `cargo test -p xgent-gateway agent::response::tests` | Wave 0 |
| SAFE-01 | Output size limit | unit | `cargo test -p xgent-gateway agent::cli_executor::tests::max_bytes` | Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test -p xgent-gateway --lib agent`
- **Per wave merge:** `cargo test -p xgent-gateway`
- **Phase gate:** Full suite green before `/gsd:verify-work`

### Wave 0 Gaps
- [ ] `gateway/src/agent/mod.rs` -- module declaration
- [ ] `gateway/src/agent/config.rs` -- config struct + parsing + env var tests
- [ ] `gateway/src/agent/placeholder.rs` -- placeholder resolution + tests
- [ ] `gateway/src/agent/cli_executor.rs` -- CLI execution + tests
- [ ] `gateway/src/agent/response.rs` -- response template + tests
- [ ] `gateway/src/agent/executor.rs` -- trait definition (no tests needed, just the trait)

## Sources

### Primary (HIGH confidence)
- Existing codebase: `gateway/src/bin/agent.rs` (317 LOC), `gateway/src/config.rs` -- established patterns
- `proto/src/gateway.proto` -- TaskAssignment message (task_id, payload bytes, metadata map)
- `gateway/Cargo.toml` -- current dependency versions verified
- `cargo tree` output -- tokio 1.50, tonic 0.14.5, clap 4.6.0 confirmed

### Secondary (MEDIUM confidence)
- [Rust community forum on serde_yaml deprecation](https://users.rust-lang.org/t/serde-and-yaml-support-status/125684) -- serde_yaml_ng is consensus replacement
- [rust-lang/rust#133119](https://github.com/rust-lang/rust/issues/133119) -- async fn in dyn trait still unstable, async-trait remains necessary
- crates.io search: serde_yaml_ng 0.10.0, async-trait 0.1.89 versions confirmed

### Tertiary (LOW confidence)
- None -- all findings verified against primary or secondary sources

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH - dependencies verified on crates.io, deprecation status confirmed
- Architecture: HIGH - module structure defined by user decisions, patterns proven in existing codebase
- Pitfalls: HIGH - pipe deadlock and kill_on_drop are well-documented Rust async process management issues

**Research date:** 2026-03-24
**Valid until:** 2026-04-24 (stable domain, Rust ecosystem changes slowly)
