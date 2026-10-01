// Checks how a deployed frontend answers the paths vulnerability scanners
// and crawlers request (src/deploy/scannerPaths.json): every probe must be a
// real 404, non-canonical spellings of real routes (e.g. a trailing slash)
// must 404 too, crawler files must match their expected status, and real routes
// must still be 200. Redirects count as failures, since a 308 to `/` is how
// a soft 404 sneaks back in. Every response must also carry the security
// headers from public/_headers.
//
// Usage:
//   node frontend/scripts/check-scanner-paths.mjs [baseUrl]
//
// baseUrl defaults to SITE_URL, then http://localhost:8788 (the port used by
// `npx wrangler pages dev dist` after `npm run build`). After a deploy, run it
// against production:
//   node frontend/scripts/check-scanner-paths.mjs https://corewar.coltcampbell.dev
//
// Branch preview deployments (*.pages.dev) sit behind Cloudflare Access, so
// every path there answers 302 to a login page; this can't check previews.
//
// Requests are sequential to stay polite to the target. Exits 0 on success,
// 1 on any failure.

import scannerPaths from '../src/deploy/scannerPaths.json' with { type: 'json' };

const BASE_URL = (process.argv[2] ?? process.env.SITE_URL ?? 'http://localhost:8788').replace(
  /\/$/,
  '',
);

const expectations = [
  ...scannerPaths.probes.map((path) => [path, 404]),
  ...scannerPaths.nonCanonicalRoutes.map((path) => [path, 404]),
  ...Object.entries(scannerPaths.crawlerFiles),
  ...scannerPaths.realRoutes.map((path) => [path, 200]),
];

console.log(`checking ${expectations.length} paths against ${BASE_URL}`);

const failures = [];
for (const [path, expected] of expectations) {
  let res;
  try {
    res = await fetch(`${BASE_URL}${path}`, { redirect: 'manual' });
    await res.arrayBuffer();
  } catch (err) {
    failures.push(`${path}: request failed: ${err.message}`);
    continue;
  }

  const problems = [];
  if (res.status !== expected) {
    // Drop the query string: redirect targets such as a Cloudflare Access
    // login carry long signed tokens that bury the useful part.
    const rawLocation = res.headers.get('location');
    const location = rawLocation && new URL(rawLocation, BASE_URL).href.split('?')[0];
    problems.push(
      `status ${res.status}, expected ${expected}${location ? ` (→ ${location})` : ''}`,
    );
  }
  if (res.headers.get('x-content-type-options') !== 'nosniff') {
    problems.push('missing security headers');
  }
  if (path === '/robots.txt' && !res.headers.get('content-type')?.startsWith('text/plain')) {
    problems.push(`content-type ${res.headers.get('content-type')}, expected text/plain`);
  }

  if (problems.length > 0) {
    failures.push(`${path}: ${problems.join('; ')}`);
  }
}

if (failures.length > 0) {
  console.error(`FAIL: ${failures.length} of ${expectations.length} paths`);
  for (const failure of failures) console.error(`  ${failure}`);
  process.exit(1);
}

console.log(`OK: all ${expectations.length} paths answered as expected`);
