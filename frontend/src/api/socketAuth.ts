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
//
// The server authenticates a connection once, from the cookies sent when it
// was opened, and never sees cookies again for that connection. So the only
// way to authenticate with a refreshed cookie is a new connection (#127).
// An earlier version asked the server to `reauthenticate` the existing
// connection instead; the server could only re-read the original cookies,
// so recovery always failed and logged the user out.
//
// The recovery loop is:
//   1. On auth_error / auth_expired, call attemptRefresh() over REST to
//      mint a fresh access-token cookie.
//   2. If refresh succeeded, reconnect the socket so the new handshake
//      carries the fresh cookie.
//   3. Every reconnect — this one, or an automatic one after a network blip
//      or server restart — replays the pending action, so the user does not
//      have to click again and a queued player stays queued.
//   4. If refresh fails, the reconnect fails, or the server still rejects
//      the replayed action, call the registered forceLogout() to drop UI
//      auth state and invoke the caller-supplied `onSessionExpired` so the
//      page can reset local state and show a friendly prompt.

import type { Socket } from 'socket.io-client';
import { attemptRefresh, forceLogout } from './session';

export type PendingSocketAction = {
  event: string;
  payload?: unknown;
};

type Options = {
  /**
   * Returns the auth-required action that should be in effect on the
   * server — e.g. `queue:join` from the click until the player leaves the
   * queue or is matched. Replayed on every reconnect, because the server
   * forgets a connection's state when it drops. Return `null` when there is
   * nothing to restore. Called at reconnect time — evaluate live state, do
   * not close over a stale snapshot.
   */
  getPendingAction?: () => PendingSocketAction | null;

  /**
   * Called after a successful refresh + reconnect. Useful for clearing UI
   * error messages left over from a prior failed attempt.
   */
  onRecovered?: () => void;

  /**
   * Called when recovery has definitively failed and the user must log in
   * again. `forceLogout()` will already have been invoked by this point;
   * the page should reset any transport-state it owned (e.g. lobby phase
   * back to idle) and surface a session-expired message.
   */
  onSessionExpired: () => void;

  /**
   * How long to wait for the reconnect after a refresh. Also the window in
   * which a second auth failure counts as definitive: the server rejecting
   * the replayed action on a connection opened with a fresh cookie means
   * refreshing again won't help. Default 5s.
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
  let lastRecoveryAt = -Infinity;
  // The first connect needs no replay: the page emits its own action, which
  // socket.io buffers until the connection is up.
  let hasConnected = socket.connected;

  function expire(): void {
    forceLogout();
    onSessionExpired();
  }

  function replayPending(): void {
    const pending = getPendingAction?.();
    if (!pending) return;
    if (pending.payload !== undefined) {
      socket.emit(pending.event, pending.payload);
    } else {
      socket.emit(pending.event);
    }
  }

  const onConnect = (): void => {
    if (hasConnected) replayPending();
    hasConnected = true;
  };

  // Resolves true once the socket reconnects, false on a connection error
  // or timeout. Listeners are always removed, so a late event after we've
  // given up doesn't fire into a dead flow.
  function reconnect(): Promise<boolean> {
    return new Promise<boolean>((resolve) => {
      const cleanup = (): void => {
        clearTimeout(timer);
        socket.off('connect', onReconnected);
        socket.off('connect_error', onFailed);
      };
      const onReconnected = (): void => {
        cleanup();
        resolve(true);
      };
      const onFailed = (): void => {
        cleanup();
        resolve(false);
      };
      const timer = setTimeout(onFailed, reauthTimeoutMs);
      socket.once('connect', onReconnected);
      socket.once('connect_error', onFailed);
      socket.disconnect();
      socket.connect();
    });
  }

  async function attemptRecovery(): Promise<void> {
    if (recovering) return;
    // We already reconnected with a fresh cookie and the server still says
    // no. Another refresh can't change that answer.
    if (Date.now() - lastRecoveryAt < reauthTimeoutMs) {
      expire();
      return;
    }
    recovering = true;
    try {
      if (!(await attemptRefresh())) {
        expire();
        return;
      }
      // Set before reconnecting: the reconnect replays the pending action,
      // and a rejection of that replay must count as the second failure.
      lastRecoveryAt = Date.now();
      if (!(await reconnect())) {
        expire();
        return;
      }
      onRecovered?.();
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

  socket.on('connect', onConnect);
  socket.on('auth_error', onAuthEvent);
  socket.on('auth_expired', onAuthEvent);

  return () => {
    socket.off('connect', onConnect);
    socket.off('auth_error', onAuthEvent);
    socket.off('auth_expired', onAuthEvent);
  };
}
