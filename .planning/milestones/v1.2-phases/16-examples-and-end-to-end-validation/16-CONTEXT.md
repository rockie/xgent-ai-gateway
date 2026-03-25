# Phase 16: Examples and End-to-End Validation - Context

**Gathered:** 2026-03-24
**Status:** Ready for planning

<domain>
## Phase Boundary

Ship working examples for all three execution modes (CLI, sync-api, async-api), a Node.js client example proving the full submit-execute-retrieve flow, and enhanced dry-run validation. No new execution modes or agent features — this phase validates and documents what Phases 13-15 built.

</domain>

<decisions>
## Implementation Decisions

### Example directory structure
- **D-01:** Top-level `examples/` directory with one subdirectory per mode: `cli-service/`, `sync-api-service/`, `async-api-service/`, `nodejs-client/`. Each is self-contained with its own config and README.
- **D-02:** Each example directory includes a tutorial-style README with step-by-step walkthrough explaining each config field, what happens at each stage, and how to modify.

### CLI example
- **D-03:** Target script is a shell echo script (`echo.sh`) that reads stdin or args and outputs a JSON result. Zero dependencies.
- **D-04:** Two separate agent configs: `agent-arg.yaml` (arg input mode) and `agent-stdin.yaml` (stdin input mode). Both runnable as-is. README walks through both.

### Sync-API and Async-API examples
- **D-05:** Extend the existing `sample_service.rs` with new endpoints rather than creating separate mock servers. Add `POST /sync` (echoes payload in JSON wrapper) and async endpoints (`POST /async/submit` returns job_id, `GET /async/status/:id` returns pending then completed after ~3 polls).
- **D-06:** Each example directory has its own `agent.yaml` pointing at the extended sample service.

### Node.js client example
- **D-07:** Three separate client scripts — one per execution mode (`cli-client.js`, `sync-api-client.js`, `async-api-client.js`). Each submits a task, polls for result, and prints it.
- **D-08:** Uses native `fetch` (Node 18+). Zero npm dependencies. `package.json` only needs start scripts.

### Dry-run enhancements
- **D-09:** Enhance `--dry-run` to print response body templates with sample placeholder values so users can see the output shape (e.g., `<stdout>` → `(sample stdout)`).
- **D-10:** For CLI mode, verify the command binary/script exists and is executable. For sync/async-api modes, verify the URL is well-formed. Report validation results.
- **D-11:** End dry-run output with a `✓ Config is valid` or `✗ Config has errors` summary line.

### Claude's Discretion
- Exact sample placeholder values used in dry-run template rendering
- How the async mock endpoint tracks job state (in-memory HashMap vs AtomicU64 counter)
- Error message formatting for dry-run validation failures
- Exact tutorial README structure and ordering
- Whether echo.sh uses jq or raw string interpolation for JSON output

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — EXMP-01 through EXMP-05 (5 requirements for this phase)

### Prior phase context
- `.planning/phases/13-config-placeholders-and-cli-execution/13-CONTEXT.md` — Config format (D-01 through D-04), placeholder engine (D-05 through D-09), CLI execution (D-10 through D-14), agent architecture (D-15 through D-17), response template (D-18, D-19)
- `.planning/phases/14-sync-api-execution/14-CONTEXT.md` — Sync-API config structure (D-01 through D-07), response extraction (D-08 through D-11), error handling (D-12 through D-14), HTTP client setup (D-15 through D-17)
- `.planning/phases/15-async-api-execution/15-CONTEXT.md` — Async-API config layout (D-01 through D-05), polling strategy (D-06 through D-08), completion/failure conditions (D-09 through D-12), response section refactor (D-17 through D-22), async-specific extraction (D-23 through D-25)

### Existing code
- `gateway/src/bin/agent.rs` — Agent binary with current `--dry-run` implementation (lines 77-101) to be enhanced
- `gateway/src/agent/config.rs` — Config structs and `load_config()` function
- `gateway/src/agent/placeholder.rs` — Placeholder resolution engine
- `gateway/sample_service.rs` — Existing echo HTTP service to be extended with sync and async endpoints

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `sample_service.rs`: Existing Axum-based echo HTTP service from v1.0. Will be extended with `/sync` and `/async/*` endpoints.
- `gateway/src/agent/config.rs`: `load_config()` already validates mode-section presence. Dry-run enhancements build on this.
- `gateway/src/agent/placeholder.rs`: Placeholder engine with `find_prefixed_placeholders()` can be used to enumerate placeholders for dry-run sample rendering.

### Established Patterns
- Agent module structure: `gateway/src/agent/{mod.rs, config.rs, executor.rs, cli_executor.rs, sync_api_executor.rs, async_api_executor.rs, placeholder.rs, response.rs, http_common.rs}`
- Config validation happens at load time — dry-run extends this with additional checks
- `ExecutionMode` enum (`Cli`, `SyncApi`, `AsyncApi`) drives mode-specific logic in agent.rs

### Integration Points
- `agent.rs` `--dry-run` branch (lines 77-101) — where enhanced dry-run output goes
- `sample_service.rs` — where new sync/async endpoints are added
- `examples/` directory — new top-level directory at repo root

</code_context>

<specifics>
## Specific Ideas

No specific requirements — open to standard approaches.

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope.

</deferred>

---

*Phase: 16-examples-and-end-to-end-validation*
*Context gathered: 2026-03-24*
