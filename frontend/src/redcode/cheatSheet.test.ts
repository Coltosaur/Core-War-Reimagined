import { describe, expect, it } from 'vitest';
import { OPCODES, MODIFIERS, ADDRESSING_MODES, PSEUDO_OPS } from './cheatSheet';

describe('cheatSheet data', () => {
  it('has all 16 ICWS 94 opcodes', () => {
    expect(OPCODES).toHaveLength(16);
    const symbols = OPCODES.map((e) => e.symbol);
    expect(symbols).toContain('DAT');
    expect(symbols).toContain('MOV');
    expect(symbols).toContain('SPL');
    expect(symbols).toContain('NOP');
    expect(symbols).toContain('SEQ');
    expect(symbols).toContain('SNE');
    expect(symbols).toContain('SLT');
  });

  it('has all 7 modifiers', () => {
    expect(MODIFIERS).toHaveLength(7);
    const symbols = MODIFIERS.map((e) => e.symbol);
    expect(symbols).toContain('.A');
    expect(symbols).toContain('.I');
    expect(symbols).toContain('.F');
  });

  it('has all 8 addressing modes', () => {
    expect(ADDRESSING_MODES).toHaveLength(8);
    const symbols = ADDRESSING_MODES.map((e) => e.symbol);
    expect(symbols).toContain('#');
    expect(symbols).toContain('$');
    expect(symbols).toContain('@');
    expect(symbols).toContain('<');
    expect(symbols).toContain('>');
  });

  it('has pseudo-ops', () => {
    expect(PSEUDO_OPS.length).toBeGreaterThanOrEqual(2);
    const symbols = PSEUDO_OPS.map((e) => e.symbol);
    expect(symbols).toContain('ORG');
    expect(symbols).toContain('END');
  });

  it('every entry has non-empty symbol, name, and desc', () => {
    const allEntries = [...OPCODES, ...MODIFIERS, ...ADDRESSING_MODES, ...PSEUDO_OPS];
    for (const entry of allEntries) {
      expect(entry.symbol.length).toBeGreaterThan(0);
      expect(entry.name.length).toBeGreaterThan(0);
      expect(entry.desc.length).toBeGreaterThan(0);
    }
  });

  it('has no duplicate symbols within each category', () => {
    for (const group of [OPCODES, MODIFIERS, ADDRESSING_MODES, PSEUDO_OPS]) {
      const symbols = group.map((e) => e.symbol);
      expect(new Set(symbols).size).toBe(symbols.length);
    }
  });
});
