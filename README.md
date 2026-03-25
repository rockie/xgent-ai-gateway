# xgent-ai-gateway

A Rust-based pull-model task gateway that brokers work between external clients and internal compute nodes. Clients submit tasks via gRPC or HTTPS and receive a task ID immediately. Internal nodes — running behind NAT/firewalls — reverse-poll the gateway to pick up tasks from their service's queue.

Each registered service maintains its own node pool with health tracking, making this a queue-based alternative to traditional load balancers where nodes pull work rather than having it pushed to them.

## Why Pull Model?

Traditional load balancers push requests to backend servers, requiring those servers to be directly addressable. This breaks when compute nodes sit behind NAT, firewalls, or in private networks with no inbound connectivity.

The pull model inverts the connection: nodes initiate outbound gRPC streams to the gateway and wait for task assignments. This means any machine that can make an outbound HTTPS connection can serve as a compute node — no port forwarding, no VPN tunnels, no service mesh required.

**Use cases:** AI inference (LLM, image generation), agent job execution, CI pipelines — any workload where tasks run seconds to minutes and nodes may be on different networks.

## Architecture

```
                        ┌──────────────────────────────────────────────┐
                        │              xgent-gateway                    │
                        │                                              │
 Clients ──────────────►│  HTTP :8080           gRPC :50051            │
  (gRPC or HTTPS)       │  ┌─────────────┐     ┌──────────────┐       │
                        │  │ Axum        │     │ Tonic        │       │
  Submit tasks ────────►│  │ REST API    │     │ TaskService  │       │
  Poll results ────────►│  │ Admin API   │     │ NodeService  │◄───── Nodes
                        │  └──────┬──────┘     └──────┬───────┘       │ (reverse-poll)
 Admin UI ─────────────►│         │                   │                │
  (React SPA)           │         ▼                   ▼                │
                        │  ┌───────────────────────────────────┐      │
                        │  │  Auth Layer (Tower middleware)      │      │
                        │  │  API Key │ mTLS │ Node Token │ Cookie │   │
                        │  └──────────────┬────────────────────┘      │
                        │                 │                            │
                        │  ┌──────────────▼────────────────────┐      │
                        │  │        Redis Streams + Sessions    │      │
                        │  │  Per-service task queues            │      │
                        │  │  Admin sessions (HttpOnly cookies)  │      │
                        │  └────────────────────────────────────┘      │
                        │                                              │
                        │  Background:                                 │
                        │  ├─ Reaper (30s) — timeout detection         │
                        │  ├─ Metrics snapshot (10s) — ring buffer     │
                        │  ├─ Gauge refresh (15s) — Prometheus         │
                        │  └─ Callback delivery — exponential backoff  │
                        └──────────────────────────────────────────────┘
```

### Task Lifecycle

```
  Client submits          Node polls           Node executes        Node reports
  ┌─────────┐          ┌───────────┐          ┌───────────┐       ┌───────────┐
  │ PENDING │──────────│ ASSIGNED  │──────────│  RUNNING  │───────│ COMPLETED │
  └─────────┘  (XREAD  └───────────┘  (local  └───────────┘       └───────────┘
               BLOCK)                  HTTP)                            │
                             │                                    or FAILED
                             │                                         │
                      (node dies)                          ┌───────────┘
                             │                             ▼
                             ▼                     ┌──────────────┐
                      ┌────────────┐               │   Callback   │
                      │  Reaper    │               │   Delivery   │
                      │  (XPENDING │               │  (if URL set)│
                      │   IDLE)    │               └──────────────┘
                      └─────┬──────┘
                            │
                            ▼
                       ┌─────────┐
                       │ FAILED  │
                       └─────────┘
```

## Features

### Dual Protocol

- **gRPC** (port 50051) — Node polling via server-streaming, task submission, result reporting, heartbeat, graceful drain
- **HTTPS** (port 8080) — REST API for task submission, status polling, admin operations

### Authentication

| Protocol | Auth Method | Purpose |
|----------|------------|---------|
| HTTPS clients | API key (Bearer token) | Task submission and result polling |
| gRPC clients | mTLS (mutual TLS) | Client certificate with fingerprint-to-service mapping |
| Internal nodes | Pre-shared token | Scoped to a specific service, validated on every RPC |
| Admin endpoints | Session cookie (Argon2) | Service registration, key management, health |

