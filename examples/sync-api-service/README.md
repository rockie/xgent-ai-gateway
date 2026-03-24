# Sync-API Execution Example

Demonstrates the agent forwarding task payloads to a synchronous HTTP endpoint and extracting the result from the JSON response.

## Prerequisites

- Gateway binary built: `cargo build -p xgent-gateway`
- Redis/Valkey running on `localhost:6379`
- Service `sync-echo` registered in the gateway
- Agent binary built: `cargo build -p xgent-gateway --bin xgent-agent`
- Sample service built: `cargo build -p xgent-gateway --example sample_service`

## Quick Start

1. **Start Redis** (if not already running):

   ```bash
   redis-server
   ```

2. **Start the gateway:**

   ```bash
   cargo run -p xgent-gateway
   ```

3. **Register the `sync-echo` service** via the admin API or admin UI.

4. **Start the sample service** (provides the sync-api echo endpoint):

   ```bash
   cargo run -p xgent-gateway --example sample_service
   ```

   This starts an HTTP server on `localhost:8090` with a `POST /sync` endpoint.

5. **Start the agent:**

   ```bash
   cargo run -p xgent-gateway --bin xgent-agent -- --config examples/sync-api-service/agent.yaml
   ```

6. **Submit a task:**

   ```bash
   # Using curl
   curl -X POST http://localhost:3000/api/v1/tasks \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer dev-api-key" \
     -d '{"service_name": "sync-echo", "payload": "Hello, sync-api!"}'

   # Using Node.js client
   cd examples/nodejs-client && node sync-api-client.js "Hello, sync-api!"
   ```

## Config Walkthrough

### agent.yaml

```yaml
gateway:
  address: "localhost:50051"    # gRPC address of the gateway
  token: "dev-token"            # Pre-shared node authentication token
  node_id: "sync-api-node"     # Unique identifier for this agent node

service:
  name: "sync-echo"            # Service queue to poll tasks from
  mode: sync-api               # Execution mode: sync-api

sync_api:
  url: "http://localhost:8090/sync"   # Target HTTP endpoint
  method: "POST"                       # HTTP method
  headers:
    Content-Type: "application/json"   # Request headers
  body: '<payload>'                    # Request body -- <payload> replaced with task payload
  timeout_secs: 30                     # HTTP request timeout

response:
  success:
    body: '<response.result.text>'     # Extract result.text from the JSON response
  max_bytes: 65536
```

### Key Fields

- **`sync_api.url`** -- The HTTP endpoint the agent calls. Can include `<payload>` or other placeholders.
- **`sync_api.body: '<payload>'`** -- The task payload is inserted as the request body. For JSON payloads, the entire body becomes the task payload string.
- **`response.success.body: '<response.result.text>'`** -- Extracts the `result.text` field from the JSON response. The `response.` prefix navigates the HTTP response body.

## What Happens

1. **Client submits task** -- POST to gateway with `service_name: "sync-echo"` and a payload string.
2. **Gateway queues task** -- Task enters the `sync-echo` service queue.
3. **Agent polls gateway** -- Picks up the task via gRPC reverse-polling.
4. **Agent sends HTTP POST** -- Forwards the payload to `localhost:8090/sync`.
5. **Sample service responds** -- The `/sync` endpoint echoes the payload in a JSON wrapper: `{"result": {"text": "...", "processed": true}}`.
6. **Agent extracts result** -- The `<response.result.text>` placeholder is resolved from the JSON response.
7. **Agent reports result** -- Sends the extracted text back to the gateway as the task result.
8. **Client retrieves result** -- GET the task by ID to see the final result.

## Customization

- **Change the target URL** to point at your own HTTP service.
- **Add authentication headers**: `headers: { Authorization: "Bearer my-token" }`.
- **Use environment variables** in the URL: `url: "http://${API_HOST}/process"`.
- **Extract nested fields** from the response: `<response.data.items[0].name>`.
- **Map multiple response fields** into a template: `body: '{"text": "<response.text>", "id": "<response.id>"}'`.
