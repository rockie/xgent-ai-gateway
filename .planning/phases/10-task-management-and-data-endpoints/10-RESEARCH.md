# Phase 10: Task Management and Data Endpoints - Research

**Researched:** 2026-03-23
**Domain:** Backend paginated task API (Rust/Axum/Redis) + Frontend task management UI (React/TanStack/shadcn)
**Confidence:** HIGH

## Summary

This phase adds task browsing, inspection, and cancellation to the admin UI backed by two new backend endpoints. The backend challenge is enumerating tasks from Redis efficiently -- tasks are stored as individual `task:{id}` hash keys with no secondary index. Redis SCAN over the `task:*` keyspace is the only viable enumeration strategy without adding new data structures. The cancel endpoint is straightforward: validate state, set to Failed, XACK the stream entry.

The frontend builds a data table with cursor-based pagination, filter controls, a slide-out detail panel (Sheet), and a cancel confirmation dialog. All UI primitives exist in the project already (Table, Sheet, AlertDialog, Badge, DropdownMenu). The main missing shadcn/ui component is Select (needed for multi-select status filter and page size dropdown).

**Primary recommendation:** Use Redis SCAN with MATCH `task:*` and COUNT hint for pagination, encoding the SCAN cursor as the API cursor. Filter in the application layer after HGETALL on each matched key. This is acceptable for admin use (low QPS, tens-to-hundreds of tasks) but would not scale to millions of keys.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Data table (not card grid) -- sortable columns: Task ID (truncated), Service, Status, Created, Completed. Consistent with node table pattern from Phase 9.
- **D-02:** Task ID column shows first 8 chars of UUID v7 with copy-to-clipboard on hover. Full ID visible in detail panel.
- **D-03:** Status displayed as colored pill/badge -- yellow Pending, blue Running, green Completed, red Failed. Consistent with node health badge pattern.
- **D-04:** Payload and result data NOT shown in the table -- detail panel only. Keeps table clean for scanning.
- **D-05:** Cursor-based pagination with Next/Previous buttons. Natural fit for Redis SCAN. No total count required.
- **D-06:** Configurable page size: 10/25/50 via dropdown control. Default 25.
- **D-07:** Service filter -- dropdown populated from existing service list endpoint.
- **D-08:** Status filter -- dropdown multi-select with checkboxes (shadcn/ui Select). Filter by multiple statuses at once.
- **D-09:** Task ID search -- search box for looking up a specific task by ID. Quick jump to known task.
- **D-10:** Date range filter NOT included.
- **D-11:** Slide-out panel (shadcn/ui Sheet) -- click a table row opens a side sheet.
- **D-12:** Panel width fixed at ~50% viewport width.
- **D-13:** Four sections in the panel: Task info header, Metadata, Payload (JSON viewer), Result (JSON viewer).
- **D-14:** Cancel action available in BOTH the detail panel AND as a table row action.
- **D-15:** Pending and Running tasks can be cancelled. Completed and Failed tasks cannot.
- **D-16:** Standard confirmation dialog with warning text.
- **D-17:** Backend `POST /v1/admin/tasks/{task_id}/cancel` marks the task as failed state with admin cancellation error message.

### Claude's Discretion
- Redis enumeration strategy for task listing (SCAN over `task:*` keys vs reading stream entries)
- Backend response format for paginated task list (cursor encoding, response shape)
- Exact JSON syntax highlighting approach (pre tag with styling vs dedicated component)
- Table row action menu design (icon button vs three-dot dropdown)
- Loading skeleton design for task table
- Empty state content for "no tasks" and "no matching tasks" (filter applied)
- Sort order defaults (newest first via UUID v7 ordering)

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| TASK-01 | Admin can view paginated task list filtered by service and status | SCAN-based enumeration with app-layer filtering; cursor pagination; TanStack Query with queryKey encoding filters |
| TASK-02 | Admin can view task detail (metadata, timestamps, assigned node, result) | Existing `get_task_status()` returns all fields needed; Sheet component for slide-out panel |
| TASK-03 | Admin can cancel a pending or running task (returns failed to client) | New `cancel_task()` on RedisQueue; state validation via `TaskState::try_transition` pattern; XACK stream entry |
| API-05 | GET /v1/admin/tasks with pagination and service/status filters | New handler in admin.rs; SCAN + HGETALL pipeline; cursor/items response shape |
| API-06 | POST /v1/admin/tasks/{task_id}/cancel endpoint | New handler; validates Pending/Running state; sets Failed + error_message |

