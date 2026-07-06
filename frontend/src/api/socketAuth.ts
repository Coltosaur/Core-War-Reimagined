// Socket.IO auth-recovery primitive. Any page that opens a Socket.IO
// connection to the backend should call `installSocketAuthRecovery` on it
// so that a mid-session access-token expiry is transparently recovered
// (or, if refresh definitively fails, cleanly surfaced as a session-expired
// state to the user).
//
// The backend contract (see backend/src/auth/socket.rs):
//   - `auth_error`   — emitted by require_auth() when the connection is
//                       anonymous but authentication is required.
//   - `auth_expired` — emitted when a valid-looking cookie is present but
//                       has expired.
//   - `reauthenticate` — client-emitted event that re-reads cookies. The
//                        backend responds with either `auth_refreshed`
//                        (success) or `auth_error` (still failing).
//
// The recovery loop is:
//   1. On auth_error / auth_expired, call attemptRefresh() over REST to
//      mint a fresh access-token cookie.
//   2. If refresh succeeded, emit `reauthenticate` and wait for the server's
//      `auth_refreshed` (success) or a second `auth_error` (fail).
//   3. On success, replay the last-emitted auth-required action so the user
//      does not have to click the button a second time.
//   4. On failure, call the registered forceLogout() to drop UI auth state
//      and invoke the caller-supplied `onSessionExpired` so the page can
//      reset local state and show a friendly prompt.

import type { Socket } from 'socket.io-client';
import { attemptRefresh, forceLogout } from './session';

export type PendingSocketAction = {
  event: string;
  payload?: unknown;
};

type Options = {
  /**
   * Returns the last auth-required action so recovery can replay it. Return
   * `null` if nothing is queued (e.g. the user is just idling in the lobby
   * with no click pending). Called at recovery time — evaluate live state,
   * do not close over a stale snapshot.
   */
  getPendingAction?: () => PendingSocketAction | null;

  /**
   * Called after a successful reauthenticate + pending-action replay.
   * Useful for clearing UI error messages left over from a prior failed
   * attempt.
   */
  onRecovered?: () => void;

  /**
   * Called when refresh has definitively failed and the user must log in
   * again. `forceLogout()` will already have been invoked by this point;
   * the page should reset any transport-state it owned (e.g. lobby phase
   * back to idle) and surface a session-expired message.
   */
  onSessionExpired: () => void;

  /**
   * Overrideable timeout for the reauthenticate round-trip. If the server
   * fails to respond within this window we treat it as a session failure
   * rather than hanging forever. Default 5s.
   */
  reauthTimeoutMs?: number;
};

const DEFAULT_REAUTH_TIMEOUT_MS = 5000;

/**
 * Attach auth-recovery handlers to a socket. Returns a teardown function
 * that removes them — call it on component unmount to avoid leaks when
 * the socket outlives the component (or, more commonly, when React
 * StrictMode double-mounts).
 */
export function installSocketAuthRecovery(socket: Socket, options: Options): () => void {
  const {
    getPendingAction,
    onRecovered,
    onSessionExpired,
    reauthTimeoutMs = DEFAULT_REAUTH_TIMEOUT_MS,
  } = options;

  // Guard against re-entry — a burst of auth_error / auth_expired (e.g. two
  // require_auth checks firing back-to-back) must not start two overlapping
  // recovery flows.
  let recovering = false;

  async function attemptRecovery(): Promise<void> {
    if (recovering) return;
    recovering = true;
    try {
      const refreshed = await attemptRefresh();
      if (!refreshed) {
        forceLogout();
        onSessionExpired();
        return;
      }

      // Race the server's response — `auth_refreshed` (success) or
      // `auth_error` (definitive failure) — against a timeout so the UI
      // never hangs waiting on a dropped socket. Whichever path resolves
      // first, we always tear down both once-listeners so a late-arriving
      // event after we've given up doesn't fire into a dead flow.
      const outcome = await new Promise<'refreshed' | 'failed'>((resolve) => {
        const cleanup = (): void => {
          clearTimeout(timer);
          socket.off('auth_refreshed', onRefreshed);
          socket.off('auth_error', onError);
        };
        const onRefreshed = (): void => {
          cleanup();
          resolve('refreshed');
        };
        const onError = (): void => {
          cleanup();
          resolve('failed');
        };
        const timer = setTimeout(() => {
          cleanup();
          resolve('failed');
        }, reauthTimeoutMs);
        socket.once('auth_refreshed', onRefreshed);
        socket.once('auth_error', onError);
        socket.emit('reauthenticate');
      });

      if (outcome === 'refreshed') {
        const pending = getPendingAction?.();
        if (pending) {
          if (pending.payload !== undefined) {
            socket.emit(pending.event, pending.payload);
          } else {
            socket.emit(pending.event);
          }
        }
        onRecovered?.();
      } else {
        forceLogout();
        onSessionExpired();
      }
    } finally {
      recovering = false;
    }
  }

  // Both auth_error and auth_expired route through the same recovery path.
  // `attemptRecovery` is a plain function returning a promise; socket.io's
  // event listeners don't await the return so we spawn-and-forget here
  // and the re-entry guard above prevents overlap.
  const onAuthEvent = (): void => {
    void attemptRecovery();
  };

  socket.on('auth_error', onAuthEvent);
  socket.on('auth_expired', onAuthEvent);

  return () => {
    socket.off('auth_error', onAuthEvent);
    socket.off('auth_expired', onAuthEvent);
  };
}
