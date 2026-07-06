import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import type { MockedFunction } from 'vitest';
import { cleanup, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { MemoryRouter } from 'react-router-dom';
import LobbyPage from './LobbyPage';
import * as AuthContextModule from '../api/AuthContext';
import * as libraryModule from '../warriors/library';
import { __resetSessionForTests } from '../api/session';

// Integration coverage for the LobbyPage side of the socket auth-recovery
// flow (issue #60). The primitive `installSocketAuthRecovery` is heavily
// unit-tested in api/socketAuth.test.ts against a static `getPendingAction`
// callback — those tests would still pass even if LobbyPage forgot to
// stash `pendingActionRef.current` before emitting, or reordered the two
// steps such that the ref reflected only the PREVIOUS action at recovery
// time. This file wires the two together and asserts the *page* honors
// the contract.

// --- socket.io-client fake ------------------------------------------------

type Listener = (...args: unknown[]) => void;

type FakeSocket = {
  on: (event: string, fn: Listener) => FakeSocket;
  off: (event: string, fn?: Listener) => FakeSocket;
  once: (event: string, fn: Listener) => FakeSocket;
  emit: ReturnType<typeof vi.fn>;
  disconnect: ReturnType<typeof vi.fn>;
  _fire: (event: string, ...args: unknown[]) => void;
};

function makeFakeSocket(): FakeSocket {
  const listeners = new Map<string, Set<Listener>>();
  const onceListeners = new Map<string, Set<Listener>>();
  const emit = vi.fn();
  const disconnect = vi.fn();

  const socket: FakeSocket = {
    on(event, fn) {
      if (!listeners.has(event)) listeners.set(event, new Set());
      listeners.get(event)!.add(fn);
      return socket;
    },
    off(event, fn) {
      if (!fn) {
        listeners.delete(event);
        onceListeners.delete(event);
        return socket;
      }
      listeners.get(event)?.delete(fn);
      onceListeners.get(event)?.delete(fn);
      return socket;
    },
    once(event, fn) {
      if (!onceListeners.has(event)) onceListeners.set(event, new Set());
      onceListeners.get(event)!.add(fn);
      return socket;
    },
    emit,
    disconnect,
    _fire(event, ...args) {
      listeners.get(event)?.forEach((fn) => fn(...args));
      const oneShots = onceListeners.get(event);
      if (oneShots) {
        onceListeners.delete(event);
        oneShots.forEach((fn) => fn(...args));
      }
    },
  };
  return socket;
}

// Module-level socket so beforeEach can reset it and the mocked `io()` can
// return the same instance for every LobbyPage mount within a single test.
let currentSocket: FakeSocket;

vi.mock('socket.io-client', () => ({
  io: vi.fn(() => currentSocket as unknown),
}));

vi.mock('../api/AuthContext');
vi.mock('../warriors/library');

type UseAuthReturn = ReturnType<typeof AuthContextModule.useAuth>;

const mockUseAuth = AuthContextModule.useAuth as MockedFunction<() => UseAuthReturn>;
const mockUseWarriorLibrary = libraryModule.useWarriorLibrary as MockedFunction<
  typeof libraryModule.useWarriorLibrary
>;

async function flushMicrotasks(n = 20): Promise<void> {
  for (let i = 0; i < n; i += 1) {
    await Promise.resolve();
  }
}

function renderLobby() {
  return render(
    <MemoryRouter initialEntries={['/lobby']}>
      <LobbyPage />
    </MemoryRouter>,
  );
}

const authedUser: UseAuthReturn = {
  user: { user_id: 'u1', username: 'vale' },
  loading: false,
  login: vi.fn(),
  register: vi.fn(),
  logout: vi.fn(),
};

const savedWarrior: libraryModule.Warrior = {
  id: 'server:11111111-1111-1111-1111-111111111111',
  label: 'Imp Redux',
  source: 'MOV.I $0, $1',
  isPreset: false,
};

const originalFetch = globalThis.fetch;

beforeEach(() => {
  // Session module holds process-wide singletons (in-flight refresh
  // promise, force-logout handler). Without this reset, a test that
  // triggers a refresh leaves `inflight` set until the fake fetch
  // resolves — a following test that expects a fresh refresh call
  // silently reuses the previous result.
  __resetSessionForTests();
  currentSocket = makeFakeSocket();
  mockUseAuth.mockReturnValue(authedUser);
  mockUseWarriorLibrary.mockReturnValue([savedWarrior]);
});

afterEach(() => {
  cleanup();
  vi.clearAllMocks();
  globalThis.fetch = originalFetch;
  __resetSessionForTests();
});

describe('LobbyPage — Find Match', () => {
  it('clicking Find Match emits queue:join with the selected warrior id', async () => {
    const user = userEvent.setup();
    renderLobby();

    await user.click(screen.getByRole('button', { name: /find match/i }));

    expect(currentSocket.emit).toHaveBeenCalledWith('queue:join', {
      warrior_id: '11111111-1111-1111-1111-111111111111',
    });
  });

  it('shows queued state after backend acks the join', async () => {
    const user = userEvent.setup();
    renderLobby();

    await user.click(screen.getByRole('button', { name: /find match/i }));
    currentSocket._fire('queue:joined', {});

    expect(await screen.findByText(/searching for opponent/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /cancel/i })).toBeInTheDocument();
  });
});

