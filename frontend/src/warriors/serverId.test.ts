import { fromServerId, isServerId, toServerId } from './serverId';

describe('server warrior ids', () => {
  it('round-trips a database id through the library id', () => {
    const local = fromServerId('3f2a');
    expect(local).toBe('server:3f2a');
    expect(isServerId(local)).toBe(true);
    expect(toServerId(local)).toBe('3f2a');
  });

  it('leaves local drafts and presets alone', () => {
    for (const id of ['user:abc-123', 'preset:imp']) {
      expect(isServerId(id)).toBe(false);
      expect(toServerId(id)).toBe(id);
    }
  });
});
