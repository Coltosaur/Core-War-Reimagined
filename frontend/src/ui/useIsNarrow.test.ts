import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { NARROW_QUERY, useIsNarrow } from './useIsNarrow';

type Listener = () => void;

function installMatchMedia(initial: boolean) {
  const listeners = new Set<Listener>();
  const mql = {
    matches: initial,
    media: NARROW_QUERY,
    addEventListener: (_: string, l: Listener) => listeners.add(l),
    removeEventListener: (_: string, l: Listener) => listeners.delete(l),
  };
  vi.stubGlobal(
    'matchMedia',
    vi.fn(() => mql),
  );
  return {
    set(next: boolean) {
      mql.matches = next;
      listeners.forEach((l) => l());
    },
    listeners,
  };
}

afterEach(() => {
  vi.unstubAllGlobals();
});

describe('useIsNarrow', () => {
  it('reflects the media query and updates on change', () => {
    const mm = installMatchMedia(false);
    const { result, unmount } = renderHook(() => useIsNarrow());
    expect(result.current).toBe(false);

    act(() => mm.set(true));
    expect(result.current).toBe(true);

    unmount();
    expect(mm.listeners.size).toBe(0);
  });

  it('is false when matchMedia is unavailable', () => {
    vi.stubGlobal('matchMedia', undefined);
    const { result } = renderHook(() => useIsNarrow());
    expect(result.current).toBe(false);
  });
});
