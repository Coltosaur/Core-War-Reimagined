import { readdirSync } from 'node:fs';
import { join } from 'node:path';

// Returns the dist-relative path of every file or directory whose name
// starts with a dot (.env, .git, .DS_Store, ...). Vite copies public/
// verbatim, so a stray dotfile there would otherwise be deployed and served.
//
// There is deliberately no allowlist. If we ever publish /.well-known/
// (e.g. security.txt), add an explicit exception here and a test for it.
export function findDotPaths(dir: string, prefix = ''): string[] {
  const found: string[] = [];
  for (const entry of readdirSync(dir, { withFileTypes: true })) {
    const rel = prefix ? `${prefix}/${entry.name}` : entry.name;
    if (entry.name.startsWith('.')) {
      found.push(rel);
    } else if (entry.isDirectory()) {
      found.push(...findDotPaths(join(dir, entry.name), rel));
    }
  }
  return found;
}
