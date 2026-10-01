// @vitest-environment node
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { afterEach, beforeEach, describe, expect, it } from 'vitest';
import { findDotPaths } from './distGuard';

describe('findDotPaths', () => {
  let dir: string;

  beforeEach(() => {
    dir = mkdtempSync(join(tmpdir(), 'dist-guard-'));
  });

  afterEach(() => {
    rmSync(dir, { recursive: true, force: true });
  });

  it('accepts a normal build output', () => {
    mkdirSync(join(dir, 'assets'));
    writeFileSync(join(dir, 'index.html'), '');
    writeFileSync(join(dir, '404.html'), '');
    writeFileSync(join(dir, '_headers'), '');
    writeFileSync(join(dir, '_redirects'), '');
    writeFileSync(join(dir, 'robots.txt'), '');
    writeFileSync(join(dir, 'assets', 'index-abc.js'), '');

    expect(findDotPaths(dir)).toEqual([]);
  });

  it('finds dotfiles and dot-directories at any depth', () => {
    mkdirSync(join(dir, 'assets', 'nested'), { recursive: true });
    mkdirSync(join(dir, '.git'));
    writeFileSync(join(dir, '.git', 'config'), '');
    writeFileSync(join(dir, '.env'), '');
    writeFileSync(join(dir, 'assets', 'nested', '.DS_Store'), '');

    expect(findDotPaths(dir).sort()).toEqual(['.env', '.git', 'assets/nested/.DS_Store']);
  });
});
