#!/usr/bin/env node
// async-api-client.js - Submit a task to the async-API echo service and retrieve the result.
// Usage: node async-api-client.js [payload]
// Requires: Node 18+ (native fetch), gateway running, agent running with async-echo config
//
// Note: The async-API service completes after ~3 agent poll cycles, so this client
// may need more polls than the CLI or sync-API examples.

const GATEWAY_URL = process.env.GATEWAY_URL || 'http://localhost:8080';
const API_KEY = process.env.API_KEY || 'ab5e7b5f5a57978bf7a15379eb29a32c86e836461ac069833b4867e8f75a61de';
const payload = process.argv[2] || 'Hello from async-api client!';

async function main() {
  console.log('Submitting task to async-API echo service...');
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
      service_name: 'async-service',
      payload: { "prompt": "一副典雅庄重的对联悬挂于厅堂之中，房间是个安静古典的中式布置，桌子上放着一些青花瓷，对联上左书“义本生知人机同道善思新”，右书“通云赋智乾坤启数高志远”， 横批“智启千问”，字体飘逸，在中间挂着一幅中国风的画作，内容是岳阳楼。", "negative_prompt": "" },
    }),
  });

  if (!submitRes.ok) {
    console.error(`Submit failed: ${submitRes.status} ${await submitRes.text()}`);
    process.exit(1);
  }

  const { task_id } = await submitRes.json();
  console.log(`Task submitted: ${task_id}`);
  console.log('Polling for result (async service takes ~3 agent poll cycles)...');

  // Poll for result — async tasks may take longer
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
