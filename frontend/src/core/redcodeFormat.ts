// Human-readable formatting of cell data from the wasm engine.
// The u8 discriminants match the Rust #[repr(u8)] enums in instruction.rs.

import type { MatchState } from 'core-war-engine';
import { GRID_COLS, CELL_SCALE } from './constants';

const OPCODE_NAMES = [
  'DAT', 'MOV', 'ADD', 'SUB', 'MUL', 'DIV', 'MOD',
  'JMP', 'JMZ', 'JMN', 'DJN', 'SPL', 'SLT', 'SEQ', 'SNE', 'NOP',
];

const MODIFIER_NAMES = ['A', 'B', 'AB', 'BA', 'F', 'X', 'I'];

const MODE_PREFIXES = ['#', '$', '*', '@', '{', '}', '<', '>'];

/** Format a single core cell as a Redcode instruction string.
 *  Example: "MOV.I $0, $1"
 */
export function formatInstruction(
  opcode: number,
  modifier: number,
  aMode: number,
  aValue: number,
  bMode: number,
  bValue: number,
): string {
  const op = OPCODE_NAMES[opcode] ?? '???';
  const mod = MODIFIER_NAMES[modifier] ?? '?';
  const a = `${MODE_PREFIXES[aMode] ?? '?'}${aValue}`;
  const b = `${MODE_PREFIXES[bMode] ?? '?'}${bValue}`;
  return `${op}.${mod} ${a}, ${b}`;
}

/** Convert pixel coordinates (relative to the grid container) to a core
 *  cell address, or -1 if the coordinates are outside the grid. */
export function cellAddressAtPixel(x: number, y: number): number {
  const col = Math.floor(x / CELL_SCALE);
  const row = Math.floor(y / CELL_SCALE);
  if (col < 0 || col >= GRID_COLS || row < 0 || row >= GRID_COLS) return -1;
  return row * GRID_COLS + col;
}

/** Read a cell from the match state and return a formatted tooltip string.
 *  Example: "#0042  ADD.AB #4, $3"
 */
export function formatCellTooltip(match: MatchState, addr: number): string {
  const instr = formatInstruction(
    match.cellOpcode(addr),
    match.cellModifier(addr),
    match.cellAMode(addr),
    match.cellAValue(addr),
    match.cellBMode(addr),
    match.cellBValue(addr),
  );
  return `#${String(addr).padStart(4, '0')}  ${instr}`;
}
