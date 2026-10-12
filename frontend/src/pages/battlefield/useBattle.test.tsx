import { act, render, waitFor } from '@testing-library/react';
import { MemoryRouter } from 'react-router-dom';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { MatchState } from 'core-war-engine';
import { useBattle } from './useBattle';

// Issue #149: the RAF loop must be cancelled explicitly on unmount, and the
// inspector refresh must not run a side effect inside a state updater.

// The real renderer is PixiJS on a canvas, which jsdom can't run.
vi.mock('../../core/coreRenderer', () => ({
  createCoreRenderer: () => ({ update: vi.fn(), destroy: vi.fn() }),
}));

type Battle = ReturnType<typeof useBattle>;

// Renders the hook with its grid ref attached to a real element, so the
// renderer effect runs the way it does on the page.
function renderBattle() {
  const current: { battle: Battle | null } = { battle: null };
  function Harness() {
    const battle = useBattle();
    current.battle = battle;
    return <div ref={battle.gridRef} />;
  }
  const view = render(
    <MemoryRouter>
      <Harness />
    </MemoryRouter>,
  );
  const battle = () => {
    if (!current.battle) throw new Error('hook not rendered');
    return current.battle;
  };
  return { ...view, battle };
}

beforeEach(() => {
  localStorage.clear();
});

afterEach(() => {
  vi.restoreAllMocks();
});

describe('useBattle', () => {
  it('cancels the pending animation frame when unmounted mid-battle', async () => {
    const raf = vi.spyOn(window, 'requestAnimationFrame').mockReturnValue(42);
    const cancel = vi.spyOn(window, 'cancelAnimationFrame');
    const { battle, unmount } = renderBattle();
    await waitFor(() => expect(battle().ready).toBe(true));

    act(() => battle().play());
    expect(raf).toHaveBeenCalled();
    expect(cancel).not.toHaveBeenCalledWith(42);

    unmount();
    expect(cancel).toHaveBeenCalledWith(42);
  });

  it('refreshes the selected cell as the battle steps', async () => {
    // Make a cell's contents depend on how far the battle has run.
    vi.spyOn(MatchState.prototype, 'cellAValue').mockImplementation(function (this: MatchState) {
      return this.steps();
    });
    const { battle } = renderBattle();
    await waitFor(() => expect(battle().ready).toBe(true));

    act(() => battle().selectCell(7));
    expect(battle().cellInfo).toMatchObject({ addr: 7, aValue: 0 });

    act(() => battle().stepOnce());
    expect(battle().cellInfo).toMatchObject({ addr: 7, aValue: 1 });
    act(() => battle().stepMany());
    expect(battle().cellInfo).toMatchObject({ addr: 7, aValue: 101 });
  });

  it('keeps a cleared selection cleared on later steps', async () => {
    const { battle } = renderBattle();
    await waitFor(() => expect(battle().ready).toBe(true));

    act(() => battle().selectCell(3));
    act(() => battle().clearCell());
    act(() => battle().stepOnce());
    expect(battle().selectedCell).toBeNull();
    expect(battle().cellInfo).toBeNull();
  });

  it('reset clears the selection, and it stays cleared on later steps', async () => {
    const { battle } = renderBattle();
    await waitFor(() => expect(battle().ready).toBe(true));

    act(() => battle().selectCell(3));
    act(() => battle().reset());
    expect(battle().selectedCell).toBeNull();
    act(() => battle().stepOnce());
    expect(battle().cellInfo).toBeNull();
  });
});
