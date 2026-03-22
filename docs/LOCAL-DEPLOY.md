# Local Deployment Guide

Run the xgent-ai-gateway locally for development and manual testing.

## Prerequisites

- Rust stable (1.85+)
- Redis/Valkey running on `127.0.0.1:6379`
- `grpcurl` (optional, for gRPC testing): `brew install grpcurl`

## 1. Build

```bash
cargo build --release -p xgent-gateway
```

Binary: `./target/release/xgent-gateway`

The agent binary (node-side proxy) is also built:

```bash
./target/release/xgent-agent --help
```

## 2. Configuration

The gateway accepts an optional TOML config file via `--config`. Without it, defaults are used.

### Minimal config (defaults work for dev)

```bash
./target/release/xgent-gateway
```

### Custom config

Create `config.toml`:

```toml
[redis]
url = "redis://127.0.0.1:6379"
result_ttl_secs = 86400          # 24h TTL for completed task data

[http]
listen_addr = "0.0.0.0:8080"

[grpc]
listen_addr = "0.0.0.0:50051"

[queue]
stream_maxlen = 10000
block_timeout_ms = 5000

[service_defaults]
task_timeout_secs = 300          # how long before reaper marks task as failed
node_stale_after_secs = 60
drain_timeout_secs = 300
max_retries = 3

[callback]
max_retries = 3                  # retry attempts after initial failure
initial_delay_ms = 1000          # base delay, doubles each retry (1s → 2s → 4s)
timeout_secs = 10                # HTTP timeout per attempt

# [admin]
# token = "your-admin-secret"   # if set, admin endpoints require this token
```

Run with config:

```bash
RUST_LOG=info ./target/release/xgent-gateway --config config.toml
```

### Environment variable overrides

Any config value can be overridden with `GATEWAY__` prefix (double underscore as separator):

```bash
GATEWAY__REDIS__URL="redis://custom-host:6379" \
GATEWAY__SERVICE_DEFAULTS__TASK_TIMEOUT_SECS=60 \
./target/release/xgent-gateway
```

## 3. Startup

The gateway spawns three concurrent tasks:

| Component | Default Address | Purpose |
|-----------|----------------|---------|
| HTTP server | `0.0.0.0:8080` | Client task submission, admin APIs |
| gRPC server | `0.0.0.0:50051` | Node polling, result reporting |
| Reaper | — (30s interval) | Detects timed-out tasks, marks as failed |

Expected startup logs:

```
INFO xgent_gateway: xgent-gateway starting
INFO xgent_gateway: connected to Redis redis_url=redis://127.0.0.1:6379
INFO xgent_gateway: auth Redis connection established
INFO xgent_gateway: background reaper started (30s interval)
INFO xgent_gateway: gRPC server starting grpc_addr=0.0.0.0:50051
INFO xgent_gateway: HTTP server starting http_addr=0.0.0.0:8080
```

## 4. Quick Smoke Test

### Register a service

```bash
curl -s -X POST http://localhost:8080/v1/admin/services \
  -H "Content-Type: application/json" \
  -d '{"name": "my-service", "task_timeout_secs": 300}'
```

### Create an API key (for clients)

```bash
curl -s -X POST http://localhost:8080/v1/admin/api-keys \
  -H "Content-Type: application/json" \
  -d '{
    "service_names": ["my-service"],
    "callback_url": "http://localhost:9999/webhook"
  }'
# Save the returned api_key — it is shown only once
```

### Create a node token (for internal nodes)

```bash
curl -s -X POST http://localhost:8080/v1/admin/node-tokens \
  -H "Content-Type: application/json" \
  -d '{"service_name": "my-service", "node_label": "worker-1"}'
# Save the returned token
```

### Submit a task (client side)

```bash
curl -s -X POST http://localhost:8080/v1/tasks \
  -H "Authorization: Bearer <api-key>" \
  -H "Content-Type: application/json" \
  -d '{
    "service_name": "my-service",
    "payload": "aGVsbG8gd29ybGQ=",
    "metadata": {"env": "test"},
    "callback_url": "http://localhost:9999/webhook"
  }'
# Returns: {"task_id": "019d1364-..."}
```

`payload` is base64-encoded opaque bytes. `callback_url` is optional and overrides the per-key default.

### Check task status

```bash
curl -s http://localhost:8080/v1/tasks/<task_id> \
  -H "Authorization: Bearer <api-key>"
```

### Node polls for tasks (gRPC)

```bash
grpcurl -plaintext \
  -import-path proto/src -proto gateway.proto \
  -rpc-header "Authorization: Bearer <node-token>" \
  -d '{"service_name": "my-service", "node_id": "node-1"}' \
  localhost:50051 xgent.gateway.v1.NodeService/PollTasks
```

This is a server-streaming call — it blocks until a task is assigned.

### Node reports result (gRPC)

