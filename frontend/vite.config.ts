import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

export default defineConfig({
  plugins: [react()],
  optimizeDeps: {
    // The wasm-pack output uses import.meta.url to locate the .wasm file.
    // Vite's dep optimizer (esbuild) can't handle this, so we exclude
    // the engine package from pre-bundling to preserve the original URL
    // resolution at runtime.
    exclude: ['core-war-engine'],
  },
});