</phase_requirements>

## Standard Stack

### Core (already in project)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| redis-rs | 1.0.x | Redis SCAN + HGETALL for task enumeration | Already used; MultiplexedConnection handles concurrent SCAN/HGETALL |
| Axum | 0.8.x | HTTP handler for new endpoints | Already used; Query extractor for pagination params |
| serde | 1.x | Serialize/deserialize task list response | Already used |
| TanStack Query | 5.95+ | Data fetching, caching, invalidation | Already used; queryKey array encodes filters for automatic cache separation |
| TanStack Router | 1.168+ | File-based routing | Already used; tasks.tsx stub exists |
| shadcn/ui | v4 | Table, Sheet, AlertDialog, Badge, DropdownMenu, Select | Already installed (except Select) |
| Sonner | 2.x | Toast notifications for cancel success/failure | Already used |

### Supporting (need to add)
| Library | Version | Purpose | When to Use |
|---------|---------|---------|-------------|
| shadcn/ui Select | v4 | Multi-select status filter, page size dropdown | Install via `npx shadcn@latest add select` |

### No New Dependencies Needed
The project already has all necessary npm packages. The only addition is generating the Select component from shadcn/ui (which is a local component, not a new npm dependency -- it uses the existing `@base-ui/react` primitives already installed).

## Architecture Patterns

### Backend: New Files/Changes

```
gateway/src/
  http/admin.rs           # ADD: list_tasks(), get_task_detail(), cancel_task() handlers
  queue/redis.rs           # ADD: list_tasks(), cancel_task() methods on RedisQueue
  main.rs                  # ADD: route registration for /v1/admin/tasks and /v1/admin/tasks/{task_id}/cancel
```

### Frontend: New Files

```
admin-ui/src/
  lib/tasks.ts                              # Types + hooks (useTasks, useTaskDetail, useCancelTask)
  routes/_authenticated/tasks.tsx           # REPLACE stub with full task list page
  components/task-table.tsx                 # Data table component
  components/task-status-badge.tsx          # Colored status badge (pending/running/completed/failed)
  components/task-detail-sheet.tsx          # Slide-out panel with 4 sections
  components/task-cancel-dialog.tsx         # Confirmation dialog for cancel
  components/json-viewer.tsx               # Formatted JSON display with copy button
```

### Pattern 1: Redis SCAN-Based Task Enumeration

**What:** Use `SCAN 0 MATCH task:* COUNT {page_size * 3}` to iterate task keys, then pipeline `HGETALL` on each matched key. Filter by service/status in the application layer. Return the SCAN cursor as the API pagination cursor.

**When to use:** Admin task listing -- low QPS, bounded key count (tasks have TTL).

**Why SCAN over stream reads:** Tasks can be in multiple service streams, but each `task:{id}` hash is the single source of truth for current state. SCAN over hashes gives a unified view across all services. Stream entries may have been trimmed (MAXLEN) while the hash still exists.

**Response shape:**
```rust
#[derive(Serialize)]
pub struct ListTasksResponse {
    pub tasks: Vec<TaskSummary>,
    pub cursor: Option<String>,  // None = no more pages
}

#[derive(Serialize)]
pub struct TaskSummary {
    pub task_id: String,
    pub state: String,
    pub service: String,
    pub created_at: String,
    pub completed_at: String,
}
```

**Important SCAN behavior:** Redis SCAN does not guarantee exact page sizes. A single SCAN call may return 0 to many results. The backend must loop SCAN calls until it fills the requested page size or the cursor returns to 0 (end of keyspace). This means the backend accumulates results across multiple SCAN iterations per API call.

**Cursor encoding:** Use the raw Redis SCAN cursor (a string/integer). When the Redis cursor returns 0, the API returns `cursor: null` to signal no more pages. The frontend sends `cursor` as a query parameter.

### Pattern 2: Task ID Direct Lookup (D-09)

