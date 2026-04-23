/// <reference types="vitest" />
import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  server: {
    fs: {
      // The wasm-pack output (engine/pkg/) lives outside frontend/.
      // Allow Vite to serve files from the repo root so the /@fs/ request
      // for core_war_engine_bg.wasm doesn't get a 403.
      allow: ['..'],
    },
  },
  optimizeDeps: {
    // The wasm-pack output uses import.meta.url to locate the .wasm file.
    // Vite's dep optimizer (esbuild) can't handle this, so we exclude
    // the engine package from pre-bundling to preserve the original URL
    // resolution at runtime.
    exclude: ['core-war-engine'],
  },
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: './src/test/setup.ts',
    alias: {
      'core-war-engine': new URL('./src/test/__mocks__/core-war-engine.ts', import.meta.url)
        .pathname,
    },
  },
});
