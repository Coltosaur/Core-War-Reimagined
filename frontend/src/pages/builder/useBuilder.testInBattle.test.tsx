import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ServerWarrior } from '../../api/warriors';

// "Test in Battle" on a saved (server:*) warrior with unsaved edits must run
// the edited code. Before this fix, handleTestInBattle routed every non-preset
// warrior through updateUserWarrior, which after #58 only touches local
// drafts — so the battle silently ran the old source and, because the Builder
// unmounts on navigate, the edits were lost.

vi.mock('../../api/useAuth', async () => {
  const { authState } = await import('../../test/helpers/mockAuth');
  return { useAuth: () => authState({ user: { user_id: 'u1', username: 'tester' } }) };
});

vi.mock('../../api/warriors', () => ({
  listWarriors: vi.fn(),
  createWarrior: vi.fn(),
  updateWarrior: vi.fn(),
  deleteWarrior: vi.fn(),
}));

const navigate = vi.fn();
vi.mock('react-router-dom', async (orig) => ({
  ...(await orig<typeof import('react-router-dom')>()),
  useNavigate: () => navigate,
}));

import * as api from '../../api/warriors';
import { createServerWarrior, getWarrior, setServerOwner } from '../../warriors/library';
import { useBuilder } from './useBuilder';

function sw(id: string, name: string, source: string): ServerWarrior {
  return {
    id,
    user_id: 'u1',
    name,
    source,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  };
}

function wrapper({ children }: { children: ReactNode }): ReactNode {
  return <MemoryRouter>{children}</MemoryRouter>;
}

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
});

describe('handleTestInBattle with a saved server warrior', () => {
  it('saves unsaved edits to the server before navigating, and battles the new source', async () => {
    setServerOwner('u1');
    vi.mocked(api.createWarrior).mockResolvedValue(sw('s1', 'Mine', 'MOV.I $0, $1'));
    await createServerWarrior('Mine', 'MOV.I $0, $1');

    const { result } = renderHook(() => useBuilder(), { wrapper });
    act(() => result.current.handleSelect('server:s1'));
    await waitFor(() => expect(result.current.source).toBe('MOV.I $0, $1'));

    act(() => result.current.handleSourceChange('DAT #0, #0'));
    expect(result.current.dirty).toBe(true);

    vi.mocked(api.updateWarrior).mockResolvedValue(sw('s1', 'Mine', 'DAT #0, #0'));
    await act(async () => {
      await result.current.handleTestInBattle();
    });

    expect(api.updateWarrior).toHaveBeenCalledWith('s1', { name: 'Mine', source: 'DAT #0, #0' });
    expect(getWarrior('server:s1')?.source).toBe('DAT #0, #0');
    expect(navigate).toHaveBeenCalledTimes(1);
    expect(navigate.mock.calls[0][0]).toMatch(/^\/battle\?red=server%3As1&/);
    // The save must land before the battle page reads the library.
    expect(vi.mocked(api.updateWarrior).mock.invocationCallOrder[0]).toBeLessThan(
      navigate.mock.invocationCallOrder[0],
    );
  });

  it('stays on the Builder (edits intact) if the save fails', async () => {
    setServerOwner('u1');
    vi.mocked(api.createWarrior).mockResolvedValue(sw('s2', 'Other', 'MOV.I $0, $1'));
    await createServerWarrior('Other', 'MOV.I $0, $1');

    const { result } = renderHook(() => useBuilder(), { wrapper });
    act(() => result.current.handleSelect('server:s2'));
    await waitFor(() => expect(result.current.source).toBe('MOV.I $0, $1'));
    act(() => result.current.handleSourceChange('DAT #1, #1'));

    vi.mocked(api.updateWarrior).mockRejectedValue(new Error('network down'));
    await act(async () => {
      await result.current.handleTestInBattle().catch(() => {});
    });

    expect(navigate).not.toHaveBeenCalled();
    expect(result.current.source).toBe('DAT #1, #1');
    expect(result.current.dirty).toBe(true);
  });
});
