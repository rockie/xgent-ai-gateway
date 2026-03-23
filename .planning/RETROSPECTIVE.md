# Project Retrospective

*A living document updated after each milestone. Lessons feed forward into future planning.*

## Milestone: v1.0 — MVP

**Shipped:** 2026-03-22
**Phases:** 7 | **Plans:** 20 | **Tasks:** 41

### What Was Built
- Complete pull-model task gateway with gRPC + HTTPS dual protocol
- Full auth stack: API key, mTLS, node tokens with TLS termination
- Service registry with per-service queues and node health tracking
- Task reliability: background reaper + callback delivery with exponential backoff
- Production packaging: Prometheus metrics, structured JSON logging, static musl binary, Docker image
- gRPC auth hardening and integration fixes closing all audit gaps
- Sample echo service for end-to-end testing

### What Worked
- Phase-by-phase layering (queue → auth → registry → reliability → observability) kept each phase coherent and testable
- Redis Streams with consumer groups provided both reliable delivery and timeout detection (XPENDING IDLE) in one primitive
- Tower middleware sharing between Axum and Tonic eliminated duplicate auth logic
- Milestone audit after Phase 5 caught gRPC auth gaps early — Phases 6-7 closed them systematically
- ~5 min average per plan execution — tight feedback loops

### What Was Inefficient
- ROADMAP.md progress table wasn't kept in sync during execution (phases 2-7 show "Not started" despite completion)
- Phase 5 initially had 3 plans but needed a 4th (05-04) for integration test fixes — planning missed test compilation impact
- Reaper retry/DLQ scope wasn't decided until mid-Phase 4 — earlier descoping would have saved planning time

### Patterns Established
- Tower Service wrapper pattern for gRPC per-RPC auth layers
- Per-test Redis DB isolation via atomic counter for parallel integration tests
- Manual hyper-util TLS accept loop for per-connection keepalive control
- Config-based mTLS identity mapping (fingerprint → services) in gateway.toml
- `pub` test-friendly functions alongside private infinite-loop wrappers (reap_timed_out_tasks vs run_reaper)

### Key Lessons
1. Plan for integration test compilation cost — adding new AppState fields breaks all existing tests
2. Descope early and explicitly — "clients resubmit" is a valid v1 strategy that keeps the gateway simple
3. Milestone audits are worth the investment — the v1.0 audit found real gaps that would have shipped broken
4. Redis Streams > BLMOVE for this use case — consumer groups give reliable delivery without custom bookkeeping

### Cost Observations
- Total execution: ~103 minutes across 20 plans
- All work completed in 2 calendar days
- 8,429 LOC Rust shipped

---

## Milestone: v1.1 — Admin Web UI

**Shipped:** 2026-03-23
**Phases:** 5 | **Plans:** 12 | **Tasks:** 28

### What Was Built
- Session-based admin auth with Argon2id password hashing and HttpOnly cookie sessions
- Vite + React 19 SPA with TanStack Router/Query, shadcn/ui, dark mode, auto-refresh
- Service and node management pages with card grid, health badges, detail views, CRUD dialogs
- Task management with SCAN-based pagination, filters, detail sheet, cancel flow
- Credential management with tabbed API keys/node tokens, one-time secret reveal, optimistic revoke
- Live dashboard with Recharts area charts, overview cards with trend arrows, service health list

### What Worked
- TanStack Router + Query combination gave type-safe routing with excellent data caching
- shadcn/ui v4 defaults (oklch colors, Geist font) looked good without custom theming
- Consistent hook pattern across all data layers (services.ts, tasks.ts, credentials.ts, metrics.ts) made each page predictable
- SCAN-based pagination kept Redis usage simple — sufficient for admin-scale workloads
- In-memory ring buffer for metrics avoided new Redis dependencies for time-series data

### What Was Inefficient
- Phase 8/9 ROADMAP checkboxes not updated during execution (showed unchecked despite completion)
- UI-01 through UI-04 requirement checkboxes went stale — caught only at audit time
- 14 human verification items accumulated across phases without browser testing

### Patterns Established
- HttpOnly cookie sessions with SameSite=None for cross-origin SPA
- Forced-dismissal dialog pattern for one-time secret reveals
- Per-card detail fetch (N+1) acceptable for admin UI scale
- Background snapshot task refreshing Prometheus gauges before ring buffer capture

### Key Lessons
1. Run milestone audit before completion — catches stale checkboxes and minor integration gaps early
2. shadcn/ui defaults are good enough — custom theming effort better spent elsewhere
3. SCAN-based pagination works for admin but would need cursor-based approach at scale

### Cost Observations
- All 5 phases completed in a single day
- ~6,600 LOC TypeScript/TSX shipped
- Consistent hook/component patterns from Phase 8 accelerated Phases 9-12

---

## Cross-Milestone Trends

| Metric | v1.0 | v1.1 |
|--------|------|------|
| Phases | 7 | 5 |
| Plans | 20 | 12 |
| Tasks | 41 | 28 |
| LOC shipped | 8,429 Rust | ~6,600 TS/TSX |
| Calendar days | 2 | 1 |

### Top Lessons (Verified Across Milestones)

1. Consistent patterns across similar components dramatically accelerate development (Tower middleware in v1.0, TanStack Query hooks in v1.1)
2. Milestone audits catch what manual tracking misses — run before completion every time
3. ROADMAP.md progress table drifts during execution — needs automated sync or post-phase update discipline
