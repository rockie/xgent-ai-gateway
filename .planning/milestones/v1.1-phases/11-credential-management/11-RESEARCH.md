# Phase 11: Credential Management - Research

**Researched:** 2026-03-23
**Domain:** Rust backend (Axum/Redis SCAN) + React frontend (TanStack Query/shadcn/ui)
**Confidence:** HIGH

## Summary

Phase 11 adds list endpoints for API keys and node tokens on the backend, extends the existing `store_api_key` and `store_node_token` functions with label and expiry support, and replaces the frontend credentials placeholder page with a tabbed data table UI featuring create and revoke flows.

The backend work is straightforward: two new GET handlers using Redis SCAN (identical pattern to Phase 10 task listing), plus extending two existing store functions to write `label` and `expires_at` fields. The frontend work follows established Phase 9/10 patterns exactly -- TanStack Query hooks in a `credentials.ts` data layer file, a tabbed page with data tables, Dialog for create, AlertDialog for revoke confirmation. The `Tabs` shadcn/ui component must be installed (not currently present).

**Primary recommendation:** Follow the Phase 10 SCAN-based listing pattern for backend, Phase 9 Dialog/AlertDialog patterns for frontend. The create flow needs a custom Dialog (not AlertDialog) because it must show the one-time secret with a forced "I've copied it" dismissal button.

<user_constraints>

## User Constraints (from CONTEXT.md)

### Locked Decisions
- **D-01:** Modal dialog with warning for secret reveal. After creation, dialog shows the raw key/token in a read-only field with copy button. Warning text: "This secret will not be shown again." Dialog has no close-X -- only an explicit "I've copied it" button to dismiss.
- **D-02:** Secret is fully visible in the modal immediately (no masking/toggle). This is the one time the admin sees it.
- **D-03:** Create form collects: service(s) (multi-select for API keys, single for node tokens), optional human-readable label, optional expiry date, and optional callback URL (API keys only).
- **D-04:** Backend extensions needed: add `label` field to `store_api_key` and `store_node_token`, add optional TTL/expiry support for both credential types in Redis.
- **D-05:** Single revoke via row action button with confirmation dialog. Dialog text: "Revoke this key? Clients using it will immediately lose access. This cannot be undone." Consistent with deregister/cancel confirmation patterns from Phase 9/10.
- **D-06:** Optimistic removal after revoke -- row disappears immediately with success toast. TanStack Query invalidates the list cache. If revoke fails, row reappears with error toast.
- **D-07:** Tabs for API keys vs node tokens on the credentials page. Each tab shows a data table. Consistent with the data table pattern from Phase 10 tasks page.
- **D-08:** Data table columns for API keys: masked hash (first 8 chars + "..."), associated services, label (if set), created date, expiry (if set), row actions (revoke).
- **D-09:** Data table columns for node tokens: masked hash (first 8 chars + "..."), service name, label (if set), created date, expiry (if set), row actions (revoke).
- **D-10:** No detail panel/sheet for credentials -- table rows contain all relevant info. Simpler than tasks which needed payload/result viewing.

### Claude's Discretion
- Backend listing approach for API keys and node tokens (SCAN over `api_keys:*` and `node_tokens:*` patterns)
- Backend response format for list endpoints
- Tab component styling and placement
- Create dialog form layout and field validation
- Loading skeleton design for credential tables
- Empty state content for "no API keys" and "no node tokens"
- How to display expiry status (expired vs active, color coding)
- Whether callback URL field is a text input or URL input with validation

### Deferred Ideas (OUT OF SCOPE)
None -- discussion stayed within phase scope

</user_constraints>

<phase_requirements>

## Phase Requirements