### Service Registry

- Register/deregister services dynamically via admin API
- Each service gets an isolated Redis Streams task queue
- Service configuration (timeouts, drain settings) persisted in Redis
- Node health tracking per service with stale detection

### Task Reliability

- **Reliable queue:** Atomic move from pending to processing list (Redis consumer groups) — no task lost on restart
- **Background reaper:** Scans XPENDING every 30s for timed-out tasks and marks them failed
- **Callback delivery:** Optional webhook notification on task completion/failure with exponential backoff (configurable retries)

### Observability

- **Structured JSON logging** with task ID, service name, and node context in every log line
- **Prometheus metrics** at `/metrics`:
  - `tasks_submitted_total` — counter by service
  - `tasks_completed_total` — counter by service and status (success/failure)
  - `task_duration_seconds` — histogram from submission to completion
  - `poll_latency_seconds` — histogram of node poll response time
  - `callback_deliveries_total` — counter by service and status
  - `errors_total` — counter by error type
  - `queue_depth` — gauge per service (refreshed every 15s)
  - `nodes_active` — gauge per service (refreshed every 15s)
- **Admin health API** at `/v1/admin/health` — active nodes, last seen time, in-flight task counts

### Node Management

- **Reverse-polling:** Nodes connect via gRPC server-streaming and block until tasks are assigned
- **Heartbeat:** Nodes send periodic heartbeats; gateway detects stale nodes
- **Graceful drain:** Nodes signal drain before shutdown — gateway stops assigning new tasks but waits for in-flight work to complete
- **Runner agent binary:** Built-in `xgent-agent` that polls the gateway and dispatches tasks to a local HTTP service

### Admin Web UI (v1.1)

A built-in React single-page application for managing and monitoring the gateway.

- **Login** — Argon2id password hashing with Redis-backed HttpOnly cookie sessions
- **Dashboard** — Overview cards (services, nodes, queue depth, throughput), live time-series charts (Recharts), color-coded service health badges
- **Service Management** — Card grid with health badges, registration dialog, detail page with node health table, deregister with confirmation
- **Task Management** — Paginated data table with service/status filters, slide-out detail sheet with JSON payload viewer, cancel with confirmation
- **Credential Management** — Tabbed API key and node token views, create with one-time secret reveal, revoke with optimistic removal
- **UI Features** — Dark/light mode with persisted preference, auto-refresh with configurable interval, loading skeletons, error states with retry, toast notifications

Tech: Vite + React 19 + TailwindCSS v4 + shadcn/ui + TanStack Router & Query + Recharts 3.x

### Production Packaging

- Single static binary (musl target, ~15-25MB)
- Multi-stage Dockerfile (Alpine 3.19 runtime)
- jemalloc allocator for musl performance
- Configurable via TOML file, environment variables, or CLI args

## Quick Start

### Prerequisites

- Rust stable (1.85+)
- Redis/Valkey running on `127.0.0.1:6379`
- `protoc` (Protocol Buffers compiler)

### Build

```bash
cargo build --release -p xgent-gateway
```

Produces two binaries:
- `target/release/xgent-gateway` — the gateway server
- `target/release/xgent-agent` — the node-side runner agent

### Run

```bash
# Start with defaults (HTTP :8080, gRPC :50051, Redis localhost:6379)
./target/release/xgent-gateway

# Or with custom config
./target/release/xgent-gateway --config gateway.toml
```

### Set Up Admin Account

Generate a password hash for `gateway.toml`:

```bash
./target/release/xgent-gateway hash-password
# Enter password at prompt, outputs Argon2id PHC hash
```

Add to `gateway.toml`:

```toml
[admin]
username = "admin"
password_hash = "$argon2id$v=19$m=19456,t=2,p=1$..."
cors_origin = "http://localhost:5173"  # For dev; omit in production if same-origin
```

### Register a Service

```bash
curl -s -X POST http://localhost:8080/v1/admin/services \
  -H "Content-Type: application/json" \
  -d '{"name": "my-service", "task_timeout_secs": 300}'
```

### Create Credentials

