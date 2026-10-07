import { colors } from '../styles/tokens';

/**
 * Per-warrior colors, indexed by warrior id (0 = first loaded warrior).
 * Two warriors is all the engine's callers load today; the third entry is
 * there so a third warrior degrades to a distinct color rather than a
 * crash. Real 3+ warrior support needs more than color
 * (patterns, a focus mode), which waits until the engine callers do.
 *
 * Grid-cell fills and text use different shades. Fills are darker because
 * they cover large blocks of the core, where full-strength red and lime
 * glare: --blood-bright and --necrotic-dim. Text needs the lighter shades
 * to stay readable at small sizes: --blood-light and --necrotic. The two
 * fills stay far apart in lightness, so the warriors remain
 * distinguishable without relying on hue.
 */
export const WARRIOR_FILL: readonly string[] = [
  colors.bloodBright,
  colors.necroticDim,
  colors.violet,
];

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
