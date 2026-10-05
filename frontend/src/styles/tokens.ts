/**
 * Typed mirror of the color tokens in tokens.css, for consumers that
 * can't read CSS custom properties: the PixiJS canvas and the Monaco
 * theme. Keys are the camelCased custom-property names (`--blood-bright`
 * → `bloodBright`). tokens.test.ts fails if a value here drifts from
 * tokens.css, so edit both together.
 */
export const colors = {
  bg: '#07070a',
  surface: '#141418',
  surfaceRaised: '#1e1e24',
  surfaceInset: '#0d0d11',
  border: '#2a2a32',
  borderStrong: '#3a3a44',
  text: '#f2f2f0',
  textMuted: '#9a9aa2',
  textDim: '#6b6b74',
  blood: '#8b0a10',
  bloodBright: '#c8102e',
  bloodLight: '#f25467',
  necrotic: '#b8ff5a',
  necroticDim: '#7fbf3f',
  violet: '#b8acf6',
} as const;

/**
 * Mirror of `--font-mono`. Monaco measures glyph widths from the literal
 * family string it's given, so it needs the stack itself, not `var()`.
 */
export const fontMono = "'JetBrains Mono', ui-monospace, 'Fira Code', monospace";

export type ColorToken = keyof typeof colors;

/** `'#rrggbb'` → `0xrrggbb`, the numeric form PixiJS expects. */
export function hexToNumber(hex: string): number {
  if (!/^#[0-9a-f]{6}$/i.test(hex)) throw new Error(`expected #rrggbb, got ${hex}`);
  return parseInt(hex.slice(1), 16);
}