```bash
grpcurl -plaintext \
  -import-path proto/src -proto gateway.proto \
  -rpc-header "Authorization: Bearer <node-token>" \
  -d '{
    "task_id": "<task-id>",
    "success": true,
    "result": "cmVzdWx0X2RhdGE="
  }' \
  localhost:50051 xgent.gateway.v1.NodeService/ReportResult
```

### Using the agent binary (node-side proxy)

Instead of manual gRPC calls, use the built-in agent that polls and dispatches tasks to a local HTTP service:

```bash
./target/release/xgent-agent \
  --gateway-addr localhost:50051 \
  --service-name my-service \
  --token <node-token> \
  --dispatch-url http://localhost:8090/execute \
  --tls-skip-verify
```

The agent will:
1. Connect to the gateway via gRPC streaming
2. Receive task assignments
3. POST task payload to `--dispatch-url`
4. Report the result back to the gateway

## 5. Admin API Reference

All admin endpoints are unauthenticated by default (set `[admin] token` in config to require auth).

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | `/v1/admin/services` | Register a service |
| GET | `/v1/admin/services` | List all services |
| GET | `/v1/admin/services/{name}` | Service details + live node health |
| DELETE | `/v1/admin/services/{name}` | Deregister service (202 Accepted) |
| POST | `/v1/admin/api-keys` | Create API key |
| POST | `/v1/admin/api-keys/revoke` | Revoke API key by `key_hash` |
| PATCH | `/v1/admin/api-keys/{key_hash}` | Update callback URL |
| POST | `/v1/admin/node-tokens` | Create node token |
| POST | `/v1/admin/node-tokens/revoke` | Revoke node token |

## 6. Callback Delivery

When a task reaches a terminal state (completed or failed), the gateway POSTs to the configured callback URL:

```json
{
  "task_id": "019d1364-1c71-75e2-a4cc-15749922f0d6",
  "state": "completed"
}
```

Retry behavior (configurable in `[callback]`):
- 4 total attempts (1 initial + `max_retries`)
- Exponential backoff: `initial_delay_ms * 2^(attempt-1)` → 1s, 2s, 4s
- Failure is fire-and-forget — task state is not affected

Callback URL resolution order:
1. Per-task `callback_url` (in submit request) — highest priority
2. Per-key `callback_url` (set at API key creation) — fallback
3. No callback if neither is set

### Testing callbacks locally

Start a simple receiver:

```bash
python3 -c '
import http.server, json

class H(http.server.BaseHTTPRequestHandler):
    def do_POST(self):
        body = json.loads(self.rfile.read(int(self.headers["Content-Length"])))
        print(f"CALLBACK: {json.dumps(body)}", flush=True)
        self.send_response(200)
        self.end_headers()
        self.wfile.write(b"ok")
    def log_message(self, *a): pass

http.server.HTTPServer(("127.0.0.1", 9999), H).serve_forever()
'
```

Then set `callback_url` to `http://127.0.0.1:9999/webhook` when submitting tasks.

## 7. Reaper (Timeout Detection)

The reaper runs every 30 seconds and scans Redis XPENDING entries for tasks that have been assigned longer than `task_timeout_secs` (per-service, default 300s).

When a timed-out task is found:
1. Task state is set to `failed`
2. Error message: `"task timed out: node did not report result within Xs"`
3. If a callback URL is configured, callback delivery is triggered
4. The stream entry is acknowledged (removed from pending)

For faster testing, use a short timeout:

```toml
[service_defaults]
task_timeout_secs = 10
```

## 8. Redis Data Schema

| Key Pattern | Type | Purpose |
|-------------|------|---------|
| `task:{uuid}` | Hash | Task state, payload, metadata, result, callback_url |
| `tasks:{service}` | Stream | Task queue per service (consumer group: `workers`) |
| `api_keys:{hash}` | Hash | API key metadata (service_names, callback_url) |
| `node_tokens:{service}:{hash}` | Hash | Node token metadata |
| `service:{name}` | Hash | Service configuration |
| `services:index` | Set | Service name index |
| `nodes:{service}` | Set | Node IDs per service |
| `node:{service}:{id}` | Hash | Node health (last_seen, in_flight_tasks) |
| `failed_count:{service}` | String | Counter of failed tasks per service |

Inspect task state directly:

```bash
redis-cli HGETALL task:<task-id>
```

## 9. Integration Tests

Run against a live Redis instance:

```bash
# All integration tests
cargo test -p xgent-gateway --test integration_test -- --ignored --nocapture
cargo test -p xgent-gateway --test auth_integration_test -- --ignored --nocapture
cargo test -p xgent-gateway --test registry_integration_test -- --ignored --nocapture
cargo test -p xgent-gateway --test reaper_callback_integration_test -- --ignored --nocapture

# Unit tests (no Redis needed)
cargo test -p xgent-gateway
```

Override Redis URL:

```bash
REDIS_URL="redis://custom:6379" cargo test -p xgent-gateway --test integration_test -- --ignored
```
