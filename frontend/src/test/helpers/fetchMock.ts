/** Minimal `Response` stand-in for fetch mocks: 2xx is `ok`, `json()` resolves to `body`. */
export function fakeResponse(status: number, body: unknown = {}): Response {
  return {
    ok: status >= 200 && status < 300,
    status,
    statusText: 'test',
    json: async () => body,
  } as Response;
}
