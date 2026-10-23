import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildIndex, resolveModel, scanText, classifyKey, unitHint, judge, missingPrices, scannable, isTestFile } from '../src/scan.mjs';
import { globToRegex } from '../src/files.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const prices = JSON.parse(readFileSync(join(here, '..', 'prices.json'), 'utf8'));
const index = buildIndex(prices);
const fx = join(here, 'fixtures');

function scanFixture(name, opts) {
  return scanText(name, readFileSync(join(fx, name), 'utf8'), index, opts);
}

function drift(name) {
  return scanFixture(name)
    .findings.filter((f) => !f.ok)
    .map((f) => `${f.line}:${f.model}:${f.field}`)
    .sort();
}

test('model ids resolve across prefixes, Bedrock ids, Vertex dates and undated Anthropic ids', () => {
  assert.equal(resolveModel(index, 'claude-opus-5-5').model, 'claude-opus-5-5');
  assert.equal(resolveModel(index, 'anthropic/claude-opus-5-5').model, 'claude-opus-5-5');
  assert.equal(resolveModel(index, 'us.anthropic.claude-sonnet-4-20250514-v1:0').model, 'claude-sonnet-4-20250514');
  assert.equal(resolveModel(index, 'claude-opus-4-1@20250805').model, 'claude-opus-4-1-20250805');
  assert.equal(resolveModel(index, 'claude-sonnet-4-5').model, 'claude-sonnet-4-5-20250929');
  assert.equal(resolveModel(index, 'claude-haiku-4-5-20251001').model, 'claude-haiku-4-5');
  assert.equal(resolveModel(index, 'models/gemini-2.5-pro').model, 'gemini-2.5-pro');
  assert.equal(resolveModel(index, 'deepseek-v4-flash').model, 'deepseek-flash');
  // OpenAI snapshots are priced separately; an unlisted snapshot must not borrow another row.
  assert.equal(resolveModel(index, 'gpt-4o-2024-08-06'), null);
  assert.equal(resolveModel(index, 'gpt-4o-2024-05-13').model, 'gpt-4o-2024-05-13');
  assert.equal(resolveModel(index, 'claude-3-opus-20240229'), null);
});

test('provider filter limits the index', () => {
  const only = buildIndex(prices, ['openai']);
  assert.equal(resolveModel(only, 'claude-opus-5-5'), null);
  assert.equal(resolveModel(only, 'gpt-4o').model, 'gpt-4o');
});

test('key classification', () => {
  assert.equal(classifyKey('input_per_mtok'), 'input');
  assert.equal(classifyKey('inputPerMillion'), 'input');
  assert.equal(classifyKey('prompt'), 'input');
  assert.equal(classifyKey('completion'), 'output');
  assert.equal(classifyKey('output_cost_per_token'), 'output');
  assert.equal(classifyKey('cache_read_input_token_cost'), 'cache_read');
  assert.equal(classifyKey('cache_creation_input_token_cost'), 'cache_write');
  assert.equal(classifyKey('cached_input'), 'cache_read');
  assert.equal(classifyKey('Cached'), 'cache_read');
  assert.equal(classifyKey('cache_write_1h'), 'cache_write');
  assert.equal(classifyKey('batch_input'), 'batch_input');
  assert.equal(classifyKey('input_tokens'), null);
  assert.equal(classifyKey('max_input_tokens'), null);
  assert.equal(classifyKey('contextWindow'), null);
  assert.equal(classifyKey('temperature'), null);
});

test('unit hints', () => {
  assert.equal(unitHint('input_cost_per_token'), 'T');
  assert.equal(unitHint('# Prices in USD per 1K tokens.'), 'K');
  assert.equal(unitHint('price_per_1k_input'), 'K');
  assert.equal(unitHint('input_per_mtok'), 'M');
  assert.equal(unitHint('// USD per million tokens'), 'M');
  assert.equal(unitHint('input'), null);
});

