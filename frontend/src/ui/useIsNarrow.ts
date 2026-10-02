import { useSyncExternalStore } from 'react';

/**
 * Below this width the app switches to its small-screen layout: bottom nav
 * bar, stacked battle screen, and a desktop-only notice in the Builder.
 * Matches the 768px breakpoint called out in #81.
 */
export const NARROW_QUERY = '(max-width: 767px)';

function getMql(): MediaQueryList | null {
  return typeof window !== 'undefined' && typeof window.matchMedia === 'function'
    ? window.matchMedia(NARROW_QUERY)
    : null;
}

function subscribe(onChange: () => void): () => void {
  const mql = getMql();
  if (!mql) return () => {};
  mql.addEventListener('change', onChange);
  return () => mql.removeEventListener('change', onChange);
}

function getSnapshot(): boolean {
  return getMql()?.matches ?? false;
}

/** True when the viewport is narrower than 768px (phones, small tablets). */
export function useIsNarrow(): boolean {
  return useSyncExternalStore(subscribe, getSnapshot, () => false);
}