```bash
# API key for clients
curl -s -X POST http://localhost:8080/v1/admin/api-keys \
  -H "Content-Type: application/json" \
  -d '{"service_names": ["my-service"]}'
# Save the returned api_key — shown only once

# Node token for internal nodes
curl -s -X POST http://localhost:8080/v1/admin/node-tokens \
  -H "Content-Type: application/json" \
  -d '{"service_name": "my-service", "node_label": "worker-1"}'
# Save the returned token
```

### Submit a Task

```bash
curl -s -X POST http://localhost:8080/v1/tasks \
  -H "Authorization: Bearer <api-key>" \
  -H "Content-Type: application/json" \
  -d '{
    "service_name": "my-service",
    "payload": {"message": "hello world"},
    "metadata": {"env": "test"}
  }'
# Returns: {"task_id": "019d1364-..."}
```

Payload is any valid JSON value (object, array, string, number, boolean, null). The gateway treats payloads as opaque -- it stores and forwards them without inspection.

### Poll for Results

```bash
curl -s http://localhost:8080/v1/tasks/<task-id> \
  -H "Authorization: Bearer <api-key>"
```

### Start a Node (Runner Agent)

```bash
./target/release/xgent-agent --config agent.yaml
```

The agent reads its configuration from `agent.yaml` (gateway address, service name, execution mode, and response mapping), connects to the gateway via gRPC, picks up task assignments, executes them using the configured mode (CLI, sync-api, or async-api), and reports results back. See `examples/` for sample configs.

Use `--dry-run` to validate the config without connecting:

```bash
./target/release/xgent-agent --config agent.yaml --dry-run
```

### Sample Echo Service

A minimal sample service is included for end-to-end testing:

```bash
cargo run --example sample_service -p xgent-gateway
# Listens on 127.0.0.1:8090, echoes payloads back
```

## Configuration

### Config File (TOML)

See [`gateway.toml`](gateway.toml) for the full default configuration.

| Section | Key | Default | Description |
|---------|-----|---------|-------------|
| `grpc` | `enabled` | `true` | Enable gRPC server |
| `grpc` | `listen_addr` | `0.0.0.0:50051` | gRPC listen address |
| `http` | `enabled` | `true` | Enable HTTP server |
| `http` | `listen_addr` | `0.0.0.0:8080` | HTTP listen address |
| `redis` | `url` | `redis://127.0.0.1:6379` | Redis connection URL |
| `redis` | `result_ttl_secs` | `86400` | TTL for completed task data (24h) |
| `queue` | `stream_maxlen` | `10000` | Max entries per Redis Stream |
| `queue` | `block_timeout_ms` | `5000` | XREADGROUP block timeout |
| `admin` | `username` | (none) | Admin login username (enables session auth) |
| `admin` | `password_hash` | (none) | Argon2id PHC-format password hash |
| `admin` | `session_ttl_secs` | `3600` | Session TTL in Redis |
| `admin` | `cookie_secure` | `true` | Set Secure flag on session cookie |
| `admin` | `cors_origin` | (none) | CORS origin for admin UI (e.g., `http://localhost:5173`) |
| `service_defaults` | `task_timeout_secs` | `300` | Reaper timeout threshold |
| `service_defaults` | `node_stale_after_secs` | `60` | Node staleness threshold |
| `service_defaults` | `drain_timeout_secs` | `300` | Max drain wait time |
| `callback` | `max_retries` | `3` | Callback retry attempts |
| `callback` | `initial_delay_ms` | `1000` | Base delay (doubles each retry) |
| `logging` | `format` | `json` | Log format (`json` or `text`) |

### Environment Variable Overrides

Any config value can be overridden with `GATEWAY__` prefix (double underscore separator):

```bash
GATEWAY__REDIS__URL="redis://custom:6379" \
GATEWAY__SERVICE_DEFAULTS__TASK_TIMEOUT_SECS=60 \
./target/release/xgent-gateway
```

### TLS Configuration

```toml
[http.tls]
cert_path = "/path/to/server.crt"
key_path = "/path/to/server.key"

[grpc.tls]
cert_path = "/path/to/server.crt"
key_path = "/path/to/server.key"
client_ca_path = "/path/to/ca.crt"  # Enables mTLS

# Optional: map client cert fingerprints to authorized services
[grpc.mtls_identity.fingerprints]
"a1b2c3d4e5f6..." = ["my-service", "other-service"]
```