**What:** When the user searches by task ID, bypass SCAN entirely. Use `HGETALL task:{id}` directly. If found, return it as a single-item result. If not found, return empty.

**When to use:** Task ID search box -- instant lookup without iteration.

### Pattern 3: Cancel Task State Machine

**What:** Cancel validates current state (must be Pending or Running), transitions to Failed, sets `error_message` to "Cancelled by administrator", and XACKs the stream entry.

**Key detail:** The existing `TaskState::try_transition` does NOT allow `Pending -> Failed`. The valid transitions are: `Pending -> Assigned`, `Assigned -> Running/Failed`, `Running -> Completed/Failed`. Cancel from Pending requires either:
1. Adding `Pending -> Failed` to the transition table (recommended -- admin cancel is a legitimate transition)
2. Bypassing `try_transition` in the cancel handler (not recommended -- breaks the invariant)

**Cancel must also XACK:** If the task has a `stream_id`, the cancel handler should XACK it to prevent a node from picking it up after cancellation. For tasks without a `stream_id` (still in pending queue), the stream entry remains but the hash state is Failed -- any node that picks it up will see it's already Failed when it tries to transition to Assigned.

### Pattern 4: Frontend Query Key Strategy

**What:** Encode all filter parameters in the TanStack Query key so cache is automatically segmented by filter combination.

```typescript
queryKey: ['tasks', { cursor, service, statuses, pageSize }]
```

**When filters change:** Reset cursor to undefined (start from beginning). TanStack Query automatically refetches.

### Anti-Patterns to Avoid
- **KEYS command for enumeration:** Never use `KEYS task:*` in production -- blocks Redis for the entire scan duration. Always use SCAN.
- **Unbounded SCAN loops:** Always cap the number of SCAN iterations per API call to prevent long-running requests if the keyspace is huge.
- **Fetching payloads in list endpoint:** Only return summary fields in the list. Full payload/result via the detail endpoint (TASK-02). The list response would be enormous otherwise.
- **Client-side pagination:** Do not fetch all tasks and paginate in the browser. Redis SCAN handles server-side pagination.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| JSON syntax highlighting | Custom parser/highlighter | `<pre>` with `JSON.stringify(obj, null, 2)` + Tailwind `font-mono` styling | Full syntax highlighting (Prism/Shiki) is overkill for admin panel; formatted JSON in a monospace `<pre>` tag with a copy button is sufficient and zero-dependency |
| Multi-select dropdown | Custom checkbox dropdown | shadcn/ui Select or a custom Popover + checkbox list using existing Popover component | Accessible, keyboard-navigable, consistent styling |
| Copy to clipboard | Manual `document.execCommand` | `navigator.clipboard.writeText()` with toast feedback | Modern API, works in all supported browsers, simpler |
| Table sorting | Custom sort logic | Sort in the backend via UUID v7 ordering (natural lexicographic sort of task IDs = chronological order) | UUID v7 task IDs are time-sortable; newest first = reverse lexicographic sort of SCAN results |
| Base64 decode + JSON parse | Manual step-by-step | Utility function: `atob()` then `JSON.parse()` with try/catch fallback to raw display | Payloads are stored as base64 in Redis; frontend decodes and attempts JSON parse |

## Common Pitfalls

### Pitfall 1: Redis SCAN Returns Variable Counts
**What goes wrong:** Expecting SCAN to return exactly `COUNT` items per call. It may return 0, 1, or many more.
**Why it happens:** COUNT is a hint, not a guarantee. Redis scans hash table slots, not keys.
**How to avoid:** Loop SCAN calls in the backend, accumulating results until the page is full or cursor returns 0. Set a maximum iteration limit (e.g., 100 iterations) to prevent infinite loops on sparse keyspaces.
**Warning signs:** Empty pages in the UI, inconsistent page sizes.

### Pitfall 2: Pending -> Failed Transition Not in State Machine
**What goes wrong:** Cancel handler fails with InvalidStateTransition when trying to cancel a Pending task.
**Why it happens:** The `TaskState::try_transition` function only allows `Pending -> Assigned`. Admin cancel from Pending is not currently a valid transition.
**How to avoid:** Add `(TaskState::Pending, TaskState::Failed)` to the `try_transition` match arms before implementing the cancel handler.
**Warning signs:** Cancel works for Running tasks but 409s on Pending tasks.

