#!/usr/bin/env node
// sync-api-client.js - Submit a task to the sync-API echo service and retrieve the result.
// Usage: node sync-api-client.js [payload]
// Requires: Node 18+ (native fetch), gateway running, agent running with sync-echo config

const GATEWAY_URL = process.env.GATEWAY_URL || 'http://localhost:3000';
const API_KEY = process.env.API_KEY || 'dev-api-key';
const payload = process.argv[2] || 'Hello from sync-api client!';

async function main() {
  console.log('Submitting task to sync-API echo service...');
  console.log(`  Gateway: ${GATEWAY_URL}`);
  console.log(`  Payload: ${payload}`);
  console.log();

  // Submit task
  const submitRes = await fetch(`${GATEWAY_URL}/v1/tasks`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      'Authorization': `Bearer ${API_KEY}`,
    },
    body: JSON.stringify({
      service_name: 'sync-echo',
      payload: { message: payload },
    }),
  });

  if (!submitRes.ok) {
    console.error(`Submit failed: ${submitRes.status} ${await submitRes.text()}`);
    process.exit(1);
  }

  const { task_id } = await submitRes.json();
  console.log(`Task submitted: ${task_id}`);
  console.log('Polling for result...');

  // Poll for result
  for (let i = 0; i < 30; i++) {
    await new Promise(r => setTimeout(r, 1000));

    const pollRes = await fetch(`${GATEWAY_URL}/v1/tasks/${task_id}`, {
      headers: { 'Authorization': `Bearer ${API_KEY}` },
    });

    const task = await pollRes.json();
    console.log(`  [poll ${i + 1}] state: ${task.state}`);

    if (task.state === 'completed') {
      console.log();
      console.log('Result:');
      console.log(JSON.stringify(task.result, null, 2));
      process.exit(0);
    }

    if (task.state === 'failed') {
      console.error();
      console.error(`Task failed: ${task.error_message}`);
      process.exit(1);
    }
  }

  console.error('Timeout: task did not complete within 30 polls');
  process.exit(1);
}

main().catch(err => { console.error(err); process.exit(1); });