## API Reference

### Client API (HTTP)

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| POST | `/v1/tasks` | API Key | Submit a task |
| GET | `/v1/tasks/{task_id}` | API Key | Get task status and result |

### Client API (gRPC)

| RPC | Service | Auth | Description |
|-----|---------|------|-------------|
| `SubmitTask` | `TaskService` | API Key | Submit a task |
| `GetTaskStatus` | `TaskService` | API Key | Get task status and result |

### Node API (gRPC)

| RPC | Service | Auth | Description |
|-----|---------|------|-------------|
| `PollTasks` | `NodeService` | Node Token | Server-streaming poll for task assignments |
| `ReportResult` | `NodeService` | Node Token | Report task completion (success/failure) |
| `Heartbeat` | `NodeService` | Node Token | Send heartbeat to gateway |
| `DrainNode` | `NodeService` | Node Token | Signal graceful drain |

### Auth API (HTTP)

| Method | Endpoint | Auth | Description |
|--------|----------|------|-------------|
| POST | `/v1/admin/auth/login` | None | Login with username/password, returns session cookie |
| POST | `/v1/admin/auth/logout` | Session | Destroy session and clear cookie |
| POST | `/v1/admin/auth/refresh` | Session | Extend session TTL |

### Admin API (HTTP, session-protected)

| Method | Endpoint | Description |
|--------|----------|-------------|
| POST | `/v1/admin/services` | Register a service |
| GET | `/v1/admin/services` | List all services |
| GET | `/v1/admin/services/{name}` | Service details + node health |
| DELETE | `/v1/admin/services/{name}` | Deregister service (drains queue) |
| GET | `/v1/admin/api-keys` | List API keys (masked) |
| POST | `/v1/admin/api-keys` | Create API key (secret shown once) |
| POST | `/v1/admin/api-keys/revoke` | Revoke API key |
| PATCH | `/v1/admin/api-keys/{key_hash}` | Update callback URL |
| GET | `/v1/admin/node-tokens` | List node tokens (masked) |
| POST | `/v1/admin/node-tokens` | Create node token (secret shown once) |
| POST | `/v1/admin/node-tokens/revoke` | Revoke node token |
| GET | `/v1/admin/tasks` | List tasks (paginated, filterable by service/status) |
| GET | `/v1/admin/tasks/{task_id}` | Task detail (metadata, timestamps, payload, result) |
| POST | `/v1/admin/tasks/{task_id}/cancel` | Cancel pending/running/assigned task |
| GET | `/v1/admin/health` | Node health dashboard data |
| GET | `/v1/admin/metrics/summary` | Dashboard overview (service count, nodes, queue depth, throughput) |
| GET | `/v1/admin/metrics/history` | Time-series ring buffer data (30min, 10s intervals) |
| GET | `/metrics` | Prometheus metrics (raw) |

## Docker

### Build

```bash
docker build -t xgent-gateway .
```

### Run

```bash
docker run -p 8080:8080 -p 50051:50051 \
  -e GATEWAY__REDIS__URL="redis://host.docker.internal:6379" \
  xgent-gateway
```

Or mount a custom config:

```bash
docker run -p 8080:8080 -p 50051:50051 \
  -v $(pwd)/gateway.toml:/etc/xgent/gateway.toml \
  xgent-gateway
```

## Project Structure

