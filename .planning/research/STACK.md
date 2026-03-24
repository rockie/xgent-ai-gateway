# Stack Research: v1.2 Flexible Agent Execution

**Domain:** Configurable execution engine for pull-model task runner agent
**Researched:** 2026-03-24
**Confidence:** HIGH

## Scope

This research covers ONLY the stack additions needed for v1.2 features:
- CLI process execution (spawn, stdin pipes, shell escaping)
- Templated HTTP client requests (configurable URL, method, headers, body)
- Async two-phase polling loops (submit + poll with timeout)
- TOML-based per-service execution config (agent.toml)

Everything below builds on the existing Tokio 1.50 + Tonic 0.14 + Axum 0.8 + reqwest 0.12 + serde 1.0 stack. No changes to that foundation.

## Recommended Stack Additions

### New Dependencies

| Technology | Version | Purpose | Why Recommended |
|------------|---------|---------|-----------------|
| **shlex** | 1.3.0 | Shell word splitting/quoting | Split configured command strings into args safely (POSIX shell rules). Patched for CVE-2024-58266. Lighter than `shell-words` (no joining needed). Used in non-interactive context (spawning processes) where `try_quote` is fully safe. 27M+ downloads. |
| **toml** | 0.8.x | TOML deserialization | Already a dev-dependency in the project. Promote to regular dependency for agent.toml parsing. Stay on 0.8.x -- the jump to 0.9/1.0 renames `parse()` to `from_str()` and has breaking API changes. The `config` crate (0.15) also uses toml 0.8 internally, avoiding duplicate compilation. |

### Already Available (No New Dependencies Needed)

| Capability | Provided By | How |
|------------|-------------|-----|
| **Async process spawn** | `tokio::process::Command` | Included in `tokio` with `features = ["full"]` (already enabled). Supports `.stdin(Stdio::piped())`, `.stdout(Stdio::piped())`, `.stderr(Stdio::piped())`, and async `wait_with_output()`. |
| **Templated HTTP requests** | `reqwest 0.12` | Already a dependency. `RequestBuilder` supports dynamic `.method()`, `.header()`, `.body()`, `.json()`. Build requests from config at runtime -- no template engine needed. |
| **JSON key-path extraction** | `serde_json::Value::pointer()` | Built into `serde_json` (already a dependency). RFC 6901 JSON Pointer syntax (`/result/id`) extracts nested values from API responses. Covers all async-api polling use cases like `response.id_path = "/data/job_id"`. |
| **Async polling loops** | `tokio::time::interval` / `tokio::time::sleep` | Built into tokio. For async-api mode: submit, then `loop { sleep(poll_interval); check_status; if done break; }` with `tokio::time::timeout` wrapping the whole loop for max duration. |
| **Environment variable interpolation** | `std::env::var` | For `${ENV_VAR}` patterns in config strings (API keys in headers, base URLs). Simple regex or manual scan to expand env vars. A 20-line function covers this without a dependency. |
| **Placeholder substitution** | `str::replace()` | For `<payload>`, `<stdout>`, `<stderr>` token substitution. These are simple string replacements, not Jinja/Handlebars templates. No templating engine warranted. |
| **Config file loading** | `toml::from_str()` + serde | Parse agent.toml directly into typed structs with `#[derive(Deserialize)]`. The `config` crate's layered approach is overkill for per-service execution config -- direct toml deserialization is cleaner and more explicit. |
| **HTTP method parsing** | `reqwest::Method` / `http::Method` | Parse configured method strings ("GET", "POST") into typed values. `http::Method` (already a transitive dep via hyper) provides `Method::from_bytes()`. |

## Installation

```toml
# In gateway/Cargo.toml [dependencies], ADD:
shlex = "1.3"
toml = "0.8"

# REMOVE from [dev-dependencies]:
# toml = "0.8"  (moved to regular deps)
```

Two crates added total. That is the complete delta to Cargo.toml.

## Alternatives Considered

