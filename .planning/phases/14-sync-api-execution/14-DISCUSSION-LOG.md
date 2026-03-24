# Phase 14: Sync-API Execution - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-24
**Phase:** 14-sync-api-execution
**Areas discussed:** Config structure, Response extraction, Error handling, HTTP client setup

---

## Config structure

### URL configuration
| Option | Description | Selected |
|--------|-------------|----------|
| Single URL template | One url field with placeholders, env vars via ${ENV_VAR} | ✓ |
| Base URL + path template | Separate base_url and path fields | |

**User's choice:** Single URL template
**Notes:** Simple, covers most cases. Env vars already work from config load.

### Body format
| Option | Description | Selected |
|--------|-------------|----------|
| Dual mode | body: '<payload>' for raw, or JSON structure with embedded placeholders | ✓ |
| Raw only | body is always raw <payload> | |

**User's choice:** Dual mode
**Notes:** Same placeholder engine, user writes the template they need.

### HTTP method default
| Option | Description | Selected |
|--------|-------------|----------|
| Default POST | method defaults to POST if omitted | ✓ |
| Required field | No default, must always specify | |

**User's choice:** Default POST

---

## Response extraction

### Key-path syntax
| Option | Description | Selected |
|--------|-------------|----------|
| JSON Pointer | RFC 6901: <response./result/text> | |
| Dot notation | <response.result.text>, numeric segments as array indices | ✓ |

**User's choice:** Dot notation
**Notes:** More familiar and readable than JSON Pointer.

### Missing path behavior
| Option | Description | Selected |
|--------|-------------|----------|
| Fail the task | Error listing path and actual response structure | ✓ |
| Empty string | Missing paths resolve to empty string | |

**User's choice:** Fail the task
**Notes:** Consistent with Phase 13 unresolved placeholder behavior (D-08).

### Array index access
| Option | Description | Selected |
|--------|-------------|----------|
| Yes, numeric segments as array indices | <response.items.0.name> | ✓ |
| No, objects only | Only object key traversal | |

**User's choice:** Yes, numeric segments as array indices

### Non-string value stringification
| Option | Description | Selected |
|--------|-------------|----------|
| JSON serialize | Numbers/booleans/objects/arrays become JSON strings | ✓ |
| ToString for primitives, fail for complex | Numbers/booleans to string, objects/arrays fail | |

**User's choice:** JSON serialize

---

## Error handling

### Non-2xx error detail
| Option | Description | Selected |
|--------|-------------|----------|
| Status + truncated body | Status code + first N bytes of response body | |
| Status + full body | Entire response body in error message | ✓ |
| Status code only | Just "HTTP 422" | |

**User's choice:** Status + full body

### Timeout
| Option | Description | Selected |
|--------|-------------|----------|
| Config timeout, fail task | timeout_secs in sync_api section, default 30s | ✓ |
| No separate timeout | Rely on reqwest default | |

**User's choice:** Config timeout, fail task

### Connection failures
| Option | Description | Selected |
|--------|-------------|----------|
| Fail task immediately | No retries, consistent with project philosophy | |
| Retry once then fail | One automatic retry on connection error | ✓ |

**User's choice:** Retry once then fail
**Notes:** Departure from "no retries" philosophy — justified for transient connection issues vs task-level failures.

---

## HTTP client setup

### Client lifecycle
| Option | Description | Selected |
|--------|-------------|----------|
| Shared client | Build once in new(), reuse for all requests | ✓ |
| Per-request client | New client per execute() call | |

**User's choice:** Shared client

### Target TLS verification
| Option | Description | Selected |
|--------|-------------|----------|
| Configurable | tls_skip_verify in sync_api section, default false | ✓ |
| Always verify | No option to skip | |
| Skip verify + custom CA cert | Both tls_skip_verify and ca_cert for target | |

**User's choice:** Configurable (tls_skip_verify only)

### Redirect policy
| Option | Description | Selected |
|--------|-------------|----------|
| Follow up to 5 | Capped from reqwest default of 10 | ✓ |
| No redirects | Treat redirect as non-2xx failure | |

**User's choice:** Follow up to 5

---

## Claude's Discretion

- Exact SyncApiSection struct field names and serde attributes
- Dot-notation path traversal implementation approach
- One-retry logic structure
- Test strategy and fixture structure
- Error message exact formatting
- Whether URL supports task placeholders in path

## Deferred Ideas

None — discussion stayed within phase scope.
