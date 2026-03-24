---
phase: quick-260324-w3j
one_liner: "Fixed Node.js client URL paths from /api/v1/tasks to /v1/tasks matching gateway routes"
tasks_completed: 1
files_changed: 4
requirements_completed: EXMP-04
commit: b1c4bed
---

# Quick Task Summary: Fix Node.js Client URL Paths

## What Changed

All 3 Node.js client scripts (`cli-client.js`, `sync-api-client.js`, `async-api-client.js`) and `README.md` used `/api/v1/tasks` as the gateway endpoint, but the gateway registers routes at `/v1/tasks` (no `/api` prefix). Every fetch call would return HTTP 404.

**Fix:** Removed `/api` prefix from 9 URL references across 4 files.

## Files Modified

- `examples/nodejs-client/cli-client.js` — 2 URLs fixed (submit + poll)
- `examples/nodejs-client/sync-api-client.js` — 2 URLs fixed
- `examples/nodejs-client/async-api-client.js` — 2 URLs fixed
- `examples/nodejs-client/README.md` — 3 references fixed (docs + diagram)

## Verification

`grep -r "/api/v1" examples/nodejs-client/` — 0 matches (confirmed clean)
