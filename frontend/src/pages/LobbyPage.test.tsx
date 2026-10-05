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
// flow (issues #60 and #127). The primitive `installSocketAuthRecovery` is heavily
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
  connected: boolean;
  emit: ReturnType<typeof vi.fn>;
  connect: ReturnType<typeof vi.fn>;
  disconnect: ReturnType<typeof vi.fn>;
  _fire: (event: string, ...args: unknown[]) => void;
};

function makeFakeSocket(): FakeSocket {
  const listeners = new Map<string, Set<Listener>>();
  const onceListeners = new Map<string, Set<Listener>>();
  const emit = vi.fn();
  const disconnect = vi.fn();
  // A reconnect that succeeds, like socket.io's after disconnect()+connect().
  const connect = vi.fn(() => queueMicrotask(() => socket._fire('connect')));

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
    connected: false,
    emit,
    connect,
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

/** Click Find Match and complete the socket's first connect. */
async function findMatch(user: ReturnType<typeof userEvent.setup>) {
  await user.click(screen.getByRole('button', { name: /find match/i }));
  currentSocket._fire('connect');
}

const joins = () => currentSocket.emit.mock.calls.filter((c) => c[0] === 'queue:join');

function mockRefresh(ok: boolean) {
  globalThis.fetch = vi.fn().mockResolvedValue({
    ok,
    status: ok ? 200 : 401,
    statusText: ok ? 'ok' : 'unauthorized',
    json: async () => ({}),
  } as Response);
}

describe('LobbyPage — socket auth recovery (issues #60, #127)', () => {
  it('auth_error after Find Match refreshes, reconnects, and replays the queue:join', async () => {
    mockRefresh(true);
    const user = userEvent.setup();
    renderLobby();

    await findMatch(user);

    // Backend sees anonymous connection and emits auth_error. If LobbyPage
    // forgot to stash the pending action into its ref BEFORE the emit
    // above, the getPendingAction callback would return null here and the
    // silent-failure bug from #60 reproduces — no replay, user stuck idle.
    currentSocket._fire('auth_error', { error: 'Authentication required' });
    await flushMicrotasks();

    expect(currentSocket.disconnect).toHaveBeenCalled();
    expect(currentSocket.connect).toHaveBeenCalled();
    expect(joins()).toHaveLength(2); // once from click, once from replay
    expect(screen.queryByText(/your session has expired/i)).not.toBeInTheDocument();
  });

  it('on refresh failure, resets phase to idle and shows the session-expired copy', async () => {
    mockRefresh(false);
    const user = userEvent.setup();
    renderLobby();

    await findMatch(user);
    currentSocket._fire('auth_error', { error: 'Authentication required' });
    await flushMicrotasks();

    expect(currentSocket.connect).not.toHaveBeenCalled();
    // Friendly copy surfaces, phase stays idle (Find Match button visible).
    expect(await screen.findByText(/your session has expired/i)).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /find match/i })).toBeInTheDocument();
  });

  it('re-joins the queue when the socket reconnects while queued', async () => {
    // The server drops a connection's queue entry when it goes away (a
    // network blip, a deploy restarting the server). Without a re-join the
    // page keeps showing "Searching" for an entry that no longer exists.
    const user = userEvent.setup();
    renderLobby();

    await findMatch(user);
    currentSocket._fire('queue:joined', {});
    expect(await screen.findByText(/searching for opponent/i)).toBeInTheDocument();

    currentSocket._fire('connect');

    expect(joins()).toHaveLength(2);
    expect(joins()[1][1]).toEqual({ warrior_id: '11111111-1111-1111-1111-111111111111' });
  });

  it('does not re-join after the player cancels', async () => {
    const user = userEvent.setup();
    renderLobby();

    await findMatch(user);
    currentSocket._fire('queue:joined', {});
    await user.click(await screen.findByRole('button', { name: /cancel/i }));
    currentSocket._fire('connect');

    expect(joins()).toHaveLength(1);
  });

  it('does not re-join after a match is found', async () => {
    const user = userEvent.setup();
    renderLobby();

    await findMatch(user);
    currentSocket._fire('queue:joined', {});
    currentSocket._fire('match:found', {
      match_id: 'm1',
      red_username: 'vale',
      blue_username: 'orion',
      red_warrior: 'a',
      blue_warrior: 'b',
    });
    currentSocket._fire('connect');

    expect(joins()).toHaveLength(1);
  });
});
