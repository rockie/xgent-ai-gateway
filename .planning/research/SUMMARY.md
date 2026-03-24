# Project Research Summary

**Project:** xgent-ai-gateway — v1.2 Flexible Agent Execution Engine
**Domain:** Configurable task execution engine for pull-model Rust task gateway
**Researched:** 2026-03-24
**Confidence:** HIGH

## Executive Summary

This milestone replaces a hardcoded HTTP POST dispatcher in the `xgent-agent` binary with a configurable, TOML-driven execution engine supporting three modes: CLI process execution, synchronous HTTP API dispatch, and two-phase async API polling. The work is scoped entirely to the agent binary (`gateway/src/bin/agent.rs`) and a new `gateway/src/agent/` module — the gateway server, proto definitions, and Redis state machine are completely untouched. The integration surface is narrow and well-understood: the current `dispatch_task()` function is the single replacement point, and the gRPC contract (`PollTasks` + `ReportResult`) does not change.

The recommended architecture is a trait-based executor pattern (`Box<dyn Executor>`) with a factory function that reads `ServiceConfig` and constructs the appropriate executor at startup. A standalone `PlaceholderEngine` resolves `<payload>`, `<stdout>`, `<stderr>`, `${ENV_VAR}`, and JSON-pointer-based response placeholders across all modes. This design keeps each execution mode independently testable, avoids coupling transport concerns into executors, and leaves the door open for future modes without structural changes. The build order flows: placeholder engine first (pure unit-testable), then config parsing, then sync-api executor (lowest risk, validates the trait pattern), then CLI, then async-api last (highest complexity).

The primary risks are implementation traps rather than architectural unknowns. Three are critical and must be addressed architecturally from the start: (1) stdin/stdout pipe deadlock on payloads exceeding 64KB — requires concurrent `tokio::join!` I/O, not sequential write-then-read; (2) shell injection if command templates ever use `sh -c` with interpolated payloads — the arg-based execution model makes this impossible by construction; (3) async-API poll loops that leak indefinitely on task cancellation — requires `CancellationToken` wiring from day one. All other pitfalls (zombie processes, SSRF via URL placeholders, thundering herd polling, per-request reqwest clients) have straightforward one-pattern fixes documented in detail.

## Key Findings

### Recommended Stack

The existing stack requires only two new dependencies. `shlex 1.3` (patched for CVE-2024-58266) handles POSIX shell word splitting for CLI command strings safely. `toml 0.8` (already a dev-dependency) is promoted to a regular dependency for `agent.toml` parsing — staying on 0.8 avoids a version conflict with the `config` crate's internal dependency. Every other capability needed for v1.2 is already available: `tokio::process::Command` (full feature already enabled), `reqwest 0.12` for templated HTTP, `serde_json::Value::pointer()` for RFC 6901 JSON Pointer extraction, and `tokio::time::sleep`/`timeout` for async polling loops.

See `.planning/research/STACK.md` for full version tables, compatibility matrix, and alternatives considered.

**Core technologies:**
- `tokio::process::Command` — async child process spawn, stdin/stdout/stderr pipes — already in stack, no new dep
- `shlex 1.3` — POSIX shell word splitting for command templates — single Cargo.toml line addition
- `toml 0.8` — agent.toml deserialization — promote from dev-dep to regular dep
- `reqwest 0.12` — templated HTTP requests for sync-api and async-api modes — already proven in agent
- `serde_json::Value::pointer()` — RFC 6901 JSON Pointer for response extraction — already in stack
- `async-trait` — enables `Box<dyn Executor>` with async methods until native dyn async traits stabilize

**Critical version notes:** Stay on `reqwest 0.12` (0.13 switches TLS backend to aws-lc-rs, breaking changes). Stay on `toml 0.8` (1.x renames `parse()` to `from_str()` and conflicts with `config` 0.15). Do not add `regex` — the placeholder syntax `<name>` and `${VAR}` is simple enough for iterative `str::find` parsing.

### Expected Features

The v1.2 milestone has a clearly defined MVP boundary. All P1 features are known and scoped. The feature dependency graph runs: TOML config + placeholder system (foundational, no dependencies) → CLI mode → sync-api mode → async-api mode (depends on sync-api HTTP machinery). Build in this order; do not parallelize.

See `.planning/research/FEATURES.md` for full feature table, competitor analysis (GA4GH TES, n8n, Temporal), and config schema design reference.

