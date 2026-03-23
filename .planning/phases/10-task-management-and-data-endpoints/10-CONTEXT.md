# Phase 10: Task Management and Data Endpoints - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Admin can browse, inspect, and cancel tasks through the UI backed by new paginated backend endpoints. This phase builds: (1) `GET /v1/admin/tasks` with cursor-based pagination and service/status filters, (2) `POST /v1/admin/tasks/{task_id}/cancel` endpoint, (3) task list page with data table, (4) task detail slide-out panel, (5) task cancel flow with confirmation.

</domain>

<decisions>
## Implementation Decisions

### Task list layout
- **D-01:** Data table (not card grid) — sortable columns: Task ID (truncated), Service, Status, Created, Completed. Consistent with node table pattern from Phase 9.
- **D-02:** Task ID column shows first 8 chars of UUID v7 with copy-to-clipboard on hover. Full ID visible in detail panel.
- **D-03:** Status displayed as colored pill/badge — yellow Pending, blue Running, green Completed, red Failed. Consistent with node health badge pattern.
- **D-04:** Payload and result data NOT shown in the table — detail panel only. Keeps table clean for scanning.

### Filtering and pagination
- **D-05:** Cursor-based pagination with Next/Previous buttons. Natural fit for Redis SCAN. No total count required.
- **D-06:** Configurable page size: 10/25/50 via dropdown control. Default 25.
- **D-07:** Service filter — dropdown populated from existing service list endpoint.
- **D-08:** Status filter — dropdown multi-select with checkboxes (shadcn/ui Select). Filter by multiple statuses at once (e.g., Pending + Running).
- **D-09:** Task ID search — search box for looking up a specific task by ID. Quick jump to known task.
- **D-10:** Date range filter NOT included — keeps the UI simpler.

### Task detail view
- **D-11:** Slide-out panel (shadcn/ui Sheet) — click a table row opens a side sheet. Task list stays visible behind.
- **D-12:** Panel width fixed at ~50% viewport width. Enough room for JSON payloads.
- **D-13:** Four sections in the panel:
  1. **Task info header** — Full task ID (copyable), status badge, service name, created/completed timestamps
  2. **Metadata** — Key-value table of task metadata/labels
  3. **Payload** — Formatted JSON viewer with syntax highlighting. Base64-decode, attempt JSON parse, fallback to raw base64 for non-JSON. Copy-to-clipboard button.
  4. **Result** — Same formatted JSON viewer for result payload (if completed/failed), plus error message if failed.

### Cancel flow
- **D-14:** Cancel action available in BOTH the detail panel AND as a table row action (dropdown menu or icon button). Both open confirmation dialog.
- **D-15:** Pending and Running tasks can be cancelled. Completed and Failed tasks cannot.
- **D-16:** Standard confirmation dialog: "Cancel task {short-id}? This will mark it as failed for the client. This action cannot be undone." with Cancel/Confirm buttons. Consistent with deregister confirmation pattern.
- **D-17:** Backend `POST /v1/admin/tasks/{task_id}/cancel` marks the task as failed state with an error message indicating admin cancellation.

### Claude's Discretion
- Redis enumeration strategy for task listing (SCAN over `task:*` keys vs reading stream entries)
- Backend response format for paginated task list (cursor encoding, response shape)
- Exact JSON syntax highlighting approach (pre tag with styling vs dedicated component)
- Table row action menu design (icon button vs three-dot dropdown)
- Loading skeleton design for task table
- Empty state content for "no tasks" and "no matching tasks" (filter applied)
- Sort order defaults (newest first via UUID v7 ordering)

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — TASK-01, TASK-02, TASK-03, API-05, API-06

### Backend task data structures
- `gateway/src/queue/redis.rs` lines 7-19 — `TaskStatus` struct (task_id, state, service, payload, result, error_message, metadata, created_at, completed_at, stream_id)
- `gateway/src/queue/redis.rs` lines 164-210 — `get_task_status()` implementation showing Redis hash key pattern `task:{id}`
- `gateway/src/queue/redis.rs` lines 98-160 — `submit_task()` showing Redis storage pattern (hash + stream)
- `gateway/src/types.rs` lines 4-23 — `TaskId` newtype, `TaskState` enum (Pending, Assigned, Running, Completed, Failed)

### Existing admin endpoints (patterns to follow)
- `gateway/src/http/admin.rs` — Service CRUD handlers, response types, admin auth middleware pattern
- `gateway/src/main.rs` lines 249-255 — Admin route registration

### Frontend foundation (Phase 8)
- `admin-ui/src/lib/api.ts` — `apiClient()` fetch wrapper with cookie auth
- `admin-ui/src/components/empty-state.tsx` — Reusable EmptyState component
- `admin-ui/src/components/error-alert.tsx` — ErrorAlert with retry button
- `admin-ui/src/components/page-skeleton.tsx` — PageSkeleton loading component
- `admin-ui/src/routes/_authenticated/tasks.tsx` — Existing stub route (placeholder to replace)

### Frontend patterns (Phase 9)
- `admin-ui/src/routes/_authenticated/services.tsx` — Service list page with card grid, registration dialog (pattern reference for list pages)
- `admin-ui/src/routes/_authenticated/services.$name.tsx` — Service detail with breadcrumbs, node table, deregister flow

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EmptyState` component: For "no tasks" state — accepts icon, heading, description
- `ErrorAlert` component: For API failure states with retry button
- `PageSkeleton` component: Customize for table skeleton
- `apiClient()`: Typed fetch wrapper — use for all task API calls
- TanStack Query: Already set up with auto-refresh from header control
- shadcn/ui components: Table, Sheet, Button, Dialog, Badge, Select already installed
- Health badge pattern from Phase 9: Color dot + status text — adapt for task status badges

### Established Patterns
- TanStack Router file-based routes under `_authenticated/`
- `credentials: 'include'` on all API calls for cookie session
- Sonner for toast notifications on mutations (cancel success/failure)
- Auto-refresh control in header via TanStack Query `refetchInterval`
- Confirmation dialog pattern from service deregister flow

### Integration Points
- `_authenticated/tasks.tsx` — Replace placeholder with task list page
- Sidebar nav already links to `/tasks` — no navigation changes needed
- Cancel mutation will POST to `/v1/admin/tasks/{task_id}/cancel` and invalidate the task list query
- Service filter dropdown populated from existing `GET /v1/admin/services` endpoint

</code_context>

<specifics>
## Specific Ideas

- Slide-out panel chosen over separate detail page — admin wants to browse tasks while viewing details, not navigate away from the list
- Task ID truncation to 8 chars matches common UUID display patterns (like Git short hashes)
- JSON formatting for payloads/results is important since this is an opaque-payload gateway — the admin UI is where operators get visibility into what's flowing through

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 10-task-management-and-data-endpoints*
*Context gathered: 2026-03-23*
