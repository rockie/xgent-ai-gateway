# Requirements: xgent-ai-gateway

**Defined:** 2026-03-24
**Core Value:** Tasks submitted by clients reliably reach internal nodes and results reliably flow back, regardless of network topology

## v1.2 Requirements

Requirements for Flexible Agent Execution milestone. Each maps to roadmap phases.

### Configuration

- [ ] **CFG-01**: Agent reads per-service execution config from agent.toml file
- [ ] **CFG-02**: Placeholder system resolves `<payload>`, `<stdout>`, `<stderr>`, `<response.path>`, `<submit_response.path>`, `<poll_response.path>` tokens in templates
- [ ] **CFG-03**: Environment variable interpolation resolves `${ENV_VAR}` in URLs, headers, and body templates
- [ ] **CFG-04**: Metadata placeholders resolve `<meta.key>` to task metadata values
- [ ] **CFG-05**: Per-service working directory (cwd) config for CLI processes
- [ ] **CFG-06**: Per-service environment variables injected into CLI processes

### CLI Execution

- [ ] **CLI-01**: Agent executes CLI commands in arg mode with `<payload>` replaced in command template
- [ ] **CLI-02**: Agent executes CLI commands in stdin mode, piping payload to process stdin
- [ ] **CLI-03**: Configurable per-service timeout kills process on expiry (kill_on_drop safety)
- [ ] **CLI-04**: Exit code 0 maps to success, non-zero maps to failure with exit code in error
- [ ] **CLI-05**: Response body template maps `<stdout>` and `<stderr>` into configurable result shape

### Sync API Execution

- [ ] **SAPI-01**: Agent dispatches HTTP request with configurable URL, method, and headers
- [ ] **SAPI-02**: Body template supports `<payload>` as entire body or embedded in JSON structure
- [ ] **SAPI-03**: Response body template maps `<response.path>` key-paths into result shape
- [ ] **SAPI-04**: Non-2xx HTTP status maps to failure with status code and body in error

### Async API Execution

- [ ] **AAPI-01**: Submit phase sends HTTP request and extracts values from response via key-path
- [ ] **AAPI-02**: Poll phase sends HTTP request at configurable interval with submit response values in URL/body
- [ ] **AAPI-03**: Completion condition checks key-path value with operators (equal, not_equal, in, not_in)
- [ ] **AAPI-04**: Failed_when condition short-circuits polling on detected failure state
- [ ] **AAPI-05**: Configurable timeout caps total submit + poll duration
- [ ] **AAPI-06**: Response body template maps poll response values into result shape

### Safety

- [ ] **SAFE-01**: Response body size limit caps result payload to prevent runaway output

### Examples

- [ ] **EXMP-01**: Example CLI script service with agent.toml config demonstrating arg and stdin modes
- [ ] **EXMP-02**: Example sync-api HTTP service with agent.toml config
- [ ] **EXMP-03**: Example async-api HTTP service with agent.toml config showing submit + poll flow
- [ ] **EXMP-04**: Node.js client example demonstrating full client → gateway → agent → result flow
- [ ] **EXMP-05**: Dry-run mode (--dry-run) validates config and prints resolved templates without executing

## Future Requirements

### Operational

- **OPS-01**: Config hot-reload (watch agent.toml for changes without restart)
- **OPS-02**: Health check per service (probe before pulling tasks)
- **OPS-03**: Retry on transient local execution failure (connection refused, 502/503/504)
- **OPS-04**: Multi-service support (single agent process serving multiple services concurrently)
- **OPS-05**: Structured logging with task context (task_id, service_name, mode, duration)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Shell execution mode (`sh -c`) | Command injection vulnerability — payloads from external clients flow into commands. Users wrap pipelines in scripts. |
| Dynamic plugin loading (.so/.dylib) | PROJECT.md constraint — opaque payloads with universal API is simpler and more secure |
| Full template engine (Jinja2/Handlebars) | Massive dependency for 6-9 placeholder types. Simple string replacement suffices. |
| Parallel task execution per service | Gateway streams one task at a time per poll. Scale horizontally with multiple agent instances. |
| Custom result transformers (Lua/WASM) | Embedded scripting runtime adds complexity. Response templates handle 95% of needs. |
| Payload file staging | Temp file lifecycle management. Use stdin pipe mode instead. |
| Webhook callback from agent | Gateway already has callback mechanism. Agent's job ends at ReportResult. |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| CFG-01 | Phase 13 | Pending |
| CFG-02 | Phase 13 | Pending |
| CFG-03 | Phase 13 | Pending |
| CFG-04 | Phase 13 | Pending |
| CFG-05 | Phase 13 | Pending |
| CFG-06 | Phase 13 | Pending |
| CLI-01 | Phase 13 | Pending |
| CLI-02 | Phase 13 | Pending |
| CLI-03 | Phase 13 | Pending |
| CLI-04 | Phase 13 | Pending |
| CLI-05 | Phase 13 | Pending |
| SAPI-01 | Phase 14 | Pending |
| SAPI-02 | Phase 14 | Pending |
| SAPI-03 | Phase 14 | Pending |
| SAPI-04 | Phase 14 | Pending |
| AAPI-01 | Phase 15 | Pending |
| AAPI-02 | Phase 15 | Pending |
| AAPI-03 | Phase 15 | Pending |
| AAPI-04 | Phase 15 | Pending |
| AAPI-05 | Phase 15 | Pending |
| AAPI-06 | Phase 15 | Pending |
| SAFE-01 | Phase 13 | Pending |
| EXMP-01 | Phase 16 | Pending |
| EXMP-02 | Phase 16 | Pending |
| EXMP-03 | Phase 16 | Pending |
| EXMP-04 | Phase 16 | Pending |
| EXMP-05 | Phase 16 | Pending |

**Coverage:**
- v1.2 requirements: 27 total
- Mapped to phases: 27
- Unmapped: 0

---
*Requirements defined: 2026-03-24*
*Last updated: 2026-03-24 after roadmap creation*