**Must have (table stakes — v1.2 launch):**
- Per-service TOML config (`agent.toml` with `[service]` + `[gateway]` sections) — replaces CLI `--dispatch-url`
- Placeholder system — `<payload>`, `<stdout>`, `<stderr>`, `<exit_code>`, `${ENV_VAR}`, `<response.path>`, `<submit_response.path>`, `<poll_response.path>`
- CLI execution: arg-based — `shlex::split` command template, replace `<payload>` in args, `tokio::process::Command` (no shell)
- CLI execution: stdin-pipe — write payload to stdin with concurrent stdout/stderr read via `tokio::join!`
- CLI timeout + exit code — `tokio::time::timeout` + `child.kill().await` + `kill_on_drop(true)` on every spawn
- CLI response body template — `<stdout>`, `<stderr>`, `<exit_code>` in success/failed body templates
- sync-api mode — configurable URL, method, headers (with `${ENV}` interpolation), body template, response body mapping
- async-api mode — submit phase (POST + JSON pointer job ID extraction) + poll phase (interval with jitter, completion condition, configurable timeout)
- Error propagation — all modes map failures to `ReportResultRequest { success: false, error_message }` with meaningful context

**Should have (add in v1.2.x patches):**
- Multi-service support — single agent process with one `tokio::spawn` per service poll loop
- Working directory per service — `Command::current_dir()`, single config field
- Environment variables per service — `Command::envs()`, map in config
- Metadata placeholders — `<meta.key>` expansion from `TaskAssignment.metadata`
- Dry-run / `--check-config` mode — validates config, prints resolved templates, exits without connecting

**Defer (v2+):**
- Config hot-reload via `notify` crate — restart-on-change is sufficient; hot-reload adds edge cases with in-flight tasks
- Health check per service — pre-flight readiness probes before pulling tasks
- Retry on transient failure — local retry before reporting failure; acceptable to report immediately for v1.2
- Structured execution metrics — per-service duration/success-rate via tracing spans

**Anti-features (never build):**
- Shell execution mode (`sh -c` with payload interpolation) — command injection vector; arg mode and stdin-pipe cover all legitimate cases
- Dynamic plugin loading — explicitly descoped in PROJECT.md; the three modes cover all practical dispatch patterns
- Full template engine (Tera/MiniJinja/Handlebars) — overkill for six fixed placeholder types; ~50 lines of `str::find` suffices
- Parallel task execution per service — horizontal scaling via multiple agent instances is the intended model

### Architecture Approach

The execution engine plugs into the existing agent at a single point: `dispatch_task()` is replaced by `executor.execute(&assignment)`. The outer reconnect loop, gRPC streaming, SIGTERM drain, and in-flight tracking are unchanged. A new `gateway/src/agent/` module tree houses the execution engine alongside the existing binary, avoiding premature workspace extraction. The agent processes one task at a time — this is the correct and intended model; the gateway's server-side flow control on the gRPC stream means no new assignment arrives until `ReportResult` is sent for the current task. The async-api poll loop runs inline within `execute()` — it is a slow async call, not a detached background task.

See `.planning/research/ARCHITECTURE.md` for the full component diagram, Rust struct definitions, config TOML schema, build order table, and anti-patterns to avoid.

**Major components:**
1. `AgentConfig` (`agent/config.rs`) — deserialize `agent.toml`, run `validate()`, resolve `${ENV_VAR}` at startup; uses tagged enum `ExecutionMode` so missing required fields are serde errors, not runtime panics
2. `Executor` trait + factory (`agent/executor/mod.rs`) — `async fn execute(&self, assignment: &TaskAssignment) -> ExecutionResult`; factory constructs `Box<dyn Executor>` from `ServiceConfig` at startup
3. `CliExecutor` (`agent/executor/cli.rs`) — `tokio::process::Command`, `kill_on_drop(true)`, concurrent stdin/stdout via `tokio::join!`, `tokio::time::timeout` wrapping
4. `SyncApiExecutor` (`agent/executor/sync_api.rs`) — absorbs current `dispatch_task`; shared `reqwest::Client` passed from startup; per-request `RequestBuilder` for headers/body/timeout
5. `AsyncApiExecutor` (`agent/executor/async_api.rs`) — two-phase: submit + poll loop; `tokio::time::sleep` (not `interval`) for jitter-friendly polling; `CancellationToken` for shutdown wiring
6. `PlaceholderEngine` (`agent/placeholder.rs`) — two-pass resolution (input phase pre-execution, output phase post-execution); `str::find`-based, no regex; unknown placeholders are hard errors
7. `ResponseMapper` (`agent/response.rs`) — applies response template with output-phase placeholder context to produce final result bytes