describe('LobbyPage — socket auth recovery (issue #60)', () => {
  it('auth_error after Find Match refreshes silently and replays the queue:join', async () => {
    // Refresh endpoint returns 200 → silent recovery is expected.
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: 'ok',
      json: async () => ({}),
    } as Response);

    const user = userEvent.setup();
    renderLobby();

    await user.click(screen.getByRole('button', { name: /find match/i }));

    // Backend sees anonymous connection and emits auth_error. If LobbyPage
    // forgot to stash the pending action into its ref BEFORE the emit
    // above, the getPendingAction callback would return null here and the
    // silent-failure bug from #60 reproduces — no replay, user stuck idle.
    currentSocket._fire('auth_error', { error: 'Authentication required' });
    await flushMicrotasks();
    currentSocket._fire('auth_refreshed', { user_id: 'u1', username: 'vale' });
    await flushMicrotasks();

    const queueCalls = currentSocket.emit.mock.calls.filter((c) => c[0] === 'queue:join');
    expect(queueCalls).toHaveLength(2); // once from click, once from replay
    // Sanity: the reauthenticate emit went out between the two queue:joins.
    const reauthCalls = currentSocket.emit.mock.calls.filter((c) => c[0] === 'reauthenticate');
    expect(reauthCalls).toHaveLength(1);
  });

  it('on refresh failure, resets phase to idle and shows the session-expired copy', async () => {
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: false,
      status: 401,
      statusText: 'unauthorized',
      json: async () => ({ error: 'Missing refresh token' }),
    } as Response);

    const user = userEvent.setup();
    renderLobby();

    await user.click(screen.getByRole('button', { name: /find match/i }));
    currentSocket._fire('auth_error', { error: 'Authentication required' });
    await flushMicrotasks();

    // No reauthenticate — the primitive gives up before that step when the
    // REST refresh fails.
    const reauthCalls = currentSocket.emit.mock.calls.filter((c) => c[0] === 'reauthenticate');
    expect(reauthCalls).toHaveLength(0);

    // Friendly copy surfaces, phase stays idle (Find Match button visible).
    expect(await screen.findByText(/your session has expired/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /find match/i })).toBeInTheDocument();
  });

  it('does not replay after queue:joined clears the pending ref (no double-join)', async () => {
    // Once the queue:joined ack lands, the pending ref is cleared so a
    // later auth_error must NOT replay the join a second time. Prevents
    // double-queue if the socket bounces after a successful queue join.
    globalThis.fetch = vi.fn().mockResolvedValue({
      ok: true,
      status: 200,
      statusText: 'ok',
      json: async () => ({}),
    } as Response);

    const user = userEvent.setup();
    renderLobby();

    await user.click(screen.getByRole('button', { name: /find match/i }));
    currentSocket._fire('queue:joined', {});
    await flushMicrotasks();

    // Now the pending ref should be null. Simulate an auth_error hitting
    // the same socket (e.g. server reboot mid-queue).
    currentSocket._fire('auth_error', { error: 'Authentication required' });
    await flushMicrotasks();
    currentSocket._fire('auth_refreshed', { user_id: 'u1', username: 'vale' });
    await flushMicrotasks();

    const queueCalls = currentSocket.emit.mock.calls.filter((c) => c[0] === 'queue:join');
    // Exactly ONE queue:join — the original click. No accidental replay.
    expect(queueCalls).toHaveLength(1);
  });
});