| Recommended | Alternative | When to Use Alternative |
|-------------|-------------|-------------------------|
| **shlex 1.3** (shell splitting) | shell-words 1.1 | Never for this project. Both parse POSIX shell words, but shlex also provides quoting/escaping which is useful if we need to construct shell commands from user input. shlex is more widely downloaded and has the CVE fix. |
| **shlex 1.3** (shell splitting) | Manual whitespace splitting | Never. Naive split breaks on quoted args (`"hello world"` becomes two tokens). Shell escaping rules are subtle -- use a battle-tested parser. |
| **toml 0.8** (config parsing) | toml 1.1 | Not yet. toml 1.0+ renames `parse()` to `from_str()` and changes error types. The `config` crate 0.15 depends on toml 0.8 internally, so upgrading pulls in two toml versions. Migrate when `config` updates its dependency. |
| **toml 0.8** (config parsing) | serde_yaml / JSON config | Never. TOML is the Rust-idiomatic config format (Cargo.toml precedent). The gateway already uses TOML (gateway.toml). Agent config should match conventions. |
| **serde_json pointer** (key-path) | serde_json_path (JSONPath RFC 9535) | Only if users need complex array filtering (`$.results[?(@.status=='done')]`). JSON Pointer (`/data/job_id`) covers all async-api polling use cases. JSONPath adds a dependency for power we do not need. |
| **serde_json pointer** (key-path) | json_dotpath crate | Never. Dot-path syntax (`data.job_id`) feels natural but JSON Pointer is an RFC 6901 standard and built into serde_json with zero dependency cost. |
| **tokio::process::Command** (CLI exec) | std::process::Command | Never in async context. Blocking process I/O stalls the Tokio runtime. `tokio::process::Command` mirrors the same API surface but is async-native. |
| **reqwest 0.12** (HTTP client) | reqwest 0.13 | Not for this milestone. 0.13 switches default TLS to aws-lc-rs (not ring), changes error types, removes feature flags. The agent already has reqwest 0.12 working. Upgrade separately when beneficial. |
| **Manual env var expansion** | envsubst crate / handlebars / tera | Overkill. We need `${API_KEY}` -> env value in a few config strings. A regex replace or manual scan handles this in under 30 lines without adding a template engine dependency. |
| **Direct toml::from_str()** | config crate layered loading | For agent.toml specifically, direct deserialization is cleaner. The `config` crate's env-var overlay and multi-source merging is valuable for gateway.toml but unnecessary for per-service execution configs that are file-only. |

## What NOT to Use

| Avoid | Why | Use Instead |
|-------|-----|-------------|
| **duct** (process orchestration) | Adds shell pipeline abstractions we do not need. The agent runs one command per task, not pipelines. Extra dependency for zero benefit. | `tokio::process::Command` directly |
| **subprocess** crate | Wraps `std::process`, not async-aware. Would block the Tokio runtime during process execution. | `tokio::process::Command` |
| **handlebars / tera** (template engines) | Full template engines are massive overkill for `<payload>` and `${ENV_VAR}` substitution. They add compile time, complexity, and a learning curve for config authors. Our substitution patterns are fixed tokens, not user-authored templates. | `str::replace()` for placeholders, manual env var expansion |
| **reqwest 0.13** | Breaking changes: default TLS backend switches from ring to aws-lc-rs, feature flags renamed (`trust-dns` removed), error inspector methods removed. No features in 0.13 that this milestone needs. | Stay on reqwest 0.12 |
| **toml 1.x** | API breaking changes, would conflict with `config` crate 0.15's internal toml 0.8 dependency causing duplicate compilation. | Stay on toml 0.8.x |
| **nix** crate (Unix process control) | Heavyweight Unix API bindings. We only need spawn + wait + pipe, which tokio::process handles completely. nix is for signal handling, ptys, raw fd manipulation. | `tokio::process` + `std::process::Stdio` |
| **serde_json_path** | JSONPath RFC 9535 implementation. Adds a dependency for complex query syntax we will never use. Simple key-path extraction (`/data/job_id`) is already built into serde_json. | `serde_json::Value::pointer()` |
| **shell-escape** crate | Only provides escaping (not splitting). We need splitting of configured command strings. And shlex provides both. | shlex 1.3 |

## Stack Patterns by Execution Mode

### CLI Mode (arg-based)

```
Config: command = "python3 /scripts/process.py --input <payload>"
```

1. Parse command template with `shlex::split()` to get `Vec<String>`
2. Replace `<payload>` placeholder in args with task payload (base64 or file path)
3. Spawn with `tokio::process::Command::new(args[0]).args(&args[1..])`
4. Capture stdout/stderr via `.stdout(Stdio::piped()).stderr(Stdio::piped())`
5. Use `wait_with_output()` for async completion
6. Map exit code 0 = success, non-zero = failure
7. Build result from stdout using response template (`<stdout>`)

### CLI Mode (stdin-pipe)

```
Config: command = "jq '.transform'" with stdin_pipe = true
```

1. Same command parsing via `shlex::split()`
2. Spawn with `.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped())`
3. Write task payload bytes to `child.stdin.take().unwrap()` then drop handle to signal EOF
4. Await `child.wait_with_output()` to collect stdout/stderr after process exits
5. Same exit code and response mapping as arg-based

### sync-api Mode

```
Config: url = "${API_BASE}/process", method = "POST", body = "<payload>"
```

1. Expand `${ENV_VAR}` in url, headers, body template strings
2. Replace `<payload>` in body template with task payload
3. Build `reqwest::Client::request(method, url).headers(configured_headers).body(body)`
4. Single request-response cycle
5. Map HTTP 2xx = success, 4xx/5xx = failure
6. Extract result from response body using optional `response_body_path` (JSON Pointer)

### async-api Mode (two-phase submit + poll)