**Key patterns to enforce:**
- `Box<dyn Executor>` via `async_trait` — trait objects, not enum dispatch; enables independent testability and future mode addition
- Config-driven factory — validation and executor construction at startup, fail-fast; no config re-parsing during task execution
- Placeholder two-phase model — input placeholders (payload, env, metadata) resolved before execution; output placeholders (stdout, stderr, response body) resolved after
- Config tagged enum (`#[serde(tag = "mode")]`) with `#[serde(deny_unknown_fields)]` — mode-specific required fields are structurally enforced at deserialization time, not at runtime

### Critical Pitfalls

See `.planning/research/PITFALLS.md` for full details, warning signs, recovery costs, integration gotchas, and a phase-to-pitfall mapping checklist.

1. **stdin/stdout pipe deadlock** — `tokio::join!` stdin write + stdout read + stderr read concurrently; never write-then-read sequentially; affects all payloads > 64KB (Linux pipe buffer). Recovery cost: MEDIUM (requires restructuring the dispatch function). Phase 1.

2. **Zombie processes from dropped child handles** — always `kill_on_drop(true)` on every `Command::new()`; always `child.kill().await` then `child.wait().await` on timeout expiry; extend SIGTERM handler to kill outstanding children before exit. Recovery cost: LOW (one-line fix) but PID exhaustion is catastrophic if missed. Phase 1.

3. **Shell injection via `sh -c`** — enforce arg-based execution (`Command::new(program).args(...)`) architecturally; never interpolate `<payload>` into a shell command string; stdin-pipe mode is the safe alternative for complex payloads. Recovery cost: HIGH (audit deployed configs, potential security incident). Phase 1.

4. **Async-API poll loop resource leak on cancellation** — wire `CancellationToken` from SIGTERM handler into every poll loop iteration; set hard `max_poll_duration` that the loop enforces unconditionally; document `cancel_url` config field for external job cleanup. Recovery cost: HIGH (significant feature work for persistent poll state if deferred). Phase 3.

5. **Cryptic serde errors for config mistakes** — use `#[serde(tag = "mode")]` enum (not flat struct with all-Option fields); add `#[serde(deny_unknown_fields)]` to catch typos; run `validate()` after deserialization with domain-appropriate error messages; ship example `agent.toml`. Recovery cost: LOW (wrapping parse errors) but operator experience is immediately broken. Phase 1.

**Additional moderate pitfalls:**
- SSRF via URL placeholder substitution — validate post-substitution URL hostname matches template hostname; `redirect(Policy::none())` on shared reqwest client. Phase 2.
- Per-request `reqwest::Client` construction — one shared client at startup, per-request `RequestBuilder` only. Phase 2.
- Thundering herd async polling — `tokio::time::sleep` (not `interval`) + randomized jitter; exponential backoff; respect `Retry-After` headers. Phase 3.
- Blocking `fork()` on runtime threads — current sequential task model naturally prevents; document; add `Semaphore` if parallel execution ever added. Phase 1.

## Implications for Roadmap

Based on the feature dependency graph and pitfall-to-phase mapping, a 4-phase structure emerges naturally. Each phase is independently deliverable and testable.

### Phase 1: Config Schema + Placeholder Engine + CLI Execution

**Rationale:** TOML config and placeholder resolution are foundational — every other feature depends on them. CLI execution is the highest-risk implementation (process lifecycle, pipe deadlock, injection) and must be locked down architecturally before HTTP modes are built. Five of the ten documented pitfalls (pipe deadlock, zombies, injection, config errors, mode enum validation) all map here. Getting these right upfront costs nothing extra; fixing them later after HTTP executors inherit the patterns is expensive.
**Delivers:** Agent reads `agent.toml`, constructs executor at startup, executes CLI tasks (arg-based and stdin-pipe), reports results with meaningful error messages. The existing hardcoded HTTP dispatch is replaced for CLI mode.
**Addresses features:** Per-service TOML config, placeholder system, CLI arg mode, CLI stdin-pipe mode, CLI timeout + exit code, CLI response body template, error propagation.
**Avoids pitfalls:** stdin/stdout deadlock (concurrent `tokio::join!`), zombie processes (`kill_on_drop(true)` + explicit kill/wait), shell injection (arg-array construction only), cryptic config errors (tagged enum + `validate()` + `deny_unknown_fields`), mode enum validation at parse time not runtime.
**Build sequence within phase:** `placeholder.rs` → `config.rs` → `executor/mod.rs` (trait only) → `executor/cli.rs` → `bin/agent.rs` TOML wiring → example CLI config.

### Phase 2: Sync-API Execution Mode

