export type CheatEntry = { symbol: string; name: string; desc: string };

export const OPCODES: CheatEntry[] = [
  { symbol: 'DAT', name: 'data', desc: 'Non-executable. Running it kills the process.' },
  { symbol: 'MOV', name: 'move', desc: 'Copy source to destination.' },
  { symbol: 'ADD', name: 'add', desc: 'dst += src.' },
  { symbol: 'SUB', name: 'subtract', desc: 'dst -= src.' },
  { symbol: 'MUL', name: 'multiply', desc: 'dst *= src.' },
  { symbol: 'DIV', name: 'divide', desc: 'dst /= src. Divide-by-zero kills the process.' },
  { symbol: 'MOD', name: 'modulo', desc: 'dst %= src. Mod-by-zero kills the process.' },
  { symbol: 'JMP', name: 'jump', desc: 'Jump to the A operand address.' },
  {
    symbol: 'JMZ',
    name: 'jump if zero',
    desc: 'Jump if the selected field(s) of B operand are zero.',
  },
  {
    symbol: 'JMN',
    name: 'jump if non-zero',
    desc: 'Jump if any selected field of B operand is non-zero.',
  },
  {
    symbol: 'DJN',
    name: 'decrement & jump if non-zero',
    desc: 'Decrement B operand field(s), then jump if non-zero.',
  },
  {
    symbol: 'SPL',
    name: 'split',
    desc: 'Spawn a new process at the A operand address. Original continues.',
  },
  { symbol: 'SEQ', name: 'skip if equal', desc: 'Skip the next instruction if A == B.' },
  { symbol: 'SNE', name: 'skip if not equal', desc: 'Skip the next instruction if A != B.' },
  { symbol: 'SLT', name: 'skip if less than', desc: 'Skip the next instruction if A < B.' },
  { symbol: 'NOP', name: 'no-op', desc: 'Do nothing. Advance PC by 1.' },
];

export const MODIFIERS: CheatEntry[] = [
  {
    symbol: '.A',
    name: 'A-field',
    desc: 'Operate on A-field of A operand → A-field of B operand.',
  },
  {
    symbol: '.B',
    name: 'B-field',
    desc: 'Operate on B-field of A operand → B-field of B operand.',
  },
  { symbol: '.AB', name: 'A → B', desc: 'A-field of source into B-field of destination.' },
  { symbol: '.BA', name: 'B → A', desc: 'B-field of source into A-field of destination.' },
  { symbol: '.F', name: 'fields', desc: 'Both fields in parallel: A→A and B→B.' },
  { symbol: '.X', name: 'cross', desc: 'Both fields crossed: A→B and B→A.' },
  {
    symbol: '.I',
    name: 'instruction',
    desc: 'Copy/compare the whole instruction, not just fields.',
  },
];

export const ADDRESSING_MODES: CheatEntry[] = [
  { symbol: '#', name: 'immediate', desc: 'Value used directly; no dereference.' },
  { symbol: '$', name: 'direct', desc: 'Offset from the current PC (the default if no prefix).' },
  { symbol: '*', name: 'A-indirect', desc: 'Follow cell\u2019s A-field to get the real address.' },
  { symbol: '@', name: 'B-indirect', desc: 'Follow cell\u2019s B-field to get the real address.' },
  { symbol: '{', name: 'A-predecrement', desc: 'Decrement A-field first, then use it as address.' },
  { symbol: '<', name: 'B-predecrement', desc: 'Decrement B-field first, then use it as address.' },
  { symbol: '}', name: 'A-postincrement', desc: 'Use A-field as address, then increment it.' },
  { symbol: '>', name: 'B-postincrement', desc: 'Use B-field as address, then increment it.' },
];

export const PSEUDO_OPS: CheatEntry[] = [
  { symbol: 'ORG', name: 'origin', desc: 'Set the starting address label for execution.' },
  { symbol: 'END', name: 'end', desc: 'Marks end of source. Optional label sets start like ORG.' },
  { symbol: 'EQU', name: 'equate', desc: 'Define a named constant: `name EQU value`.' },
];
