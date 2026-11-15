#!/usr/bin/env node
// Rewrites prices.toml, crate/prices.json and crate/src/data.rs from prices.json without
// fetching anything. Run it after editing a hand-checked row (see MANUAL_SOURCES in meta.mjs).
//
//   node scripts/build.mjs

import { readFileSync, writeFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { toToml } from './toml.mjs';
import { toRust } from './rust.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const text = readFileSync(join(root, 'prices.json'), 'utf8');
const doc = JSON.parse(text);
writeFileSync(join(root, 'prices.toml'), toToml(doc));
writeFileSync(join(root, 'crate', 'src', 'data.rs'), toRust(doc));
writeFileSync(join(root, 'crate', 'prices.json'), text);
console.log(`rebuilt from ${doc.models.length} rows`);