**Rationale:** Sync-api directly replaces the existing `dispatch_task()` function and is the simplest HTTP executor. Building it second validates the `Box<dyn Executor>` trait pattern with minimal new risk before tackling async-api's complexity. Pitfalls for shared reqwest client and SSRF via URL placeholders map here.
**Delivers:** Agent dispatches tasks to a configurable HTTP endpoint with templated URL, method, headers, and body. Response body mapping via JSON Pointer. This makes the old `--dispatch-url` flag fully obsolete.
**Uses:** Shared `reqwest::Client` (created at startup, passed to factory), `serde_json::Value::pointer()` for response mapping, `HeaderValue::from_str()` for header injection prevention.
**Implements:** `SyncApiExecutor`, `ResponseMapper`, expanded placeholder engine (`<response.path>` resolution).
**Avoids pitfalls:** Shared reqwest client (constructed once at startup, not per-request), URL-encoding of placeholder values in path segments, header injection prevention via `HeaderValue::from_str()`, SSRF via post-substitution URL hostname validation.

### Phase 3: Async-API Execution Mode

**Rationale:** Depends on sync-api HTTP machinery for the submit phase — cannot be built in parallel with Phase 2. Most complex feature: two-phase execution, poll loop, completion condition evaluation, cancellation. Pitfalls for thundering herd (polling) and poll loop resource leak on cancellation map here.
**Delivers:** Agent handles tasks requiring submit + poll against external async APIs (ML inference, video processing, any request-reply pattern). Configurable poll interval, timeout, completion condition, and optional external job cancellation.
**Implements:** `AsyncApiExecutor`, poll loop with `tokio::time::sleep` + jitter, `CancellationToken` wiring to SIGTERM handler, JSON Pointer completion condition evaluation, submit-response placeholder extraction for poll URL construction.
**Avoids pitfalls:** Poll loop resource leak on SIGTERM (`CancellationToken`), thundering herd (sleep-based polling + jitter, not `tokio::time::interval`), hard `max_poll_duration` enforcement, documented orphaned external job mitigation.

### Phase 4: End-to-End Validation + Examples

**Rationale:** Integration testing requires a running gateway + Redis + mock target services. This phase cannot be parallelized with phases 1-3 and ships the documentation and examples that make the feature externally usable.
**Delivers:** End-to-end test suite (gateway + agent with each mode + mock HTTP service), one example `agent.toml` per execution mode with inline comments, Node.js client example (submit task via HTTP, agent executes via each mode, poll result from gateway), `--check-config` flag for offline config validation.
**Addresses features:** Example service configs (P1), Node.js client example (P1), dry-run mode (P2 — low effort here).

### Phase Ordering Rationale

- Phase 1 before Phase 2: Placeholder engine is a shared dependency for all modes; config schema must be locked before any executor is built; CLI exposes the highest-risk patterns (process lifecycle, injection) that must be correct before HTTP modes reuse the same placeholder and response-mapping abstractions.
- Phase 2 before Phase 3: The async-api submit phase reuses sync-api HTTP machinery directly — building async-api first would require implementing the same HTTP code twice.
- Phases 1-3 before Phase 4: Integration tests require all three execution modes to exist; end-to-end validation is meaningless without the complete feature set.
- Multi-service support (P2) deferred to after Phase 4: Currently one agent per service is the design; multi-service adds concurrent poll loops and per-service reconnect state, a non-trivial `bin/agent.rs` change that should not risk delaying the MVP.

### Research Flags

Phases likely needing deeper research during planning:
- **Phase 3 (Async-API external job cancellation):** External API cancellation semantics vary widely — there is no standard "cancel a job" pattern. The `cancel_url` config field approach needs API-specific validation. The question of whether to persist poll state to Redis (enabling resume on agent restart) requires an explicit scope decision before implementation starts.
- **Phase 4 (Integration test harness):** The end-to-end test environment setup (gateway + Redis + agent + mock HTTP service) may benefit from a brief research spike on test harness patterns in the existing codebase before writing tests.

Phases with standard patterns (skip research-phase):
- **Phase 1 (Config + CLI):** All patterns are well-documented. `tokio::process::Command`, `shlex`, serde tagged enums — solutions for every pitfall are specific and confirmed. No research needed.
- **Phase 2 (Sync-API):** `reqwest::Client` patterns, JSON Pointer extraction, URL encoding — well-understood, no research needed.
- **Phase 3 (Async-API poll mechanics):** `tokio::time::sleep` + jitter + `CancellationToken` — standard Tokio patterns, no research needed. The external cancellation question is the only open item.

## Confidence Assessment

