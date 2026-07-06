import { act, renderHook, waitFor } from '@testing-library/react';
import type { ReactNode } from 'react';
import { MemoryRouter } from 'react-router-dom';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { parseWarrior } from 'core-war-engine';
import { useBuilder } from './useBuilder';

// useBuilder pulls in useAuth, which requires an AuthProvider. Stub it out —
// the fresh-mount bug reproduces for both anonymous and authenticated users,
// and the auth path is not the code under test.
vi.mock('../../api/AuthContext', () => ({
  useAuth: () => ({
    user: null,
    loading: false,
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
  }),
}));

function wrapper({ children }: { children: ReactNode }): ReactNode {
  return <MemoryRouter>{children}</MemoryRouter>;
}

// The shared parseWarrior mock in src/test/__mocks__/core-war-engine.ts
// always succeeds, which is exactly what let issue #85 slip through — the
// real engine rejects empty input with EmptyWarrior, but the mock did not.
// For these tests we substitute a behavior-faithful implementation.
const EMPTY_MESSAGE = 'warrior source has no instructions';

type ParsedWarriorLike = ReturnType<typeof parseWarrior>;

function fakeParsed(name: string): ParsedWarriorLike {
  return {
    name: () => name,
    author: () => null,
    instructionCount: () => 1,
    startOffset: () => 0,
  } as unknown as ParsedWarriorLike;
}

describe('useBuilder', () => {
  beforeEach(() => {
    localStorage.clear();
    vi.mocked(parseWarrior).mockReset();
    vi.mocked(parseWarrior).mockImplementation((source: string) => {
      if (!source.trim()) throw new Error(EMPTY_MESSAGE);
      return fakeParsed('Fake Warrior');
    });
  });

  it('lazy-initializes source and label from the first library warrior', () => {
    const { result } = renderHook(() => useBuilder(), { wrapper });

    // The first library entry is always a preset (see warriors/library.ts —
    // computeSnapshot returns [...PRESETS, ...userWarriors]).
    expect(result.current.selectedId).toMatch(/^preset:/);
    expect(result.current.source).not.toBe('');
    expect(result.current.source).toBe(result.current.presets[0].source);
    expect(result.current.label).toBe(result.current.presets[0].label);
  });

  it('reports a successful parse once wasm loads on a fresh mount (issue #85)', async () => {
    const { result } = renderHook(() => useBuilder(), { wrapper });

    // Before wasm loads, no parse has been attempted.
    expect(result.current.wasmReady).toBe(false);
    expect(result.current.parseStatus).toBeNull();

    // Wait for the async init() promise to resolve and the parse effect
    // to run against the (non-empty) initial source.
    await waitFor(() => {
      expect(result.current.wasmReady).toBe(true);
      expect(result.current.parseStatus).not.toBeNull();
    });

    // The core regression: on a fresh mount with a preset selected, the
    // status bar must report a successful parse — NOT EmptyWarrior.
    expect(result.current.parseStatus?.ok).toBe(true);

    // And parseWarrior was called with the preset body, not with ''.
    const calls = vi.mocked(parseWarrior).mock.calls;
    expect(calls.length).toBeGreaterThan(0);
    for (const [arg] of calls) {
      expect(arg.trim()).not.toBe('');
    }
  });

  it('re-syncs source and label when the selection changes while not dirty', async () => {
    const { result } = renderHook(() => useBuilder(), { wrapper });
    await waitFor(() => expect(result.current.wasmReady).toBe(true));

    const initialId = result.current.selectedId;
    const initialSource = result.current.source;
    const nextPreset = result.current.presets.find((p) => p.id !== initialId);
    expect(nextPreset).toBeDefined();

    act(() => {
      result.current.handleSelect(nextPreset!.id);
    });

    expect(result.current.selectedId).toBe(nextPreset!.id);
    expect(result.current.source).toBe(nextPreset!.source);
    expect(result.current.label).toBe(nextPreset!.label);
    expect(result.current.source).not.toBe(initialSource);
    expect(result.current.dirty).toBe(false);
  });

  it('handleSourceChange marks the buffer dirty and does not get reverted by the sync effect', async () => {
    const { result } = renderHook(() => useBuilder(), { wrapper });
    await waitFor(() => expect(result.current.wasmReady).toBe(true));

    const originalSource = result.current.source;
    const edited = originalSource + '\n; edited by test';

    act(() => {
      result.current.handleSourceChange(edited);
    });

    expect(result.current.source).toBe(edited);
    expect(result.current.dirty).toBe(true);

    // A no-op re-render must not clobber the user's edits back to the
    // library value — the sync effect is gated on `!dirty`.
    // We can nudge a re-render by calling handleLabelChange (also dirty=true).
    act(() => {
      result.current.handleLabelChange(result.current.label);
    });

    expect(result.current.source).toBe(edited);
  });
});
