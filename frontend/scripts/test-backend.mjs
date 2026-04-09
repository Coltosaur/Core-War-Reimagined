// End-to-end smoke test for the Rust backend.
// Hits the /health endpoint, then opens a Socket.IO connection,
// waits for connect, then disconnects cleanly. Exits non-zero on any failure.
//
// Usage (with backend running on port 3001):
//   node frontend/scripts/test-backend.mjs

import { io } from 'socket.io-client';

const BACKEND_URL = process.env.BACKEND_URL ?? 'http://localhost:3001';

console.log(`testing backend at ${BACKEND_URL}`);

// 1. HTTP health check
try {
  const res = await fetch(`${BACKEND_URL}/health`);
  const body = await res.json();
  console.log(`  GET /health → ${res.status} ${JSON.stringify(body)}`);
  if (res.status !== 200 || body.status !== 'ok') {
    throw new Error(`unexpected health response`);
  }
} catch (err) {
  console.error(`FAIL: health check failed: ${err.message}`);
  process.exit(1);
}

// 2. Socket.IO connect
const socket = io(BACKEND_URL, {
  transports: ['websocket'],
  reconnection: false,
  timeout: 4000,
});

try {
  await new Promise((resolve, reject) => {
    const t = setTimeout(() => reject(new Error('connect timeout')), 5000);
    socket.on('connect', () => {
      clearTimeout(t);
      console.log(`  socket.io connected, sid = ${socket.id}`);
      resolve();
    });
    socket.on('connect_error', (err) => {
      clearTimeout(t);
      reject(err);
    });
  });

  socket.disconnect();
  console.log(`  socket.io disconnected`);
} catch (err) {
  console.error(`FAIL: socket.io: ${err.message}`);
  process.exit(1);
}

console.log('OK — backend reachable end-to-end');
process.exit(0);