test('judge reads per-1K, per-1M and per-token numbers', () => {
  const row = prices.models.find((m) => m.model === 'gpt-4o');
  assert.equal(judge(row, 'input', 2.5, null, 0.01).ok, true);
  assert.equal(judge(row, 'input', 0.0025, null, 0.01).ok, true);
  assert.equal(judge(row, 'input', 2.5e-6, null, 0.01).ok, true);
  assert.equal(judge(row, 'input', 0.0025, 'K', 0.01).unit, 'K');
  const bad = judge(row, 'input', 0.005, 'K', 0.01);
  assert.equal(bad.ok, false);
  assert.equal(bad.perM, 5);
  // A context window is not a price.
  assert.equal(judge(row, 'input', 128000, null, 0.01), null);
  assert.equal(judge(row, 'input', 0, null, 0.01), null);
});

test('judge accepts batch, long-context, other tiers, scheduled and 1.1x regional prices', () => {
  const opus = prices.models.find((m) => m.model === 'claude-opus-5-5');
  assert.equal(judge(opus, 'input', 2, 'M', 0.01).ok, true); // batch
  assert.equal(judge(opus, 'input', 8, 'M', 0.01).ok, true); // fast mode
  assert.equal(judge(opus, 'input', 4.4, 'M', 0.01).ok, true); // US-only 1.1x
  const argon = prices.models.find((m) => m.model === 'gemini-4-argon');
  assert.equal(judge(argon, 'input', 4, 'M', 0.01).ok, true); // published Dec 1 price
  const ds = prices.models.find((m) => m.model === 'deepseek-flash');
  assert.equal(judge(ds, 'input', 0.15, 'M', 0.01).ok, true); // off-peak
  const o4 = prices.models.find((m) => m.model === 'o4-mini');
  // 1.1x applies to standard prices only, not to other tiers (0.5 fast x 1.1 = 0.55).
  assert.equal(judge(o4, 'cache_read', 0.55, 'M', 0.01).ok, false);
});

test('tolerance', () => {
  const row = prices.models.find((m) => m.model === 'claude-sonnet-5-5');
  assert.equal(judge(row, 'input', 2.03, 'M', 0.01).ok, false);
  assert.equal(judge(row, 'input', 2.03, 'M', 0.02).ok, true);
});

test('rust match arms: struct literals and (input, output) tuples', () => {
  assert.deepEqual(drift('rust_match.rs'), ['14:claude-sonnet-5-5:cache_read', '38:claude-sonnet-5:input', '38:claude-sonnet-5:output']);
  const r = scanFixture('rust_match.rs');
  assert.ok(r.priced.has('claude-opus-4-1-20250805'), 'undated claude-opus-4-1 resolves');
  assert.ok(r.priced.has('claude-haiku-4-5'));
});

test('python dict in per-1K units', () => {
  assert.deepEqual(drift('pricing.py'), ['6:gpt-5:output']);
  const f = scanFixture('pricing.py').findings.find((x) => x.model === 'gpt-5' && x.field === 'output');
  assert.equal(f.unit, 'K');
  assert.equal(f.perM, 12);
  assert.equal(f.expected, 10);
});

test('LiteLLM-shaped JSON with per-token costs and above-200k keys', () => {
  assert.deepEqual(drift('litellm_like.json'), ['22:deepseek-flash:input']);
  const all = scanFixture('litellm_like.json').findings;
  assert.ok(all.every((f) => f.unit === 'T'));
  assert.ok(!all.some((f) => f.written === '200000' || f.written === '64000'), 'token limits are not prices');
});

test('YAML maps and list items', () => {
  assert.deepEqual(drift('prices.yaml'), ['12:gemini-2.5-flash:output', '20:gpt-6-luna:output']);
  assert.ok(scanFixture('prices.yaml').priced.has('gpt-6.1-sol'));
});

test('TOML tables and arrays of tables', () => {
  assert.deepEqual(drift('models.toml'), ['12:claude-fable-5-1:cache_read']);
  const p = scanFixture('models.toml').priced;
  assert.ok(p.has('gpt-5.4-mini') && p.has('gemini-2.5-flash-lite') && p.has('claude-opus-4-8'));
});

test('TypeScript records with the id as a field', () => {
  assert.deepEqual(drift('costs.ts'), ['21:gpt-5.6-terra:output']);
});

