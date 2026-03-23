# Roadmap: xgent-ai-gateway

## Milestones

- ✅ **v1.0 MVP** — Phases 1-7 (shipped 2026-03-22)
- **v1.1 Admin Web UI** — Phases 8-12 (in progress)

## Phases

<details>
<summary>v1.0 MVP (Phases 1-7) — SHIPPED 2026-03-22</summary>

- [x] Phase 1: Core Queue Loop (3/3 plans) — completed 2026-03-21
- [x] Phase 2: Authentication and TLS (3/3 plans) — completed 2026-03-21
- [x] Phase 3: Service Registry and Node Health (3/3 plans) — completed 2026-03-22
- [x] Phase 4: Task Reliability and Callbacks (2/2 plans) — completed 2026-03-22
- [x] Phase 5: Observability and Packaging (4/4 plans) — completed 2026-03-22
- [x] Phase 6: gRPC Auth Hardening (2/2 plans) — completed 2026-03-22
- [x] Phase 7: Integration Fixes, Sample Service, and Cleanup (3/3 plans) — completed 2026-03-22

Full details: `.planning/milestones/v1.0-ROADMAP.md`

</details>

### v1.1 Admin Web UI (In Progress)

**Milestone Goal:** Add an admin web UI as a separate React SPA for managing and monitoring the gateway -- login, dashboard, service/node/task/credential management.

- [ ] **Phase 8: Frontend Foundation and Backend Auth** - Scaffold Vite+React app, implement auth endpoints and login flow, establish UI patterns
- [ ] **Phase 9: Service and Node Management** - Service CRUD pages and node health pages using existing backend endpoints
- [ ] **Phase 10: Task Management and Data Endpoints** - New backend task endpoints, task list/detail/cancel pages
- [ ] **Phase 11: Credential Management** - API key and node token list endpoints, credential CRUD pages
- [ ] **Phase 12: Dashboard and Metrics Visualization** - Dashboard overview cards, live charts, service health badges

## Phase Details

### Phase 8: Frontend Foundation and Backend Auth
**Goal**: Admin can log in to a working React app shell with established UI patterns for all subsequent pages
**Depends on**: Phase 7 (v1.0 complete)
**Requirements**: AUTH-01, AUTH-02, AUTH-03, AUTH-04, API-01, API-02, UI-01, UI-02, UI-03, UI-04, UI-05
**Success Criteria** (what must be TRUE):
  1. Admin can log in with username/password and sees a sidebar navigation shell
  2. Admin session persists across page refreshes and auto-refreshes before expiry
  3. Admin can log out and is redirected to the login page with session cleaned up
  4. All pages display loading skeletons while fetching, error states with retry buttons on failure, and helpful empty states when no data exists
  5. App has dark mode toggle that persists preference, responsive layout at 1280px+, auto-refresh controls, and toast notifications on mutations
**Plans**: 3 plans

Plans:
- [x] 08-01-PLAN.md — Backend session auth endpoints, config changes, CORS, middleware replacement
- [x] 08-02-PLAN.md — Frontend SPA scaffolding, routing, login page, API client, auth hooks
- [ ] 08-03-PLAN.md — App shell (sidebar + header), dark mode, auto-refresh, UI pattern components

### Phase 9: Service and Node Management
**Goal**: Admin can view, create, and manage services and inspect node health from the UI
**Depends on**: Phase 8
**Requirements**: SVC-01, SVC-02, SVC-03, SVC-04, NODE-01, NODE-02
**Success Criteria** (what must be TRUE):
  1. Admin can view a list of all registered services and click through to service detail showing config, connected nodes, and queue depth
  2. Admin can register a new service via a form and deregister an existing service with a confirmation dialog
  3. Admin can view per-service node list with health status indicators and see node details including in-flight tasks, drain status, and last seen time
**Plans**: TBD

Plans:
- [ ] 09-01: TBD
- [ ] 09-02: TBD

### Phase 10: Task Management and Data Endpoints
**Goal**: Admin can browse, inspect, and cancel tasks through the UI backed by new paginated backend endpoints
**Depends on**: Phase 8
**Requirements**: TASK-01, TASK-02, TASK-03, API-05, API-06
**Success Criteria** (what must be TRUE):
  1. Admin can view a paginated task list filterable by service and status
  2. Admin can click a task to view its full detail including metadata, timestamps, assigned node, and result payload
  3. Admin can cancel a pending or running task with a confirmation dialog, and the task is marked failed for the client
**Plans**: TBD

Plans:
- [ ] 10-01: TBD
- [ ] 10-02: TBD

### Phase 11: Credential Management
**Goal**: Admin can manage API keys and node tokens for all services through the UI
**Depends on**: Phase 8
**Requirements**: CRED-01, CRED-02, CRED-03, CRED-04, CRED-05, CRED-06, API-03, API-04
**Success Criteria** (what must be TRUE):
  1. Admin can list API keys showing masked hashes and associated services, and list node tokens per service showing masked hashes and labels
  2. Admin can create a new API key or node token and sees the secret value exactly once with a copy-to-clipboard button
  3. Admin can revoke an API key or node token with a confirmation dialog
**Plans**: TBD

Plans:
- [ ] 11-01: TBD
- [ ] 11-02: TBD

### Phase 12: Dashboard and Metrics Visualization
**Goal**: Admin sees a live operational dashboard with metrics charts and service health indicators on first login
**Depends on**: Phase 9, Phase 10
**Requirements**: DASH-01, DASH-02, DASH-03
**Success Criteria** (what must be TRUE):
  1. Admin sees overview cards showing service count, active nodes, aggregate queue depth, and task throughput
  2. Admin sees live time-series charts for throughput and queue depth that auto-update every 10-15 seconds
  3. Admin sees color-coded service health badges (green/yellow/red) reflecting real node and queue state
**Plans**: TBD

Plans:
- [ ] 12-01: TBD
- [ ] 12-02: TBD

## Progress

**Execution Order:**
Phases execute in numeric order: 8 -> 9 -> 10 -> 11 -> 12
Phases 9, 10, and 11 all depend only on Phase 8 and could execute in any order. Phase 12 depends on 9 and 10.

| Phase | Milestone | Plans Complete | Status | Completed |
|-------|-----------|----------------|--------|-----------|
| 1. Core Queue Loop | v1.0 | 3/3 | Complete | 2026-03-21 |
| 2. Authentication and TLS | v1.0 | 3/3 | Complete | 2026-03-21 |
| 3. Service Registry and Node Health | v1.0 | 3/3 | Complete | 2026-03-22 |
| 4. Task Reliability and Callbacks | v1.0 | 2/2 | Complete | 2026-03-22 |
| 5. Observability and Packaging | v1.0 | 4/4 | Complete | 2026-03-22 |
| 6. gRPC Auth Hardening | v1.0 | 2/2 | Complete | 2026-03-22 |
| 7. Integration Fixes, Sample Service, and Cleanup | v1.0 | 3/3 | Complete | 2026-03-22 |
| 8. Frontend Foundation and Backend Auth | v1.1 | 0/3 | Planning | - |
| 9. Service and Node Management | v1.1 | 0/? | Not started | - |
| 10. Task Management and Data Endpoints | v1.1 | 0/? | Not started | - |
| 11. Credential Management | v1.1 | 0/? | Not started | - |
| 12. Dashboard and Metrics Visualization | v1.1 | 0/? | Not started | - |
