// Pure PixiJS renderer for the core memory grid. No React dependency —
// the React wrapper (App.tsx) calls create/update/destroy imperatively.
//
// Coloring strategy: cells are colored by **which warrior last wrote to
// them** (cell ownership), not by opcode. This is the standard Core War
// visualization approach — it makes bombs visible, shows territory, and
// distinguishes the Imp's trail from the Dwarf's code.
//
// Rendering: an offscreen 100×80 canvas holds one pixel per cell. Each
// frame, ownership bytes are mapped to RGBA warrior colors via a flat
// lookup table, written into the canvas's ImageData, then uploaded to a
// PixiJS texture on a single Sprite scaled up with NEAREST filtering.

import { Application, BaseTexture, Sprite, Texture, SCALE_MODES } from 'pixi.js';
import { CORE_SIZE, GRID_COLS, GRID_ROWS, CELL_SCALE } from './constants';

// Warrior color palette: index = ownership value from the engine.
// 0 = unowned (dark), 1 = warrior 0, 2 = warrior 1, ...
// Pre-computed as flat RGBA for zero-allocation hot-path access.
const WARRIOR_COLORS: [number, number, number][] = [
  [17, 17, 23], // 0: unowned — near-black
  [233, 69, 96], // 1: warrior 0 — red/warm
  [79, 195, 247], // 2: warrior 1 — blue/cool
  [76, 175, 80], // 3: warrior 2 — green (future)
  [255, 171, 0], // 4: warrior 3 — amber (future)
];

const WARRIOR_RGBA = new Uint8Array(WARRIOR_COLORS.length * 4);
for (let i = 0; i < WARRIOR_COLORS.length; i++) {
  const [r, g, b] = WARRIOR_COLORS[i];
  WARRIOR_RGBA[i * 4] = r;
  WARRIOR_RGBA[i * 4 + 1] = g;
  WARRIOR_RGBA[i * 4 + 2] = b;
  WARRIOR_RGBA[i * 4 + 3] = 255;
}

export interface CoreRenderer {
  /** Write ownership colors into the grid. Call once per animation frame. */
  update(ownership: Uint8Array): void;
  /** Tear down the PixiJS app and remove the canvas from the DOM. */
  destroy(): void;
}

export function createCoreRenderer(container: HTMLElement): CoreRenderer {
  const app = new Application({
    width: GRID_COLS * CELL_SCALE,
    height: GRID_ROWS * CELL_SCALE,
    backgroundColor: 0x0a0a0a,
    antialias: false,
  });
  container.appendChild(app.view as HTMLCanvasElement);

  // Offscreen canvas: one pixel per core cell.
  const offscreen = document.createElement('canvas');
  offscreen.width = GRID_COLS;
  offscreen.height = GRID_ROWS;
  const ctx = offscreen.getContext('2d')!;
  const imageData = ctx.createImageData(GRID_COLS, GRID_ROWS);

  // PixiJS texture backed by the offscreen canvas.
  const baseTex = BaseTexture.from(offscreen, {
    scaleMode: SCALE_MODES.NEAREST,
  });
  const sprite = new Sprite(new Texture(baseTex));
  sprite.scale.set(CELL_SCALE);
  app.stage.addChild(sprite);

  return {
    update(ownership: Uint8Array) {
      const data = imageData.data;
      for (let i = 0; i < CORE_SIZE; i++) {
        // Clamp owner to the palette size to avoid OOB reads.
        const owner = Math.min(ownership[i], WARRIOR_COLORS.length - 1);
        const src = owner * 4;
        const dst = i * 4;
        data[dst] = WARRIOR_RGBA[src];
        data[dst + 1] = WARRIOR_RGBA[src + 1];
        data[dst + 2] = WARRIOR_RGBA[src + 2];
        data[dst + 3] = 255;
      }
      ctx.putImageData(imageData, 0, 0);
      baseTex.update();
    },

    destroy() {
      app.destroy(true, { children: true, texture: true, baseTexture: true });
    },
  };
}
