import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import {
  __resetSessionForTests,
  attemptRefresh,
  forceLogout,
  isAuthEndpoint,
  registerSessionHandlers,
  SessionExpiredError,
} from './session';
import { ApiError } from './client';

const REFRESH_URL_SUFFIX = '/api/auth/refresh';

function mockFetchWithDelay(status: number, delayMs: number): ReturnType<typeof vi.fn> {
  return vi.fn().mockImplementation(
    () =>
      new Promise((resolve) => {
        setTimeout(
          () =>
            resolve({
              ok: status >= 200 && status < 300,
              status,
              json: async () => ({}),
            } as Response),
          delayMs,
        );
      }),
  );
}

describe('session', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    __resetSessionForTests();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    __resetSessionForTests();
  });

  describe('SessionExpiredError', () => {
    it('is an ApiError with status 401 and a friendly default message', () => {
      const err = new SessionExpiredError();
      expect(err).toBeInstanceOf(ApiError);
      expect(err.status).toBe(401);
      expect(err.message).toMatch(/session has expired/i);
      expect(err.name).toBe('SessionExpiredError');
    });

    it('accepts a custom message', () => {
      const err = new SessionExpiredError('custom');
      expect(err.message).toBe('custom');
    });
  });

  describe('isAuthEndpoint', () => {
    it.each([
      ['/api/auth/refresh', true],
      ['/api/auth/login', true],
      ['/api/auth/register', true],
      ['/api/auth/logout', true],
      ['/api/auth/me', false],
      ['/api/warriors', false],
      ['/api/warriors/foo', false],
    ])('returns %s for %s', (path, expected) => {
      expect(isAuthEndpoint(path)).toBe(expected);
    });

    it('ignores query strings', () => {
      expect(isAuthEndpoint('/api/auth/login?next=/lobby')).toBe(true);
      expect(isAuthEndpoint('/api/warriors?page=1')).toBe(false);
    });

    it('tolerates trailing slashes', () => {
      expect(isAuthEndpoint('/api/auth/login/')).toBe(true);
    });
  });

  describe('attemptRefresh', () => {
    it('resolves true when POST /api/auth/refresh returns 2xx', async () => {
      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => ({}),
      } as Response);
      await expect(attemptRefresh()).resolves.toBe(true);
      expect(globalThis.fetch).toHaveBeenCalledTimes(1);
      const [url, init] = (globalThis.fetch as unknown as ReturnType<typeof vi.fn>).mock.calls[0];
      expect(String(url)).toContain(REFRESH_URL_SUFFIX);
      expect((init as RequestInit).method).toBe('POST');
      expect((init as RequestInit).credentials).toBe('include');
    });

    it('resolves false when the endpoint returns 401', async () => {
      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: false,
        status: 401,
        json: async () => ({}),
      } as Response);
      await expect(attemptRefresh()).resolves.toBe(false);
    });

    it('resolves false on a network error (does not throw)', async () => {
      globalThis.fetch = vi.fn().mockRejectedValue(new TypeError('network dead'));
      await expect(attemptRefresh()).resolves.toBe(false);
    });

    it('single-flights concurrent callers — one fetch, one shared resolution', async () => {
      globalThis.fetch = mockFetchWithDelay(200, 25);
      const [a, b, c] = await Promise.all([attemptRefresh(), attemptRefresh(), attemptRefresh()]);
      expect([a, b, c]).toEqual([true, true, true]);
      expect(globalThis.fetch).toHaveBeenCalledTimes(1);
    });

    it('allows a fresh attempt after the previous one settles', async () => {
      globalThis.fetch = mockFetchWithDelay(200, 5);
      await attemptRefresh();
      await attemptRefresh();
      expect(globalThis.fetch).toHaveBeenCalledTimes(2);
    });

    it('propagates failure to concurrent callers and still clears in-flight state', async () => {
      globalThis.fetch = mockFetchWithDelay(401, 10);
      const [a, b] = await Promise.all([attemptRefresh(), attemptRefresh()]);
      expect([a, b]).toEqual([false, false]);
      expect(globalThis.fetch).toHaveBeenCalledTimes(1);

      // A follow-up succeeds — in-flight guard cleared.
      globalThis.fetch = vi.fn().mockResolvedValue({
        ok: true,
        status: 200,
        json: async () => ({}),
      } as Response);
      await expect(attemptRefresh()).resolves.toBe(true);
    });
  });

  describe('forceLogout / registerSessionHandlers', () => {
    it('invokes the registered handler', () => {
      const spy = vi.fn();
      registerSessionHandlers({ onForceLogout: spy });
      forceLogout();
      expect(spy).toHaveBeenCalledTimes(1);
    });

    it('is a no-op before any handler is registered', () => {
      // Reset to defaults; verify forceLogout does not throw.
      __resetSessionForTests();
      expect(() => forceLogout()).not.toThrow();
    });

    it('unregisters via the returned teardown, restoring the no-op default', () => {
      const spy = vi.fn();
      const unregister = registerSessionHandlers({ onForceLogout: spy });
      unregister();
      forceLogout();
      expect(spy).not.toHaveBeenCalled();
    });

    it('teardown from a stale registration does not clobber a newer one', () => {
      const oldSpy = vi.fn();
      const newSpy = vi.fn();
      const oldTeardown = registerSessionHandlers({ onForceLogout: oldSpy });
      registerSessionHandlers({ onForceLogout: newSpy });
      // The teardown from the *older* registration should be a no-op now
      // because the current handlers are the newer ones.
      oldTeardown();
      forceLogout();
      expect(oldSpy).not.toHaveBeenCalled();
      expect(newSpy).toHaveBeenCalledTimes(1);
    });
  });
});
