// Central session-recovery primitive shared by the REST client (client.ts)
// and any Socket.IO consumer (socketAuth.ts). Answers a single question —
// "can I get a fresh access-token cookie right now?" — and coordinates a
// forced-logout hook when the answer is definitively no.
//
// Both transports need this because the backend's access-token cookie is a
// short-lived JWT (15 min) with a much longer refresh-token cookie (7 days)
// gated behind POST /api/auth/refresh. Neither the fetch wrapper nor the
// Socket.IO client used to know how to trade the second for the first —
// this module is the single place that does.

export { SessionExpiredError } from './errors';

const API_BASE = import.meta.env.VITE_API_URL ?? 'http://localhost:3001';
const REFRESH_PATH = '/api/auth/refresh';

type SessionHandlers = {
  /**
   * Called after refresh has definitively failed. The caller (AuthContext)
   * wires this to `setUser(null)` so the UI drops back to the logged-out
   * state without waiting for a component re-render.
   */
  onForceLogout: () => void;
};

// Nothing is registered before AuthProvider mounts. The default is a no-op
// so `forceLogout()` from an interceptor firing during that narrow window
// doesn't blow up — the initial getMe() 401 already puts the user in the
// logged-out state anyway.
let handlers: SessionHandlers = {
  onForceLogout: () => {},
};

/**
 * Register session handlers (called once by AuthProvider on mount). Returns
 * an unregister function that restores the no-op defaults; call from the
 * effect cleanup so a hot-reloaded AuthProvider doesn't leave a stale
 * closure over an unmounted setState.
 */
export function registerSessionHandlers(next: SessionHandlers): () => void {
  handlers = next;
  return () => {
    if (handlers === next) {
      handlers = { onForceLogout: () => {} };
    }
  };
}

/**
 * Trigger the registered forced-logout handler. Idempotent — extra calls
 * during a burst-of-401s recovery are harmless because setUser(null) is
 * itself idempotent.
 */
export function forceLogout(): void {
  handlers.onForceLogout();
}

// Single-flight guard. A burst of parallel 401s (typical on tab-focus with
// several sync calls in flight) must trigger exactly one POST /api/auth/refresh,
// with every caller awaiting the same promise. Cleared *after* resolution so
// a follow-up call gets a fresh attempt rather than the cached result.
let inflight: Promise<boolean> | null = null;

/**
 * Attempt to mint a fresh access-token cookie. Returns true on success,
 * false on any failure (network, non-2xx, missing refresh cookie). Never
 * throws — callers use the boolean to branch (retry vs give up).
 *
 * Uses raw fetch, not the `api` client, so refresh failures don't recurse
 * into the interceptor.
 */
export function attemptRefresh(): Promise<boolean> {
  if (inflight) return inflight;

  inflight = (async () => {
    try {
      const res = await fetch(`${API_BASE}${REFRESH_PATH}`, {
        method: 'POST',
        credentials: 'include',
        headers: { 'Content-Type': 'application/json' },
      });
      return res.ok;
    } catch {
      // Network error — treat as refresh failure. Caller will surface a
      // session-expired error to the user.
      return false;
    } finally {
      inflight = null;
    }
  })();

  return inflight;
}

/**
 * URL-path check used by the REST interceptor to bypass refresh for the
 * auth endpoints themselves (otherwise a 401 on /login would loop into
 * /refresh → 401 → /login). Exported for testing.
 */
export function isAuthEndpoint(path: string): boolean {
  // Match ignores query string; trailing slash is tolerated for symmetry
  // with backend routes that may or may not carry one.
  const bare = path.split('?')[0].replace(/\/+$/, '');
  return (
    bare === '/api/auth/refresh' ||
    bare === '/api/auth/login' ||
    bare === '/api/auth/register' ||
    bare === '/api/auth/logout'
  );
}

// Test-only reset hook. Kept out of the public surface — callers should
// never touch it in production.
export function __resetSessionForTests(): void {
  inflight = null;
  handlers = { onForceLogout: () => {} };
}