| Area | Confidence | Notes |
|------|------------|-------|
| Stack | HIGH | Only 2 new dependencies. Everything else already in the project. Versions verified on crates.io. No version conflicts identified. |
| Features | HIGH | MVP boundary is explicitly defined. Feature dependency graph is precise. P1/P2/P3 prioritization is opinionated with clear rationale. Anti-features are explicitly justified. |
| Architecture | HIGH | Builds directly on the existing 317-line agent. Integration point is a single function replacement. Trait pattern, factory pattern, and config schema are standard Rust patterns with documented prior art and code sketches. |
| Pitfalls | HIGH | 10 pitfalls identified with prevention patterns, warning signs, recovery costs, and phase mapping. Pipe deadlock, zombie processes, and shell injection are well-documented Rust process management issues sourced from official docs and cve reports. |

**Overall confidence:** HIGH

### Gaps to Address

- **Config schema: `[service]` vs `[[services]]` inconsistency.** ARCHITECTURE.md recommends `[service]` (singular) for v1.2. FEATURES.md example shows `[[services]]` (array). This must be resolved before Phase 1 locks the TOML schema — changing the format after Phase 1 is a breaking config change. Recommendation: use `[service]` (singular) for v1.2; multi-service can adopt `[[services]]` array in a later version.
- **Async-API external job cancellation scope.** Pitfalls research identifies orphaned external API jobs as HIGH recovery cost, but the full solution (Redis-persisted poll state, resume-on-restart, `cancel_url` config) is significant feature work. Roadmap planning must explicitly scope this for Phase 3: either include a minimal `cancel_url` attempt-on-SIGTERM or document it as a known limitation with operational mitigation.
- **`async-trait` vs native async traits.** Rust stable 1.85+ has async fns in traits, but `dyn async Trait` is not yet stable (tracking rust-lang/rust#133119). Using `async_trait` proc macro is the correct approach for now. Worth re-checking at the start of Phase 1 in case the tracking issue closes, but not a blocker.
- **`reqwest 0.12` → `0.13` migration.** Both STACK.md and the existing codebase correctly stay on 0.12 for v1.2. This should be tracked as a post-v1.2 follow-on to avoid accumulating technical debt.

## Sources

### Primary (HIGH confidence)
- Existing codebase: `gateway/src/bin/agent.rs` — 317-line current implementation; `dispatch_task()` integration point confirmed
- Existing codebase: `gateway/Cargo.toml` — reqwest 0.12, serde_json, tonic, tokio `full` features confirmed present
- [tokio::process::Command docs](https://docs.rs/tokio/latest/tokio/process/struct.Command.html) — async process API, `kill_on_drop`, stdin/stdout pipe patterns
- [shlex 1.3.0 on crates.io](https://crates.io/crates/shlex/1.3.0) — CVE-2024-58266 fix confirmed, 27M+ downloads
- [serde_json::Value::pointer docs](https://docs.rs/serde_json/latest/serde_json/) — RFC 6901 JSON Pointer built-in, no additional dependency
- [toml 0.8.x on crates.io](https://crates.io/crates/toml) — compatibility with `config` 0.15 confirmed
- [tokio::process zombie issue](https://github.com/tokio-rs/tokio/issues/2685) — `kill_on_drop` behavior and background reaping limitations documented

### Secondary (MEDIUM confidence)
- [std::process pipe deadlock](https://github.com/rust-lang/rust/issues/45572) — 64KB pipe buffer deadlock pattern documented
- [Microsoft Async Request-Reply Pattern](https://learn.microsoft.com/en-us/azure/architecture/patterns/asynchronous-request-reply) — submit/poll/completion model reference
- [OWASP Command Injection Defense](https://cheatsheetseries.owasp.org/cheatsheets/OS_Command_Injection_Defense_Cheat_Sheet.html) — arg-array vs `sh -c` guidance
- [OWASP SSRF Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Server_Side_Request_Forgery_Prevention_Cheat_Sheet.html) — URL hostname validation approach
- [GA4GH Task Execution Service spec](https://ga4gh.github.io/task-execution-schemas/docs/) — validates CLI mode executor/stdin/stdout design

### Tertiary (LOW confidence / needs validation)
- [reqwest 0.13 changelog](https://github.com/seanmonstar/reqwest/blob/master/CHANGELOG.md) — breaking changes documented; deferring upgrade is correct but full migration scope not analyzed
- External API cancellation patterns — no standard exists; `DELETE /jobs/{id}` approach assumed reasonable but must be validated per target API during Phase 3

---
*Research completed: 2026-03-24*
*Ready for roadmap: yes*