### Pitfall 3: Race Condition Between Cancel and Node Pickup
**What goes wrong:** Admin cancels a Pending task, but a node picks it up from the stream simultaneously.
**Why it happens:** Cancel sets the hash to Failed, but the stream entry is still there. XREADGROUP can deliver it to a node before the cancel's XACK.
**How to avoid:** The node's transition to Assigned will succeed (hash says Failed but node wrote Assigned). Mitigate by having the node check state after assignment. However, this is an inherent race condition in the architecture -- acceptable for admin use. The worst case is a task runs despite cancellation, which the admin can observe.
**Warning signs:** Task shows as Running after being cancelled.

### Pitfall 4: Task TTL Causes Disappearing Tasks
**What goes wrong:** Tasks vanish from the list between page loads because their Redis TTL expired.
**Why it happens:** Task hashes have an EXPIRE set during `submit_task()` (configured via `result_ttl_secs`).
**How to avoid:** This is by design -- expired tasks should disappear. The UI should handle this gracefully (task detail returns 404, show "Task not found or expired" message).
**Warning signs:** Task visible in list but 404 when clicking for details.

### Pitfall 5: Base64 Payload Display
**What goes wrong:** Payload shows as gibberish because it contains binary data that is not valid JSON.
**Why it happens:** The gateway treats payloads as opaque bytes. Some may be JSON, some may be protobuf or other binary formats.
**How to avoid:** In the JSON viewer: base64 decode, attempt `JSON.parse()`, if successful show formatted JSON, otherwise show raw base64 string with a note "Binary payload (base64)". Never assume payloads are JSON.
**Warning signs:** JSON parse errors in browser console, blank payload section.

### Pitfall 6: Missing Select Component
**What goes wrong:** Build fails because shadcn/ui Select is not installed.
**Why it happens:** Phase 8/9 did not need Select. The multi-select status filter and page size dropdown require it.
**How to avoid:** Run `npx shadcn@latest add select` before implementing filter controls. Also consider adding `popover` and `checkbox` if building a custom multi-select.
**Warning signs:** Import errors for `@/components/ui/select`.

## Code Examples

### Backend: SCAN-Based Task Listing
```rust
// In gateway/src/queue/redis.rs
use axum::extract::Query;

#[derive(Debug, Deserialize)]
pub struct ListTasksParams {
    pub cursor: Option<String>,
    pub page_size: Option<usize>,
    pub service: Option<String>,
    pub status: Option<String>,  // comma-separated: "pending,running"
}

pub async fn list_tasks(
    &self,
    cursor: Option<&str>,
    page_size: usize,
    service_filter: Option<&str>,
    status_filter: &[TaskState],
) -> Result<(Vec<TaskStatus>, Option<String>), GatewayError> {
    let mut conn = self.conn.clone();
    let mut redis_cursor: u64 = cursor
        .and_then(|c| c.parse().ok())
        .unwrap_or(0);
    let mut results = Vec::new();
    let mut iterations = 0;
    let max_iterations = 200;

    while results.len() < page_size && iterations < max_iterations {
        iterations += 1;
        // SCAN with MATCH task:* and COUNT hint
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(redis_cursor)
            .arg("MATCH")
            .arg("task:*")
            .arg("COUNT")
            .arg(page_size * 2)
            .query_async(&mut conn)
            .await
            .map_err(GatewayError::Redis)?;

        for key in &keys {
            if results.len() >= page_size {
                break;
            }
            let task_id_str = key.strip_prefix("task:").unwrap_or(key);
            let task_id = TaskId::from(task_id_str.to_string());
            match self.get_task_status(&task_id).await {
                Ok(status) => {
                    // Apply filters
                    if let Some(svc) = service_filter {
                        if status.service != svc {
                            continue;
                        }
                    }
                    if !status_filter.is_empty()
                        && !status_filter.contains(&status.state)
                    {
                        continue;
                    }
                    results.push(status);
                }
                Err(_) => continue, // key expired between SCAN and HGETALL
            }
        }

        redis_cursor = next_cursor;
        if redis_cursor == 0 {
            break; // End of keyspace
        }
    }

    let next_cursor = if redis_cursor == 0 {
        None
    } else {
        Some(redis_cursor.to_string())
    };

    Ok((results, next_cursor))
}
```

