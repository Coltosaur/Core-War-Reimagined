import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { Socket } from 'socket.io-client';
import { installSocketAuthRecovery, type PendingSocketAction } from './socketAuth';
import { __resetSessionForTests, registerSessionHandlers } from './session';

// Minimal EventEmitter-shaped stand-in for a socket.io-client Socket. We
// don't need the transport — just the on/off/once/emit surface the
// installer relies on. `emit` is spied so the test can assert what was
// sent, and manual `_fire()` drives incoming events.
type FakeSocket = Socket & {
  _fire: (event: string, ...args: unknown[]) => void;
  _emitSpy: ReturnType<typeof vi.fn>;
};

function makeFakeSocket(): FakeSocket {
  const listeners = new Map<string, Set<(...args: unknown[]) => void>>();
  const onceListeners = new Map<string, Set<(...args: unknown[]) => void>>();
  const emitSpy = vi.fn();

  const on = (event: string, fn: (...args: unknown[]) => void): FakeSocket => {
    if (!listeners.has(event)) listeners.set(event, new Set());
    listeners.get(event)!.add(fn);
    return socket;
  };
  const off = (event: string, fn?: (...args: unknown[]) => void): FakeSocket => {
    if (!fn) {
      listeners.delete(event);
      onceListeners.delete(event);
      return socket;
    }
    listeners.get(event)?.delete(fn);
    onceListeners.get(event)?.delete(fn);
    return socket;
  };
  const once = (event: string, fn: (...args: unknown[]) => void): FakeSocket => {
    if (!onceListeners.has(event)) onceListeners.set(event, new Set());
    onceListeners.get(event)!.add(fn);
    return socket;
  };

  const socket = {
    on,
    off,
    once,
    emit: emitSpy,
    _emitSpy: emitSpy,
    _fire: (event: string, ...args: unknown[]) => {
      listeners.get(event)?.forEach((fn) => fn(...args));
      const oneShots = onceListeners.get(event);
      if (oneShots) {
        onceListeners.delete(event);
        oneShots.forEach((fn) => fn(...args));
      }
    },
  } as unknown as FakeSocket;

  return socket;
}

function mockFetchResponse(status: number): ReturnType<typeof vi.fn> {
  return vi.fn().mockResolvedValue({
    ok: status >= 200 && status < 300,
    status,
    statusText: 'test',
    json: async () => ({}),
  } as Response);
}

// Small helper — flush the current microtask queue N times so awaited
// promise chains inside the recovery flow can settle before we assert.
async function flushMicrotasks(n = 10): Promise<void> {
  for (let i = 0; i < n; i += 1) {
    await Promise.resolve();
  }
}

