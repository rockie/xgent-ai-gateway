#!/usr/bin/env node
// syntax-check.js - EXMP-04: Verify all client scripts parse without syntax errors.
// Run: node syntax-check.js
// Exits 0 if all pass, 1 if any fail.
//
// This script uses child_process to run `node --check` on each client file
// so that it works as a standalone test without requiring any test framework.

const { execFileSync } = require('child_process');
const path = require('path');

const scripts = [
  'cli-client.js',
  'sync-api-client.js',
  'async-api-client.js',
];

let allPassed = true;

for (const script of scripts) {
  const fullPath = path.join(__dirname, script);
  try {
    execFileSync(process.execPath, ['--check', fullPath], { stdio: 'pipe' });
    console.log(`  ok  ${script}`);
  } catch (err) {
    console.error(`  FAIL  ${script}`);
    console.error(err.stderr ? err.stderr.toString() : err.message);
    allPassed = false;
  }
}

if (allPassed) {
  console.log('\nAll client scripts pass syntax check.');
  process.exit(0);
} else {
  console.error('\nOne or more client scripts failed syntax check.');
  process.exit(1);
}
