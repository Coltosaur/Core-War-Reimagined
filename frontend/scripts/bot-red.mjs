// Matchmaking bot — plays as the seeded `_test_red` user with the Imp warrior.
// Run as a separate Node process. See bot-common.mjs for the shared flow.

import { runBot } from './bot-common.mjs';

const password = process.env.TEST_USER_PASSWORD;
if (!password) {
  console.error(
    JSON.stringify({
      event: 'config:missing',
      data: { error: 'TEST_USER_PASSWORD env var required (load from backend/.env)' },
    }),
  );
  process.exit(1);
}

await runBot({
  role: 'red',
  username: '_test_red',
  password,
  expectedWarriorName: 'Imp',
});
