import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { api, ApiError } from './client';
import { __resetSessionForTests, registerSessionHandlers, SessionExpiredError } from './session';

type FetchMockPlan = Array<{ status: number; body?: unknown }>;

function planFetch(plan: FetchMockPlan): ReturnType<typeof vi.fn> {
  let idx = 0;
  return vi.fn().mockImplementation(async (): Promise<Response> => {
    const entry = plan[Math.min(idx, plan.length - 1)];
    idx += 1;
    return {
      ok: entry.status >= 200 && entry.status < 300,
      status: entry.status,
      statusText: 'test',
      json: async () => entry.body ?? {},
    } as Response;
  });
}

describe('api client refresh-on-401 interceptor', () => {
  const originalFetch = globalThis.fetch;

  beforeEach(() => {
    __resetSessionForTests();
  });

  afterEach(() => {
    globalThis.fetch = originalFetch;
    __resetSessionForTests();
  });

  it('passes 2xx responses straight through without touching refresh', async () => {
    globalThis.fetch = planFetch([{ status: 200, body: { ok: true } }]);
    await expect(api.get<{ ok: boolean }>('/api/warriors')).resolves.toEqual({ ok: true });
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });

  it('recovers a 401 by refreshing and retrying — no error surfaces', async () => {
    // Sequence: warriors 401, refresh 200, warriors 200
    globalThis.fetch = planFetch([
      { status: 401, body: { error: 'Missing access token' } },
      { status: 200 },
      { status: 200, body: { warriors: [] } },
    ]);
    await expect(api.get<{ warriors: unknown[] }>('/api/warriors')).resolves.toEqual({
      warriors: [],
    });
    // 1st: warriors 401, 2nd: refresh, 3rd: warriors retry.
    expect(globalThis.fetch).toHaveBeenCalledTimes(3);
  });

  it('clears user via forceLogout and throws SessionExpiredError when refresh fails', async () => {
    globalThis.fetch = planFetch([
      { status: 401, body: { error: 'Missing access token' } },
      { status: 401, body: { error: 'Missing refresh token' } },
    ]);
    const forceLogoutSpy = vi.fn();
    registerSessionHandlers({ onForceLogout: forceLogoutSpy });

    await expect(api.get('/api/warriors')).rejects.toBeInstanceOf(SessionExpiredError);
    expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
    expect(globalThis.fetch).toHaveBeenCalledTimes(2);
  });

  it('clears user and throws SessionExpiredError when retry still 401s after successful refresh', async () => {
    globalThis.fetch = planFetch([
      { status: 401 },
      { status: 200 }, // refresh
      { status: 401 }, // retry still 401
    ]);
    const forceLogoutSpy = vi.fn();
    registerSessionHandlers({ onForceLogout: forceLogoutSpy });

    await expect(api.get('/api/warriors')).rejects.toBeInstanceOf(SessionExpiredError);
    expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
    expect(globalThis.fetch).toHaveBeenCalledTimes(3);
  });

  it('does NOT attempt refresh on 401 from auth endpoints (avoids loops)', async () => {
    globalThis.fetch = planFetch([{ status: 401, body: { error: 'Invalid credentials' } }]);
    await expect(
      api.post('/api/auth/login', { username_or_email: 'x', password: 'y' }),
    ).rejects.toMatchObject({ status: 401, message: 'Invalid credentials' });
    expect(globalThis.fetch).toHaveBeenCalledTimes(1);
  });

  it('propagates non-401 errors without engaging refresh', async () => {
    globalThis.fetch = planFetch([{ status: 500, body: { error: 'Internal server error' } }]);
    await expect(api.get('/api/warriors')).rejects.toBeInstanceOf(ApiError);
    await expect(api.get('/api/warriors')).rejects.toMatchObject({ status: 500 });
  });

  it('a burst of parallel 401s triggers exactly one refresh', async () => {
    // Plan: infinite refresh interleaved with per-request 401s and then 200s.
    // We can't easily use a positional plan here because ordering across
    // parallel calls is nondeterministic. Instead, hand-roll a fetch mock
    // that classifies by URL and refresh-succeeds after the first call.
    let refreshCount = 0;
    const perPathState: Record<string, { hit: number }> = {};
    globalThis.fetch = vi.fn().mockImplementation(async (url: string): Promise<Response> => {
      const path = new URL(String(url), 'http://x').pathname;
      if (path === '/api/auth/refresh') {
        refreshCount += 1;
        return {
          ok: true,
          status: 200,
          statusText: 'ok',
          json: async () => ({}),
        } as Response;
      }
      const state = (perPathState[path] ??= { hit: 0 });
      state.hit += 1;
      const status = state.hit === 1 ? 401 : 200;
      return {
        ok: status === 200,
        status,
        statusText: 'test',
        json: async () => ({ path, hit: state.hit }),
      } as Response;
    });

    const results = await Promise.all([
      api.get<{ hit: number }>('/api/warriors'),
      api.get<{ hit: number }>('/api/profile'),
      api.get<{ hit: number }>('/api/leaderboard'),
    ]);
    for (const r of results) {
      expect(r.hit).toBe(2);
    }
    expect(refreshCount).toBe(1);
  });

  it('sends credentials on every request so the cookie jar rides along', async () => {
    globalThis.fetch = planFetch([{ status: 200, body: {} }]);
    await api.get('/api/warriors');
    const [, init] = (globalThis.fetch as unknown as ReturnType<typeof vi.fn>).mock.calls[0];
    expect((init as RequestInit).credentials).toBe('include');
  });
});
