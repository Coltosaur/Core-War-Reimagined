import { colors } from '../styles/tokens';

/**
 * Per-warrior colors, indexed by warrior id (0 = first loaded warrior).
 * Two warriors is all the engine's callers load today; the third entry is
 * there so a third warrior degrades to a distinct color rather than a
 * crash. Real 3+ warrior support needs more than color
 * (patterns, a focus mode), which waits until the engine callers do.
 *
 * Fills (grid cells, borders) and text use different reds: --blood-bright
 * is too dark for small text on our surfaces, so text gets --blood-light.
 */
export const WARRIOR_FILL: readonly string[] = [colors.bloodBright, colors.necrotic, colors.violet];

export const WARRIOR_TEXT: readonly string[] = [
  'var(--blood-light)',
  'var(--necrotic)',
  'var(--violet)',
];

/** Fallback for an out-of-range id: neutral rather than misattributed. */
export const NEUTRAL_TEXT = 'var(--text-muted)';

export function warriorText(id: number): string {
  return WARRIOR_TEXT[id] ?? NEUTRAL_TEXT;
}
