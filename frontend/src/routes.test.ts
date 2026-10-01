// @vitest-environment node
import { existsSync, readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';
import scannerPaths from './deploy/scannerPaths.json';

type CrawlerPath = keyof typeof scannerPaths.crawlerFiles;

// public/_redirects is an allowlist of SPA routes: anything it doesn't
// rewrite is served by Cloudflare Pages as a real 404. That
// means the route list lives in two places, main.tsx and _redirects. These
// tests keep them in lockstep so a new page can't silently 404 in
// production, and a removed page can't linger as a soft 404 (200 OK).

const mainSource = readFileSync(new URL('./main.tsx', import.meta.url), 'utf8');
const redirectsSource = readFileSync(new URL('../public/_redirects', import.meta.url), 'utf8');

// `/` is served by index.html directly and `*` is the in-app NotFoundPage;
// neither needs a rewrite.
const routePaths = [...mainSource.matchAll(/<Route\s[^>]*path="([^"]+)"/g)]
  .map((m) => m[1])
  .filter((p) => p !== '/' && p !== '*');

const rules = redirectsSource
  .split('\n')
  .map((line) => line.trim())
  .filter((line) => line && !line.startsWith('#'))
  .map((line) => {
    const [from, to, status] = line.split(/\s+/);
    return { from, to, status };
  });

describe('SPA route allowlist (public/_redirects)', () => {
  it('finds the routes in main.tsx', () => {
    // Guards the regex itself: if main.tsx is restructured so nothing
    // matches, every other assertion here would pass vacuously.
    expect(routePaths.length).toBeGreaterThan(0);
    expect(mainSource).toContain('path="*"');
  });

  it('has a rewrite for every route and no stale rewrites', () => {
    expect(rules.map((r) => r.from).sort()).toEqual([...routePaths].sort());
  });

  it('only rewrites to / with a 200', () => {
    // Not /index.html: Pages answers that with a 308 redirect to `/`.
    for (const rule of rules) {
      expect(rule, rule.from).toMatchObject({ to: '/', status: '200' });
    }
  });

  it('has no splat rule that would turn every 404 into a soft 404', () => {
    for (const rule of rules) {
      expect(rule.from, rule.from).not.toContain('*');
    }
  });
});

// Pages matching for the rule shapes we allow: literal segments plus
// `:placeholder` segments, which match exactly one non-empty segment. Splats
// are rejected above, so this doesn't need to handle them.
function ruleMatches(from: string, path: string): boolean {
  const ruleSegments = from.split('/');
  const pathSegments = path.split('/');
  return (
    ruleSegments.length === pathSegments.length &&
    ruleSegments.every((seg, i) =>
      seg.startsWith(':') ? pathSegments[i] !== '' : seg === pathSegments[i],
    )
  );
}

// Paths that vulnerability scanners and crawlers request (see
// src/deploy/scannerPaths.json, also used by scripts/check-scanner-paths.mjs
// against a live server). Each must be a real 404, not a soft-404 200.
describe('scanner paths', () => {
  const crawlerPaths = Object.keys(scannerPaths.crawlerFiles);
  const crawlerPathsExpecting404 = Object.entries(scannerPaths.crawlerFiles)
    .filter(([, status]) => status === 404)
    .map(([path]) => path);

  it('matches every rule against a real route, so the matcher is not vacuous', () => {
    for (const rule of rules) {
      expect(
        scannerPaths.realRoutes.some((p) => ruleMatches(rule.from, p)),
        `no realRoutes entry exercises ${rule.from}`,
      ).toBe(true);
    }
  });

  it('never rewrites a scanner path to the app', () => {
    for (const path of [...scannerPaths.probes, ...crawlerPathsExpecting404]) {
      const hit = rules.find((r) => ruleMatches(r.from, path));
      expect(hit?.from, `${path} would be served with 200`).toBeUndefined();
    }
  });

  // nonCanonicalRoutes are alternate spellings of real routes, e.g. a trailing
  // slash. Rules match exactly, so these 404 rather than serving the page
  // under a second URL. Deliberate: internal links never use them.
  it('does not serve real routes under non-canonical spellings', () => {
    for (const path of scannerPaths.nonCanonicalRoutes) {
      const hit = rules.find((r) => ruleMatches(r.from, path));
      expect(hit?.from, `${path} would be served with 200`).toBeUndefined();
    }
  });

  it('only lists non-canonical spellings of real routes', () => {
    // Guards against the list drifting into plain junk paths, which belong
    // in probes, or outliving the route it was paired with.
    for (const path of scannerPaths.nonCanonicalRoutes) {
      expect(
        rules.some((r) => ruleMatches(r.from, path.replace(/\/$/, ''))),
        path,
      ).toBe(true);
    }
  });

  it('ships a static file for exactly the crawler paths that expect 200', () => {
    for (const path of crawlerPaths) {
      const shipped = existsSync(new URL(`../public${path}`, import.meta.url));
      expect(shipped, path).toBe(scannerPaths.crawlerFiles[path as CrawlerPath] === 200);
    }
  });

  it('ships no static file at any probe path', () => {
    for (const path of scannerPaths.probes) {
      expect(existsSync(new URL(`../public${path}`, import.meta.url)), path).toBe(false);
    }
  });
});
