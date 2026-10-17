import { isServerId, libraryIdFor, serverIdOf } from './serverId';

describe('server warrior ids', () => {
  it('round-trips a database id through the library id', () => {
    const local = libraryIdFor('3f2a');
    expect(local).toBe('server:3f2a');
    expect(isServerId(local)).toBe(true);
    expect(serverIdOf(local)).toBe('3f2a');
  });

  it('leaves local drafts and presets alone', () => {
    for (const id of ['user:abc-123', 'preset:imp']) {
      expect(isServerId(id)).toBe(false);
      expect(serverIdOf(id)).toBe(id);
    }
  });
});
