---
phase: 11-credential-management
verified: 2026-03-23T08:15:00Z
status: human_needed
score: 11/11 must-haves verified
re_verification: false
human_verification:
  - test: "Verify forced-dismissal dialog cannot be closed via Escape key or outside click"
    expected: "Pressing Escape or clicking outside the secret reveal dialog does nothing; only the 'I've copied it' button closes it"
    why_human: "The base-ui onOpenChange reason filtering and disablePointerDismissal rely on runtime dialog behavior that cannot be confirmed by static analysis alone"
  - test: "Verify optimistic revoke rollback on failure"
    expected: "If a revoke API call fails, the revoked row reappears in the table and an error toast is shown"
    why_human: "Rollback requires a network failure condition to trigger; verified by code inspection but runtime behavior needs confirmation"
  - test: "Verify one-time secret is fully visible (not masked) in reveal dialog"
    expected: "The full raw 64-character hex key or token is displayed in the dialog code element without any masking"
    why_human: "Visual verification needed; the code renders {secret} in a code block but the actual rendering must be confirmed against masking or truncation"
  - test: "Verify the credentials page loads at /credentials with both tabs visible"
    expected: "Navigating to /credentials shows the page header, 'API Keys' and 'Node Tokens' tab triggers, and either empty state or table data for each"
    why_human: "End-to-end page rendering with TanStack Router and the tab state wiring requires browser verification"
---

# Phase 11: Credential Management Verification Report

**Phase Goal:** Credential management — list, create, revoke API keys and node tokens from the admin UI, with one-time secret reveal
**Verified:** 2026-03-23T08:15:00Z
**Status:** human_needed
**Re-verification:** No — initial verification

## Goal Achievement

### Observable Truths

| # | Truth | Status | Evidence |
|---|-------|--------|----------|
| 1 | GET /v1/admin/api-keys returns JSON list with masked hash fields, service names, label, created_at, expires_at, and callback_url | VERIFIED | `list_api_keys` handler in admin.rs line 149, `ApiKeyListItem` struct line 134, SCAN loop in api_key.rs line 165. Route registered at main.rs line 230. |
| 2 | GET /v1/admin/node-tokens returns JSON list with masked hash, service name, label, created_at, and expires_at | VERIFIED | `list_node_tokens` handler in admin.rs line 264, `NodeTokenListItem` struct line 250, SCAN loop in node_token.rs line 96. Route registered at main.rs line 243. |
| 3 | POST /v1/admin/api-keys accepts optional label and expires_at fields and stores them in Redis | VERIFIED | `CreateApiKeyRequest` in admin.rs includes `pub label: Option<String>` and `pub expires_at: Option<String>` (lines 23-25). Handler passes both to `store_api_key` (lines 54-55). `store_api_key` writes them with HSET (api_key.rs lines 63-64). |
| 4 | POST /v1/admin/node-tokens accepts optional label and expires_at fields and stores them in Redis | VERIFIED | `CreateNodeTokenRequest` includes `expires_at: Option<String>`. Handler passes `node_label` and `expires_at` to `store_node_token` (admin.rs lines 199-200). |
| 5 | Expired credentials are returned in list endpoints but rejected at auth time | VERIFIED | `list_api_keys` / `list_node_tokens` perform no expiry filter (all records returned). `lookup_api_key` returns `Ok(None)` on expired key (api_key.rs line 85-88). `validate_node_token` returns `Ok(false)` on expired token (node_token.rs lines 71-74). |
| 6 | Admin can see tabbed page with API Keys and Node Tokens tabs, each showing a data table | VERIFIED | credentials.tsx imports Tabs, TabsList, TabsTrigger (lines 5, 112-115), renders `CredentialTable` in each tab content (lines 118, 129). Page heading "Credentials" at line 101. No "Coming Soon" or placeholder text present. |
| 7 | Admin can create an API key and sees raw key exactly once in forced-dismissal dialog | VERIFIED | `CreateCredentialDialog` calls `useCreateApiKey`, passes response secret to `onCreated`. `SecretRevealDialog` renders `{secret}` in a code block (line 79), uses `disablePointerDismissal` and `onOpenChange` reason filter for escape-key/outside-press/close-press, `showCloseButton={false}`. "I've copied it" is sole dismiss path. |
| 8 | Admin can create a node token and sees raw token exactly once | VERIFIED | Same pattern — `useCreateNodeToken` mutation, `onCreated(response.token)` triggers reveal dialog with `credentialType="node token"`. |
| 9 | Admin can revoke an API key or node token via row action with confirmation dialog | VERIFIED | `CredentialTable` has "Revoke" button triggering `onRevoke` callback. `RevokeCredentialDialog` uses `AlertDialog` with exact text "Clients using it will immediately lose access. This cannot be undone." (revoke-credential-dialog.tsx line 38). |
| 10 | Revoked credentials disappear immediately (optimistic) with success toast, reappear on failure | VERIFIED | `useRevokeApiKey` and `useRevokeNodeToken` implement full optimistic update pattern: `onMutate` snapshots data, filters out revoked item, `onError` restores snapshot, `onSuccess` shows toast, `onSettled` invalidates (credentials.ts lines 117-140, 152-175). |
| 11 | Data tables show masked hash (first 8 chars + ...), services, label, created date, expiry, and revoke action | VERIFIED | `CredentialTable` calls `maskHash(hash)` (credential-table.tsx line 52), renders service_names, label, formatted dates, `isExpired()` check for red styling, and Trash2 revoke button (line 1 import). |

