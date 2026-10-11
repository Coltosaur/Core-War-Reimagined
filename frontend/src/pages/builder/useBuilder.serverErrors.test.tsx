import { act, render, renderHook, screen, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ServerWarrior } from '../../api/warriors';

// Issue #148: every warriors-API call in the Builder was `try/finally` with
// no catch, so a failed save/create/delete cleared the spinner and then said
// nothing — a failed save looked like it worked. These pin that each failure
// produces a status-bar message and leaves the Builder's state as it was.

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

import * as api from '../../api/warriors';
import { ApiError, SessionExpiredError } from '../../api/client';
import { createServerWarrior, getWarrior, setServerOwner } from '../../warriors/library';
import { useBuilder } from './useBuilder';
import EditorStatus from './EditorStatus';

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

// Seeds one saved server warrior and renders the Builder with it selected.
async function renderWithSavedWarrior(id: string) {
  setServerOwner('u1');
  vi.mocked(api.createWarrior).mockResolvedValueOnce(sw(id, 'Mine', 'MOV.I $0, $1'));
  await createServerWarrior('Mine', 'MOV.I $0, $1');
  const hook = renderHook(() => useBuilder(), { wrapper });
  act(() => hook.result.current.handleSelect(`server:${id}`));
  await waitFor(() => expect(hook.result.current.source).toBe('MOV.I $0, $1'));
  return hook;
}

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
  vi.spyOn(window, 'confirm').mockReturnValue(true);
});

describe('Builder server failures', () => {
  it('a failed save reports the server message and keeps the edits unsaved', async () => {
    const { result } = await renderWithSavedWarrior('s1');
    act(() => result.current.handleSourceChange('DAT #0, #0'));

    vi.mocked(api.updateWarrior).mockRejectedValue(new ApiError(500, 'Internal server error'));
    await act(() => result.current.handleSave());

    expect(result.current.actionError).toBe("Couldn't save warrior: Internal server error");
    expect(result.current.saving).toBe(false);
    expect(result.current.dirty).toBe(true);
    expect(result.current.source).toBe('DAT #0, #0');
    expect(getWarrior('server:s1')?.source).toBe('MOV.I $0, $1');
  });

  it('a network failure gets readable copy, not the raw fetch error', async () => {
    const { result } = await renderWithSavedWarrior('s2');
    act(() => result.current.handleSourceChange('DAT #0, #0'));

    vi.mocked(api.updateWarrior).mockRejectedValue(new TypeError('Failed to fetch'));
    await act(() => result.current.handleSave());

    expect(result.current.actionError).toBe(
      "Couldn't save warrior: the server couldn't be reached",
    );
  });

  it('an expired session surfaces the session-expired copy', async () => {
    const { result } = await renderWithSavedWarrior('s3');
    act(() => result.current.handleSourceChange('DAT #0, #0'));

    vi.mocked(api.updateWarrior).mockRejectedValue(new SessionExpiredError());
    await act(() => result.current.handleSave());

    expect(result.current.actionError).toBe(
      "Couldn't save warrior: Your session has expired. Please log in again.",
    );
  });

  it('the next successful action clears the message', async () => {
    const { result } = await renderWithSavedWarrior('s4');
    act(() => result.current.handleSourceChange('DAT #0, #0'));

    vi.mocked(api.updateWarrior).mockRejectedValueOnce(new ApiError(503, 'Unavailable'));
    await act(() => result.current.handleSave());
    expect(result.current.actionError).not.toBeNull();

    vi.mocked(api.updateWarrior).mockResolvedValueOnce(sw('s4', 'Mine', 'DAT #0, #0'));
    await act(() => result.current.handleSave());
    expect(result.current.actionError).toBeNull();
    expect(result.current.dirty).toBe(false);
  });

  it('a failed delete reports it and keeps the warrior selected and listed', async () => {
    const { result } = await renderWithSavedWarrior('s5');

    vi.mocked(api.deleteWarrior).mockRejectedValue(new ApiError(404, 'Warrior not found'));
    await act(() => result.current.handleDelete());

    expect(result.current.actionError).toBe("Couldn't delete warrior: Warrior not found");
    expect(result.current.selectedId).toBe('server:s5');
    expect(result.current.userWarriors.map((w) => w.id)).toContain('server:s5');
  });

  it.each([
    ['handleNew', "Couldn't create warrior: Too many warriors"],
    ['handleDuplicate', "Couldn't duplicate warrior: Too many warriors"],
    ['handleImport', "Couldn't import warrior: Too many warriors"],
  ] as const)('a failed %s reports it and keeps the selection', async (handler, message) => {
    const { result } = await renderWithSavedWarrior(`s-${handler}`);

    vi.mocked(api.createWarrior).mockRejectedValue(new ApiError(422, 'Too many warriors'));
    await act(async () => {
      if (handler === 'handleImport') await result.current.handleImport('Imp', 'MOV 0, 1');
      else await result.current[handler]();
    });

    expect(result.current.actionError).toBe(message);
    expect(result.current.saving).toBe(false);
    expect(result.current.selectedId).toBe(`server:s-${handler}`);
  });

  it('selecting another warrior dismisses the message', async () => {
    const { result } = await renderWithSavedWarrior('s6');

    vi.mocked(api.deleteWarrior).mockRejectedValue(new ApiError(500, 'boom'));
    await act(() => result.current.handleDelete());
    expect(result.current.actionError).not.toBeNull();

    act(() => result.current.handleSelect(result.current.presets[0].id));
    expect(result.current.actionError).toBeNull();
  });
});

describe('EditorStatus', () => {
  it('shows the action error in the status bar', () => {
    render(
      <EditorStatus
        wasmReady
        parseStatus={{ ok: true, name: null }}
        selected={undefined}
        dirty
        actionError="Couldn't save warrior: boom"
      />,
    );
    expect(screen.getByRole('status')).toHaveTextContent("✗ Couldn't save warrior: boom");
  });
});
