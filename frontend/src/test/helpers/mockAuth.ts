import { vi } from 'vitest';
// Type-only import: this module must not load useAuth at runtime, so a
// `vi.mock('.../api/useAuth', async () => ...)` factory can import it.
import type { useAuth } from '../../api/useAuth';

export type UseAuthReturn = ReturnType<typeof useAuth>;

/** A `useAuth()` result: anonymous, not loading, fresh spies unless overridden. */
export function authState(overrides: Partial<UseAuthReturn> = {}): UseAuthReturn {
  return {
    user: null,
    loading: false,
    login: vi.fn(),
    register: vi.fn(),
    logout: vi.fn(),
    ...overrides,
  };
}
