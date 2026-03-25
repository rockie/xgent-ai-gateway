# Roadmap: xgent-ai-gateway

## Milestones

- ✅ **v1.0 MVP** — Phases 1-7 (shipped 2026-03-22)
- ✅ **v1.1 Admin Web UI** — Phases 8-12 (shipped 2026-03-23)
- 🚧 **v1.2 Flexible Agent Execution** — Phases 13-16 (in progress)

## Phases

<details>
<summary>v1.0 MVP (Phases 1-7) — SHIPPED 2026-03-22</summary>

- [x] Phase 1: Core Queue Loop (3/3 plans) — completed 2026-03-21
- [x] Phase 2: Authentication and TLS (3/3 plans) — completed 2026-03-21
- [x] Phase 3: Service Registry and Node Health (3/3 plans) — completed 2026-03-22
- [x] Phase 4: Task Reliability and Callbacks (2/2 plans) — completed 2026-03-22
- [x] Phase 5: Observability and Packaging (4/4 plans) — completed 2026-03-22
- [x] Phase 6: gRPC Auth Hardening (2/2 plans) — completed 2026-03-22
- [x] Phase 7: Integration Fixes, Sample Service, and Cleanup (3/3 plans) — completed 2026-03-22

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

<details>
<summary>v1.1 Admin Web UI (Phases 8-12) — SHIPPED 2026-03-23</summary>

- [x] Phase 8: Frontend Foundation and Backend Auth (3/3 plans) — completed 2026-03-23
- [x] Phase 9: Service and Node Management (2/2 plans) — completed 2026-03-23
- [x] Phase 10: Task Management and Data Endpoints (3/3 plans) — completed 2026-03-23
- [x] Phase 11: Credential Management (2/2 plans) — completed 2026-03-23
- [x] Phase 12: Dashboard and Metrics Visualization (2/2 plans) — completed 2026-03-23

Full details: `.planning/milestones/v1.1-ROADMAP.md`

</details>

### v1.2 Flexible Agent Execution (In Progress)

**Milestone Goal:** Make the runner agent a configurable execution engine supporting CLI, sync-api, and async-api invocation modes with templated request/response mapping.

- [x] **Phase 13: Config, Placeholders, and CLI Execution** — YAML config parsing, placeholder engine, and CLI arg/stdin execution modes (completed 2026-03-24)
- [x] **Phase 14: Sync-API Execution** — HTTP dispatch with configurable URL, method, headers, body template, and response mapping (completed 2026-03-24)
- [x] **Phase 15: Async-API Execution** — Two-phase submit + poll with completion conditions, failure detection, and timeout (completed 2026-03-24)
- [x] **Phase 16: Examples and End-to-End Validation** — Example configs for all modes, Node.js client example, and dry-run validation (completed 2026-03-24)
- [x] **Phase 17: Fix Node.js Client API Contract** — Fix `task.status` → `task.state` field mismatch and base64 result decoding in all Node.js clients (completed 2026-03-24)
- [x] **Phase 19: JSON Payload Format (Remove Base64 Requirement)** — Replace base64-encoded bytes payloads with any valid JSON value across HTTP, gRPC, agent, and examples (completed 2026-03-25)

## Phase Details

### Phase 13: Config, Placeholders, and CLI Execution
**Goal**: Agent reads YAML config (agent.yaml) and executes CLI tasks with safe process management and templated results
**Depends on**: Phase 12 (existing agent binary from v1.0)
**Requirements**: CFG-01, CFG-02, CFG-03, CFG-04, CFG-05, CFG-06, CLI-01, CLI-02, CLI-03, CLI-04, CLI-05, SAFE-01
**Success Criteria** (what must be TRUE):
  1. Agent starts with an `agent.yaml` file and constructs the correct executor based on the configured mode
  2. Agent executes a CLI command in arg mode with payload substituted into the command template and reports the result back to the gateway
  3. Agent executes a CLI command in stdin mode, piping the payload to the process and capturing stdout/stderr without deadlock on large payloads
  4. Agent kills a CLI process that exceeds its configured timeout and reports failure with a meaningful error message
  5. Response body template produces a configurable result shape using `<stdout>`, `<stderr>`, `<payload>`, `<metadata.key>`, and `${ENV_VAR}` placeholders