| ID | Description | Research Support |
|----|-------------|------------------|
| CRED-01 | Admin can list API keys (masked hash, associated services) | Backend: SCAN `api_keys:*` pattern + HGETALL per key. Frontend: `useApiKeys()` hook + data table. |
| CRED-02 | Admin can create API key (shown once with copy-to-clipboard) | Backend: existing `create_api_key` handler extended with label/expiry. Frontend: Dialog with secret reveal + copy button. |
| CRED-03 | Admin can revoke API key with confirmation | Backend: existing `revoke_api_key` handler. Frontend: AlertDialog confirmation + optimistic removal. |
| CRED-04 | Admin can list node tokens per service (masked hash, label) | Backend: SCAN `node_tokens:*:*` pattern + HGETALL per key. Frontend: `useNodeTokens()` hook + data table. |
| CRED-05 | Admin can create node token (shown once with copy-to-clipboard) | Backend: existing `create_node_token` handler extended with label/expiry. Frontend: shared secret reveal Dialog. |
| CRED-06 | Admin can revoke node token with confirmation | Backend: existing `revoke_node_token` handler. Frontend: AlertDialog confirmation + optimistic removal. |
| API-03 | GET /v1/admin/api-keys list endpoint | New handler in `admin.rs`, SCAN-based listing function in `api_key.rs`. Response: `{ api_keys: [...] }`. |
| API-04 | GET /v1/admin/node-tokens list endpoint | New handler in `admin.rs`, SCAN-based listing function in `node_token.rs`. Response: `{ node_tokens: [...] }`. |

</phase_requirements>

## Standard Stack

### Core (already in project)
| Library | Version | Purpose | Why Standard |
|---------|---------|---------|--------------|
| Axum | 0.8.x | HTTP handlers for list endpoints | Already used for all admin routes |
| redis-rs | 1.0.x | SCAN + HGETALL for credential listing | Already used for task listing (Phase 10) |
| TanStack Query | 5.x | Data fetching hooks for credential lists | Already used for services/tasks |
| shadcn/ui | v4 | Table, Dialog, AlertDialog, Tabs components | Already used throughout admin UI |
| Sonner | (via shadcn) | Toast notifications for create/revoke | Already used for all mutations |

### New Component to Install
| Component | shadcn name | Purpose |
|-----------|-------------|---------|
| Tabs | `tabs` | API keys / node tokens tab switching on credentials page |

**Installation:**
```bash
cd admin-ui && npx shadcn@latest add tabs
```

### Alternatives Considered
| Instead of | Could Use | Tradeoff |
|------------|-----------|----------|
| SCAN-based listing | Redis SET index of key hashes | SET index adds write complexity on create/delete but gives O(1) membership + SMEMBERS listing. SCAN is simpler and consistent with Phase 10. |
| Tabs component | URL-based routing (two separate pages) | Tabs are simpler, keep credentials together, match D-07 decision. |

## Architecture Patterns

### Backend: New Files/Changes

```
gateway/src/
├── auth/
│   ├── api_key.rs          # ADD: list_api_keys() function, ADD label+expires_at to store_api_key()
│   └── node_token.rs       # ADD: list_node_tokens() function, ADD label to store_node_token() (already has node_label)
├── http/
│   └── admin.rs            # ADD: list_api_keys handler, list_node_tokens handler, extend create request structs
└── main.rs                 # ADD: GET routes for /v1/admin/api-keys and /v1/admin/node-tokens
```

### Frontend: New Files/Changes

```
admin-ui/src/
├── lib/
│   └── credentials.ts                    # NEW: types, useApiKeys(), useNodeTokens(), useCreateApiKey(), useRevokeApiKey(), useCreateNodeToken(), useRevokeNodeToken()
├── components/
│   ├── credential-table.tsx              # NEW: shared table for API keys and node tokens
│   ├── create-credential-dialog.tsx      # NEW: create form dialog (service selection, label, expiry, callback URL)
│   ├── secret-reveal-dialog.tsx          # NEW: one-time secret display with copy + "I've copied it" button
│   └── revoke-credential-dialog.tsx      # NEW: AlertDialog confirmation for revoke
├── components/ui/
│   └── tabs.tsx                          # NEW: install via shadcn CLI
└── routes/_authenticated/
    └── credentials.tsx                   # REPLACE: placeholder with tabbed credential page
```

### Pattern 1: SCAN-Based Redis Listing (Backend)
**What:** Use Redis SCAN to iterate over key patterns and HGETALL each match
**When to use:** Listing credentials by key pattern (`api_keys:*`, `node_tokens:*:*`)
**Example:**
```rust
// Source: gateway/src/queue/redis.rs (existing task listing pattern)
pub async fn list_api_keys(
    conn: &mut redis::aio::MultiplexedConnection,
) -> Result<Vec<ClientMetadata>, redis::RedisError> {
    let mut cursor: u64 = 0;
    let mut results = Vec::new();
    loop {
        let (next_cursor, keys): (u64, Vec<String>) = redis::cmd("SCAN")
            .arg(cursor)
            .arg("MATCH")
            .arg("api_keys:*")
            .arg("COUNT")
            .arg(100)
            .query_async(conn)
            .await?;
        for key in &keys {
            let hash: std::collections::HashMap<String, String> =
                conn.hgetall(key).await.unwrap_or_default();
            if hash.is_empty() { continue; }
            // Parse fields into ClientMetadata...
            results.push(/* parsed metadata */);
        }
        cursor = next_cursor;
        if cursor == 0 { break; }
    }
    Ok(results)
}
```

