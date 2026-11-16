#!/usr/bin/env node
// Regenerates every file built from prices.json without fetching anything: prices.toml, the
// Rust crate's data (crate/prices.json, crate/src/data.rs), the Go module's embedded copy
// (go/prices.json), the Python package's data
// (python/src/llm_prices/_data.py) and the npm package's data (js/src/data.ts). Run it after
// editing a hand-checked row (see MANUAL_SOURCES in meta.mjs).
//
//   node scripts/build.mjs           rewrite the generated files
//   node scripts/build.mjs --check   exit 1 when a generated file is out of date (CI)

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { generatedFiles } from './outputs.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const text = readFileSync(join(root, 'prices.json'), 'utf8');
const doc = JSON.parse(text);
const files = generatedFiles(doc, text);

if (process.argv.includes('--check')) {
  const stale = Object.entries(files)
    .filter(([path, content]) => !existsSync(join(root, path)) || readFileSync(join(root, path), 'utf8') !== content)
    .map(([path]) => path);
  if (stale.length) {
    console.error(`out of date with prices.json (run node scripts/build.mjs):\n${stale.map((p) => `  ${p}`).join('\n')}`);
    process.exit(1);
  }
  console.log(`${Object.keys(files).length} generated files match prices.json (${doc.models.length} rows)`);
} else {
  for (const [path, content] of Object.entries(files)) writeFileSync(join(root, path), content);
  console.log(`rebuilt ${Object.keys(files).join(', ')} from ${doc.models.length} rows`);
}