**Plans:** 3/3 plans complete
Plans:
- [x] 13-01-PLAN.md — Config structs, placeholder engine, executor trait, response template
- [x] 13-02-PLAN.md — CLI executor with arg/stdin modes, timeout, exit code mapping
- [x] 13-03-PLAN.md — Agent binary integration (wire config + executor into poll loop)

### Phase 14: Sync-API Execution
**Goal**: Agent dispatches tasks to configurable HTTP endpoints with templated requests and response mapping
**Depends on**: Phase 13
**Requirements**: SAPI-01, SAPI-02, SAPI-03, SAPI-04
**Success Criteria** (what must be TRUE):
  1. Agent sends an HTTP request to a configured URL with the configured method, headers (including env var interpolation), and body template
  2. Agent extracts values from the HTTP response body using JSON Pointer key-paths and maps them into the result via response body template
  3. Agent reports failure with HTTP status code and response body when the target returns a non-2xx status
**Plans:** 2/2 plans complete
Plans:
- [x] 14-01-PLAN.md — SyncApiSection config, validation, SyncApiExecutor with HTTP dispatch and response extraction
- [x] 14-02-PLAN.md — Agent binary wiring for sync-api mode

### Phase 15: Async-API Execution
**Goal**: Agent handles two-phase async APIs by submitting a job, polling for completion, and extracting the final result
**Depends on**: Phase 14
**Requirements**: AAPI-01, AAPI-02, AAPI-03, AAPI-04, AAPI-05, AAPI-06
**Success Criteria** (what must be TRUE):
  1. Agent submits a job via HTTP and extracts a job identifier from the submit response using a configured key-path
  2. Agent polls a configured endpoint at a regular interval using values from the submit response in the poll URL/body, and detects completion via a key-path condition check
  3. Agent short-circuits polling and reports failure when a configured failed_when condition matches the poll response
  4. Agent enforces a total timeout on the combined submit + poll duration and reports failure on expiry
  5. Agent produces the final result by mapping poll response values into a response body template
**Plans:** 2/2 plans complete
Plans:
- [x] 15-01-PLAN.md — Response section refactor, shared http_common module, ExecutionResult headers field
- [x] 15-02-PLAN.md — AsyncApiExecutor config, implementation, condition evaluation, agent binary wiring

### Phase 16: Examples and End-to-End Validation
**Goal**: Ship working examples for all three execution modes and a client-side example that proves the full submit-execute-retrieve flow
**Depends on**: Phase 15
**Requirements**: EXMP-01, EXMP-02, EXMP-03, EXMP-04, EXMP-05
**Success Criteria** (what must be TRUE):
  1. A CLI example service with agent.toml config runs successfully, demonstrating both arg and stdin modes
  2. A sync-api example service with agent.toml config runs successfully against a local HTTP endpoint
  3. An async-api example service with agent.toml config runs successfully, completing a submit + poll cycle
  4. A Node.js client example submits a task via the gateway HTTP API, the agent executes it, and the client retrieves the result
  5. Running `--dry-run` validates the agent.toml config and prints resolved templates without connecting to the gateway or executing any tasks
**Plans:** 3/3 plans complete
Plans:
- [x] 16-01-PLAN.md -- Extended sample_service, example configs, and echo script
- [x] 16-02-PLAN.md -- Enhanced --dry-run validation with template preview
- [x] 16-03-PLAN.md -- Node.js client examples and tutorial READMEs

### Phase 17: Fix Node.js Client API Contract
**Goal**: Fix runtime-breaking field name mismatch and base64 encoding in Node.js client examples (gap closure from milestone audit)
**Depends on**: Phase 16
**Requirements**: EXMP-04
**Gap Closure:** Closes EXMP-04, integration gap (clients → gateway API), and Node.js E2E flow gap from v1.2 audit
**Success Criteria** (what must be TRUE):
  1. All three Node.js clients read `task.state` (not `task.status`) from the gateway response
  2. All three Node.js clients decode `task.result` from base64 before displaying

## Progress

