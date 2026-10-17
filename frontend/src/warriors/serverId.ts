// Library ids for warriors saved on the server are the database id with a
// `server:` prefix (local drafts are `user:*`, presets `preset:*`).

const SERVER_PREFIX = 'server:';

/** Whether a library id belongs to a warrior saved on the server. */
export function isServerId(id: string): boolean {
  return id.startsWith(SERVER_PREFIX);
}

/** Library id for the server warrior with this database id. */
export function fromServerId(serverId: string): string {
  return SERVER_PREFIX + serverId;
}

/** The database id behind a `server:` library id (other ids pass through). */
export function toServerId(localId: string): string {
  return isServerId(localId) ? localId.slice(SERVER_PREFIX.length) : localId;
}
