import { describe, expect, it, vi } from 'vitest';
import { render, screen } from '@testing-library/react';
import BattleStatus from './BattleStatus';
import { ALL_DEAD, ONGOING, TIE, VICTORY } from './battleResult';

vi.mock('core-war-engine', () => ({ engineVersion: () => '0.0.0-test' }));

const warriors = [
  { name: 'Imp', alive: false, procs: 0 },
  { name: 'Dwarf', alive: true, procs: 1 },
];

function renderStatus(resultCode: number, resultWinner = -1) {
  render(
    <BattleStatus
      ready
      stepCount={1234}
      warriors={warriors}
      resultCode={resultCode}
      resultWinner={resultWinner}
    />,
  );
}

describe('BattleStatus result banner', () => {
  it('is absent while the battle is ongoing', () => {
    renderStatus(ONGOING);
    expect(screen.queryByRole('status')).not.toBeInTheDocument();
  });

  it('names the winner and tints the banner with the winning seat color', () => {
    renderStatus(VICTORY, 1);
    const banner = screen.getByRole('status');
    expect(banner).toHaveTextContent('Dwarf wins!');
    expect(banner.style.getPropertyValue('--tint')).toBe('var(--necrotic)');
  });

  it('tints a tie violet', () => {
    renderStatus(TIE);
    expect(screen.getByRole('status').style.getPropertyValue('--tint')).toBe('var(--violet)');
  });

  it('tints a mutual wipe-out muted', () => {
    renderStatus(ALL_DEAD);
    expect(screen.getByRole('status').style.getPropertyValue('--tint')).toBe('var(--text-muted)');
  });
});