```
Config: submit.url, submit.id_path = "/data/job_id", poll.url = "${API_BASE}/status/<id>"
        poll.status_path = "/status", poll.completed_values = ["done", "failed"]
        poll.interval_secs = 5, poll.timeout_secs = 300
```

1. **Phase 1 (submit):** Build and send request like sync-api mode
2. Extract job ID from response using `serde_json::Value::pointer(id_path)`
3. **Phase 2 (poll):** Loop with `tokio::time::sleep(Duration::from_secs(interval))`
   - Build poll request, replacing `<id>` placeholder with extracted job ID
   - Parse response JSON, extract status via `Value::pointer(status_path)`
   - Compare against `completed_values` list
   - On match: extract result from response body, determine success/failure
   - `tokio::time::timeout(Duration::from_secs(timeout), poll_loop)` wraps entire loop
4. Timeout expiry = task failure with "poll timeout" error message

### Environment Variable Expansion

```rust
// Simple expansion function -- no crate needed
fn expand_env_vars(s: &str) -> String {
    // Match ${VAR_NAME} patterns
    // Replace with std::env::var("VAR_NAME").unwrap_or_default()
    // ~15-20 lines with a simple state machine or regex
}
```

## Version Compatibility

| Package | Compatible With | Notes |
|---------|-----------------|-------|
| shlex 1.3 | Rust 1.46+ (MSRV) | No async runtime dependency. Pure string parsing. Compatible with everything in the stack. |
| toml 0.8.x | serde 1.0, config 0.15 | config 0.15 uses toml 0.8 internally. Same version avoids duplicate compilation. |
| tokio::process | tokio 1.50 (already used) | Part of tokio `full` feature set, already enabled in Cargo.toml. No additional feature flags needed. |
| reqwest 0.12 | tokio 1.x, rustls 0.23 | Already proven in the project. No version change needed for templated requests. |
| serde_json pointer | serde_json 1.0 (already used) | `Value::pointer()` stable since serde_json 1.0. No version concern. |

## Confidence Assessment

| Area | Confidence | Reasoning |
|------|------------|-----------|
| tokio::process for CLI | HIGH | Part of tokio core, extensively documented, mirrors std::process API. Thousands of production Rust tools use it. |
| shlex for shell parsing | HIGH | Verified 1.3.0 on crates.io with CVE fix. 27M+ downloads. Simple, focused, battle-tested crate. |
| toml 0.8 for config | HIGH | Already a dev-dep in project. config crate uses it internally. Well-understood, no risk. |
| reqwest 0.12 for HTTP | HIGH | Already proven in the agent's dispatch_task function. No changes needed for templated requests -- just build requests dynamically from config. |
| serde_json pointer for key-path | HIGH | RFC 6901 standard, built into serde_json, zero dependency cost. Sufficient for `/data/job_id` style paths. |
| Manual env var expansion | HIGH | std::env::var is trivial. Pattern well-established across Docker, shell, CI tools. |
| Staying on reqwest 0.12 | MEDIUM | 0.13 exists with breaking changes. No features in 0.13 needed for v1.2. Safe to defer upgrade. |
| Staying on toml 0.8 | MEDIUM | 1.1.0 exists. Staying on 0.8 avoids config crate version conflict. Safe to defer. |

## Sources

- [tokio::process::Command docs](https://docs.rs/tokio/latest/tokio/process/struct.Command.html) -- async process spawning API verified
- [tokio::process::Child docs](https://docs.rs/tokio/latest/tokio/process/struct.Child.html) -- stdin/stdout pipe handling
- [shlex on crates.io](https://crates.io/crates/shlex/1.3.0) -- version 1.3.0 verified
- [shlex CVE-2024-58266](https://windowsforum.com/threads/rust-shlex-quoting-gap-upgrades-1-2-1-and-1-3-0-for-safe-shells.392673/) -- security fix confirmed in 1.3.0
- [shell-words on crates.io](https://crates.io/crates/shell-words) -- version 1.1.1 verified, considered but shlex preferred
- [toml on crates.io](https://crates.io/crates/toml) -- latest is 1.1.0, staying on 0.8.x for config crate compatibility
- [reqwest on crates.io](https://crates.io/crates/reqwest) -- latest is 0.13.2, staying on 0.12 to avoid breaking changes
- [reqwest 0.13 changelog](https://github.com/seanmonstar/reqwest/blob/master/CHANGELOG.md) -- TLS backend switch, feature flag removals documented
- [serde_json::Value::pointer](https://docs.rs/serde_json/latest/serde_json/value/enum.Value.html) -- RFC 6901 JSON Pointer built-in method
- [serde_json_path](https://docs.rs/serde_json_path/latest/serde_json_path/) -- evaluated and rejected (unnecessary complexity)

---
*Stack research for: v1.2 Flexible Agent Execution engine*
*Researched: 2026-03-24*