**Execution Order:**
Phases execute in numeric order: 13 → 14 → 15 → 16

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Queue Loop | v1.0 | 3/3 | Complete | 2026-03-21 |
| 2. Authentication and TLS | v1.0 | 3/3 | Complete | 2026-03-21 |
| 3. Service Registry and Node Health | v1.0 | 3/3 | Complete | 2026-03-22 |
| 4. Task Reliability and Callbacks | v1.0 | 2/2 | Complete | 2026-03-22 |
| 5. Observability and Packaging | v1.0 | 4/4 | Complete | 2026-03-22 |
| 6. gRPC Auth Hardening | v1.0 | 2/2 | Complete | 2026-03-22 |
| 7. Integration Fixes, Sample Service, and Cleanup | v1.0 | 3/3 | Complete | 2026-03-22 |
| 8. Frontend Foundation and Backend Auth | v1.1 | 3/3 | Complete | 2026-03-23 |
| 9. Service and Node Management | v1.1 | 2/2 | Complete | 2026-03-23 |
| 10. Task Management and Data Endpoints | v1.1 | 3/3 | Complete | 2026-03-23 |
| 11. Credential Management | v1.1 | 2/2 | Complete | 2026-03-23 |
| 12. Dashboard and Metrics Visualization | v1.1 | 2/2 | Complete | 2026-03-23 |
| 13. Config, Placeholders, and CLI Execution | v1.2 | 3/3 | Complete    | 2026-03-24 |
| 14. Sync-API Execution | v1.2 | 2/2 | Complete    | 2026-03-24 |
| 15. Async-API Execution | v1.2 | 2/2 | Complete    | 2026-03-24 |
| 16. Examples and End-to-End Validation | v1.2 | 3/3 | Complete    | 2026-03-24 |
| 17. Fix Node.js Client API Contract | v1.2 | 0/0 (quick fix) | Complete | 2026-03-24 |
| 18. Tech Debt Cleanup | v1.2 | 3/3 | Complete | 2026-03-25 |
| 19. JSON Payload Format | v1.2 | 3/3 | Complete    | 2026-03-25 |

### Phase 18: Tech Debt Cleanup

**Goal:** Resolve clippy/compiler warnings, eliminate duplicated node health queries, refactor tracing init, and standardize admin error handling
**Requirements**: TD-01, TD-02, TD-03, TD-04, TD-05
**Depends on:** Phase 17
**Plans:** 3/3 plans complete

Plans:
- [x] 18-01-PLAN.md — Fix all clippy and compiler warnings (Default impls, FromStr trait, clamp, unused assignments)
- [x] 18-02-PLAN.md — Deduplicate node health fetching in admin.rs and metrics.rs
- [x] 18-03-PLAN.md — Refactor init_tracing duplication and standardize admin handler error types

### Phase 19: JSON Payload Format (Remove Base64 Requirement)
**Goal:** Replace base64-encoded bytes payloads with any valid JSON value across HTTP, gRPC, agent, and examples. Binary blobs will be transferred through S3-like infrastructure instead.
**Depends on:** Phase 18
**Requirements:** EXMP-04
**Gap Closure:** Closes EXMP-04 (partial → satisfied), integration gap (payload encoding mismatch), and Node.js E2E flow gap from v1.2 audit
**Success Criteria** (what must be TRUE):
  1. Gateway HTTP submit accepts `payload` as any valid JSON value (object, string, number, array, boolean, null) — not base64
  2. gRPC `.proto` `payload` field is `string` (JSON-encoded) instead of `bytes`
  3. Agent deserializes JSON payloads from Redis and passes them to executors
  4. Gateway task retrieval endpoint returns JSON payload/result (not base64)
  5. All 3 Node.js clients send JSON object payloads and work end-to-end without encoding
  6. README documents JSON payload contract with no base64 references
**Plans:** 3/3 plans complete

Plans:
- [x] 19-01-PLAN.md — Proto bytes->string, Redis queue String types, executor/response/placeholder String types
- [x] 19-02-PLAN.md — HTTP handlers accept/return JSON, gRPC handlers use String, all executors produce String results
- [x] 19-03-PLAN.md — Update tests, Node.js clients, and documentation for JSON payloads