### Pattern 2: TanStack Query Hook (Frontend)
**What:** Data layer hooks following services.ts/tasks.ts pattern
**When to use:** All credential data fetching and mutations
**Example:**
```typescript
// Source: admin-ui/src/lib/services.ts (existing pattern)
export function useApiKeys() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['api-keys'],
    queryFn: () => apiClient<ListApiKeysResponse>('/v1/admin/api-keys'),
    refetchInterval: effectiveInterval,
  })
}

export function useCreateApiKey() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (data: CreateApiKeyRequest) =>
      apiClient<CreateApiKeyResponse>('/v1/admin/api-keys', {
        method: 'POST',
        body: JSON.stringify(data),
      }),
    // NOTE: Do NOT invalidate on success here -- the secret reveal dialog
    // needs the response data. Invalidate after the reveal dialog closes.
  })
}
```

### Pattern 3: Forced Dismissal Dialog (Frontend - New Pattern)
**What:** Dialog without close-X, dismissable only via explicit "I've copied it" button
**When to use:** One-time secret reveal after credential creation (D-01)
**Example:**
```typescript
// Use shadcn Dialog with onInteractOutside prevented
<Dialog open={open}>
  <DialogContent
    onInteractOutside={(e) => e.preventDefault()}
    onEscapeKeyDown={(e) => e.preventDefault()}
    // shadcn Dialog renders a close button by default via DialogContent.
    // Override by hiding it with className or restructuring.
    className="[&>button]:hidden"
  >
    {/* Secret display + "I've copied it" button */}
  </DialogContent>
</Dialog>
```

### Pattern 4: Optimistic Revoke with Rollback (Frontend)
**What:** Remove row immediately on revoke, reappear if API fails (D-06)
**When to use:** Credential revocation
**Example:**
```typescript
export function useRevokeApiKey() {
  const queryClient = useQueryClient()
  return useMutation({
    mutationFn: (keyHash: string) =>
      apiClient<void>('/v1/admin/api-keys/revoke', {
        method: 'POST',
        body: JSON.stringify({ key_hash: keyHash }),
      }),
    onMutate: async (keyHash) => {
      await queryClient.cancelQueries({ queryKey: ['api-keys'] })
      const previous = queryClient.getQueryData(['api-keys'])
      queryClient.setQueryData(['api-keys'], (old: any) => ({
        ...old,
        api_keys: old.api_keys.filter((k: any) => k.key_hash !== keyHash),
      }))
      return { previous }
    },
    onError: (_err, _keyHash, context) => {
      queryClient.setQueryData(['api-keys'], context?.previous)
      toast.error('Failed to revoke API key.')
    },
    onSuccess: () => {
      toast.success('API key revoked')
    },
    onSettled: () => {
      queryClient.invalidateQueries({ queryKey: ['api-keys'] })
    },
  })
}
```

### Anti-Patterns to Avoid
- **Returning raw secrets in list endpoints:** List endpoints MUST only return the hash (or masked hash). The raw key/token is only returned at creation time.
- **Sharing create mutation response across components:** The create mutation response contains the raw secret. Pass it explicitly to the reveal dialog; never store it in query cache.
- **Using AlertDialog for create flow:** AlertDialog is for confirmations. Use Dialog for the create form and secret reveal, AlertDialog only for revoke confirmation.

## Don't Hand-Roll

| Problem | Don't Build | Use Instead | Why |
|---------|-------------|-------------|-----|
| Clipboard copy | Manual `document.execCommand` | `navigator.clipboard.writeText()` | Already used in task-table.tsx. Modern API, works in secure contexts. |
| Tab state management | Custom state + conditional rendering | shadcn `Tabs` component | Accessible, keyboard-navigable, handles ARIA roles automatically. |
| Confirmation dialogs | Custom modal logic | shadcn `AlertDialog` | Already established pattern in deregister-dialog.tsx and task-cancel-dialog.tsx. |
| Toast notifications | Custom notification system | Sonner (via shadcn) | Already wired up project-wide. |

