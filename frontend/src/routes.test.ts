// @vitest-environment node
import { readFileSync } from 'node:fs';
import { describe, expect, it } from 'vitest';

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
