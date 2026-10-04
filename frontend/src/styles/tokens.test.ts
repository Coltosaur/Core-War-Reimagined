// @vitest-environment node
import { readFileSync } from 'node:fs';
import { colors, fontMono, hexToNumber } from './tokens';

// Read from disk: Vitest stubs out CSS imports, `?raw` included.
const tokensCss = readFileSync(new URL('./tokens.css', import.meta.url), 'utf8');

/** Every `--name: #hex;` declaration in tokens.css, keyed by name. */
function cssHexTokens(css: string): Map<string, string> {
  const out = new Map<string, string>();
  for (const m of css.matchAll(/--([a-z0-9-]+):\s*(#[0-9a-f]{3,8})\s*;/gi)) {
    out.set(m[1], m[2].toLowerCase());
  }
  return out;
}

const camel = (name: string) => name.replace(/-([a-z0-9])/g, (_, c: string) => c.toUpperCase());

describe('tokens.ts mirror', () => {
  const css = cssHexTokens(tokensCss);

  it('finds the hex tokens in tokens.css', () => {
    expect(css.size).toBeGreaterThan(10);
  });

  it('mirrors every hex token in tokens.css exactly', () => {
    const fromCss = Object.fromEntries([...css].map(([name, hex]) => [camel(name), hex]));
    expect(colors).toEqual(fromCss);
  });

  it('mirrors --font-mono exactly', () => {
    expect(tokensCss).toMatch(/--font-mono:\s*([^;]+);/);
    expect(tokensCss.match(/--font-mono:\s*([^;]+);/)![1]).toBe(fontMono);
  });
});

describe('hexToNumber', () => {
  it('converts #rrggbb to a number', () => {
    expect(hexToNumber('#c8102e')).toBe(0xc8102e);
  });

  it('rejects anything else', () => {
    expect(() => hexToNumber('#fff')).toThrow();
    expect(() => hexToNumber('var(--bg)')).toThrow();
  });
});
