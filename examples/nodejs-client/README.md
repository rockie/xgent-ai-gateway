# Node.js Client Examples

Three scripts demonstrating task submission and result retrieval via the gateway HTTP API. Each script targets a different agent execution mode (CLI, sync-API, async-API).

## Prerequisites

- **Node.js 18+** (for native `fetch` support)
- Gateway running with an API key configured
- Agent running for the target service (see each service's README)

## Quick Start

### CLI Echo Client

Submit a task to the `cli-echo` service:

```bash
node cli-client.js "Hello, CLI!"
```

### Sync-API Echo Client

Submit a task to the `sync-echo` service:

```bash
node sync-api-client.js "Hello, sync-api!"
```

### Async-API Echo Client

Submit a task to the `async-echo` service (may take a few more seconds due to async polling):

```bash
node async-api-client.js "Hello, async-api!"
```

### Using npm Scripts

```bash
npm run cli-client
npm run sync-api-client
npm run async-api-client
```

## Environment Variables

| Variable      | Default                  | Description                          |
| ------------- | ------------------------ | ------------------------------------ |
| `GATEWAY_URL` | `http://localhost:3000`  | Base URL of the gateway HTTP API     |
| `API_KEY`     | `dev-api-key`            | API key for the Authorization header |

Example with custom values:

```bash
GATEWAY_URL=https://gateway.example.com API_KEY=my-secret-key node cli-client.js "test"
```

## How It Works

All three scripts follow the same pattern:

1. **Submit** -- POST to `/api/v1/tasks` with a `service_name` and `payload`. The gateway returns a `task_id`.
2. **Poll** -- GET `/api/v1/tasks/{task_id}` every second until the task status is `completed` or `failed` (max 30 polls).
3. **Print** -- Display the task result on success, or the error message on failure.

### Task Lifecycle

```
Client                    Gateway                   Agent
  |                         |                         |
  |-- POST /api/v1/tasks -->|                         |
  |<-- { task_id } ---------|                         |
  |                         |<-- gRPC poll -----------|
  |                         |--- task payload ------->|
  |                         |                         |-- execute
  |                         |<-- result --------------|
  |-- GET /tasks/:id ------>|                         |
  |<-- { status, result } --|                         |
```

## No Dependencies

These scripts use Node.js built-in `fetch` (available since Node 18). There is no `node_modules` directory and no `npm install` step required. The `package.json` only provides convenience start scripts.

## Troubleshooting

- **`fetch is not defined`** -- You need Node.js 18 or later. Check with `node --version`.
- **`Submit failed: 401`** -- The API key is incorrect. Set `API_KEY` to a valid key.
- **`Submit failed: 404`** -- The service is not registered. Register `cli-echo`, `sync-echo`, or `async-echo` in the gateway.
- **Timeout after 30 polls** -- The agent may not be running or may not be configured for the correct service.
