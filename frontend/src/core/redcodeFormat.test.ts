import { describe, expect, it } from 'vitest';
import { formatInstruction, cellAddressAtPixel } from './redcodeFormat';
import { CELL_SCALE, CORE_SIZE, GRID_COLS, GRID_ROWS } from './constants';

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
  // The grid at its native size: one cell per CELL_SCALE pixels.
  const W = GRID_COLS * CELL_SCALE;
  const H = GRID_ROWS * CELL_SCALE;

  it('returns 0 for the top-left corner', () => {
    expect(cellAddressAtPixel(0, 0, W, H)).toBe(0);
  });

  it('returns correct address for a known cell', () => {
    expect(cellAddressAtPixel(CELL_SCALE * 5, CELL_SCALE * 2, W, H)).toBe(2 * GRID_COLS + 5);
  });

  it('returns -1 for negative coordinates', () => {
    expect(cellAddressAtPixel(-1, 0, W, H)).toBe(-1);
    expect(cellAddressAtPixel(0, -1, W, H)).toBe(-1);
  });

  it('returns -1 for coordinates beyond grid bounds', () => {
    expect(cellAddressAtPixel(W, 0, W, H)).toBe(-1);
    expect(cellAddressAtPixel(0, H, W, H)).toBe(-1);
  });

  it('maps the last cell in the core, not past it', () => {
    expect(cellAddressAtPixel(W - 1, H - 1, W, H)).toBe(CORE_SIZE - 1);
  });

  it('maps proportionally when the grid is scaled down', () => {
    // A 343px-wide grid (a 375px phone minus gutters): cell (5, 2) sits at
    // the same fraction of the rendered size as at native size.
    const w = 343;
    const h = (343 * GRID_ROWS) / GRID_COLS;
    const x = (5.5 / GRID_COLS) * w;
    const y = (2.5 / GRID_ROWS) * h;
    expect(cellAddressAtPixel(x, y, w, h)).toBe(2 * GRID_COLS + 5);
  });

  it('returns -1 for a zero-size grid', () => {
    expect(cellAddressAtPixel(0, 0, 0, 0)).toBe(-1);
  });
});
