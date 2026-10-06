import { warriorText } from '../../core/warriorColors';

export type Outcome = { text: string; color: string };

/**
 * The headline for a finished match. The text is from the viewer's point of
 * view (Victory / Defeat, or "<name> wins" for a spectator); the color is
 * always the winning seat's, matching the Battlefield's result banner.
 */
export function matchOutcome(
  result: string,
  red: string,
  blue: string,
  me: string | undefined,
): Outcome {
  if (result !== 'red_win' && result !== 'blue_win') {
    return { text: 'Draw', color: 'var(--violet)' };
  }
  const winnerSeat = result === 'red_win' ? 0 : 1;
  const winner = winnerSeat === 0 ? red : blue;
  const text = !me ? `${winner} wins` : winner === me ? 'Victory!' : 'Defeat';
  return { text, color: warriorText(winnerSeat) };
}
