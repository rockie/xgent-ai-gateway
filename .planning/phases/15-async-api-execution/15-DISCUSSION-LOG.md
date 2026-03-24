# Phase 15: Async-API Execution - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-24
**Phase:** 15-async-api-execution
**Areas discussed:** Config layout, Polling strategy, Completion conditions, Error handling

---

## Config layout

| Option | Description | Selected |
|--------|-------------|----------|
| Nested sub-sections | `submit:` and `poll:` sub-sections within `async_api`, each mirroring sync-api flat field pattern. Shared `timeout_secs` and `tls_skip_verify` at async_api level. | ✓ |
| Flat with prefixes | All fields at async_api level: `submit_url`, `submit_method`, `poll_url`, etc. Fewer nesting levels but verbose. | |
| Shared + overrides | Common fields at async_api level, submit and poll only override what differs. | |

**User's choice:** Nested sub-sections — consistent with sync-api field pattern internally, natural for two distinct HTTP calls.
**Notes:** User explicitly requested consistency with CLI and sync-api modes. Recommended layout was presented first per user request.

---

## Polling strategy

### Poll interval

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed interval | Poll every N seconds (default 5s). Simple, predictable. | ✓ |
| Exponential backoff | Start at interval_secs, double each poll up to max. More config complexity. | |

**User's choice:** Fixed interval, default 5s

### Total timeout

| Option | Description | Selected |
|--------|-------------|----------|
| 300s (5 min) | Matches CLI mode default. Consistent across modes. | ✓ |
| 600s (10 min) | More generous for long-running APIs. | |

**User's choice:** 300s default, consistent with CLI mode

---

## Completion conditions

**User's direct input (not via menu):**
1. `in`/`not_in` operators should use YAML arrays, not comma-separated strings
2. Check `completed_when` first, then `failed_when`

### failed_when requirement

| Option | Description | Selected |
|--------|-------------|----------|
| Optional | `completed_when` required, `failed_when` optional. Timeout is fallback failure path. | ✓ |
| Both required | Force both conditions to prevent silent timeout hangs. | |

**User's choice:** `failed_when` optional

---

## Error handling

### Poll request failures

| Option | Description | Selected |
|--------|-------------|----------|
| Retry once, then fail task | Same as sync-api D-13. One retry for transient network issues. | ✓ |
| Keep polling until timeout | Treat errors as transient, log warning, continue. | |
| Configurable max retries | Add `poll_retries` field. More knobs. | |

**User's choice:** Retry once, consistent with sync-api

### Submit request failures

| Option | Description | Selected |
|--------|-------------|----------|
| Yes, retry once | Same one-retry pattern as sync-api and poll. Connection-level only. | ✓ |
| No retry, fail immediately | One-shot call, caller resubmits. | |

**User's choice:** Retry once, consistent across all HTTP calls

### Timeout behavior

| Option | Description | Selected |
|--------|-------------|----------|
| Cancel mid-flight | `tokio::time::timeout` wraps entire loop. Cancels in-flight poll request. | ✓ |
| Wait for current poll | Let in-flight request finish, check timeout after. Soft limit. | |

**User's choice:** Cancel mid-flight — timeout means exactly `timeout_secs`

---

## Claude's Discretion

- Struct names and serde attributes for config sections
- Condition operator enum implementation
- Poll loop internal structure
- Test strategy and fixtures
- Error message formatting
- How `extract_json_value` and `find_response_placeholders` are shared between sync-api and async-api modules

## Deferred Ideas

None — discussion stayed within phase scope