### Backend: Cancel Task
```rust
// In gateway/src/queue/redis.rs
pub async fn cancel_task(&self, task_id: &TaskId) -> Result<(), GatewayError> {
    let hash_key = format!("task:{}", task_id);
    let mut conn = self.conn.clone();

    let fields: HashMap<String, String> = redis::cmd("HGETALL")
        .arg(&hash_key)
        .query_async(&mut conn)
        .await
        .map_err(GatewayError::Redis)?;

    if fields.is_empty() {
        return Err(GatewayError::TaskNotFound(task_id.0.clone()));
    }

    let current_state =
        TaskState::from_str(fields.get("state").map(|s| s.as_str()).unwrap_or(""))?;

    // Validate cancellation is allowed (Pending or Running)
    current_state.try_transition(TaskState::Failed)?;

    let completed_at = chrono::Utc::now().to_rfc3339();
    let service = fields.get("service").cloned().unwrap_or_default();
    let stream_id = fields.get("stream_id").cloned().unwrap_or_default();

    let mut pipe = redis::pipe();
    pipe.cmd("HSET")
        .arg(&hash_key)
        .arg("state").arg(TaskState::Failed.as_str())
        .arg("error_message").arg("Cancelled by administrator")
        .arg("completed_at").arg(&completed_at)
        .ignore();

    if !stream_id.is_empty() {
        let stream_key = format!("tasks:{}", service);
        pipe.cmd("XACK")
            .arg(&stream_key)
            .arg("workers")
            .arg(&stream_id)
            .ignore();
    }

    let _: () = pipe
        .query_async(&mut conn)
        .await
        .map_err(GatewayError::Redis)?;

    Ok(())
}
```

### Frontend: Task Query Hook Pattern
```typescript
// admin-ui/src/lib/tasks.ts
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiClient } from './api'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'
import { toast } from 'sonner'

export interface TaskSummary {
  task_id: string
  state: string
  service: string
  created_at: string
  completed_at: string
}

export interface ListTasksResponse {
  tasks: TaskSummary[]
  cursor: string | null
}

export interface TaskFilters {
  cursor?: string
  page_size?: number
  service?: string
  status?: string  // comma-separated
  task_id?: string // direct lookup
}

export function useTasks(filters: TaskFilters) {
  const { effectiveInterval } = useAutoRefresh()
  const params = new URLSearchParams()
  if (filters.cursor) params.set('cursor', filters.cursor)
  if (filters.page_size) params.set('page_size', String(filters.page_size))
  if (filters.service) params.set('service', filters.service)
  if (filters.status) params.set('status', filters.status)
  if (filters.task_id) params.set('task_id', filters.task_id)

  return useQuery({
    queryKey: ['tasks', filters],
    queryFn: () =>
      apiClient<ListTasksResponse>(
        `/v1/admin/tasks?${params.toString()}`
      ),
    refetchInterval: effectiveInterval,
  })
}

export function useCancelTask() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (taskId: string) =>
      apiClient<void>(
        `/v1/admin/tasks/${encodeURIComponent(taskId)}/cancel`,
        { method: 'POST' },
      ),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['tasks'] })
      toast.success('Task cancelled')
    },
    onError: (error: Error) => {
      toast.error('Failed to cancel task. ' + error.message)
    },
  })
}
```

