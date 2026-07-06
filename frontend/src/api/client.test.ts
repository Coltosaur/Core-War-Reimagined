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

  // Mutating-flow coverage. Issue #86 explicitly called out that the failure
  // mode we care about is silent 401s on `save warrior / queue match / edit
  // profile` after the access token expires — not just noisy GET /warriors
  // 401s. These tests prove POST/PUT/DELETE go through the same
  // refresh-and-retry path AND that the request body / method survive the
  // retry (i.e. the second attempt lands with the same payload, not an
  // empty one).
  describe('mutating flows survive a mid-session 401', () => {
    it('POST body is preserved across refresh + retry', async () => {
      globalThis.fetch = planFetch([
        { status: 401, body: { error: 'Missing access token' } },
        { status: 200 }, // refresh
        { status: 201, body: { id: 'w1', name: 'Imp Redux' } }, // retried POST
      ]);

      const body = { name: 'Imp Redux', source: 'MOV.I $0, $1' };
      await expect(api.post('/api/warriors', body)).resolves.toEqual({
        id: 'w1',
        name: 'Imp Redux',
      });

      const calls = (globalThis.fetch as unknown as ReturnType<typeof vi.fn>).mock.calls;
      expect(calls).toHaveLength(3);
      // First and third calls are both POST /api/warriors with the same body;
      // if the retry silently drops the body, save-warrior would 400 the user.
      const [firstUrl, firstInit] = calls[0];
      const [retriedUrl, retriedInit] = calls[2];
      expect(String(firstUrl)).toContain('/api/warriors');
      expect(String(retriedUrl)).toContain('/api/warriors');
      expect((firstInit as RequestInit).method).toBe('POST');
      expect((retriedInit as RequestInit).method).toBe('POST');
      expect((firstInit as RequestInit).body).toBe(JSON.stringify(body));
      expect((retriedInit as RequestInit).body).toBe(JSON.stringify(body));
    });

    it('PUT body is preserved across refresh + retry', async () => {
      globalThis.fetch = planFetch([
        { status: 401 },
        { status: 200 }, // refresh
        { status: 200, body: { id: 'w1', name: 'Imp Redux v2' } }, // retried PUT
      ]);

      const body = { name: 'Imp Redux v2', source: 'MOV.I $0, $1' };
      await expect(api.put('/api/warriors/w1', body)).resolves.toEqual({
        id: 'w1',
        name: 'Imp Redux v2',
      });

      const calls = (globalThis.fetch as unknown as ReturnType<typeof vi.fn>).mock.calls;
      expect(calls).toHaveLength(3);
      expect((calls[0][1] as RequestInit).method).toBe('PUT');
      expect((calls[2][1] as RequestInit).method).toBe('PUT');
      expect((calls[2][1] as RequestInit).body).toBe(JSON.stringify(body));
    });

    it('DELETE method survives refresh + retry (204 handled correctly)', async () => {
      globalThis.fetch = planFetch([
        { status: 401 },
        { status: 200 }, // refresh
        { status: 204 }, // retried DELETE
      ]);

      await expect(api.delete('/api/warriors/w1')).resolves.toBeUndefined();

      const calls = (globalThis.fetch as unknown as ReturnType<typeof vi.fn>).mock.calls;
      expect(calls).toHaveLength(3);
      expect((calls[0][1] as RequestInit).method).toBe('DELETE');
      expect((calls[2][1] as RequestInit).method).toBe('DELETE');
    });

    it('when refresh fails, mutating action throws SessionExpiredError — save silently 401ing is the exact bug from #86', async () => {
      globalThis.fetch = planFetch([
        { status: 401 }, // POST /warriors
        { status: 401 }, // refresh — both cookies dead
      ]);
      const forceLogoutSpy = vi.fn();
      registerSessionHandlers({ onForceLogout: forceLogoutSpy });

      await expect(
        api.post('/api/warriors', { name: 'x', source: 'MOV.I $0, $1' }),
      ).rejects.toBeInstanceOf(SessionExpiredError);
      expect(forceLogoutSpy).toHaveBeenCalledTimes(1);
      // Critically: only two fetches — the refusal is decisive, we do NOT
      // silently swallow the write attempt and then retry with the same
      // now-dead cookies.
      expect(globalThis.fetch).toHaveBeenCalledTimes(2);
    });
  });
});