**Key insight:** Every UI pattern needed in this phase has an existing implementation in Phase 9 or 10. The credential page is essentially a simpler version of the tasks page (no detail sheet, no pagination, no filters) with a create flow added.

## Common Pitfalls

### Pitfall 1: Redis SCAN Pattern for Node Tokens
**What goes wrong:** Node token keys use a three-segment pattern `node_tokens:<service>:<hash>`, not two-segment like API keys `api_keys:<hash>`. A naive SCAN with `node_tokens:*` will match, but parsing requires splitting on `:` and handling the service name extraction.
**Why it happens:** Different key structures between the two credential types.
**How to avoid:** Use `node_tokens:*:*` as the SCAN MATCH pattern. When parsing, strip the `node_tokens:` prefix, then split on `:` to get `(service_name, token_hash)`.
**Warning signs:** Node tokens listing returns empty or malformed service names.

### Pitfall 2: TTL/Expiry in Redis vs Application-Level Expiry
**What goes wrong:** Using Redis TTL (`EXPIRE`) means expired credentials silently disappear from listings. Admin cannot see that a credential expired -- it just vanishes.
**Why it happens:** Redis TTL deletes keys automatically.
**How to avoid:** Store `expires_at` as a hash field (application-level expiry). Check expiry in the auth middleware (`lookup_api_key`/`validate_node_token`) and reject expired credentials. List endpoint returns expired credentials with an "expired" status so admin can see and clean them up.
**Warning signs:** Credentials disappear from listings without admin action.

### Pitfall 3: Create API Key Request Body Changes
**What goes wrong:** The existing `CreateApiKeyRequest` struct in `admin.rs` has `service_names` and `callback_url`. Adding `label` and `expires_at` fields is additive and backward-compatible, but the frontend must send the new fields.
**Why it happens:** Backend struct extension without frontend coordination.
**How to avoid:** Add fields as `Option<String>` / `Option<String>` (ISO 8601 for expiry) to the existing request struct. Existing API callers (non-UI) continue to work since new fields are optional.
**Warning signs:** 400 errors from missing required fields, or labels not being stored.

### Pitfall 4: Dialog Close Prevention
**What goes wrong:** shadcn/ui Dialog has a built-in close button (X) in DialogContent. D-01 requires no close-X -- only the "I've copied it" button should dismiss.
**Why it happens:** Default DialogContent renders a close button.
**How to avoid:** Hide the close button with `className="[&>button]:hidden"` on DialogContent, and prevent escape/outside-click with `onInteractOutside` and `onEscapeKeyDown` event prevention.
**Warning signs:** Admin can close the secret reveal dialog without copying the secret.

### Pitfall 5: Multi-Select for API Key Services
**What goes wrong:** API keys can be associated with multiple services (D-03). A standard `<Select>` only allows single selection. Need a multi-select component.
**Why it happens:** shadcn/ui does not have a built-in multi-select component.
**How to avoid:** Use a Popover with Checkboxes for service selection (same pattern as status filter in tasks page). The services list is fetched via `useServices()` hook. Or use multiple Badge + X buttons for selected services.
**Warning signs:** Admin can only select one service for API keys.

## Code Examples

### Backend: List API Keys Handler
```rust
// In admin.rs
#[derive(Debug, Serialize)]
pub struct ApiKeyListItem {
    pub key_hash: String,
    pub service_names: Vec<String>,
    pub label: Option<String>,
    pub created_at: String,
    pub expires_at: Option<String>,
    pub callback_url: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ListApiKeysResponse {
    pub api_keys: Vec<ApiKeyListItem>,
}

pub async fn list_api_keys(
    State(state): State<Arc<AppState>>,
) -> Result<Json<ListApiKeysResponse>, GatewayError> {
    let keys = api_key::list_api_keys(&mut state.auth_conn.clone())
        .await
        .map_err(GatewayError::Redis)?;
    Ok(Json(ListApiKeysResponse { api_keys: keys }))
}
```