**Score:** 11/11 truths verified

### Required Artifacts

| Artifact | Status | Details |
|----------|--------|---------|
| `gateway/src/auth/api_key.rs` | VERIFIED | Contains `pub async fn list_api_keys`, `pub label: Option<String>`, `pub expires_at: Option<String>`, expiry check via `chrono::DateTime::parse_from_rfc3339`, SCAN pattern `api_keys:*`. 200+ lines. |
| `gateway/src/auth/node_token.rs` | VERIFIED | Contains `pub async fn list_node_tokens`, `pub expires_at: Option<String>` on `NodeTokenMetadata`, expiry check in `validate_node_token`, SCAN pattern `node_tokens:*:*`. |
| `gateway/src/http/admin.rs` | VERIFIED | Contains `pub struct ApiKeyListItem`, `pub struct ListApiKeysResponse`, `pub struct NodeTokenListItem`, `pub struct ListNodeTokensResponse`, `pub async fn list_api_keys`, `pub async fn list_node_tokens`. Extended create structs with label/expires_at. |
| `gateway/src/main.rs` | VERIFIED | `/v1/admin/api-keys` route chains `.get(http::admin::list_api_keys)` (line 230). `/v1/admin/node-tokens` route chains `.get(http::admin::list_node_tokens)` (line 243). |
| `admin-ui/src/lib/credentials.ts` | VERIFIED | 168 lines. Exports all 6 hooks (`useApiKeys`, `useNodeTokens`, `useCreateApiKey`, `useCreateNodeToken`, `useRevokeApiKey`, `useRevokeNodeToken`) plus `maskHash` and `isExpired`. Optimistic update pattern with `cancelQueries` and rollback present. API calls target `/v1/admin/api-keys` and `/v1/admin/node-tokens`. |
| `admin-ui/src/components/credential-table.tsx` | VERIFIED | 81 lines. Exports `CredentialTable`, imports and calls `maskHash` and `isExpired`, renders Trash2 revoke button. |
| `admin-ui/src/components/create-credential-dialog.tsx` | VERIFIED | 250 lines. Exports `CreateCredentialDialog`, imports `useServices` for service selection dropdown, has `type="date"` native date input, includes `callback_url` field for API keys. |
| `admin-ui/src/components/secret-reveal-dialog.tsx` | VERIFIED | 104 lines. Exports `SecretRevealDialog`, contains "This secret will not be shown again.", `disablePointerDismissal`, `onOpenChange` reason filter (escape-key, outside-press, close-press), `showCloseButton={false}`, `navigator.clipboard.writeText(secret)`, "I've copied it" dismiss button. |
| `admin-ui/src/components/revoke-credential-dialog.tsx` | VERIFIED | 51 lines. Exports `RevokeCredentialDialog`, uses `AlertDialog`, contains exact text "Clients using it will immediately lose access. This cannot be undone." |
| `admin-ui/src/routes/_authenticated/credentials.tsx` | VERIFIED | 233 lines. Imports and renders Tabs, CredentialTable, CreateCredentialDialog, SecretRevealDialog, RevokeCredentialDialog. Uses `useApiKeys` and `useNodeTokens`. No "Coming Soon" or placeholder text. Full create and revoke state management present. |
| `admin-ui/src/components/ui/tabs.tsx` | VERIFIED | Exists (installed via shadcn). |

