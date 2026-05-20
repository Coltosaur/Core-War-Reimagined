// End-to-end matchmaking test orchestrator.
//
// Spawns bot-red.mjs and bot-blue.mjs as separate Node processes, captures
// their JSON-lines stdout, and asserts cross-bot properties: both bots got
// distinct socket sids, both saw match:found for the same match_id, both saw
// match:result. Exits 0 on pass, non-zero on fail.
//
// Prerequisites:
//   1. docker compose up -d (postgres + redis)
//   2. cargo run --bin seed-test-users --features dev-fixtures (in backend/) — one-time
//   3. cargo run (in backend/) — leave running
//
// Usage:
//   node frontend/scripts/matchmaking-e2e.mjs

import { spawn } from 'node:child_process';
import { readFileSync, existsSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, resolve } from 'node:path';

const __dirname = dirname(fileURLToPath(import.meta.url));

function loadEnvFile(path) {
  if (!existsSync(path)) return {};
  const out = {};
  for (const line of readFileSync(path, 'utf-8').split('\n')) {
    const trimmed = line.trim();
    if (!trimmed || trimmed.startsWith('#')) continue;
    const eq = trimmed.indexOf('=');
    if (eq < 0) continue;
    out[trimmed.slice(0, eq).trim()] = trimmed.slice(eq + 1).trim();
  }
  return out;
}

const backendEnv = loadEnvFile(resolve(__dirname, '../../backend/.env'));
const password = process.env.TEST_USER_PASSWORD ?? backendEnv.TEST_USER_PASSWORD;
if (!password) {
  console.error(
    'TEST_USER_PASSWORD not set. Either export it or define it in backend/.env.',
  );
  process.exit(1);
}

function startBot(name) {
  const script = resolve(__dirname, name);
  const child = spawn('node', [script], {
    env: { ...process.env, TEST_USER_PASSWORD: password },
    stdio: ['ignore', 'pipe', 'pipe'],
  });
  const events = [];
  let stderrBuf = '';
  let stdoutBuf = '';

  child.stdout.setEncoding('utf-8');
  child.stdout.on('data', (chunk) => {
    stdoutBuf += chunk;
    let idx;
    while ((idx = stdoutBuf.indexOf('\n')) >= 0) {
      const line = stdoutBuf.slice(0, idx);
      stdoutBuf = stdoutBuf.slice(idx + 1);
      if (!line.trim()) continue;
      try {
        events.push(JSON.parse(line));
      } catch {
        events.push({ raw: line });
      }
    }
  });
  child.stderr.setEncoding('utf-8');
  child.stderr.on('data', (chunk) => {
    stderrBuf += chunk;
  });

  const exitPromise = new Promise((resolveExit) => {
    child.on('exit', (code) => resolveExit({ code, events, stderr: stderrBuf }));
  });

  return { child, exitPromise };
}

console.log('spawning bot-red and bot-blue as separate Node processes...');
const red = startBot('bot-red.mjs');
// Brief stagger to make ordering deterministic (red joins queue first, blue triggers match).
await new Promise((r) => setTimeout(r, 250));
const blue = startBot('bot-blue.mjs');

const [redResult, blueResult] = await Promise.all([red.exitPromise, blue.exitPromise]);

function dumpBot(label, r) {
  console.log(`\n--- ${label} events (${r.events.length}) ---`);
  for (const e of r.events) console.log('  ' + JSON.stringify(e));
  console.log(`${label} exit code: ${r.code}`);
  if (r.stderr) console.log(`${label} stderr:\n${r.stderr}`);
}

dumpBot('bot-red', redResult);
dumpBot('bot-blue', blueResult);

function findEvent(events, name) {
  return events.find((e) => e.event === name);
}

const failures = [];

if (redResult.code !== 0) failures.push(`bot-red exited with code ${redResult.code}`);
if (blueResult.code !== 0) failures.push(`bot-blue exited with code ${blueResult.code}`);

const redConnect = findEvent(redResult.events, 'socket:connect');
const blueConnect = findEvent(blueResult.events, 'socket:connect');

if (!redConnect) failures.push('bot-red never connected');
if (!blueConnect) failures.push('bot-blue never connected');

