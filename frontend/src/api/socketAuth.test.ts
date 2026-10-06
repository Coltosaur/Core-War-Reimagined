import { afterEach, beforeEach, describe, expect, it, vi, type Mock } from 'vitest';
import type { Socket } from 'socket.io-client';
import { installSocketAuthRecovery, type PendingSocketAction } from './socketAuth';
import { __resetSessionForTests, registerSessionHandlers } from './session';

// Minimal EventEmitter-shaped stand-in for a socket.io-client Socket. We
// don't need the transport — just the on/off/once/emit surface the
// installer relies on, plus disconnect/connect. `emit` is spied so the test
// can assert what was sent, and manual `_fire()` drives incoming events.
// `connect()` fires 'connect' on a microtask, like a reconnect that succeeds;
// set `_connectFails` to fire 'connect_error' instead.
type FakeSocket = Socket & {
  _fire: (event: string, ...args: unknown[]) => void;
  _emitSpy: ReturnType<typeof vi.fn>;
  _connectSpy: ReturnType<typeof vi.fn>;
  _disconnectSpy: ReturnType<typeof vi.fn>;
  _connectFails: boolean;
};

function makeFakeSocket(): FakeSocket {
  const listeners = new Map<string, Set<(...args: unknown[]) => void>>();
  const onceListeners = new Map<string, Set<(...args: unknown[]) => void>>();
  const emitSpy = vi.fn();
  const disconnectSpy = vi.fn();
  const connectSpy = vi.fn(() => {
    queueMicrotask(() => socket._fire(socket._connectFails ? 'connect_error' : 'connect'));
  });

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
    connected: false,
    on,
    off,
    once,
    emit: emitSpy,
    connect: connectSpy,
    disconnect: disconnectSpy,
    _emitSpy: emitSpy,
    _connectSpy: connectSpy,
    _disconnectSpy: disconnectSpy,
    _connectFails: false,
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

function mockFetchResponse(status: number): Mock<typeof fetch> {
  return vi.fn<typeof fetch>().mockResolvedValue({
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

  const joinW1: PendingSocketAction = { event: 'queue:join', payload: { warrior_id: 'w1' } };

  /** A socket that has completed its first connect, with recovery installed. */
  function setup(options: Partial<Parameters<typeof installSocketAuthRecovery>[1]> = {}) {
    const socket = makeFakeSocket();
    const onSessionExpired = vi.fn();
    const onRecovered = vi.fn();
    const forceLogoutSpy = vi.fn();
    registerSessionHandlers({ onForceLogout: forceLogoutSpy });
    const teardown = installSocketAuthRecovery(socket, {
      getPendingAction: () => joinW1,
      onRecovered,
      onSessionExpired,
      ...options,
    });
    socket._fire('connect');
    return { socket, onSessionExpired, onRecovered, forceLogoutSpy, teardown };
  }

  const calls = (socket: FakeSocket, event: string) =>
    socket._emitSpy.mock.calls.filter((c) => c[0] === event);

  it('does not replay on the first connect (the page emits its own action)', () => {
    const { socket } = setup();
    expect(socket._emitSpy).not.toHaveBeenCalled();
  });

  it('on auth_error → refresh → reconnects with the fresh cookie and replays the pending action', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const { socket, onRecovered, onSessionExpired } = setup();

    socket._fire('auth_error', { error: 'Authentication required' });
    await flushMicrotasks(20);

    // A new handshake is the only way the server sees the refreshed
    // cookie; asking it to re-check the old connection can't work (#127).
    expect(socket._disconnectSpy).toHaveBeenCalledTimes(1);
    expect(socket._connectSpy).toHaveBeenCalledTimes(1);
    expect(calls(socket, 'reauthenticate')).toHaveLength(0);
    expect(socket._emitSpy).toHaveBeenCalledWith('queue:join', { warrior_id: 'w1' });
    expect(onRecovered).toHaveBeenCalledTimes(1);
    expect(onSessionExpired).not.toHaveBeenCalled();
  });

  it('on auth_expired → refresh → also reconnects and replays', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const { socket, onSessionExpired } = setup();

    socket._fire('auth_expired', { message: 'expired' });
    await flushMicrotasks(20);

    expect(socket._connectSpy).toHaveBeenCalledTimes(1);
    expect(socket._emitSpy).toHaveBeenCalledWith('queue:join', { warrior_id: 'w1' });
    expect(onSessionExpired).not.toHaveBeenCalled();
  });

  it('replays the pending action after an automatic reconnect', () => {
    // A network blip or server restart: socket.io reconnects on its own and
    // the server has forgotten the old connection's queue entry.
    const { socket } = setup();
    socket._fire('connect');
    expect(socket._emitSpy).toHaveBeenCalledWith('queue:join', { warrior_id: 'w1' });
  });

  it('emits without payload when the pending action has none', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const { socket } = setup({ getPendingAction: () => ({ event: 'ping' }) });

    socket._fire('auth_error', {});
    await flushMicrotasks(20);

    const pingCall = socket._emitSpy.mock.calls.find((c) => c[0] === 'ping');
    expect(pingCall).toEqual(['ping']);
  });

  it('reconnects but emits nothing when there is no pending action', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const { socket, onRecovered } = setup({ getPendingAction: () => null });

    socket._fire('auth_error', {});
    await flushMicrotasks(20);

    expect(socket._connectSpy).toHaveBeenCalledTimes(1);
    expect(socket._emitSpy).not.toHaveBeenCalled();
    expect(onRecovered).toHaveBeenCalledTimes(1);
  });

  it('on refresh failure → forceLogout + onSessionExpired, no reconnect', async () => {
    globalThis.fetch = mockFetchResponse(401);
    const { socket, onSessionExpired, forceLogoutSpy } = setup();

    socket._fire('auth_error', {});
    await flushMicrotasks(20);

    expect(socket._connectSpy).not.toHaveBeenCalled();
    expect(socket._emitSpy).not.toHaveBeenCalled();
    expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
    expect(onSessionExpired).toHaveBeenCalledTimes(1);
  });

  it('when the reconnect fails → forceLogout + onSessionExpired', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const { socket, onSessionExpired, onRecovered, forceLogoutSpy } = setup();
    socket._connectFails = true;

    socket._fire('auth_error', {});
    await flushMicrotasks(20);

    expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
    expect(onSessionExpired).toHaveBeenCalledTimes(1);
    expect(onRecovered).not.toHaveBeenCalled();
  });

  it('times out a reconnect that never completes', async () => {
    vi.useFakeTimers();
    try {
      globalThis.fetch = mockFetchResponse(200);
      const { socket, onSessionExpired } = setup({ reauthTimeoutMs: 100 });
      socket._connectSpy.mockImplementation(() => {});

      socket._fire('auth_error', {});
      await vi.advanceTimersByTimeAsync(150);

      expect(onSessionExpired).toHaveBeenCalledTimes(1);
    } finally {
      vi.useRealTimers();
    }
  });

  it('gives up if the server rejects the replayed action on the fresh connection', async () => {
    // Refreshing again can't help: the connection already carries the
    // newest cookie the server will issue.
    globalThis.fetch = mockFetchResponse(200);
    const { socket, onSessionExpired, forceLogoutSpy } = setup();

    socket._fire('auth_error', {});
    await flushMicrotasks(20);
    socket._fire('auth_error', { error: 'Authentication required' });
    await flushMicrotasks(20);

    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
    expect(socket._connectSpy).toHaveBeenCalledTimes(1);
    expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
    expect(onSessionExpired).toHaveBeenCalledTimes(1);
  });

  it('recovers again from an auth error long after the last recovery', async () => {
    vi.useFakeTimers();
    try {
      globalThis.fetch = mockFetchResponse(200);
      const { socket, onSessionExpired } = setup({ reauthTimeoutMs: 100 });

      socket._fire('auth_error', {});
      await vi.advanceTimersByTimeAsync(0);
      // Fifteen minutes later the new access token expires too.
      await vi.advanceTimersByTimeAsync(15 * 60 * 1000);
      socket._fire('auth_error', {});
      await vi.advanceTimersByTimeAsync(0);

      expect(socket._connectSpy).toHaveBeenCalledTimes(2);
      expect(onSessionExpired).not.toHaveBeenCalled();
    } finally {
      vi.useRealTimers();
    }
  });

  it('guards against re-entry — a burst of auth_error events does not stack recoveries', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const { socket, onSessionExpired } = setup();

    socket._fire('auth_error', {});
    socket._fire('auth_error', {});
    socket._fire('auth_error', {});
    await flushMicrotasks(20);

    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
    expect(socket._connectSpy).toHaveBeenCalledTimes(1);
    expect(calls(socket, 'queue:join')).toHaveLength(1);
    expect(onSessionExpired).not.toHaveBeenCalled();
  });

  it('teardown removes the connect, auth_error and auth_expired handlers', async () => {
    globalThis.fetch = mockFetchResponse(200);
    const { socket, onSessionExpired, teardown } = setup();

    teardown();
    socket._fire('connect');
    socket._fire('auth_error', {});
    socket._fire('auth_expired', {});
    await flushMicrotasks(20);

    expect(globalThis.fetch).not.toHaveBeenCalled();
    expect(socket._emitSpy).not.toHaveBeenCalled();
    expect(onSessionExpired).not.toHaveBeenCalled();
  });
});
