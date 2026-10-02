import { beforeEach, describe, expect, it, vi } from 'vitest';
import type { ServerWarrior } from '../api/warriors';

// Regression tests for #58: server warriors must survive a hard refresh on
// any page, and syncing must never clobber local drafts.

vi.mock('../api/warriors', () => ({
  listWarriors: vi.fn(),
  createWarrior: vi.fn(),
  updateWarrior: vi.fn(),
  deleteWarrior: vi.fn(),
}));

const DRAFTS_KEY = 'corewar.warriors.v1';
const SERVER_KEY = 'corewar.serverWarriors.v1';

function sw(id: string, name: string, source = 'MOV.I $0, $1'): ServerWarrior {
  return {
    id,
    user_id: 'u1',
    name,
    source,
    created_at: '2026-01-01T00:00:00Z',
    updated_at: '2026-01-01T00:00:00Z',
  };
}

function listResponse(warriors: ServerWarrior[]) {
  return { warriors, total: warriors.length, page: 1, per_page: 100 };
}

// Fresh module instance == a hard page reload (module state re-hydrates
// from localStorage).
async function loadLibrary() {
  vi.resetModules();
  const api = await import('../api/warriors');
  const lib = await import('./library');
  return { api: vi.mocked(api), lib };
}

const ids = (list: { id: string; isPreset: boolean }[]) =>
  list.filter((w) => !w.isPreset).map((w) => w.id);

beforeEach(() => {
  localStorage.clear();
  vi.clearAllMocks();
});

