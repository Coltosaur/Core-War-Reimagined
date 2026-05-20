import { describe, expect, it } from 'vitest';
import { computeTargetStep } from './useMatchReplay';

describe('computeTargetStep', () => {
  const PLAYBACK_START = 1_000_000;
  const STEPS_PER_SEC = 2000;
  const STEPS_TAKEN = 80_000;

  it('returns 0 when wall clock is exactly the playback start', () => {
    expect(computeTargetStep(PLAYBACK_START, PLAYBACK_START, STEPS_PER_SEC, STEPS_TAKEN)).toBe(0);
  });

  it('returns 0 for negative elapsed (client clock ahead of server)', () => {
    expect(
      computeTargetStep(PLAYBACK_START - 5000, PLAYBACK_START, STEPS_PER_SEC, STEPS_TAKEN),
    ).toBe(0);
  });

  it('returns 1000 after 500ms at 2000 steps/sec', () => {
    expect(
      computeTargetStep(PLAYBACK_START + 500, PLAYBACK_START, STEPS_PER_SEC, STEPS_TAKEN),
    ).toBe(1000);
  });

  it('floors fractional steps (no rounding up)', () => {
    expect(computeTargetStep(PLAYBACK_START + 1, PLAYBACK_START, STEPS_PER_SEC, STEPS_TAKEN)).toBe(
      2,
    );
    expect(
      computeTargetStep(PLAYBACK_START + 100, PLAYBACK_START, STEPS_PER_SEC, STEPS_TAKEN),
    ).toBe(200);
  });

  it('caps at steps_taken once elapsed time would exceed the canonical match length', () => {
    // 40s × 2000 steps/s = 80_000 — the exact boundary
    expect(
      computeTargetStep(PLAYBACK_START + 40_000, PLAYBACK_START, STEPS_PER_SEC, STEPS_TAKEN),
    ).toBe(STEPS_TAKEN);
    // Far past
    expect(
      computeTargetStep(PLAYBACK_START + 999_999, PLAYBACK_START, STEPS_PER_SEC, STEPS_TAKEN),
    ).toBe(STEPS_TAKEN);
  });

  it('caps correctly for short matches (less than max_steps)', () => {
    const shortMatch = 3000;
    expect(
      computeTargetStep(PLAYBACK_START + 5_000, PLAYBACK_START, STEPS_PER_SEC, shortMatch),
    ).toBe(shortMatch);
  });

  it('scales with steps_per_sec', () => {
    expect(computeTargetStep(PLAYBACK_START + 1000, PLAYBACK_START, 1000, STEPS_TAKEN)).toBe(1000);
    expect(computeTargetStep(PLAYBACK_START + 1000, PLAYBACK_START, 4000, STEPS_TAKEN)).toBe(4000);
  });
});
