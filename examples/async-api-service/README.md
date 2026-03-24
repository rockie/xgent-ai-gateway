# Async-API Execution Example

Demonstrates the agent performing a two-phase async HTTP flow: submit a job, then poll a status endpoint until the job completes. This is ideal for long-running external processes.

## Prerequisites

- Gateway binary built: `cargo build -p xgent-gateway`
- Redis/Valkey running on `localhost:6379`
- Service `async-echo` registered in the gateway
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

3. **Register the `async-echo` service** via the admin API or admin UI.

4. **Start the sample service** (provides the async-api endpoints):

   ```bash
   cargo run -p xgent-gateway --example sample_service
   ```

   This starts an HTTP server on `localhost:8090` with `POST /async/submit` and `GET /async/status/:id` endpoints. Jobs complete after 3 poll requests.

5. **Start the agent:**

   ```bash
   cargo run -p xgent-gateway --bin xgent-agent -- --config examples/async-api-service/agent.yaml
   ```

6. **Submit a task:**

   ```bash
   # Using curl
   curl -X POST http://localhost:3000/api/v1/tasks \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer dev-api-key" \
     -d '{"service_name": "async-echo", "payload": "Hello, async-api!"}'

   # Using Node.js client
   cd examples/nodejs-client && node async-api-client.js "Hello, async-api!"
   ```

## Config Walkthrough

### agent.yaml

```yaml
gateway:
  address: "localhost:50051"    # gRPC address of the gateway
  token: "dev-token"            # Pre-shared node authentication token
  node_id: "async-api-node"    # Unique identifier for this agent node

service:
  name: "async-echo"           # Service queue to poll tasks from
  mode: async-api              # Execution mode: async-api

async_api:
  timeout_secs: 120            # Overall timeout for submit + all polls

  submit:
    url: "http://localhost:8090/async/submit"    # Job submission endpoint
    method: "POST"
    headers:
      Content-Type: "application/json"
    body: '<payload>'                             # Task payload becomes request body

  poll:
    url: "http://localhost:8090/async/status/<submit_response.job_id>"
    #                                        ^--- Extracted from the submit response
    method: "GET"
    interval_secs: 2           # Poll every 2 seconds

  completed_when:              # Condition: job is done
    path: "status"             # JSON path to check in poll response
    operator: equal            # Comparison operator
    value: "completed"         # Expected value

  failed_when:                 # Condition: job has failed
    path: "status"
    operator: in
    value: ["failed", "error"]

response:
  success:
    body: '{"result": "<poll_response.result.text>", "processed": "<poll_response.result.processed>"}'
    #      ^--- Template with values extracted from the final poll response
  max_bytes: 65536
```

### Key Fields

- **`async_api.submit`** -- First HTTP request to kick off the job. The response is available as `<submit_response.*>` for use in the poll URL.
- **`async_api.poll.url`** -- Contains `<submit_response.job_id>`, which is replaced with the `job_id` field from the submit response. This builds the status URL dynamically.
- **`async_api.poll.interval_secs`** -- How often the agent checks the poll endpoint.
- **`completed_when`** -- The agent evaluates this condition against each poll response. When `status == "completed"`, the job is done.
- **`failed_when`** -- If `status` is `"failed"` or `"error"`, the agent reports a failure immediately.
- **`response.success.body`** -- Template with `<poll_response.*>` placeholders resolved from the final successful poll response.

## What Happens

1. **Client submits task** -- POST to gateway with `service_name: "async-echo"` and a payload.
2. **Gateway queues task** -- Task enters the `async-echo` service queue.
3. **Agent polls gateway** -- Picks up the task via gRPC reverse-polling.
4. **Agent submits job** -- POST to `localhost:8090/async/submit` with the payload. Receives `{"job_id": "..."}`.
5. **Agent polls for completion** -- GET `localhost:8090/async/status/{job_id}` every 2 seconds. The sample service returns `"status": "pending"` for the first 2 polls.
6. **Job completes** -- On the 3rd poll, the sample service returns `"status": "completed"` with the result.
7. **Agent extracts result** -- `<poll_response.result.text>` and `<poll_response.result.processed>` are resolved from the final poll response.
8. **Agent reports result** -- The templated JSON string is sent back to the gateway as the task result.
9. **Client retrieves result** -- GET the task by ID to see the final result.

## Customization

- **Change the submit/poll URLs** to point at your own async job API.
- **Adjust `interval_secs`** based on your job's expected duration (lower for fast jobs, higher for slow ones).
- **Modify `completed_when`** for different response shapes: `path: "data.state"`, `operator: in`, `value: ["done", "success"]`.
- **Add `failed_when`** conditions to catch error states early rather than waiting for timeout.
- **Increase `timeout_secs`** for long-running jobs (minutes to hours).
- **Use headers for auth**: `submit.headers: { Authorization: "Bearer ${JOB_API_KEY}" }`.
