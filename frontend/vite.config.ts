import { copyFileSync } from 'node:fs';
import { join, resolve } from 'node:path';
import type { Plugin } from 'vite';
// vitest/config's defineConfig is Vite's plus the `test` block.
import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import { findDotPaths } from './src/deploy/distGuard';

// Cloudflare Pages serves dist/404.html, with a real 404 status, for any
// path that public/_redirects doesn't rewrite. Making it a copy of the
// built index.html means the SPA still boots on a 404, so visitors get the
// full app shell and the NotFoundPage instead of a bare error.
//
// It also fails the build if dist/ contains any dotfile (.env, .git, ...).
// Scanners probe those paths constantly; failing here means a leaked secret
// can't reach a deploy, rather than relying on a 404 that a stray file
// would silently override.
function pagesDeployOutput(): Plugin {
  let outDir = 'dist';
  return {
    name: 'pages-deploy-output',
    apply: 'build',
    configResolved(config) {
      outDir = resolve(config.root, config.build.outDir);
    },
    closeBundle() {
      const dotPaths = findDotPaths(outDir);
      if (dotPaths.length > 0) {
        throw new Error(
          `Refusing to build: dist/ contains dotfiles that would be publicly served: ${dotPaths.join(', ')}. ` +
            'Remove them from public/. (Vite never empties dist/.git, so a stale one needs deleting by hand.)',
        );
      }
      copyFileSync(join(outDir, 'index.html'), join(outDir, '404.html'));
    },
  };
}

export default defineConfig({
  plugins: [react(), pagesDeployOutput()],
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
