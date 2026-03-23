# Phase 10: Task Management and Data Endpoints - Discussion Log

> **Audit trail only.** Do not use as input to planning, research, or execution agents.
> Decisions are captured in CONTEXT.md — this log preserves the alternatives considered.

**Date:** 2026-03-23
**Phase:** 10-task-management-and-data-endpoints
**Areas discussed:** Task list layout, Filtering and pagination, Task detail view, Cancel flow

---

## Task List Layout

| Option | Description | Selected |
|--------|-------------|----------|
| Data table | Table with sortable columns — best for scanning many tasks. Consistent with node table pattern. | ✓ |
| Card grid | One card per task. Better visual density per item but worse for large volumes. | |
| Compact list | Minimal rows, no borders. Very dense, good for monitoring. | |

**User's choice:** Data table
**Notes:** Consistent with node table pattern from Phase 9

| Option | Description | Selected |
|--------|-------------|----------|
| Color badge | Colored pill matching task state — yellow/blue/green/red. Consistent with node health badges. | ✓ |
| Text only | Plain text status with no color coding. | |
| Icon + text | Status icon plus colored text. | |

**User's choice:** Color badge

| Option | Description | Selected |
|--------|-------------|----------|
| Truncated with copy | First 8 chars of UUID v7, copy-to-clipboard on hover. Full ID in detail. | ✓ |
| Full UUID | Complete UUID in the table. | |
| Short hash | 6-char hash, loses time-sort property. | |

**User's choice:** Truncated with copy

| Option | Description | Selected |
|--------|-------------|----------|
| No — detail page only | Keep table clean. Payloads can be large. | ✓ |
| Payload size indicator | Show size in bytes. | |
| Truncated preview | First 50 chars of decoded payload. | |

**User's choice:** No — detail only

---

## Filtering and Pagination

| Option | Description | Selected |
|--------|-------------|----------|
| Cursor-based | Next/Previous with cursor tokens. Natural fit for Redis SCAN. | ✓ |
| Offset-based | Traditional page numbers. Requires counting all keys (expensive). | |
| Infinite scroll | Load more on scroll. | |

**User's choice:** Cursor-based

**Filters selected (multi-select):**
- ✓ Service filter — dropdown from existing service list
- ✓ Status filter — multi-select dropdown with checkboxes
- ✗ Date range filter — excluded, keeps UI simpler
- ✓ Task ID search — search box for quick lookup

| Option | Description | Selected |
|--------|-------------|----------|
| Dropdown multi-select | shadcn/ui Select with checkboxes. Filter by multiple statuses. | ✓ |
| Tab bar | Horizontal tabs, one status at a time. | |
| Toggle buttons | Row of toggleable filter pills. | |

**User's choice:** Dropdown multi-select

| Option | Description | Selected |
|--------|-------------|----------|
| 25 per page | Good balance for admin scanning. | |
| 50 per page | More data, less paging. | |
| Configurable (10/25/50) | Dropdown to choose page size. | ✓ |

**User's choice:** Configurable (10/25/50)

---

## Task Detail View

| Option | Description | Selected |
|--------|-------------|----------|
| Separate detail page | /tasks/$taskId page with breadcrumbs. Consistent with service detail. | |
| Slide-out panel | Side sheet keeps list visible. | ✓ |
| Expandable row | Inline expansion. | |

**User's choice:** Slide-out panel
**Notes:** User chose panel over page — wants to browse tasks while viewing details

| Option | Description | Selected |
|--------|-------------|----------|
| Formatted JSON with copy | Parse base64, syntax-highlight JSON, fallback to raw. Copy button. | ✓ |
| Raw base64 | Show raw string as-is. | |
| Hex dump + text | Hex representation with ASCII sidebar. | |

**User's choice:** Formatted JSON with copy

**Detail sections selected (multi-select):**
- ✓ Task info header — ID, status badge, service, timestamps
- ✓ Metadata section — key-value table
- ✓ Payload section — formatted JSON viewer
- ✓ Result section — formatted JSON viewer + error message

| Option | Description | Selected |
|--------|-------------|----------|
| Fixed wide (~50% viewport) | Enough room for JSON, list still visible. | ✓ |
| Adjustable/draggable | User can resize panel edge. | |
| Full width overlay | Takes full screen like modal. | |

**User's choice:** Fixed wide

---

## Cancel Flow

| Option | Description | Selected |
|--------|-------------|----------|
| Detail panel + table row | Cancel in both locations. Both open confirmation dialog. | ✓ |
| Detail panel only | Requires opening detail to cancel. | |
| Table row only | Quick but no detail context. | |

**User's choice:** Detail panel + table row

| Option | Description | Selected |
|--------|-------------|----------|
| Pending + Running | Both can be cancelled. Backend marks as failed. Per TASK-03. | ✓ |
| Pending only | Only unassigned tasks. | |
| All non-terminal | Same as Pending + Running in practice. | |

**User's choice:** Pending + Running

| Option | Description | Selected |
|--------|-------------|----------|
| Standard confirmation | "Cancel task {id}? Marked as failed. Cannot be undone." | ✓ |
| Type-to-confirm | Require typing task ID. Extra friction. | |
| No confirmation | Immediate cancel. | |

**User's choice:** Standard confirmation

---

## Claude's Discretion

- Redis enumeration strategy for task listing
- Backend response format for paginated list
- JSON syntax highlighting approach
- Table row action menu design
- Loading skeleton design
- Empty state content
- Sort order defaults