test('Go map of struct literals', () => {
  assert.deepEqual(drift('pricing.go'), ['13:o4-mini:cache_read']);
});

test('literals divided down to per-token', () => {
  assert.deepEqual(drift('divide.rs'), ['7:claude-opus-5:output']);
  const f = scanFixture('divide.rs').findings.find((x) => x.line === 6);
  assert.equal(f.unit, 'M');
});

test('no false positives on limits, usage counts, model lists and settings', () => {
  const r = scanFixture('noise.ts');
  assert.deepEqual(r.findings, []);
  assert.ok(r.seen.has('claude-sonnet-5-5'));
});

test('missing prices are reported only for providers the repo prices', () => {
  const a = scanText('a.py', 'PRICES = {"gpt-4o": {"input": 2.5, "output": 10.0}}\n', index);
  const b = scanText('b.py', 'client.create(model="gpt-5.4-mini")\nother(model="claude-opus-5-5")\n', index);
  const seen = new Map([...a.seen, ...b.seen]);
  const priced = new Set([...a.priced, ...b.priced]);
  const miss = missingPrices(seen, priced, new Set(['openai']));
  assert.deepEqual(miss.map((m) => m.model), ['gpt-5.4-mini']);
});

test('every fixture drift is marked as stale in a comment, and nothing else is', () => {
  for (const name of readdirSync(fx).filter((n) => !n.endsWith('.json'))) {
    const lines = readFileSync(join(fx, name), 'utf8').split(/\r?\n/);
    const flagged = new Set(scanFixture(name).findings.filter((f) => !f.ok).map((f) => f.line));
    const marked = new Set(lines.map((l, i) => (/stale/.test(l) ? i + 1 : 0)).filter(Boolean));
    assert.deepEqual([...flagged].sort(), [...marked].sort(), name);
  }
});

test('file filters', () => {
  assert.ok(scannable('src/pricing.rs'));
  assert.ok(scannable('config/models.yaml'));
  assert.ok(!scannable('package-lock.json'));
  assert.ok(!scannable('README.md'));
  assert.ok(globToRegex('**').test('a/b/c.py'));
  assert.ok(globToRegex('src/**').test('src/x/y.ts'));
  assert.ok(!globToRegex('src/**').test('lib/x.ts'));
  assert.ok(globToRegex('**/*.{py,ts}').test('a/b.ts'));
  assert.ok(globToRegex('**/*.py').test('top.py'));
});

test('calls, logs and usage records with numbers are not read as prices', () => {
  assert.deepEqual(scanFixture('noise_calls.rs').findings, []);
});

test('Rust #[cfg(test)] modules are skipped unless include-tests is on', () => {
  const on = scanFixture('noise_calls.rs', { includeTests: true }).findings.filter((f) => !f.ok);
  assert.deepEqual(on.map((f) => `${f.line}:${f.model}:${f.field}`).sort(), ['15:gpt-4o:input', '15:gpt-4o:output', '16:claude-sonnet-5-5:input', '16:claude-sonnet-5-5:output']);
});

test('test files are recognised', () => {
  for (const f of ['tests/a.rs', 'src/__tests__/p.ts', 'pkg/price_test.go', 'test_costs.py', 'web/cost.spec.ts', 'benches/b.rs', 'a/conftest.py']) assert.ok(isTestFile(f), f);
  for (const f of ['src/pricing.rs', 'lib/latest.py', 'src/contest.ts']) assert.ok(!isTestFile(f), f);
});

test('a match arm or case label for several models checks each model', () => {
  assert.deepEqual(drift('group_arms.rs'), ['9:claude-opus-5-5:input', '9:claude-opus-5-5:output']);
  const r = scanFixture('group_arms.rs');
  for (const m of ['claude-opus-4-7', 'claude-opus-4-6', 'claude-opus-4-5', 'claude-opus-5']) assert.ok(r.priced.has(m), m);
  assert.deepEqual(drift('group_case.go'), []);
  const g = scanFixture('group_case.go');
  for (const m of ['gpt-5', 'gpt-5.1', 'gpt-5-mini']) assert.ok(g.priced.has(m), m);
});
