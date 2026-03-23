# Phase 11: Credential Management - Context

**Gathered:** 2026-03-23
**Status:** Ready for planning

<domain>
## Phase Boundary

Admin can manage API keys and node tokens for all services through the UI. This phase builds: (1) `GET /v1/admin/api-keys` list endpoint (API-03), (2) `GET /v1/admin/node-tokens` list endpoint (API-04), (3) backend extensions for label and expiry on both credential types, (4) credential list page with tabs for API keys and node tokens, (5) create credential flow with one-time secret reveal, (6) revoke credential flow with confirmation dialog.

</domain>

<decisions>
## Implementation Decisions

### Create flow & secret reveal
- **D-01:** Modal dialog with warning for secret reveal. After creation, dialog shows the raw key/token in a read-only field with copy button. Warning text: "This secret will not be shown again." Dialog has no close-X — only an explicit "I've copied it" button to dismiss.
- **D-02:** Secret is fully visible in the modal immediately (no masking/toggle). This is the one time the admin sees it.
- **D-03:** Create form collects: service(s) (multi-select for API keys, single for node tokens), optional human-readable label, optional expiry date, and optional callback URL (API keys only).
- **D-04:** Backend extensions needed: add `label` field to `store_api_key` and `store_node_token`, add optional TTL/expiry support for both credential types in Redis.

### Revoke flow
- **D-05:** Single revoke via row action button with confirmation dialog. Dialog text: "Revoke this key? Clients using it will immediately lose access. This cannot be undone." Consistent with deregister/cancel confirmation patterns from Phase 9/10.
- **D-06:** Optimistic removal after revoke — row disappears immediately with success toast. TanStack Query invalidates the list cache. If revoke fails, row reappears with error toast.

### Page layout
- **D-07:** Tabs for API keys vs node tokens on the credentials page. Each tab shows a data table. Consistent with the data table pattern from Phase 10 tasks page.

### Credential display
- **D-08:** Data table columns for API keys: masked hash (first 8 chars + "..."), associated services, label (if set), created date, expiry (if set), row actions (revoke).
- **D-09:** Data table columns for node tokens: masked hash (first 8 chars + "..."), service name, label (if set), created date, expiry (if set), row actions (revoke).
- **D-10:** No detail panel/sheet for credentials — table rows contain all relevant info. Simpler than tasks which needed payload/result viewing.

### Claude's Discretion
- Backend listing approach for API keys and node tokens (SCAN over `api_keys:*` and `node_tokens:*` patterns)
- Backend response format for list endpoints
- Tab component styling and placement
- Create dialog form layout and field validation
- Loading skeleton design for credential tables
- Empty state content for "no API keys" and "no node tokens"
- How to display expiry status (expired vs active, color coding)
- Whether callback URL field is a text input or URL input with validation

</decisions>

<canonical_refs>
## Canonical References

**Downstream agents MUST read these before planning or implementing.**

### Requirements
- `.planning/REQUIREMENTS.md` — CRED-01 through CRED-06, API-03, API-04

### Backend credential modules (existing create/revoke, need list endpoints)
- `gateway/src/auth/api_key.rs` — `generate_api_key()`, `store_api_key()`, `lookup_api_key()`, `revoke_api_key()`, `ClientMetadata` struct, Redis key pattern `api_keys:<hash>`
- `gateway/src/auth/node_token.rs` — `generate_node_token()`, `store_node_token()`, `validate_node_token()`, `revoke_node_token()`, `NodeTokenMetadata` struct, Redis key pattern `node_tokens:<service>:<hash>`

### Existing admin endpoints (patterns to follow)
- `gateway/src/http/admin.rs` lines 30-200 — Existing `create_api_key`, `revoke_api_key`, `update_api_key_callback`, `create_node_token`, `revoke_node_token` handlers
- `gateway/src/main.rs` lines 249-255 — Admin route registration

### Frontend foundation (Phase 8)
- `admin-ui/src/lib/api.ts` — `apiClient()` fetch wrapper with cookie auth
- `admin-ui/src/components/empty-state.tsx` — EmptyState component
- `admin-ui/src/components/error-alert.tsx` — ErrorAlert with retry button
- `admin-ui/src/routes/_authenticated/credentials.tsx` — Existing placeholder page to replace

### Frontend patterns (Phase 9/10)
- `admin-ui/src/lib/tasks.ts` — TanStack Query hooks pattern for data fetching (follow for credentials)
- `admin-ui/src/lib/services.ts` — Service hooks pattern (follow for credentials)
- `admin-ui/src/routes/_authenticated/tasks.tsx` — Data table with filters, row actions, confirmation dialog pattern

</canonical_refs>

<code_context>
## Existing Code Insights

### Reusable Assets
- `EmptyState` component: For "no credentials" tab states
- `ErrorAlert` component: For API failure states with retry button
- `apiClient()`: Typed fetch wrapper — use for all credential API calls
- TanStack Query: Already set up with auto-refresh from header control
- shadcn/ui components: Table, Tabs, Button, Dialog, Badge, Input already available
- Confirmation dialog pattern from Phase 9 (deregister) and Phase 10 (cancel)
- Toast notifications via Sonner for create/revoke success/failure

### Established Patterns
- TanStack Router file-based routes under `_authenticated/`
- `credentials: 'include'` on all API calls for cookie session
- Data table with row actions pattern from Phase 10 tasks page
- SCAN-based Redis key listing pattern from Phase 10 task listing
- Hook files in `admin-ui/src/lib/` (services.ts, tasks.ts) — create matching `credentials.ts`

### Integration Points
- `_authenticated/credentials.tsx` — Replace placeholder with credential management page
- Sidebar nav already links to `/credentials` — no navigation changes needed
- Backend `admin.rs` already has create/revoke handlers — add list handlers alongside
- Redis SCAN over `api_keys:*` and `node_tokens:*:*` key patterns for listing

</code_context>

<specifics>
## Specific Ideas

- Create form is richer than minimal: includes optional label, optional expiry date, and optional callback URL (API keys) alongside service selection
- Backend needs extending: `store_api_key` and `store_node_token` need label field; both need optional TTL/expiry support in Redis
- The "I've copied it" button (instead of close-X) prevents admins from accidentally closing the secret reveal without copying — important for operational safety

</specifics>

<deferred>
## Deferred Ideas

None — discussion stayed within phase scope

</deferred>

---

*Phase: 11-credential-management*
*Context gathered: 2026-03-23*
