# CLI Execution Example

Demonstrates the agent executing a shell script, supporting both **arg mode** (payload as command argument) and **stdin mode** (payload piped to stdin).

## Prerequisites

- Gateway binary built: `cargo build -p xgent-gateway`
- Redis/Valkey running on `localhost:6379`
- Service `cli-echo` registered in the gateway (via admin UI or API)
- Agent binary built: `cargo build -p xgent-gateway --bin xgent-agent`

## Quick Start

1. **Start Redis** (if not already running):

   ```bash
   redis-server
   ```

2. **Start the gateway:**

   ```bash
   cargo run -p xgent-gateway
   ```

3. **Register the `cli-echo` service** via the admin API or admin UI.

4. **Start the agent in arg mode:**

   ```bash
   cargo run -p xgent-gateway --bin xgent-agent -- --config examples/cli-service/agent-arg.yaml
   ```

5. **Submit a task** using the Node.js client or curl:

   ```bash
   # Using curl
   curl -X POST http://localhost:8080/v1/tasks \
     -H "Content-Type: application/json" \
     -H "Authorization: Bearer dev-api-key" \
     -d '{"service_name": "cli-echo", "payload": {"message": "Hello, CLI!"}}'

   # Using Node.js client
   cd examples/nodejs-client && node cli-client.js "Hello, CLI!"
   ```

6. **Try stdin mode** by stopping the agent and restarting with the stdin config:

   ```bash
   cargo run -p xgent-gateway --bin xgent-agent -- --config examples/cli-service/agent-stdin.yaml
   ```

## Config Walkthrough

### agent-arg.yaml

```yaml
gateway:
  address: "localhost:50051"    # gRPC address of the gateway
  token: "dev-token"            # Pre-shared node authentication token
  node_id: "cli-arg-node"      # Unique identifier for this agent node

service:
  name: "cli-echo"             # Service queue to poll tasks from
  mode: cli                    # Execution mode: cli, sync-api, or async-api

cli:
  command: ["./examples/cli-service/echo.sh", "<payload>"]
  #         ^--- Script path                   ^--- Replaced with task payload at runtime
  input_mode: arg              # Payload delivered as command argument
  timeout_secs: 30             # Kill the process after 30 seconds

response:
  success:
    body: '<stdout>'           # Map the script's stdout to the task result
  max_bytes: 65536             # Truncate output beyond 64 KB
```

### agent-stdin.yaml

Same as above except:

```yaml
cli:
  command: ["./examples/cli-service/echo.sh"]  # No <payload> in args
  input_mode: stdin            # Payload piped to the script's stdin
```

### echo.sh

The echo script reads input from either args or stdin and outputs a JSON result:

```bash
if [ -n "$1" ]; then
  INPUT="$1"         # Arg mode: read from first argument
else
  INPUT=$(cat)       # Stdin mode: read from stdin
fi
echo '{"output": "processed: ${INPUT}", ...}'
```

## What Happens

1. **Client submits task** -- POST to gateway with `service_name: "cli-echo"` and a payload string.
2. **Gateway queues task** -- Task enters the `cli-echo` service queue with status `queued`.
3. **Agent polls gateway** -- The agent reverse-polls via gRPC and picks up the task.
4. **Agent runs echo.sh** -- In arg mode, `<payload>` in the command array is replaced with the task payload. In stdin mode, the payload is piped to the script's stdin.
5. **Script produces output** -- echo.sh outputs a JSON string to stdout.
6. **Agent maps response** -- The `<stdout>` placeholder in the response template is replaced with the script's stdout.
7. **Agent reports result** -- Result sent back to the gateway, task status becomes `completed`.
8. **Client retrieves result** -- GET the task by ID to see the final result.

## Customization

- **Replace echo.sh** with your own script (Python, Ruby, any executable). Update the `command` path.
- **Add environment variables** to the `cli` section: `env: { MY_VAR: "value" }`.
- **Adjust timeout** for long-running scripts by increasing `timeout_secs`.
- **Change response mapping** -- use `<stderr>` to capture error output, or combine: `"stdout: <stdout>, stderr: <stderr>"`.
