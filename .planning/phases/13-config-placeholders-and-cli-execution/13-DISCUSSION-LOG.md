# Phase 13: Config, Placeholders, and CLI Execution - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-24
**Phase:** 13-config-placeholders-and-cli-execution
**Areas discussed:** Config file structure, Placeholder engine design, CLI process management, Agent binary changes

---

## Config file structure

### Config file format
| Option | Description | Selected |
|--------|-------------|----------|
| TOML (agent.toml) | Consistent with gateway.toml, Rust-idiomatic | |
| YAML (agent.yaml) | Tree structure more readable for nested dispatch config | ✓ |

**User's choice:** YAML — "request dispatch部分最好是用yaml的树状配置，更直观" (tree-structured config for request dispatch is more intuitive)

### Mode selection
| Option | Description | Selected |
|--------|-------------|----------|
| Top-level mode field | mode: cli at top level, explicit | ✓ |
| Mode as section presence | Whichever section present determines mode | |
| Nested under execution key | All dispatch config under execution tree | |

**User's choice:** Top-level mode field (confirmed via sample config review)

### Placeholder variable naming
| Option | Description | Selected |
|--------|-------------|----------|
| `<meta.key>` | Shorter, less formal | |
| `<metadata.key>` | Aligned with API contract (task.metadata) | ✓ |

**User's choice:** Align with API — `<service_name>`, `<payload>`, `<metadata.key>`. User said: "让变量和api规范一致"

### CLI args
| Option | Description | Selected |
|--------|-------------|----------|
| Keep as overrides | CLI args override YAML values, backwards-compat | |
| Drop CLI args, YAML only | Only --config flag, all config in YAML | ✓ |

**User's choice:** Drop CLI args — YAML only with --config flag

---

## Placeholder engine design

### Unresolved token handling
| Option | Description | Selected |
|--------|-------------|----------|
| Error on unresolved | Fail task with clear error listing unresolved token | ✓ |
| Replace with empty string | Silent replacement, lenient | |
| Keep token as literal | Leave raw token in output | |

**User's choice:** Error on unresolved

### Resolution strategy
| Option | Description | Selected |
|--------|-------------|----------|
| Single pass only | One scan, no recursive resolution, safe from injection | ✓ |
| Two-pass with escaping | Allow nested resolution, more powerful but complex | |

**User's choice:** Single pass only

### ${ENV_VAR} resolution timing
| Option | Description | Selected |
|--------|-------------|----------|
| Config load time | Resolve once at startup, fail fast | ✓ |
| Per-task execution time | Resolve fresh each task, allows env changes | |

**User's choice:** Config load time

---

## CLI process management

### Large output handling
| Option | Description | Selected |
|--------|-------------|----------|
| Truncate with marker | Cap at max_bytes, task succeeds | |
| Fail the task | Kill process, report failure if output exceeds limit | ✓ |
| Stream and discard excess | Keep last max_bytes (tail) | |

**User's choice:** Fail the task

### Stdin deadlock prevention
| Option | Description | Selected |
|--------|-------------|----------|
| Spawn separate tokio tasks | Concurrent stdin write + stdout/stderr read | ✓ |
| Write all stdin first | Sequential, risks deadlock on large output | |

**User's choice:** Spawn separate tasks

### Timeout kill behavior
| Option | Description | Selected |
|--------|-------------|----------|
| SIGKILL immediately | Force kill on timeout, guaranteed dead | ✓ |
| SIGTERM then SIGKILL | Graceful shutdown with grace period | |

**User's choice:** SIGKILL immediately

---

## Agent binary changes

### Architecture
| Option | Description | Selected |
|--------|-------------|----------|
| Refactor into modules | gateway/src/agent/ module tree, bin/agent.rs as entrypoint | ✓ |
| Separate crate | New agent/ crate in workspace | |
| Keep flat in bin/agent.rs | Everything in one file | |

**User's choice:** Refactor into modules

### CLI args retention
| Option | Description | Selected |
|--------|-------------|----------|
| Keep as overrides | CLI args override YAML for Docker/debugging | |
| Drop CLI args | Only --config, all config in YAML | ✓ |

**User's choice:** Drop CLI args, YAML only

---

## Claude's Discretion

- Exact YAML config struct field names and serde attributes
- Placeholder regex pattern implementation
- Error message formatting
- Test strategy and fixtures
- tokio::process::Command usage details

## Deferred Ideas

None
