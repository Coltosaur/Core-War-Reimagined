// @vitest-environment node
import { readFileSync } from 'node:fs';

// Read from disk: Vitest stubs out CSS imports, `?raw` included. jsdom has
// no layout, so this pins the rule; the 375x740 check is in #175.
const css = readFileSync(new URL('./EditorStatus.module.css', import.meta.url), 'utf8');

describe('EditorStatus on phones (#175)', () => {
  it('sticks the status bar to the bottom below the 48rem breakpoint', () => {
    const phone = css.match(/@media \(max-width: 47\.99rem\)\s*\{([\s\S]*?)\n\}/);
    expect(phone).not.toBeNull();
    expect(phone![1]).toMatch(/\.status\s*\{[^}]*position:\s*sticky;[^}]*bottom:\s*0;/);
  });
});
