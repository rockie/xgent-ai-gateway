# Phase 16: Examples and End-to-End Validation - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-24
**Phase:** 16-examples-and-end-to-end-validation
**Areas discussed:** Example directory structure, Example service implementations, Node.js client, Dry-run depth

---

## Example Directory Structure

| Option | Description | Selected |
|--------|-------------|----------|
| One dir per mode | examples/cli-service/, sync-api-service/, async-api-service/, nodejs-client/ — each self-contained | ✓ |
| Flat examples dir | All agent.yaml files in one examples/ dir with mode prefix | |
| Inside gateway crate | gateway/examples/ using Cargo's example convention | |

**User's choice:** One dir per mode
**Notes:** None

### README Depth

| Option | Description | Selected |
|--------|-------------|----------|
| Quick-start only | Prerequisites, how to run, expected output — ~20 lines each | |
| Tutorial-style | Step-by-step walkthrough explaining each config field | ✓ |
| You decide | Claude picks the appropriate depth per example | |

**User's choice:** Tutorial-style
**Notes:** None

---

## Example Service Implementations

### Mock Service Strategy

| Option | Description | Selected |
|--------|-------------|----------|
| Reuse sample_service.rs | Extend existing Rust echo service with /sync and /async endpoints | ✓ |
| Separate mock per mode | Tiny Node.js or Python server per example dir | |
| External public APIs | Point at httpbin.org and real services | |

**User's choice:** Reuse sample_service.rs
**Notes:** None

### CLI Target Script

| Option | Description | Selected |
|--------|-------------|----------|
| Shell echo script | Simple echo.sh that reads stdin or args and outputs JSON | ✓ |
| Python script | Small Python script for JSON processing | |
| Both shell and Python | Two CLI examples showing different targets | |

**User's choice:** Shell echo script
**Notes:** None

### CLI Config Files

| Option | Description | Selected |
|--------|-------------|----------|
| Two configs | agent-arg.yaml and agent-stdin.yaml — each runnable as-is | ✓ |
| One config with comments | Single agent.yaml with commented-out alternative | |

**User's choice:** Two configs
**Notes:** None

---

## Node.js Client Example

### Client Scope

| Option | Description | Selected |
|--------|-------------|----------|
| Full flow, one mode | Single index.js demonstrating one mode | |
| All three modes | Three scripts (cli-client.js, sync-api-client.js, async-api-client.js) | ✓ |
| TypeScript package | Structured TS project with types and reusable client class | |

**User's choice:** All three modes
**Notes:** None

### HTTP Client Library

| Option | Description | Selected |
|--------|-------------|----------|
| Native fetch | Zero dependencies, requires Node 18+ | ✓ |
| node-fetch | Wider Node version support (14+) | |
| axios | More feature-rich but heavier | |

**User's choice:** Native fetch
**Notes:** None

---

## Dry-run Output Depth

### Template Resolution

| Option | Description | Selected |
|--------|-------------|----------|
| Add resolved templates | Print response body templates with sample placeholder values | ✓ |
| Templates + connectivity | Resolved templates plus gateway connection test | |
| Keep current output | Existing config summary is enough | |

**User's choice:** Add resolved templates
**Notes:** None

### Target Validation

| Option | Description | Selected |
|--------|-------------|----------|
| Config + target check | Verify CLI binary exists/executable, validate URLs for API modes | ✓ |
| Config only | Only validate YAML structure and field values | |

**User's choice:** Config + target check
**Notes:** None

---

## Claude's Discretion

- Exact sample placeholder values used in dry-run template rendering
- How the async mock endpoint tracks job state
- Error message formatting for dry-run validation failures
- Exact tutorial README structure and ordering
- Whether echo.sh uses jq or raw string interpolation

## Deferred Ideas

None — discussion stayed within phase scope.
