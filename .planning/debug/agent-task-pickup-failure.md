---
status: awaiting_human_verify
trigger: "Agent connects to gateway successfully, client submits task which succeeds, but the agent never receives the task. The task stays in pending state until the client polling times out."
created: 2026-03-25T00:00:00Z
updated: 2026-03-25T00:05:00Z
---

## Current Focus

hypothesis: CONFIRMED. redis-rs 1.0.5 MultiplexedConnection has a default response_timeout of 500ms. XREADGROUP BLOCK 5000 (5s) is killed after 500ms by the internal timeout, producing an IO "timed out" error. This was silently caught by the error handler and returned as Ok(None). Messages arriving after 500ms but before 5000ms were orphaned.
test: Applied fix -- blocking_conn now uses AsyncConnectionConfig with response_timeout=None
expecting: Agent should now receive tasks within the BLOCK timeout window
next_action: Human verification -- user should test the full flow (gateway + agent + client)

## Symptoms

expected: Agent polls for tasks via gRPC PollTasks stream, receives task assignments when tasks are submitted
actual: Agent logs "connected to gateway" then sits idle forever. Task submitted via HTTP stays in "pending". Gateway shows NO log entries related to polling activity.
errors: No errors visible -- the system is silent. Agent shows "connected to gateway" only. Gateway shows "task submitted" but no poll activity.
reproduction: 1. Start gateway 2. Start agent 3. Submit task via HTTP 4. Agent never picks up task, client polls 30 times and times out
started: Since redis-rs 1.0 upgrade (which introduced 500ms default response_timeout)

## Eliminated

- hypothesis: Stream key mismatch between submit and poll
  evidence: Both use "tasks:{service}" format with ServiceName type -- verified consistent
  timestamp: 2026-03-25T00:04:00Z

## Evidence

- timestamp: 2026-03-25T00:01:00Z
  checked: poll_task() in redis.rs lines 298-371
  found: Parse error handler at line 329 catches ErrorKind::Parse and returns Ok(None) silently. "timed out" error handler at line 330 also returns Ok(None) silently. No logging anywhere.
  implication: Both timeout and parse failures produce identical silent Ok(None) -- impossible to diagnose.

- timestamp: 2026-03-25T00:02:00Z
  checked: poll.rs loop structure lines 96-208
  found: Ok(None) path does `continue` with no logging. Zero observability for the poll cycle.
  implication: Even with debug logging enabled, no output reveals what poll_task returns.

- timestamp: 2026-03-25T00:03:00Z
  checked: blocking_conn creation in RedisQueue::new()
  found: Uses get_multiplexed_async_connection() with default config. Does NOT set response_timeout.
  implication: Default 500ms response_timeout applies to blocking_conn.

- timestamp: 2026-03-25T00:04:00Z
  checked: redis-rs 1.0.5 source in cargo cache: client.rs line 180
  found: DEFAULT_RESPONSE_TIMEOUT = Some(Duration::from_millis(500)). This is applied to ALL MultiplexedConnections created via get_multiplexed_async_connection().
  implication: XREADGROUP BLOCK 5000ms will ALWAYS be terminated at 500ms by the internal response timeout. The response is orphaned in the multiplexer pipeline.

- timestamp: 2026-03-25T00:05:00Z
  checked: AsyncConnectionConfig API in redis-rs 1.0.5
  found: get_multiplexed_async_connection_with_config() accepts AsyncConnectionConfig with set_response_timeout(None) to disable the timeout.
  implication: Fix is to create blocking_conn with response_timeout=None.

## Resolution

root_cause: redis-rs 1.0.5 MultiplexedConnection has a default response_timeout of 500ms. The blocking_conn used for XREADGROUP BLOCK 5000ms was created with this default, causing every BLOCK call to be terminated after 500ms with an IO "timed out" error. This error was silently caught by `Err(e) if e.to_string().contains("timed out") => return Ok(None)` and returned as "no tasks available." The poll loop then re-issued the BLOCK, creating a fast 500ms cycle that never successfully received messages -- because any message arriving between 500ms-5000ms was orphaned when the response timeout fired first.
fix: (1) Configure blocking_conn with AsyncConnectionConfig::new().set_response_timeout(None) so XREADGROUP BLOCK can complete naturally. (2) Remove the silent "timed out" catch-all -- with no response timeout, IO timeouts now indicate real connection problems and should be surfaced as errors. (3) Add tracing at trace/debug/warn levels for all poll_task outcomes.
verification: cargo check passes, all 149 library tests pass (0 failures)
files_changed:
  - gateway/src/queue/redis.rs
  - gateway/src/grpc/poll.rs