### Key Link Verification

| From | To | Via | Status | Details |
|------|----|-----|--------|---------|
| `gateway/src/http/admin.rs` | `gateway/src/auth/api_key.rs` | `api_key::list_api_keys()` call | WIRED | admin.rs line 152: `api_key::list_api_keys(&mut state.auth_conn.clone())` |
| `gateway/src/http/admin.rs` | `gateway/src/auth/node_token.rs` | `node_token::list_node_tokens()` call | WIRED | admin.rs line 267: `node_token::list_node_tokens(&mut state.auth_conn.clone())` |
| `gateway/src/main.rs` | `gateway/src/http/admin.rs` | Route registration with `.get()` | WIRED | main.rs line 230: `.get(http::admin::list_api_keys)`, line 243: `.get(http::admin::list_node_tokens)` |
| `admin-ui/src/lib/credentials.ts` | `/v1/admin/api-keys` | `apiClient` fetch calls | WIRED | credentials.ts line 63: `apiClient<ListApiKeysResponse>('/v1/admin/api-keys')`, line 85: POST to same path |
| `admin-ui/src/lib/credentials.ts` | `/v1/admin/node-tokens` | `apiClient` fetch calls | WIRED | credentials.ts line 72: `apiClient<ListNodeTokensResponse>('/v1/admin/node-tokens')`, line 98: POST to same path |
| `admin-ui/src/routes/_authenticated/credentials.tsx` | `admin-ui/src/lib/credentials.ts` | Hook imports | WIRED | credentials.tsx lines 8-15: imports `useApiKeys`, `useNodeTokens`, `useRevokeApiKey`, `useRevokeNodeToken` from `@/lib/credentials` |
| `admin-ui/src/components/secret-reveal-dialog.tsx` | `navigator.clipboard` | Clipboard API | WIRED | secret-reveal-dialog.tsx line 35: `await navigator.clipboard.writeText(secret)` |

### Requirements Coverage

