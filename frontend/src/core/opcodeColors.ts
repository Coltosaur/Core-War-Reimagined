// Opcode color palette for the core visualizer. Grouped by instruction
// category for visual coherence against a dark background.
//
// Encoding: OPCODE_RGBA is a flat Uint8Array of 16 * 4 = 64 bytes.
// Access: OPCODE_RGBA[opcode * 4 + channel] where channel is 0=R, 1=G, 2=B, 3=A.
// This avoids object allocation in the hot render path.
//
// Opcode discriminants (from engine #[repr(u8)]):
//   DAT=0  MOV=1  ADD=2  SUB=3  MUL=4  DIV=5  MOD=6
//   JMP=7  JMZ=8  JMN=9  DJN=10 SPL=11 SLT=12 SEQ=13 SNE=14 NOP=15

const OPCODE_RGB: [number, number, number][] = [
  [26, 26, 46], // 0  DAT — near-black (dead/empty cells dominate, should recede)
  [233, 69, 96], // 1  MOV — red (aggressive, the most common warrior action)
  [15, 155, 88], // 2  ADD — green (arithmetic)
  [13, 122, 70], // 3  SUB — darker green
  [52, 168, 83], // 4  MUL — brighter green
  [27, 107, 58], // 5  DIV — deep green
  [45, 138, 86], // 6  MOD — mid green
  [79, 195, 247], // 7  JMP — light blue (unconditional jump)
  [41, 182, 246], // 8  JMZ — blue (conditional jump)
  [2, 136, 209], // 9  JMN — darker blue
  [2, 119, 189], // 10 DJN — deep blue (loop/decrement jump)
  [255, 171, 0], // 11 SPL — amber/gold (split is special, stands out)
  [206, 147, 216], // 12 SLT — light purple (comparison)
  [171, 71, 188], // 13 SEQ — purple
  [123, 31, 162], // 14 SNE — dark purple
  [51, 51, 51], // 15 NOP — dark gray (no-op, subtle)
];

export const OPCODE_RGBA = new Uint8Array(16 * 4);
for (let i = 0; i < 16; i++) {
  const [r, g, b] = OPCODE_RGB[i];
  OPCODE_RGBA[i * 4] = r;
  OPCODE_RGBA[i * 4 + 1] = g;
  OPCODE_RGBA[i * 4 + 2] = b;
  OPCODE_RGBA[i * 4 + 3] = 255;
}

// Hex colors for non-hot-path uses (legends, UI elements).
export const OPCODE_HEX: number[] = OPCODE_RGB.map(([r, g, b]) => (r << 16) | (g << 8) | b);
