import { describe, expect, it } from 'vitest';
import { formatInstruction, cellAddressAtPixel } from './redcodeFormat';
import { CELL_SCALE, GRID_COLS } from './constants';

describe('formatInstruction', () => {
  it('formats DAT.F #0, #0', () => {
    expect(formatInstruction(0, 4, 0, 0, 0, 0)).toBe('DAT.F #0, #0');
  });

  it('formats MOV.I $0, $1', () => {
    expect(formatInstruction(1, 6, 1, 0, 1, 1)).toBe('MOV.I $0, $1');
  });

  it('formats ADD.AB #4, $3 (Dwarf-style)', () => {
    expect(formatInstruction(2, 2, 0, 4, 1, 3)).toBe('ADD.AB #4, $3');
  });

  it('handles all addressing modes', () => {
    expect(formatInstruction(1, 6, 0, 1, 0, 2)).toBe('MOV.I #1, #2');
    expect(formatInstruction(1, 6, 1, 1, 1, 2)).toBe('MOV.I $1, $2');
    expect(formatInstruction(1, 6, 2, 1, 2, 2)).toBe('MOV.I *1, *2');
    expect(formatInstruction(1, 6, 3, 1, 3, 2)).toBe('MOV.I @1, @2');
    expect(formatInstruction(1, 6, 4, 1, 4, 2)).toBe('MOV.I {1, {2');
    expect(formatInstruction(1, 6, 5, 1, 5, 2)).toBe('MOV.I }1, }2');
    expect(formatInstruction(1, 6, 6, 1, 6, 2)).toBe('MOV.I <1, <2');
    expect(formatInstruction(1, 6, 7, 1, 7, 2)).toBe('MOV.I >1, >2');
  });

  it('handles negative values', () => {
    expect(formatInstruction(7, 0, 1, -5, 1, 0)).toBe('JMP.A $-5, $0');
  });

  it('falls back to ??? for invalid opcode', () => {
    expect(formatInstruction(99, 0, 0, 0, 0, 0)).toBe('???.A #0, #0');
  });

  it('falls back to ? for invalid modifier', () => {
    expect(formatInstruction(0, 99, 0, 0, 0, 0)).toBe('DAT.? #0, #0');
  });

  it('falls back to ? for invalid addressing mode', () => {
    expect(formatInstruction(0, 0, 99, 0, 0, 0)).toBe('DAT.A ?0, #0');
  });
});

describe('cellAddressAtPixel', () => {
  it('returns 0 for the top-left corner', () => {
    expect(cellAddressAtPixel(0, 0)).toBe(0);
  });

  it('returns correct address for a known cell', () => {
    expect(cellAddressAtPixel(CELL_SCALE * 5, CELL_SCALE * 2)).toBe(2 * GRID_COLS + 5);
  });

  it('returns -1 for negative coordinates', () => {
    expect(cellAddressAtPixel(-1, 0)).toBe(-1);
    expect(cellAddressAtPixel(0, -1)).toBe(-1);
  });

  it('returns -1 for coordinates beyond grid bounds', () => {
    expect(cellAddressAtPixel(CELL_SCALE * GRID_COLS, 0)).toBe(-1);
    expect(cellAddressAtPixel(0, CELL_SCALE * GRID_COLS)).toBe(-1);
  });

  it('maps last valid cell correctly', () => {
    const x = CELL_SCALE * (GRID_COLS - 1);
    const y = CELL_SCALE * (GRID_COLS - 1);
    expect(cellAddressAtPixel(x, y)).toBe((GRID_COLS - 1) * GRID_COLS + (GRID_COLS - 1));
  });
});