### Backend: Extended store_api_key Signature
```rust
// In api_key.rs -- add label and expires_at parameters
pub async fn store_api_key(
    conn: &mut redis::aio::MultiplexedConnection,
    key_hash: &str,
    service_names: &[String],
    callback_url: Option<&str>,
    label: Option<&str>,         // NEW
    expires_at: Option<&str>,    // NEW -- ISO 8601 string
) -> Result<(), redis::RedisError> {
    let redis_key = format!("api_keys:{key_hash}");
    let now = chrono::Utc::now().to_rfc3339();
    let services_csv = service_names.join(",");
    let mut pipe = redis::pipe();
    pipe.hset(&redis_key, "service_names", &services_csv)
        .hset(&redis_key, "created_at", &now);
    if let Some(url) = callback_url {
        pipe.hset(&redis_key, "callback_url", url);
    }
    if let Some(lbl) = label {
        pipe.hset(&redis_key, "label", lbl);
    }
    if let Some(exp) = expires_at {
        pipe.hset(&redis_key, "expires_at", exp);
    }
    pipe.query_async(conn).await
}
```

### Frontend: Credential Hooks (credentials.ts)
```typescript
// Source: follows admin-ui/src/lib/services.ts pattern exactly
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'
import { apiClient } from './api'
import { useAutoRefresh } from '@/hooks/use-auto-refresh'
import { toast } from 'sonner'

export interface ApiKeyListItem {
  key_hash: string
  service_names: string[]
  label: string | null
  created_at: string
  expires_at: string | null
  callback_url: string | null
}

export interface ListApiKeysResponse {
  api_keys: ApiKeyListItem[]
}

export function useApiKeys() {
  const { effectiveInterval } = useAutoRefresh()
  return useQuery({
    queryKey: ['api-keys'],
    queryFn: () => apiClient<ListApiKeysResponse>('/v1/admin/api-keys'),
    refetchInterval: effectiveInterval,
  })
}
```

### Frontend: Revoke Confirmation Dialog
```typescript
// Source: follows admin-ui/src/components/task-cancel-dialog.tsx pattern
// Uses AlertDialog, destructive styling, same button states
<AlertDialog open={open} onOpenChange={onOpenChange}>
  <AlertDialogContent>
    <AlertDialogHeader>
      <AlertDialogTitle>Revoke this key?</AlertDialogTitle>
      <AlertDialogDescription>
        Clients using it will immediately lose access. This cannot be undone.
      </AlertDialogDescription>
    </AlertDialogHeader>
    <AlertDialogFooter>
      <AlertDialogCancel>Cancel</AlertDialogCancel>
      <AlertDialogAction
        className="bg-destructive text-destructive-foreground hover:bg-destructive/90"
        disabled={revokeMutation.isPending}
        onClick={handleRevoke}
      >
        {revokeMutation.isPending ? 'Revoking...' : 'Revoke'}
      </AlertDialogAction>
    </AlertDialogFooter>
  </AlertDialogContent>
</AlertDialog>
```

### Route Registration (main.rs)
```rust
// Add GET to existing POST routes
.route(
    "/v1/admin/api-keys",
    axum::routing::post(http::admin::create_api_key)
        .get(http::admin::list_api_keys),       // ADD
)
.route(
    "/v1/admin/node-tokens",
    axum::routing::post(http::admin::create_node_token)
        .get(http::admin::list_node_tokens),     // ADD
)
```

## State of the Art

| Old Approach | Current Approach | When Changed | Impact |
|--------------|------------------|--------------|--------|
| Separate create and reveal steps | Single create mutation returns raw secret, displayed in immediate dialog | Phase 11 design | Simpler UX, one fewer API call |
| No credential listing | SCAN-based listing endpoints | Phase 11 | Admin can see all credentials at a glance |
| No label/expiry on credentials | `label` and `expires_at` hash fields | Phase 11 | Better credential lifecycle management |

## Open Questions

1. **Service multi-select UX**
   - What we know: API keys support multiple services. Tasks page uses Popover+Checkbox for multi-select status filter.
   - What's unclear: Best UX for service multi-select in create dialog -- Popover+Checkbox or inline chips with autocomplete?
   - Recommendation: Use Popover+Checkbox pattern (already proven in tasks page). Show selected count on trigger button.

2. **Expiry date input component**
   - What we know: HTML `<input type="date">` works but styling varies across browsers.
   - What's unclear: Whether to use native date input or a date picker component.
   - Recommendation: Use native `<input type="date">` (simplest, no new dependency). The shadcn Calendar component would require installing additional components.

