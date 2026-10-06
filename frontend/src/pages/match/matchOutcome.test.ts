import { describe, expect, it } from 'vitest';
import { matchOutcome } from './matchOutcome';
import { warriorText } from '../../core/warriorColors';

describe('matchOutcome', () => {
  it('tells the winner Victory! in their seat color', () => {
    expect(matchOutcome('blue_win', 'alice', 'bob', 'bob')).toEqual({
      text: 'Victory!',
      color: warriorText(1),
    });
  });

  it('tells the loser Defeat, still in the winning seat color', () => {
    expect(matchOutcome('red_win', 'alice', 'bob', 'bob')).toEqual({
      text: 'Defeat',
      color: warriorText(0),
    });
  });

  it('names the winner for a signed-out viewer', () => {
    expect(matchOutcome('red_win', 'alice', 'bob', undefined).text).toBe('alice wins');
  });

  it.each(['tie', 'all_dead'])('reports %s as a Draw', (result) => {
    expect(matchOutcome(result, 'alice', 'bob', 'alice')).toEqual({
      text: 'Draw',
      color: 'var(--violet)',
    });
  });
});
