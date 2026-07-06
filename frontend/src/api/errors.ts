// Error hierarchy for the API layer. Lifted out of `client.ts` and
// `session.ts` because those two modules would otherwise form a circular
// import — session.ts needs ApiError to subclass, and client.ts needs
// SessionExpiredError to throw. Both types live here; both files
// re-export whatever they need to keep their public surfaces stable.

/**
 * Any non-2xx response the server sends back — the base class other error
 * types (like SessionExpiredError) subclass. The interceptor and any
 * higher-level catch that wants to inspect status/message should key off
 * `instanceof ApiError`.
 */
export class ApiError extends Error {
  constructor(
    public status: number,
    message: string,
  ) {
    super(message);
    this.name = 'ApiError';
  }
}

/**
 * Thrown when refresh has definitively failed and the caller should treat
 * the session as gone. Subclasses `ApiError` so existing
 * `instanceof ApiError` catches (e.g. `getMe()` returning null on 401)
 * keep working; carries user-facing copy instead of raw backend
 * "Missing access token" strings.
 */
export class SessionExpiredError extends ApiError {
  constructor(message = 'Your session has expired. Please log in again.') {
    super(401, message);
    this.name = 'SessionExpiredError';
  }
}