if (redConnect && blueConnect) {
  if (redConnect.data.sid === blueConnect.data.sid) {
    failures.push(
      `DISTINCT-SIDS check FAILED — both bots got sid=${redConnect.data.sid}. ` +
        `Two separate Node processes should never share a sid; if this fires, ` +
        `something is deeply wrong (server reusing sids, or processes aren't actually separate).`,
    );
  } else {
    console.log(`\n✓ distinct sids — red=${redConnect.data.sid}, blue=${blueConnect.data.sid}`);
  }
}

const redFound = findEvent(redResult.events, 'match:found');
const blueFound = findEvent(blueResult.events, 'match:found');

if (!redFound) failures.push('bot-red never received match:found');
if (!blueFound) failures.push('bot-blue never received match:found');

if (redFound && blueFound) {
  if (redFound.data.match_id !== blueFound.data.match_id) {
    failures.push(
      `match:found mismatch — red=${redFound.data.match_id}, blue=${blueFound.data.match_id}`,
    );
  } else {
    console.log(`✓ both saw match:found — match_id=${redFound.data.match_id}`);
  }
}

const redStart = findEvent(redResult.events, 'match:start');
const blueStart = findEvent(blueResult.events, 'match:start');

if (!redStart) failures.push('bot-red never received match:start');
if (!blueStart) failures.push('bot-blue never received match:start');

if (redStart && blueStart) {
  const requiredFields = [
    'match_id',
    'red_username',
    'blue_username',
    'red_warrior_source',
    'blue_warrior_source',
    'core_size',
    'max_steps',
    'red_start',
    'blue_start',
    'steps_taken',
    'result',
    'playback_start_time_ms',
    'steps_per_sec',
  ];
  for (const f of requiredFields) {
    if (!(f in redStart.data)) failures.push(`bot-red match:start missing field "${f}"`);
    if (!(f in blueStart.data)) failures.push(`bot-blue match:start missing field "${f}"`);
    if (
      f in redStart.data &&
      f in blueStart.data &&
      JSON.stringify(redStart.data[f]) !== JSON.stringify(blueStart.data[f])
    ) {
      failures.push(
        `match:start field "${f}" differs across bots — red=${JSON.stringify(redStart.data[f])}, blue=${JSON.stringify(blueStart.data[f])}`,
      );
    }
  }
  if (failures.length === 0 || !failures.some((f) => f.startsWith('match:start'))) {
    console.log(
      `✓ both saw match:start — steps_taken=${redStart.data.steps_taken}, steps_per_sec=${redStart.data.steps_per_sec}, playback_start_time_ms=${redStart.data.playback_start_time_ms}`,
    );
  }
}

function eventIndex(events, name) {
  return events.findIndex((e) => e.event === name);
}

for (const [label, events] of [
  ['bot-red', redResult.events],
  ['bot-blue', blueResult.events],
]) {
  const startIdx = eventIndex(events, 'match:start');
  const resultIdx = eventIndex(events, 'match:result');
  if (startIdx >= 0 && resultIdx >= 0 && startIdx >= resultIdx) {
    failures.push(`${label} received match:result before match:start (idx ${resultIdx} vs ${startIdx})`);
  }
}

const redOutcome = findEvent(redResult.events, 'match:result');
const blueOutcome = findEvent(blueResult.events, 'match:result');

if (!redOutcome) failures.push('bot-red never received match:result');
if (!blueOutcome) failures.push('bot-blue never received match:result');

if (redOutcome && blueOutcome) {
  if (redOutcome.data.match_id !== blueOutcome.data.match_id) {
    failures.push(
      `match:result mismatch — red=${redOutcome.data.match_id}, blue=${blueOutcome.data.match_id}`,
    );
  } else if (redOutcome.data.result !== blueOutcome.data.result) {
    failures.push(
      `match result outcome mismatch — red saw "${redOutcome.data.result}", blue saw "${blueOutcome.data.result}"`,
    );
  } else {
    console.log(
      `✓ both saw match:result — outcome="${redOutcome.data.result}", steps=${redOutcome.data.steps_taken}`,
    );
  }
}

if (failures.length) {
  console.log('\n=== FAILED ===');
  for (const f of failures) console.log(`  - ${f}`);
  process.exit(1);
}

console.log('\n=== PASS ===');
process.exit(0);
