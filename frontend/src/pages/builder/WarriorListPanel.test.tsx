import { afterEach, describe, expect, it, vi } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import WarriorListPanel from './WarriorListPanel';
import type { Warrior } from '../../warriors/library';

afterEach(cleanup);

const imp: Warrior = { id: 'preset:imp', label: 'Imp', source: '', isPreset: true };
const mine: Warrior = { id: 'u1', label: 'My Bomber', source: '', isPreset: false };

function renderPanel(userWarriors: Warrior[] = [mine], selectedId = 'preset:imp') {
  const onSelect = vi.fn();
  const onNew = vi.fn();
  render(
    <WarriorListPanel
      presets={[imp]}
      userWarriors={userWarriors}
      selectedId={selectedId}
      onSelect={onSelect}
      onNew={onNew}
    />,
  );
  return { onSelect, onNew };
}

describe('WarriorListPanel', () => {
  it('selects a warrior from the keyboard', async () => {
    const user = userEvent.setup();
    const { onSelect } = renderPanel();
    // Expand the phone-layout panel so the list is reachable.
    await user.click(screen.getByRole('button', { name: /Warriors/ }));
    screen.getByRole('button', { name: /My Bomber/ }).focus();
    await user.keyboard('{Enter}');
    expect(onSelect).toHaveBeenCalledWith('u1');
  });

  it('marks only the selected warrior as current', async () => {
    const user = userEvent.setup();
    renderPanel([mine], 'u1');
    await user.click(screen.getByRole('button', { name: /Warriors/ }));
    expect(screen.getByRole('button', { name: /My Bomber/ })).toHaveAttribute(
      'aria-current',
      'true',
    );
    expect(screen.getByRole('button', { name: /Imp/ })).not.toHaveAttribute('aria-current');
  });

  it('shows the empty state when the user has no warriors', () => {
    renderPanel([]);
    expect(screen.getByText(/None yet/)).toBeInTheDocument();
  });

  it('creates a new warrior', async () => {
    const user = userEvent.setup();
    const { onNew } = renderPanel();
    await user.click(screen.getByRole('button', { name: /Warriors/ }));
    await user.click(screen.getByRole('button', { name: /New Warrior/ }));
    expect(onNew).toHaveBeenCalledOnce();
  });
});
