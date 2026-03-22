# Requirements: xgent-ai-gateway

**Defined:** 2026-03-22
**Core Value:** Tasks submitted by clients reliably reach internal nodes and results reliably flow back, regardless of network topology

## v1.1 Requirements

Requirements for Admin Web UI milestone. Each maps to roadmap phases.

### Authentication

- [ ] **AUTH-01**: Admin can log in with username and password
- [ ] **AUTH-02**: Admin session persists via secure HttpOnly cookie
- [ ] **AUTH-03**: Admin session auto-refreshes before expiry
- [ ] **AUTH-04**: Admin can log out with session cleanup

### Dashboard

- [ ] **DASH-01**: Admin sees overview cards (service count, active nodes, queue depth, task throughput)
- [ ] **DASH-02**: Admin sees live time-series charts for throughput and queue depth (polling every 10-15s)
- [ ] **DASH-03**: Admin sees color-coded service health badges (green/yellow/red)

### Service Management

- [ ] **SVC-01**: Admin can view list of all registered services
- [ ] **SVC-02**: Admin can view service detail (config, connected nodes, queue depth)
- [ ] **SVC-03**: Admin can register a new service via form
- [ ] **SVC-04**: Admin can deregister a service with confirmation dialog

### Node Management

- [ ] **NODE-01**: Admin can view per-service node list with health status
- [ ] **NODE-02**: Admin can see node details (in-flight tasks, drain status, last seen)

### Task Management

- [ ] **TASK-01**: Admin can view paginated task list filtered by service and status
- [ ] **TASK-02**: Admin can view task detail (metadata, timestamps, assigned node, result)
- [ ] **TASK-03**: Admin can cancel a pending or running task (returns failed to client)

### Credential Management

- [ ] **CRED-01**: Admin can list API keys (masked hash, associated services)
- [ ] **CRED-02**: Admin can create API key (shown once with copy-to-clipboard)
- [ ] **CRED-03**: Admin can revoke API key with confirmation
- [ ] **CRED-04**: Admin can list node tokens per service (masked hash, label)
- [ ] **CRED-05**: Admin can create node token (shown once with copy-to-clipboard)
- [ ] **CRED-06**: Admin can revoke node token with confirmation

### Frontend Foundation

- [ ] **UI-01**: All pages show loading skeletons, error states with retry, and empty state guidance
- [ ] **UI-02**: Toast notifications for success/failure on all mutations
- [ ] **UI-03**: Dark mode toggle with persisted preference
- [ ] **UI-04**: Auto-refresh with configurable interval (5s/15s/30s/off) and pause toggle
- [ ] **UI-05**: Responsive layout for 1280px+ screens

### Backend API

- [ ] **API-01**: POST /v1/admin/auth/login endpoint (returns session token)
- [ ] **API-02**: POST /v1/admin/auth/refresh endpoint (extends session)
- [ ] **API-03**: GET /v1/admin/api-keys list endpoint
- [ ] **API-04**: GET /v1/admin/node-tokens list endpoint
- [ ] **API-05**: GET /v1/admin/tasks with pagination and service/status filters
- [ ] **API-06**: POST /v1/admin/tasks/{task_id}/cancel endpoint

## Future Requirements

### Deferred from v1.1

- **EDIT-01**: Admin can edit service config inline (timeout, max_nodes) without delete/recreate
- **KB-01**: Keyboard shortcuts for quick navigation (g+d, g+s, g+t)
- **PERF-01**: Task latency percentile display (p50/p95/p99 from histogram metrics)

## Out of Scope

| Feature | Reason |
|---------|--------|
| Task retry / resubmit | Never for this project — clients resubmit on failure; gateway stays simple |
| Log viewer | Deferred — high-volume streaming needs separate backend (Loki/ELK) |
| WebSocket real-time updates | Poll + callback covers all practical use cases; TanStack Query polling sufficient |
| Embedded Grafana iframe | Auth complexity, CORS/CSP issues; parse /metrics directly instead |
| Role-based access control | Single-admin gateway; RBAC is massive complexity for zero current value |
| Multi-gateway federation | Each gateway is independent by design; different product |
| Task scheduling / cron | External schedulers submit to gateway; out of scope per PROJECT.md |
| Task payload editing | Gateway treats payloads as opaque bytes; editing meaningless |

## Traceability

| Requirement | Phase | Status |
|-------------|-------|--------|
| AUTH-01 | Phase 8 | Pending |
| AUTH-02 | Phase 8 | Pending |
| AUTH-03 | Phase 8 | Pending |
| AUTH-04 | Phase 8 | Pending |
| DASH-01 | Phase 12 | Pending |
| DASH-02 | Phase 12 | Pending |
| DASH-03 | Phase 12 | Pending |
| SVC-01 | Phase 9 | Pending |
| SVC-02 | Phase 9 | Pending |
| SVC-03 | Phase 9 | Pending |
| SVC-04 | Phase 9 | Pending |
| NODE-01 | Phase 9 | Pending |
| NODE-02 | Phase 9 | Pending |
| TASK-01 | Phase 10 | Pending |
| TASK-02 | Phase 10 | Pending |
| TASK-03 | Phase 10 | Pending |
| CRED-01 | Phase 11 | Pending |
| CRED-02 | Phase 11 | Pending |
| CRED-03 | Phase 11 | Pending |
| CRED-04 | Phase 11 | Pending |
| CRED-05 | Phase 11 | Pending |
| CRED-06 | Phase 11 | Pending |
| UI-01 | Phase 8 | Pending |
| UI-02 | Phase 8 | Pending |
| UI-03 | Phase 8 | Pending |
| UI-04 | Phase 8 | Pending |
| UI-05 | Phase 8 | Pending |
| API-01 | Phase 8 | Pending |
| API-02 | Phase 8 | Pending |
| API-03 | Phase 11 | Pending |
| API-04 | Phase 11 | Pending |
| API-05 | Phase 10 | Pending |
| API-06 | Phase 10 | Pending |

**Coverage:**
- v1.1 requirements: 33 total
- Mapped to phases: 33
- Unmapped: 0

---
*Requirements defined: 2026-03-22*
*Last updated: 2026-03-22 after roadmap creation*