```
xgent-ai-gateway/
├── Cargo.toml              # Workspace: proto + gateway crates
├── Dockerfile              # Multi-stage build (musl static binary)
├── gateway.toml            # Default configuration
├── proto/
│   └── src/gateway.proto   # gRPC service definitions
├── gateway/
│   ├── src/
│   │   ├── main.rs          # Entry point, server startup
│   │   ├── lib.rs           # Library target (for tests)
│   │   ├── config.rs        # Layered config (TOML + env + CLI)
│   │   ├── state.rs         # Shared AppState (queue, auth, metrics)
│   │   ├── types.rs         # Task, TaskState, ServiceConfig
│   │   ├── error.rs         # Error types
│   │   ├── metrics.rs       # Prometheus metrics registry
│   │   ├── metrics_history.rs # Ring buffer for dashboard time-series
│   │   ├── auth/            # API key + node token + session auth
│   │   ├── grpc/            # Tonic services (TaskService, NodeService, auth layers)
│   │   ├── http/            # Axum handlers (submit, result, admin, auth)
│   │   ├── queue/           # Redis Streams queue (submit, poll, report, CRUD)
│   │   ├── registry/        # Service registry + node health tracking
│   │   ├── reaper/          # Background timeout detection (XPENDING scan)
│   │   ├── callback/        # Webhook delivery with exponential backoff
│   │   ├── tls/             # rustls config builders (HTTP TLS, gRPC mTLS)
│   │   └── bin/
│   │       └── agent.rs     # Runner agent binary (node-side proxy)
│   ├── examples/
│   │   └── sample_service.rs # Echo service for E2E testing
│   └── tests/               # Integration tests (require Redis)
└── admin-ui/                # React SPA (Vite + React 19)
    ├── src/
    │   ├── routes/           # TanStack Router file-based routes
    │   │   ├── _authenticated/
    │   │   │   ├── index.tsx       # Dashboard (overview cards, charts, health)
    │   │   │   ├── services.tsx    # Service list (card grid, registration)
    │   │   │   ├── services.$name.tsx # Service detail (config, nodes, deregister)
    │   │   │   ├── tasks.tsx       # Task list (filters, detail sheet, cancel)
    │   │   │   └── credentials.tsx # API keys + node tokens (tabs, create, revoke)
    │   │   └── login.tsx           # Login page
    │   ├── components/       # shadcn/ui + custom components
    │   └── lib/              # API client, hooks, utilities
    └── package.json
```

## Tech Stack

### Gateway (Rust)

| Component | Technology | Version |
|-----------|-----------|---------|
| Language | Rust | stable 1.85+ |
| Async runtime | Tokio | 1.50+ |
| gRPC | Tonic + Prost | 0.14.x |
| HTTP | Axum | 0.8.x |
| TLS | rustls | 0.23.x |
| Task queue | Redis Streams | redis-rs 1.0.x |
| Auth | Argon2id (password) + HttpOnly cookies (sessions) | argon2 0.5.x |
| Metrics | Prometheus | 0.14.x |
| Logging | tracing | 0.1.x |
| Allocator (musl) | jemalloc | tikv-jemallocator 0.6 |

### Admin UI (TypeScript)

| Component | Technology | Version |
|-----------|-----------|---------|
| Bundler | Vite | 6.x |
| Framework | React | 19.x |
| Routing | TanStack Router | file-based |
| Data fetching | TanStack Query | 5.x |
| Components | shadcn/ui | v4 (oklch) |
| Styling | TailwindCSS | v4 |
| Charts | Recharts | 3.x |

## Testing

```bash
# Unit tests (no Redis needed)
cargo test -p xgent-gateway

# Integration tests (requires Redis on localhost:6379)
cargo test -p xgent-gateway --test integration_test -- --ignored
cargo test -p xgent-gateway --test auth_integration_test -- --ignored
cargo test -p xgent-gateway --test registry_integration_test -- --ignored
cargo test -p xgent-gateway --test reaper_callback_integration_test -- --ignored
cargo test -p xgent-gateway --test grpc_auth_test -- --ignored

# Override Redis URL
REDIS_URL="redis://custom:6379" cargo test -p xgent-gateway --test integration_test -- --ignored
```

## Known Limitations

- **No task retries:** Failed tasks are terminal. Clients resubmit on failure.
- **No dead letter queue:** Failed tasks remain in Redis with TTL; no separate DLQ.
- **No HTTP node polling:** Nodes must use gRPC. The runner agent proxies to local HTTP services.
- **Single Redis instance:** No built-in clustering or replication (use Redis Sentinel/Cluster externally).
- **No rate limiting:** Deploy behind an API gateway (nginx/Envoy) for rate limiting.
- **Single admin account:** Configured in `gateway.toml`; no RBAC or multi-user support.
- **No log viewer:** Admin UI shows metrics but not application logs; use external log aggregation.

## License

MIT
