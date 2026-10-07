import { warriorText } from '../../core/warriorColors';

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
