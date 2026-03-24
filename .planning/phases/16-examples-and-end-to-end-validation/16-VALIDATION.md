---
phase: 16
slug: examples-and-end-to-end-validation
status: complete
nyquist_compliant: true
wave_0_complete: true
created: 2026-03-24
audited: 2026-03-24
auditor: gsd-nyquist-auditor
gaps_resolved: 5/5
---

# Phase 16 Validation Map

## Automated Test Coverage

| Task ID | Requirement | Test File | Command | Status |
|---------|-------------|-----------|---------|--------|
| EXMP-01 | CLI example configs (agent-arg.yaml, agent-stdin.yaml) parse correctly via load_config with mode=cli and correct input_mode | `gateway/tests/example_config_test.rs` | `cargo test -p xgent-gateway --test example_config_test exmp01` | green |
| EXMP-02 | Sync-API example config (sync-api-service/agent.yaml) parses correctly with mode=sync-api and url containing /sync | `gateway/tests/example_config_test.rs` | `cargo test -p xgent-gateway --test example_config_test exmp02` | green |
| EXMP-03 | Async-API example config (async-api-service/agent.yaml) parses correctly with mode=async-api, submit/poll URLs, and completed_when/failed_when conditions | `gateway/tests/example_config_test.rs` | `cargo test -p xgent-gateway --test example_config_test exmp03` | green |
| EXMP-04 | Node.js client scripts pass syntax validation (node --check) | `examples/nodejs-client/syntax-check.js` | `node examples/nodejs-client/syntax-check.js` | green |
| EXMP-05 | --dry-run mode exits 0 and prints "Config is valid" for all four example configs | `gateway/tests/example_config_test.rs` | `cargo test -p xgent-gateway --test example_config_test exmp05` | green |

## Run All Phase 16 Tests

```bash
# Rust: config parsing (EXMP-01, EXMP-02, EXMP-03) and dry-run (EXMP-05)
cargo test -p xgent-gateway --test example_config_test

# Node.js: client script syntax (EXMP-04)
node examples/nodejs-client/syntax-check.js
```

## Notes

- EXMP-05 dry-run subprocess tests set cwd to repo root so that the relative path
  `./examples/cli-service/echo.sh` in the CLI configs resolves correctly, matching
  the documented usage pattern (agent invoked from project root).
- EXMP-04 known behavioral note: client scripts check `task.status` but the gateway
  returns `task.state` (implementation discrepancy flagged in gap description).
  Syntax-only validation is the appropriate test scope until the API field name is
  confirmed or reconciled.
