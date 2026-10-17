// Library ids for warriors saved on the server are the database id with a
// `server:` prefix (local drafts are `user:*`, presets `preset:*`).
// This is its own module, not part of library.ts, because tests that
// auto-mock library.ts would turn these helpers into mocks too.

const SERVER_PREFIX = 'server:';

/** Whether a library id belongs to a warrior saved on the server. */
export function isServerId(id: string): boolean {
  return id.startsWith(SERVER_PREFIX);
}

/** Library id for the server warrior with this database id. */
export function libraryIdFor(serverId: string): string {
  return SERVER_PREFIX + serverId;
}

/** The database id behind a `server:` library id (other ids pass through). */
export function serverIdOf(libraryId: string): string {
  return isServerId(libraryId) ? libraryId.slice(SERVER_PREFIX.length) : libraryId;
}
