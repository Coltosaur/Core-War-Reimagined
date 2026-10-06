import { vi, type Mock } from 'vitest';
import type { Socket } from 'socket.io-client';

// Minimal EventEmitter-shaped stand-in for a socket.io-client Socket. We
// don't need the transport — just the on/off/once/emit surface the
// installer relies on, plus disconnect/connect. `emit` is spied so the test
// can assert what was sent, and manual `_fire()` drives incoming events.
// `connect()` fires 'connect' on a microtask, like a reconnect that succeeds;
// set `_connectFails` to fire 'connect_error' instead.
// `emit`/`connect`/`disconnect` are the spies themselves; the `_*Spy` fields
// alias them.
export type FakeSocket = Socket & {
  emit: Mock;
  connect: Mock;
  disconnect: Mock;
  _fire: (event: string, ...args: unknown[]) => void;
  _emitSpy: Mock;
  _connectSpy: Mock;
  _disconnectSpy: Mock;
  _connectFails: boolean;
};

export function makeFakeSocket(): FakeSocket {
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
