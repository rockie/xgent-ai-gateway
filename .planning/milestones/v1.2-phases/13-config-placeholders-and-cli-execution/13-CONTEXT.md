# Phase 13: Config, Placeholders, and CLI Execution - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Agent reads YAML config (`agent.yaml`) and executes CLI tasks with safe process management and templated results. Replaces the current hardcoded HTTP POST dispatch in `agent.rs`. Sync-API and async-API modes are separate phases (14, 15).

</domain>

<decisions>
## Implementation Decisions

### Config file format
- **D-01:** Use YAML (`agent.yaml`) instead of TOML — tree-structured dispatch config is more readable in YAML. Breaks naming convention with `gateway.toml` but dispatch config benefits significantly.
- **D-02:** Add `serde_yaml` dependency for YAML parsing.
- **D-03:** Top-level `mode: cli` field determines execution mode. Only the active mode's section is required.
- **D-04:** CLI args dropped — only `--config <path>` and `--dry-run` flags. All config in YAML. Env vars work via `${ENV_VAR}` in YAML values.

### Placeholder variables
- **D-05:** Three core task variables aligned with the API contract: `<service_name>`, `<payload>`, `<metadata.key>` (dot-access into task metadata map).
- **D-06:** Execution-specific variables: `<stdout>`, `<stderr>` (CLI mode); `<response.path>` (sync-api, Phase 14); `<submit_response.path>`, `<poll_response.path>` (async-api, Phase 15).
- **D-07:** Environment variable interpolation uses `${ENV_VAR}` syntax, resolved once at config load time. Missing env var = agent fails to start.
- **D-08:** Unresolved task placeholders (e.g., `<metadata.missing_key>`) fail the task with a clear error listing the unresolved token and available keys.
- **D-09:** Single-pass resolution only — no recursive/nested placeholder expansion. Resolved values containing `<token>` syntax are NOT re-resolved. This prevents injection from untrusted payload/stdout data.

### CLI process management
- **D-10:** Two CLI input modes: `arg` (payload substituted into command template) and `stdin` (payload piped to process stdin).
- **D-11:** Stdin mode uses concurrent tokio tasks: one writes payload to stdin (then closes), one reads stdout, one reads stderr. Prevents deadlock when pipe buffers fill.
- **D-12:** Timeout enforcement: SIGKILL immediately on expiry via `child.kill()`. No SIGTERM grace period. Simple and reliable.
- **D-13:** Output exceeding `max_bytes` fails the task (not truncated). Error includes actual size and configured limit.
- **D-14:** Exit code 0 = success, non-zero = failure with exit code in error message.

### Agent architecture
- **D-15:** Refactor `agent.rs` into modules: `gateway/src/agent/{mod.rs, config.rs, executor.rs, cli_executor.rs, placeholder.rs, response.rs}`. `bin/agent.rs` remains as the main entrypoint.
- **D-16:** `Executor` trait using `async_trait` for `Box<dyn Executor>` (per STATE.md decision — re-check native dyn async traits at implementation time).
- **D-17:** Phase 14 adds `sync_api_executor.rs`, Phase 15 adds `async_api_executor.rs` to the same module structure.

### Response template
- **D-18:** Shared `response` section across all modes with `body` template and `max_bytes` limit.
- **D-19:** `body` template is a string with placeholder tokens. Not a structured YAML object — the template itself produces the result shape.

### Claude's Discretion
- Exact YAML config struct field names and serde attributes
- Placeholder regex pattern implementation details
- Error message formatting
- Test strategy and test fixture structure
- Whether to use `tokio::process::Command` directly or wrap it

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — CFG-01 through CFG-06, CLI-01 through CLI-05, SAFE-01 (12 requirements for this phase)

### Existing agent code
- `gateway/src/bin/agent.rs` — Current agent binary (317 LOC). Hardcoded HTTP POST dispatch in `dispatch_task()`. Poll loop, reconnection, graceful drain all stay; dispatch logic gets replaced.
- `gateway/src/config.rs` — Gateway config pattern using `config` crate + serde. Reference for config loading conventions (though agent switches to YAML).

### Project decisions
- `.planning/PROJECT.md` — Key Decisions table, especially: `async_trait` for `Box<dyn Executor>`, stay on `reqwest 0.12` and `toml 0.8`, use `[service]` singular in config
- `.planning/STATE.md` — Blocker note: re-check `async-trait` vs native async traits (rust-lang/rust#133119) at Phase 13 start

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `gateway/src/bin/agent.rs` — Poll loop, gRPC connection setup, reconnection with exponential backoff, SIGTERM graceful drain, `ReportResultRequest` construction. All reusable as-is; only `dispatch_task()` gets replaced by the executor.
- `gateway/src/config.rs` — Pattern reference for serde config with defaults, though agent will use `serde_yaml` instead of the `config` crate.
- `xgent_proto::TaskAssignment` — The protobuf message carrying `task_id`, `payload`, `metadata` (HashMap<String, String>). Metadata keys flow into `<metadata.key>` placeholders.

### Established Patterns
- Structured logging with `tracing` — agent already uses `tracing::info!` / `tracing::error!` with field context
- Error handling via `Box<dyn std::error::Error>` — simple, consistent across agent code
- `reqwest::Client` for HTTP dispatch — retained for sync-api mode in Phase 14, not needed for CLI mode

### Integration Points
- `bin/agent.rs::main()` — Config loading happens here; switches from `Cli::parse()` to YAML config load
- `bin/agent.rs::run_poll_loop()` — Where `dispatch_task()` is called; replaced with `executor.execute(assignment).await`
- `ReportResultRequest` — Result reporting format unchanged; executor returns `(success: bool, result: Vec<u8>, error_message: String)`

</code_context>

<specifics>
## Specific Ideas

- Config format preference: YAML tree structure over TOML for better readability of nested dispatch configs (user's explicit preference)
- Sample configs provided during discussion for CLI arg, CLI stdin, and async-api modes — use as reference for documentation and examples
- Placeholder naming aligned with API contract: `<service_name>`, `<payload>`, `<metadata.key>` (not `<meta.key>`)

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 13-config-placeholders-and-cli-execution*
*Context gathered: 2026-03-24*
