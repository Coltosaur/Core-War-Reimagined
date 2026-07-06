import { ApiError, SessionExpiredError } from './errors';
import { attemptRefresh, forceLogout, isAuthEndpoint } from './session';

// Re-exported so existing consumers `import { ApiError } from './client'`
// keep working — the class itself now lives in `./errors` to break a
// circular import with `./session`.
export { ApiError, SessionExpiredError };

const API_BASE = import.meta.env.VITE_API_URL ?? 'http://localhost:3001';

async function performRequest(path: string, options: RequestInit): Promise<Response> {
  return fetch(`${API_BASE}${path}`, {
    credentials: 'include',
    headers: { 'Content-Type': 'application/json', ...options.headers },
    ...options,
  });
}

async function parseErrorBody(res: Response): Promise<string> {
  const body = await res.json().catch(() => ({ error: res.statusText }));
  return body.error ?? res.statusText;
}

async function request<T>(path: string, options: RequestInit = {}): Promise<T> {
  let res = await performRequest(path, options);

  // Refresh-on-401 interceptor. The 15-minute access-token cookie will
  // routinely expire mid-session; the backend also gives us a 7-day refresh
  // cookie that can silently mint a fresh access token. Auth endpoints bypass
  // this — a 401 from /login means "wrong password", not "session expired",
  // and looping /refresh → /login → 401 would be a bad time.
  if (res.status === 401 && !isAuthEndpoint(path)) {
    const refreshed = await attemptRefresh();
    if (refreshed) {
      res = await performRequest(path, options);
      if (res.status === 401) {
        // Retry still failed after a successful refresh — the endpoint is
        // rejecting us for a real reason (e.g. deleted user, revoked
        // permission). Surface it as a session-expired error and clear
        // client-side auth state so the UI can prompt for re-login.
        forceLogout();
        throw new SessionExpiredError();
      }
    } else {
      // Refresh cookie missing / expired / rejected — same treatment.
      forceLogout();
      throw new SessionExpiredError();
    }
  }

  if (!res.ok) {
    throw new ApiError(res.status, await parseErrorBody(res));
  }

  if (res.status === 204) return undefined as T;
  return res.json();
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: 'POST', body: body ? JSON.stringify(body) : undefined }),
  put: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: 'PUT', body: body ? JSON.stringify(body) : undefined }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
};