| Requirement | Source Plan | Description | Status | Evidence |
|-------------|-------------|-------------|--------|----------|
| CRED-01 | 11-02-PLAN.md | Admin can list API keys (masked hash, associated services) | SATISFIED | `useApiKeys` hook fetches GET /v1/admin/api-keys. `CredentialTable` renders `maskHash(key_hash)` and `service_names`. |
| CRED-02 | 11-02-PLAN.md | Admin can create API key (shown once with copy-to-clipboard) | SATISFIED | `CreateCredentialDialog` + `SecretRevealDialog` with forced dismissal, copy button, full secret visible in code block. |
| CRED-03 | 11-02-PLAN.md | Admin can revoke API key with confirmation | SATISFIED | `RevokeCredentialDialog` with AlertDialog confirmation. `useRevokeApiKey` mutation with optimistic removal. |
| CRED-04 | 11-01-PLAN.md + 11-02-PLAN.md | Admin can list node tokens per service (masked hash, label) | SATISFIED | Backend list endpoint exists. `useNodeTokens` hook. `CredentialTable` renders `maskHash(token_hash)`, `service_name`, label. |
| CRED-05 | 11-02-PLAN.md | Admin can create node token (shown once with copy-to-clipboard) | SATISFIED | Same create + reveal flow, `useCreateNodeToken` mutation returns raw `token` field shown in dialog. |
| CRED-06 | 11-02-PLAN.md | Admin can revoke node token with confirmation | SATISFIED | `useRevokeNodeToken` with optimistic update. Revoke confirmation dialog. |
| API-03 | 11-01-PLAN.md | GET /v1/admin/api-keys list endpoint | SATISFIED | Handler registered at main.rs line 230, calls `api_key::list_api_keys`, returns `ListApiKeysResponse` JSON. |
| API-04 | 11-01-PLAN.md | GET /v1/admin/node-tokens list endpoint | SATISFIED | Handler registered at main.rs line 243, calls `node_token::list_node_tokens`, returns `ListNodeTokensResponse` JSON. |

No orphaned requirements found. All 8 requirement IDs declared in plan frontmatter are accounted for and satisfied.

### Anti-Patterns Found

No blockers or warnings found.

- No TODO/FIXME/PLACEHOLDER comments in any phase 11 file
- No stub return patterns (`return null`, `return {}`, `return []` as final output)
- No placeholder text ("Coming Soon" absent from credentials.tsx — confirmed)
- All data flows from API calls through hooks to rendered table cells; no hardcoded empty arrays passed to display components
- `useRevokeApiKey` and `useRevokeNodeToken` filter the cache data (not static empty arrays) and restore on error

### Human Verification Required

#### 1. Forced-Dismissal Dialog Runtime Behavior

**Test:** Open the credentials page, create an API key, and when the secret reveal dialog appears: press Escape, click outside the dialog, and look for any X button
**Expected:** None of these actions close the dialog; only the "I've copied it" button dismisses it
**Why human:** The `onOpenChange` reason filter (`escape-key`, `outside-press`, `close-press`) and `disablePointerDismissal` are base-ui v4 props. The implementation is correct by code inspection but the actual dismiss-blocking behavior requires runtime confirmation, as the base-ui API version in use must match these prop names.

#### 2. Optimistic Revoke Rollback on Failure

**Test:** Revoke a credential while the gateway is unreachable (or mock a network error)
**Expected:** The row disappears immediately on click, then reappears and an error toast is shown when the request fails
**Why human:** The rollback path (`onError` restoring `context.previous`) is code-verified but requires a failure condition to observe the runtime behavior.

#### 3. Secret Fully Visible (Not Masked) in Reveal Dialog

**Test:** Create an API key and inspect the reveal dialog
**Expected:** The full raw 64-character hex string is displayed without truncation or masking
**Why human:** The code renders `{secret}` in a `<code>` element, which is correct, but visual verification confirms no CSS `text-overflow` or character-limiting is applied.

#### 4. Page Loads and Tab State Works End-to-End

**Test:** Navigate to /credentials in the running admin UI
**Expected:** "Credentials" heading appears, both "API Keys" and "Node Tokens" tab triggers render, switching tabs shows the appropriate table or empty state
**Why human:** TanStack Router route registration and the tab `value`/`onValueChange` state synchronization requires a running browser to confirm.

### Gaps Summary

No gaps found. All 11 observable truths pass at all three levels (exists, substantive, wired). All 8 requirement IDs are satisfied. No anti-patterns detected.

The four human verification items are runtime/visual behaviors that automated grep-based analysis cannot confirm. They are the remaining risk surface: the base-ui forced-dismissal mechanism and the optimistic rollback path.

---

_Verified: 2026-03-23T08:15:00Z_
_Verifier: Claude (gsd-verifier)_
