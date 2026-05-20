// Shared helpers for the matchmaking bot scripts (bot-red.mjs, bot-blue.mjs).
//
// Each bot logs in as a seeded test user (see backend `seed-test-users` binary),
// fetches its seeded warrior, joins the matchmaking queue over Socket.IO, and
// emits a JSON-lines event log on stdout for the orchestrator to consume.
//
// The bots are intended to run as independent Node processes — the orchestrator
// in matchmaking-e2e.mjs spawns each via `node bot-{role}.mjs`. Running them as
// separate processes sidesteps any same-process socket.io-client Manager
// sharing concerns (the multiplex behavior). `forceNew: true` + websocket-only
// transports are kept as defense-in-depth.

import { io } from 'socket.io-client';

const BACKEND_URL = process.env.BACKEND_URL ?? 'http://localhost:3001';
const FRONTEND_URL = process.env.FRONTEND_URL ?? 'http://localhost:5173';

export function logEvent(event, data = {}, extra = {}) {
  const entry = {
    timestamp: new Date().toISOString(),
    event,
    data,
    ...extra,
  };
  console.log(JSON.stringify(entry));
}

export async function login(username, password) {
  const res = await fetch(`${BACKEND_URL}/api/auth/login`, {
    method: 'POST',
    headers: {
      'Content-Type': 'application/json',
      // CSRF middleware checks Origin against FRONTEND_URL for non-GET requests.
      Origin: FRONTEND_URL,
    },
    body: JSON.stringify({ username_or_email: username, password }),
  });

  if (!res.ok) {
    const body = await res.text();
    throw new Error(`login failed: HTTP ${res.status} ${body}`);
  }

  // Set-Cookie may arrive as multiple headers; reassemble a Cookie header for replay.
  const setCookies = res.headers.getSetCookie?.() ?? [];
  if (setCookies.length === 0) {
    throw new Error('login: no Set-Cookie headers in response');
  }
  const cookieHeader = setCookies.map((c) => c.split(';', 1)[0]).join('; ');
  const body = await res.json();
  return { user: body, cookieHeader };
}

export async function listWarriors(cookieHeader) {
  const res = await fetch(`${BACKEND_URL}/api/warriors`, {
    method: 'GET',
    headers: { Cookie: cookieHeader },
  });
  if (!res.ok) {
    const body = await res.text();
    throw new Error(`list warriors failed: HTTP ${res.status} ${body}`);
  }
  return res.json();
}

export function connectSocket(cookieHeader) {
  return io(BACKEND_URL, {
    transports: ['websocket'],
    forceNew: true,
    extraHeaders: { Cookie: cookieHeader },
    timeout: 10_000,
    reconnection: false,
  });
}

export async function runBot({ role, username, password, expectedWarriorName, timeoutMs = 30_000 }) {
  logEvent('bot:start', { role, username });

  let session;
  try {
    session = await login(username, password);
    logEvent('login:ok', { username: session.user.username, user_id: session.user.user_id });
  } catch (err) {
    logEvent('login:fail', { error: err.message });
    process.exit(1);
  }

  let warrior;
  try {
    const { warriors } = await listWarriors(session.cookieHeader);
    warrior = warriors.find((w) => w.name === expectedWarriorName);
    if (!warrior) {
      logEvent('warrior:missing', {
        expected: expectedWarriorName,
        available: warriors.map((w) => w.name),
      });
      process.exit(1);
    }
    logEvent('warrior:found', { id: warrior.id, name: warrior.name });
  } catch (err) {
    logEvent('warriors:fail', { error: err.message });
    process.exit(1);
  }

  const socket = connectSocket(session.cookieHeader);

  let exitCode = 2;
  let timedOut = true;

  const timeout = setTimeout(() => {
    if (timedOut) {
      logEvent('timeout', { after_ms: timeoutMs });
      socket.disconnect();
      process.exit(2);
    }
  }, timeoutMs);

  function finish(code) {
    timedOut = false;
    exitCode = code;
    clearTimeout(timeout);
    logEvent('bot:done', { exitCode });
    socket.disconnect();
    // Give the disconnect packet a tick to flush before exiting.
    setImmediate(() => process.exit(exitCode));
  }

  socket.on('connect', () => {
    logEvent('socket:connect', { sid: socket.id });
    socket.emit('queue:join', { warrior_id: warrior.id });
  });

  socket.on('connect_error', (err) => {
    logEvent('socket:connect_error', { message: err.message ?? String(err) });
  });

  socket.on('disconnect', (reason) => {
    logEvent('socket:disconnect', { reason });
  });

  const passthrough = ['queue:joined', 'queue:left', 'queue:error', 'match:found', 'auth_error'];
  for (const ev of passthrough) {
    socket.on(ev, (data) => {
      logEvent(ev, data, { sid: socket.id });
      if (ev === 'queue:error' || ev === 'auth_error') {
        finish(1);
      }
    });
  }

  socket.on('match:result', (data) => {
    logEvent('match:result', data, { sid: socket.id });
    finish(0);
  });
}
