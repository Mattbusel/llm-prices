// The repository root is itself an ES module package (npm i github:Mattbusel/llm-prices#v1,
// jsDelivr, esm.sh): index.mjs is the npm package's build and data.mjs holds the rows.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { GENERATED, PRICES_VERSION, all, lookup, lookupProvider } from '../index.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const prices = JSON.parse(readFileSync(join(root, 'prices.json'), 'utf8'));
const pkg = JSON.parse(readFileSync(join(root, 'package.json'), 'utf8'));

test('the root module serves the whole table', () => {
  assert.equal(GENERATED, prices.generated);
  const text = readFileSync(join(root, 'prices.json'), 'utf8').replace(/\r\n/g, '\n');
  assert.equal(PRICES_VERSION, `sha256:${createHash('sha256').update(text, 'utf8').digest('hex')}`);
  assert.equal(all().length, prices.models.length);
  assert.equal(lookup('claude-sonnet-4-6').input, 3);
  assert.equal(lookupProvider('deepseek', 'deepseek-chat').model, 'deepseek-flash');
  assert.equal(lookupProvider('groq', 'openai/gpt-oss-120b').provider, 'groq');
});

test('package.json points at the root module and ships only what it needs', () => {
  assert.equal(pkg.exports['.'].default, './index.mjs');
  assert.equal(pkg.exports['.'].types, './index.d.ts');
  assert.deepEqual(pkg.files.sort(), ['LICENSE', 'data.d.mts', 'data.mjs', 'index.d.ts', 'index.mjs', 'prices.json']);
  for (const f of ['crate/Cargo.toml', 'python/pyproject.toml']) {
    assert.match(readFileSync(join(root, f), 'utf8'), new RegExp(`^version = "${pkg.version.replace(/\./g, '\\.')}"$`, 'm'), f);
  }
  assert.equal(JSON.parse(readFileSync(join(root, 'js', 'package.json'), 'utf8')).version, pkg.version);
});
