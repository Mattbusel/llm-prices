import { readdirSync, statSync } from 'node:fs';
import { join, relative, sep } from 'node:path';

const SKIP_DIRS = new Set(['.git', 'node_modules', 'target', 'dist', 'build', 'vendor', '.venv', 'venv', '__pycache__', '.next', '.tox', 'coverage', '.cache']);

/** Converts a glob (**, *, ?, {a,b}) to a RegExp over forward-slash relative paths. */
export function globToRegex(glob) {
  let g = glob.trim().replace(/^\.\//, '');
  if (g.endsWith('/')) g += '**';
  let re = '';
  for (let i = 0; i < g.length; i++) {
    const c = g[i];
    if (c === '*') {
      if (g[i + 1] === '*') {
        const slash = g[i + 2] === '/';
        re += slash ? '(?:.*/)?' : '.*';
        i += slash ? 2 : 1;
      } else re += '[^/]*';
    } else if (c === '?') re += '[^/]';
    else if (c === '{') {
      const j = g.indexOf('}', i);
      re += '(?:' + g.slice(i + 1, j).split(',').map((s) => s.replace(/[.+^$()|[\]\\]/g, '\\$&').replace(/\*/g, '[^/]*')).join('|') + ')';
      i = j;
    } else re += c.replace(/[.+^$()|[\]\\]/g, '\\$&');
  }
  return new RegExp('^' + re + '$');
}

export function splitList(s) {
  return String(s || '')
    .split(/[\n,]/)
    .map((x) => x.trim())
    .filter(Boolean);
}

/** Lists files under root matching any include glob and no exclude glob. */
export function listFiles(root, include, exclude, maxBytes = 1_000_000) {
  const inc = (include.length ? include : ['**']).map(globToRegex);
  const exc = exclude.map(globToRegex);
  const out = [];
  const walk = (dir) => {
    let entries;
    try {
      entries = readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }
    for (const e of entries) {
      const abs = join(dir, e.name);
      const rel = relative(root, abs).split(sep).join('/');
      if (e.isDirectory()) {
        if (SKIP_DIRS.has(e.name)) continue;
        if (exc.some((r) => r.test(rel) || r.test(rel + '/'))) continue;
        walk(abs);
      } else if (e.isFile()) {
        if (!inc.some((r) => r.test(rel))) continue;
        if (exc.some((r) => r.test(rel))) continue;
        try {
          if (statSync(abs).size > maxBytes) continue;
        } catch {
          continue;
        }
        out.push(rel);
      }
    }
  };
  walk(root);
  return out.sort();
}