3. **Expired credential behavior in auth middleware**
   - What we know: Expiry will be stored as a hash field, not Redis TTL.
   - What's unclear: Should `lookup_api_key` / `validate_node_token` be updated in this phase to check expiry?
   - Recommendation: Yes -- add expiry check to auth middleware in this phase. An expired credential that still authenticates defeats the purpose of expiry.

## Validation Architecture

### Test Framework
| Property | Value |
|----------|-------|
| Framework (backend) | Rust built-in `#[cfg(test)]` + integration tests in `gateway/tests/` |
| Framework (frontend) | None configured -- no vitest/jest setup in admin-ui |
| Config file (backend) | Cargo.toml (test deps) |
| Quick run command (backend) | `cargo test --package gateway` |
| Full suite command (backend) | `cargo test --package gateway` |

### Phase Requirements to Test Map
| Req ID | Behavior | Test Type | Automated Command | File Exists? |
|--------|----------|-----------|-------------------|-------------|
| CRED-01 | List API keys returns masked hashes + services | unit | `cargo test --package gateway -- test_list_api_keys` | No -- Wave 0 |
| CRED-02 | Create API key returns raw secret + stores hash | unit | `cargo test --package gateway -- test_generate_api_key` | Yes (api_key.rs) |
| CRED-03 | Revoke API key deletes from Redis | integration | `cargo test --package gateway -- auth_integration` | Partial |
| CRED-04 | List node tokens returns masked hashes + labels | unit | `cargo test --package gateway -- test_list_node_tokens` | No -- Wave 0 |
| CRED-05 | Create node token returns raw secret + stores hash | unit | `cargo test --package gateway -- test_generate_node_token` | Yes (node_token.rs) |
| CRED-06 | Revoke node token deletes from Redis | integration | `cargo test --package gateway -- auth_integration` | Partial |
| API-03 | GET /v1/admin/api-keys returns JSON list | integration | `cargo test --package gateway -- auth_integration` | No -- Wave 0 |
| API-04 | GET /v1/admin/node-tokens returns JSON list | integration | `cargo test --package gateway -- auth_integration` | No -- Wave 0 |

### Sampling Rate
- **Per task commit:** `cargo test --package gateway`
- **Per wave merge:** `cargo test --package gateway`
- **Phase gate:** Full backend test suite green

### Wave 0 Gaps
- [ ] `gateway/src/auth/api_key.rs` -- unit tests for `list_api_keys()` function
- [ ] `gateway/src/auth/node_token.rs` -- unit tests for `list_node_tokens()` function
- [ ] No frontend test infrastructure exists (no vitest/jest) -- frontend testing is manual-only for this project

## Sources

### Primary (HIGH confidence)
- `gateway/src/auth/api_key.rs` -- existing `store_api_key`, `generate_api_key`, `lookup_api_key`, `revoke_api_key` implementations
- `gateway/src/auth/node_token.rs` -- existing `store_node_token`, `generate_node_token`, `validate_node_token`, `revoke_node_token` implementations
- `gateway/src/http/admin.rs` -- existing create/revoke handlers, request/response structs
- `gateway/src/queue/redis.rs` lines 386-480 -- SCAN-based listing pattern (task listing)
- `gateway/src/main.rs` lines 225-280 -- admin route registration
- `admin-ui/src/lib/tasks.ts` -- TanStack Query hook pattern
- `admin-ui/src/lib/services.ts` -- TanStack Query hook pattern
- `admin-ui/src/components/task-table.tsx` -- data table + row actions + copy pattern
- `admin-ui/src/components/task-cancel-dialog.tsx` -- AlertDialog confirmation pattern
- `admin-ui/src/components/service-registration-dialog.tsx` -- Dialog create form pattern
- `admin-ui/src/routes/_authenticated/tasks.tsx` -- page layout with filters pattern
- `admin-ui/src/components/ui/` -- 20 shadcn components already installed (Tabs is NOT yet installed)

### Secondary (MEDIUM confidence)
- shadcn/ui Tabs component -- standard Radix UI Tabs primitive, well-documented

## Metadata

**Confidence breakdown:**
- Standard stack: HIGH -- all libraries already in use, no new dependencies except Tabs component
- Architecture: HIGH -- every pattern has an existing implementation in Phase 9/10 to follow
- Pitfalls: HIGH -- based on direct code inspection of existing implementations

**Research date:** 2026-03-23
**Valid until:** 2026-04-22 (stable -- no external dependency changes expected)