describe('installSocketAuthRecovery', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    __resetSessionForTests();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    __resetSessionForTests();
  });

  it('registers auth_error and auth_expired handlers and returns a teardown', () => {
    const socket = makeFakeSocket();
    const teardown = installSocketAuthRecovery(socket, {
      onSessionExpired: () => {},
    });
    expect(typeof teardown).toBe('function');
  });

  it('on auth_error → refresh success → emits reauthenticate → replays pending action', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const socket = makeFakeSocket();
    const pending: PendingSocketAction = {
      event: 'queue:join',
      payload: { warrior_id: 'w1' },
    };
    const onRecovered = vi.fn();
    const onSessionExpired = vi.fn();

    installSocketAuthRecovery(socket, {
      getPendingAction: () => pending,
      onRecovered,
      onSessionExpired,
    });

    socket._fire('auth_error', { error: 'Authentication required' });
    await flushMicrotasks(20);

    // Refresh emit sent
    expect(socket._emitSpy).toHaveBeenCalledWith('reauthenticate');
    // Simulate backend acknowledging the reauth
    socket._fire('auth_refreshed', { user_id: 'u1', username: 'alice' });
    await flushMicrotasks();

    // Pending action was replayed
    expect(socket._emitSpy).toHaveBeenCalledWith('queue:join', pending.payload);
    expect(onRecovered).toHaveBeenCalledTimes(1);
    expect(onSessionExpired).not.toHaveBeenCalled();
  });

  it('on auth_expired → refresh success → still replays pending action', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const socket = makeFakeSocket();
    const onSessionExpired = vi.fn();

    installSocketAuthRecovery(socket, {
      getPendingAction: () => ({ event: 'queue:join', payload: { warrior_id: 'w1' } }),
      onSessionExpired,
    });

    socket._fire('auth_expired', { message: 'expired' });
    await flushMicrotasks(20);
    socket._fire('auth_refreshed', { user_id: 'u1', username: 'alice' });
    await flushMicrotasks();

    expect(socket._emitSpy).toHaveBeenCalledWith('reauthenticate');
    expect(socket._emitSpy).toHaveBeenCalledWith('queue:join', { warrior_id: 'w1' });
    expect(onSessionExpired).not.toHaveBeenCalled();
  });

  it('emits without payload when the pending action has none', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const socket = makeFakeSocket();
    installSocketAuthRecovery(socket, {
      getPendingAction: () => ({ event: 'ping' }),
      onSessionExpired: vi.fn(),
    });

    socket._fire('auth_error', {});
    await flushMicrotasks(20);
    socket._fire('auth_refreshed', {});
    await flushMicrotasks();

    // The pending emit should be called with the single event arg only.
    // First emit is the reauthenticate; second is the pending 'ping'.
    const pingCall = socket._emitSpy.mock.calls.find((c) => c[0] === 'ping');
    expect(pingCall).toBeDefined();
    expect(pingCall).toHaveLength(1);
  });

  it('no-ops the replay when there is no pending action', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const socket = makeFakeSocket();
    const onRecovered = vi.fn();

    installSocketAuthRecovery(socket, {
      getPendingAction: () => null,
      onRecovered,
      onSessionExpired: vi.fn(),
    });

    socket._fire('auth_error', {});
    await flushMicrotasks(20);
    socket._fire('auth_refreshed', {});
    await flushMicrotasks();

    // Only the reauthenticate emit should have gone out; nothing else.
    expect(socket._emitSpy).toHaveBeenCalledTimes(1);
    expect(socket._emitSpy).toHaveBeenCalledWith('reauthenticate');
    expect(onRecovered).toHaveBeenCalledTimes(1);
  });

  it('on refresh failure → forceLogout + onSessionExpired, no reauthenticate emitted', async () => {
    globalThis.fetch = mockFetchResponse(401);
    const forceLogoutSpy = vi.fn();
    registerSessionHandlers({ onForceLogout: forceLogoutSpy });

    const socket = makeFakeSocket();
    const onSessionExpired = vi.fn();
    installSocketAuthRecovery(socket, {
      getPendingAction: () => ({ event: 'queue:join', payload: { warrior_id: 'w1' } }),
      onSessionExpired,
    });

    socket._fire('auth_error', {});
    await flushMicrotasks(20);

    expect(socket._emitSpy).not.toHaveBeenCalled();
    expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
    expect(onSessionExpired).toHaveBeenCalledTimes(1);
  });

  it('on refresh success but second-round auth_error from backend → forceLogout + onSessionExpired', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const forceLogoutSpy = vi.fn();
    registerSessionHandlers({ onForceLogout: forceLogoutSpy });

    const socket = makeFakeSocket();
    const onSessionExpired = vi.fn();
    installSocketAuthRecovery(socket, {
      getPendingAction: () => ({ event: 'queue:join', payload: { warrior_id: 'w1' } }),
      onSessionExpired,
    });

    socket._fire('auth_error', {});
    await flushMicrotasks(20);

    // Server rejects reauth (e.g. race — refresh cookie invalidated between
    // the REST call and the socket emit).
    socket._fire('auth_error', { error: 'Reauthentication failed' });
    await flushMicrotasks();

    expect(socket._emitSpy).toHaveBeenCalledWith('reauthenticate');
    expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
    expect(onSessionExpired).toHaveBeenCalledTimes(1);
  });

  it('guards against re-entry — a burst of auth_error events does not stack recoveries', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const socket = makeFakeSocket();
    installSocketAuthRecovery(socket, {
      getPendingAction: () => ({ event: 'queue:join', payload: { warrior_id: 'w1' } }),
      onSessionExpired: vi.fn(),
    });

    socket._fire('auth_error', {});
    socket._fire('auth_error', {});
    socket._fire('auth_error', {});
    await flushMicrotasks(20);
    socket._fire('auth_refreshed', {});
    await flushMicrotasks();

    // Only one reauthenticate emit despite three triggers.
    const reauthCalls = socket._emitSpy.mock.calls.filter((c) => c[0] === 'reauthenticate');
    expect(reauthCalls).toHaveLength(1);
    // And only one pending-action replay.
    const queueCalls = socket._emitSpy.mock.calls.filter((c) => c[0] === 'queue:join');
    expect(queueCalls).toHaveLength(1);
  });

  it('teardown removes both auth_error and auth_expired handlers', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const socket = makeFakeSocket();
    const onSessionExpired = vi.fn();
    const teardown = installSocketAuthRecovery(socket, {
      onSessionExpired,
    });

    teardown();
    socket._fire('auth_error', {});
    socket._fire('auth_expired', {});
    await flushMicrotasks(20);

    expect(globalThis.fetch).not.toHaveBeenCalled();
    expect(onSessionExpired).not.toHaveBeenCalled();
  });

  it('times out and treats no auth_refreshed response as a session failure', async () => {
    vi.useFakeTimers();
    globalThis.fetch = mockFetchResponse(200);
    const forceLogoutSpy = vi.fn();
    registerSessionHandlers({ onForceLogout: forceLogoutSpy });

    const socket = makeFakeSocket();
    const onSessionExpired = vi.fn();
    installSocketAuthRecovery(socket, {
      getPendingAction: () => ({ event: 'queue:join', payload: {} }),
      onSessionExpired,
      reauthTimeoutMs: 100,
    });

    socket._fire('auth_error', {});
    // Let the refresh fetch resolve.
    await vi.advanceTimersByTimeAsync(0);
    // Advance past the reauth timeout without firing auth_refreshed.
    await vi.advanceTimersByTimeAsync(150);

    expect(socket._emitSpy).toHaveBeenCalledWith('reauthenticate');
    expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
    expect(onSessionExpired).toHaveBeenCalledTimes(1);
    vi.useRealTimers();
  });
});