describe('server warrior persistence (#58)', () => {
  it('server warriors survive a reload without a re-sync', async () => {
    const first = await loadLibrary();
    first.api.listWarriors.mockResolvedValue(listResponse([sw('a', 'Alpha')]));
    first.lib.setServerOwner('u1');
    await first.lib.syncFromServer();
    expect(ids(first.lib.listWarriors())).toEqual(['server:a']);
    first.api.listWarriors.mockClear();

    // Hard refresh on /lobby: no Builder mount, no sync yet.
    const second = await loadLibrary();
    expect(second.api.listWarriors).not.toHaveBeenCalled();
    expect(ids(second.lib.listWarriors())).toEqual(['server:a']);
    expect(second.lib.getWarrior('server:a')?.label).toBe('Alpha');
  });

  it('sync preserves local drafts instead of replacing them', async () => {
    const { api, lib } = await loadLibrary();
    const draft = lib.createUserWarrior('Draft', 'DAT #0, #0');
    api.listWarriors.mockResolvedValue(listResponse([sw('a', 'Alpha')]));
    lib.setServerOwner('u1');
    await lib.syncFromServer();
    expect(ids(lib.listWarriors())).toEqual([draft.id, 'server:a']);

    const drafts = JSON.parse(localStorage.getItem(DRAFTS_KEY)!);
    expect(drafts.map((w: { id: string }) => w.id)).toEqual([draft.id]);
  });

  it('sync drops server entries deleted elsewhere', async () => {
    const { api, lib } = await loadLibrary();
    lib.setServerOwner('u1');
    api.listWarriors.mockResolvedValueOnce(listResponse([sw('a', 'A'), sw('b', 'B')]));
    await lib.syncFromServer();
    api.listWarriors.mockResolvedValueOnce(listResponse([sw('b', 'B')]));
    await lib.syncFromServer();
    expect(ids(lib.listWarriors())).toEqual(['server:b']);
    expect(JSON.parse(localStorage.getItem(SERVER_KEY)!).warriors).toHaveLength(1);
  });

  it('a failed sync keeps the cached list', async () => {
    const first = await loadLibrary();
    first.api.listWarriors.mockResolvedValue(listResponse([sw('a', 'Alpha')]));
    first.lib.setServerOwner('u1');
    await first.lib.syncFromServer();

    const second = await loadLibrary();
    second.api.listWarriors.mockRejectedValue(new Error('offline'));
    second.lib.setServerOwner('u1');
    await second.lib.syncFromServer();
    expect(ids(second.lib.listWarriors())).toEqual(['server:a']);
  });

  it('create/update/delete persist to the cache', async () => {
    const { api, lib } = await loadLibrary();
    lib.setServerOwner('u1');
    api.createWarrior.mockResolvedValue(sw('n', 'New'));
    const w = await lib.createServerWarrior('New', 'MOV.I $0, $1');
    api.updateWarrior.mockResolvedValue(sw('n', 'Renamed'));
    await lib.updateServerWarrior(w.id, { label: 'Renamed' });

    const reloaded = await loadLibrary();
    expect(reloaded.lib.getWarrior('server:n')?.label).toBe('Renamed');

    reloaded.api.deleteWarrior.mockResolvedValue(undefined);
    reloaded.lib.setServerOwner('u1');
    await reloaded.lib.deleteServerWarrior('server:n');
    const again = await loadLibrary();
    expect(again.lib.getWarrior('server:n')).toBeUndefined();
  });

  it('clearServerWarriors (logout) drops the cache but keeps drafts', async () => {
    const { api, lib } = await loadLibrary();
    const draft = lib.createUserWarrior('Draft', 'NOP');
    api.listWarriors.mockResolvedValue(listResponse([sw('a', 'Alpha')]));
    lib.setServerOwner('u1');
    await lib.syncFromServer();

    lib.clearServerWarriors();
    expect(ids(lib.listWarriors())).toEqual([draft.id]);
    expect(localStorage.getItem(SERVER_KEY)).toBeNull();
  });

  it('a different user never sees the previous user’s cached warriors', async () => {
    const first = await loadLibrary();
    first.api.listWarriors.mockResolvedValue(listResponse([sw('a', 'Alpha')]));
    first.lib.setServerOwner('u1');
    await first.lib.syncFromServer();

    const second = await loadLibrary();
    second.lib.setServerOwner('u2');
    expect(ids(second.lib.listWarriors())).toEqual([]);
  });

  it('a sync that started before a mutation does not overwrite it', async () => {
    const { api, lib } = await loadLibrary();
    lib.setServerOwner('u1');
    let resolveList!: (v: ReturnType<typeof listResponse>) => void;
    api.listWarriors
      .mockReturnValueOnce(new Promise((r) => (resolveList = r)))
      .mockResolvedValueOnce(listResponse([sw('n', 'New')]));
    const pending = lib.syncFromServer();

    api.createWarrior.mockResolvedValue(sw('n', 'New'));
    await lib.createServerWarrior('New', 'NOP');
    resolveList(listResponse([]));
    await pending;

    expect(ids(lib.listWarriors())).toEqual(['server:n']);
  });

  // Review finding 2 on #111: a sync that bailed because of a concurrent
  // mutation was never retried, so on a fresh login (list just emptied by
  // setServerOwner) clicking "New" while the first fetch was in flight left
  // the user with only the new warrior until a reload.
  it('a sync interrupted by a mutation re-fetches instead of giving up', async () => {
    const { api, lib } = await loadLibrary();
    lib.setServerOwner('u1');
    let resolveFirst!: (v: ReturnType<typeof listResponse>) => void;
    api.listWarriors
      .mockReturnValueOnce(new Promise((r) => (resolveFirst = r)))
      .mockResolvedValueOnce(listResponse([sw('a', 'A'), sw('b', 'B'), sw('n', 'New')]));
    const pending = lib.syncFromServer();

    api.createWarrior.mockResolvedValue(sw('n', 'New'));
    await lib.createServerWarrior('New', 'NOP');
    resolveFirst(listResponse([sw('a', 'A'), sw('b', 'B')]));
    await pending;

    expect(api.listWarriors).toHaveBeenCalledTimes(2);
    expect(ids(lib.listWarriors())).toEqual(['server:a', 'server:b', 'server:n']);
  });

  it('a sync that keeps getting interrupted stops retrying after a bound', async () => {
    const { api, lib } = await loadLibrary();
    lib.setServerOwner('u1');
    api.listWarriors.mockImplementation(async () => {
      // Every fetch races a mutation.
      lib.clearServerWarriors();
      lib.setServerOwner('u1');
      return listResponse([]);
    });
    await lib.syncFromServer();
    expect(vi.mocked(api.listWarriors).mock.calls.length).toBeLessThanOrEqual(3);
  });

  // Review finding 3 on #111: create/update/delete applied their result even
  // if the account changed while the request was in flight, leaking the old
  // user's warrior into the new user's list and persisted cache.
  describe('account switch while a save is in flight', () => {
    async function heldOwnerSwitch<T>(
      start: (lib: Awaited<ReturnType<typeof loadLibrary>>['lib']) => Promise<T>,
      hold: (api: Awaited<ReturnType<typeof loadLibrary>>['api']) => (v: ServerWarrior) => void,
    ) {
      const { api, lib } = await loadLibrary();
      lib.setServerOwner('u1');
      const release = hold(api);
      const pending = start(lib);
      lib.setServerOwner('u2');
      release(sw('x', 'Leaked'));
      await pending;
      return lib;
    }

    it('createServerWarrior does not add to the new user', async () => {
      const lib = await heldOwnerSwitch(
        (l) => l.createServerWarrior('Leaked', 'NOP'),
        (api) => {
          let r!: (v: ServerWarrior) => void;
          api.createWarrior.mockReturnValue(new Promise((res) => (r = res)));
          return (v) => r(v);
        },
      );
      expect(ids(lib.listWarriors())).toEqual([]);
      expect(JSON.parse(localStorage.getItem(SERVER_KEY)!)).toEqual({
        userId: 'u2',
        warriors: [],
      });
    });

    it('updateServerWarrior does not touch the new user', async () => {
      const lib = await heldOwnerSwitch(
        (l) => l.updateServerWarrior('server:x', { label: 'Leaked' }),
        (api) => {
          let r!: (v: ServerWarrior) => void;
          api.updateWarrior.mockReturnValue(new Promise((res) => (r = res)));
          return (v) => r(v);
        },
      );
      expect(ids(lib.listWarriors())).toEqual([]);
    });
  });
});