### Frontend: Base64 Payload Decoding Utility
```typescript
// Decode base64 payload, attempt JSON parse
export function decodePayload(base64: string): { type: 'json'; data: unknown } | { type: 'binary'; raw: string } {
  if (!base64) return { type: 'binary', raw: '' }
  try {
    const decoded = atob(base64)
    const parsed = JSON.parse(decoded)
    return { type: 'json', data: parsed }
  } catch {
    return { type: 'binary', raw: base64 }
  }
}
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| shadcn/ui v2 Radix-based Select | shadcn/ui v4 Base UI-based Select | 2025 | Component API may differ from v2 docs; use `npx shadcn@latest add select` to get correct version |
| Redis KEYS command | Redis SCAN command | Redis 2.8+ | SCAN is non-blocking, cursor-based; KEYS blocks entire Redis instance |

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework | cargo test (Rust, backend) |
| Config file | Cargo.toml |
| Quick run command | `cargo test --lib -p gateway` |
| Full suite command | `cargo test -p gateway` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| API-05 | GET /v1/admin/tasks returns paginated tasks with filters | integration | `cargo test --test admin_tasks -p gateway -- --ignored` | No -- Wave 0 |
| API-06 | POST /v1/admin/tasks/{id}/cancel sets Failed state | integration | `cargo test --test admin_tasks -p gateway -- --ignored` | No -- Wave 0 |
| TASK-01 | Task list page renders with filter controls | manual | Browser verification | N/A |
| TASK-02 | Task detail sheet shows all 4 sections | manual | Browser verification | N/A |
| TASK-03 | Cancel flow shows confirmation, executes, refreshes list | manual | Browser verification | N/A |

### Sampling Rate
- **Per task commit:** `cargo test --lib -p gateway`
- **Per wave merge:** `cargo test -p gateway`
- **Phase gate:** Full suite green + manual UI verification

### Wave 0 Gaps
- [ ] `gateway/src/queue/redis.rs` -- add `list_tasks` and `cancel_task` unit tests (gated with `#[ignore]` like existing tests)
- [ ] Verify `Pending -> Failed` transition added to `try_transition` -- add unit test in `types.rs`

## Open Questions

1. **Sort order for SCAN results**
   - What we know: Redis SCAN does not guarantee any ordering. UUID v7 task IDs are lexicographically time-sortable.
   - What's unclear: Whether to sort in the backend (collect all SCAN results, sort by task_id descending) or accept arbitrary SCAN order.
   - Recommendation: Sort in the backend after collecting the page. With page sizes of 10-50, sorting a Vec of TaskSummary by `task_id` descending is trivial. This gives newest-first ordering per the user's expectation.

2. **Multi-select for status filter**
   - What we know: shadcn/ui v4 Select component exists but is a single-select by default. Multi-select with checkboxes needs a custom component built from Popover + Checkbox primitives.
   - What's unclear: Whether shadcn/ui v4 has a built-in multi-select or requires composition.
   - Recommendation: Build a simple multi-select using shadcn/ui Popover + Checkbox components (add both via `npx shadcn@latest add popover checkbox`). This is a common shadcn/ui pattern -- a Popover trigger that contains a list of Checkbox items. Keep it simple.

3. **Task ID search vs filter interaction**
   - What we know: D-09 specifies a task ID search box. When a user searches by exact ID, the backend should do a direct HGETALL lookup, not SCAN.
   - What's unclear: Should the search clear other filters, or layer on top?
   - Recommendation: Task ID search is a separate mode. When a task ID is entered, bypass filters and do a direct lookup. Clear the search to return to filtered list view. This is simpler and matches the "quick jump" intent.

## Sources

### Primary (HIGH confidence)
- Project codebase: `gateway/src/queue/redis.rs` -- existing task data structures, submit/get/report patterns
- Project codebase: `gateway/src/types.rs` -- TaskState enum, try_transition valid transitions
- Project codebase: `gateway/src/http/admin.rs` -- existing admin handler patterns, response types
- Project codebase: `admin-ui/src/lib/services.ts` -- TanStack Query hook patterns, mutation patterns
- Project codebase: `admin-ui/src/components/node-table.tsx` -- Table component usage pattern
- Project codebase: `admin-ui/src/components/deregister-dialog.tsx` -- AlertDialog confirmation pattern

### Secondary (MEDIUM confidence)
- Redis SCAN documentation -- cursor behavior, COUNT hint semantics, variable result count
- shadcn/ui v4 component catalog -- Select, Popover, Checkbox availability

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies
- Architecture: HIGH -- patterns directly derived from existing codebase (Phase 8/9)
- Backend SCAN strategy: MEDIUM -- correct for admin use but performance at scale is untested
- Pitfalls: HIGH -- identified from code analysis of existing state machine and Redis patterns

**Research date:** 2026-03-23
**Valid until:** 2026-04-23 (stable stack, no external dependency changes expected)
