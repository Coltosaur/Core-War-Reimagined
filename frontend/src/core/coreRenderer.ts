// Pure PixiJS renderer for the core memory grid. No React dependency —
// the React wrapper (App.tsx) calls create/update/destroy imperatively.
//
// Rendering strategy: an offscreen 100×80 canvas holds one pixel per cell.
// Each frame, opcode colors are written into the canvas's ImageData buffer,
// then uploaded to a PixiJS texture on a single Sprite scaled up with
// NEAREST filtering for crisp pixel edges.
//
// Hot-path cost per frame:
//   - JS: 32KB of byte copies into ImageData (~0.1ms)
//   - Browser: one putImageData call
//   - GPU: one texture re-upload via baseTex.update()
// Total well under 1ms, leaving plenty of budget for 60fps.

import {
  Application,
  BaseTexture,
  Sprite,
  Texture,
  SCALE_MODES,
} from 'pixi.js';
import { CORE_SIZE, GRID_COLS, GRID_ROWS, CELL_SCALE } from './constants';
import { OPCODE_RGBA } from './opcodeColors';

export interface CoreRenderer {
  /** Write opcode colors into the grid. Call once per animation frame. */
  update(opcodes: Uint8Array): void;
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

  // PixiJS texture backed by the offscreen canvas. NEAREST scaling gives
  // crisp pixel edges when the sprite is scaled up.
  const baseTex = BaseTexture.from(offscreen, {
    scaleMode: SCALE_MODES.NEAREST,
  });
  const sprite = new Sprite(new Texture(baseTex));
  sprite.scale.set(CELL_SCALE);
  app.stage.addChild(sprite);

  return {
    update(opcodes: Uint8Array) {
      const data = imageData.data;
      for (let i = 0; i < CORE_SIZE; i++) {
        const src = opcodes[i] * 4;
        const dst = i * 4;
        data[dst] = OPCODE_RGBA[src];
        data[dst + 1] = OPCODE_RGBA[src + 1];
        data[dst + 2] = OPCODE_RGBA[src + 2];
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
