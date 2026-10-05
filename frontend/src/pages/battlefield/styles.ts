import { warriorText } from '../../core/warriorColors';

// Still used by the Match viewer, which hasn't moved to CSS Modules yet.
export const PARSE_ERROR_STYLE: React.CSSProperties = {
  padding: '0.5rem 1rem',
  color: 'var(--blood-light)',
  border: '1px solid var(--blood)',
  borderRadius: 'var(--radius-sm)',
  backgroundColor: 'color-mix(in srgb, var(--blood) 12%, transparent)',
  fontSize: 'var(--fs-200)',
};

export const ONGOING = 0;
export const VICTORY = 1;
export const TIE = 2;
export const ALL_DEAD = 3;

export function resultBanner(
  code: number,
  winnerId: number,
  names: string[],
): { text: string; color: string } | null {
  switch (code) {
    case VICTORY: {
      const name = names[winnerId] ?? `Warrior ${winnerId}`;
      return { text: `${name} wins!`, color: warriorText(winnerId) };
    }
    case TIE:
      return { text: 'Tie \u2014 step limit reached', color: 'var(--violet)' };
    case ALL_DEAD:
      return { text: 'All warriors eliminated \u2014 no winner', color: 'var(--text-muted)' };
    default:
      return null;
  }
}
