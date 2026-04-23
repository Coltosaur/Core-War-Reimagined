import { describe, expect, it } from 'vitest';
import { OPCODE_RGBA, OPCODE_HEX } from './opcodeColors';

describe('OPCODE_RGBA', () => {
  it('has exactly 64 bytes (16 opcodes * 4 channels)', () => {
    expect(OPCODE_RGBA.length).toBe(64);
  });

  it('has alpha=255 for all opcodes', () => {
    for (let i = 0; i < 16; i++) {
      expect(OPCODE_RGBA[i * 4 + 3]).toBe(255);
    }
  });

  it('DAT (opcode 0) is near-black', () => {
    const r = OPCODE_RGBA[0];
    const g = OPCODE_RGBA[1];
    const b = OPCODE_RGBA[2];
    expect(r).toBeLessThan(50);
    expect(g).toBeLessThan(50);
    expect(b).toBeLessThan(50);
  });

  it('MOV (opcode 1) is red-ish', () => {
    const r = OPCODE_RGBA[4];
    const g = OPCODE_RGBA[5];
    expect(r).toBeGreaterThan(200);
    expect(g).toBeLessThan(100);
  });
});

describe('OPCODE_HEX', () => {
  it('has 16 entries', () => {
    expect(OPCODE_HEX.length).toBe(16);
  });

  it('all values are valid RGB hex numbers', () => {
    for (const hex of OPCODE_HEX) {
      expect(hex).toBeGreaterThanOrEqual(0);
      expect(hex).toBeLessThanOrEqual(0xffffff);
    }
  });

  it('matches RGBA values for MOV (opcode 1)', () => {
    const r = OPCODE_RGBA[4];
    const g = OPCODE_RGBA[5];
    const b = OPCODE_RGBA[6];
    expect(OPCODE_HEX[1]).toBe((r << 16) | (g << 8) | b);
  });
});
