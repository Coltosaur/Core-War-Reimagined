import { act, renderHook, waitFor } from '@testing-library/react';
import { useEffect, type ReactNode } from 'react';
import { MemoryRouter, useLocation } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ServerWarrior } from '../../api/warriors';

// The Battlefield's "Edit" link opens /builder?warrior=<id>. These cover the
// Builder honoring it, including a saved warrior that only reaches the
// library after the server sync lands.

vi.mock('../../api/AuthContext', () => ({
  useAuth: () => ({
    user: { user_id: 'u1', username: 'tester' },
    loading: false,
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
  }),
}));

vi.mock('../../api/warriors', () => ({
  listWarriors: vi.fn(),
  createWarrior: vi.fn(),
  updateWarrior: vi.fn(),
  deleteWarrior: vi.fn(),
}));

import * as api from '../../api/warriors';
import { createServerWarrior, setServerOwner } from '../../warriors/library';
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

const onLocation = vi.fn<(search: string) => void>();
function LocationProbe() {
  const { search } = useLocation();
  useEffect(() => onLocation(search), [search]);
  return null;
}
const currentWarriorParam = () =>
  new URLSearchParams(onLocation.mock.lastCall?.[0] ?? '').get('warrior');

function renderAt(url: string) {
  const wrapper = ({ children }: { children: ReactNode }): ReactNode => (
    <MemoryRouter initialEntries={[url]}>
      {children}
      <LocationProbe />
    </MemoryRouter>
  );
  return renderHook(() => useBuilder(), { wrapper });
}

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
  setServerOwner('u1');
});

describe('useBuilder ?warrior= param', () => {
  it('opens the warrior named in the URL', async () => {
    const { result } = renderAt('/builder?warrior=preset%3Adwarf');
    expect(result.current.selectedId).toBe('preset:dwarf');
    await waitFor(() => expect(result.current.label).toBe('Dwarf'));
  });

  it('falls back to the first warrior for an unknown id', () => {
    const { result } = renderAt('/builder?warrior=nope');
    expect(result.current.selectedId).toBe('preset:imp');
  });

  it('opens a saved warrior once the server sync delivers it', async () => {
    const { result } = renderAt('/builder?warrior=server%3As9');
    expect(result.current.selectedId).toBe('preset:imp');

    vi.mocked(api.createWarrior).mockResolvedValue(sw('s9', 'Late', 'MOV.I $0, $1'));
    await act(async () => {
      await createServerWarrior('Late', 'MOV.I $0, $1');
    });

    await waitFor(() => expect(result.current.selectedId).toBe('server:s9'));
    await waitFor(() => expect(result.current.source).toBe('MOV.I $0, $1'));
  });

  it("doesn't override a warrior the user picked while waiting", async () => {
    const { result } = renderAt('/builder?warrior=server%3As8');
    act(() => result.current.handleSelect('preset:mice'));

    vi.mocked(api.createWarrior).mockResolvedValue(sw('s8', 'Late', 'MOV.I $0, $1'));
    await act(async () => {
      await createServerWarrior('Late', 'MOV.I $0, $1');
    });

    expect(result.current.selectedId).toBe('preset:mice');
  });

  it('keeps the URL in step with what the user selects', async () => {
    const { result } = renderAt('/builder?warrior=preset%3Adwarf');
    act(() => result.current.handleSelect('preset:scanner'));
    await waitFor(() => expect(currentWarriorParam()).toBe('preset:scanner'));
  });

  it('leaves a pending URL alone until the user picks something', () => {
    renderAt('/builder?warrior=server%3As7');
    expect(currentWarriorParam()).toBe('server:s7');
  });
});
