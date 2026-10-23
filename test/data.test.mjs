import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseAnthropic } from '../scripts/parse/anthropic.mjs';
import { parseOpenAI } from '../scripts/parse/openai.mjs';
import { parseGemini } from '../scripts/parse/gemini.mjs';
import { parseDeepSeek } from '../scripts/parse/deepseek.mjs';
import { ParseError } from '../scripts/parse/common.mjs';
import { toToml } from '../scripts/toml.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const page = (f) => readFileSync(join(here, 'pages', f), 'utf8');
const prices = JSON.parse(readFileSync(join(root, 'prices.json'), 'utf8'));
const get = (id) => prices.models.find((m) => m.model === id);

test('every row has a source URL, an ISO date, a unit, and numbers or null', () => {
  assert.ok(prices.models.length >= 70);
  const ids = new Set();
  for (const m of prices.models) {
    assert.ok(!ids.has(m.model), `duplicate ${m.model}`);
    ids.add(m.model);
    assert.match(m.source_url, /^https:\/\//);
    assert.match(m.checked, /^\d{4}-\d{2}-\d{2}$/);
    assert.equal(m.unit, 'USD per 1M tokens');
    for (const f of ['input', 'output', 'cache_read', 'batch_input', 'batch_output']) {
      assert.ok(m[f] === null || (typeof m[f] === 'number' && m[f] > 0), `${m.model}.${f}`);
      if (m[f] === null) assert.match(m.note, new RegExp(`no price: .*${f}`), `${m.model} explains null ${f}`);
    }
  }
});

test('the values the drift bugs in the wild got wrong', () => {
  // Cache reads at 0.05x on Sonnet 5.5 and Opus 5.5, 0.025x on Fable 5.1.
  assert.equal(get('claude-sonnet-5-5').cache_read, 0.1);
  assert.equal(get('claude-opus-5-5').cache_read, 0.2);
  assert.equal(get('claude-fable-5-1').cache_read, 0.25);
  assert.equal(get('claude-fable-5').cache_read, 1);
  // Haiku 5.5 is tiered at 100,000 prompt tokens.
  const h = get('claude-haiku-5-5');
  assert.deepEqual([h.input, h.output, h.cache_read], [0.1, 0.5, 0.01]);
  assert.equal(h.long_context.threshold_tokens, 100000);
  assert.deepEqual([h.long_context.input, h.long_context.output, h.long_context.cache_read], [0.5, 2.5, 0.05]);
  assert.deepEqual(h.cache_write, { '5m': 0.125, '1h': 0.2 });
  // Sonnet 5 stayed at $2/$10.
  assert.deepEqual([get('claude-sonnet-5').input, get('claude-sonnet-5').output], [2, 10]);
});

test('TOML output has one [[models]] per row and no nulls', () => {
  const toml = toToml(prices);
  assert.equal((toml.match(/^\[\[models\]\]$/gm) || []).length, prices.models.length);
  assert.ok(!/null/.test(toml.replace(/"[^"]*"/g, '')));
  assert.equal(readFileSync(join(root, 'prices.toml'), 'utf8'), toml, 'prices.toml is regenerated from prices.json');
});

test('no em dashes in shipped text', () => {
  for (const f of ['README.md', 'action.yml', 'prices.json', 'prices.toml', 'crate/README.md']) {
    assert.ok(!readFileSync(join(root, f), 'utf8').includes('\u2014'), f);
  }
});

test('anthropic parser reads the saved page', () => {
  const { rows, unknown } = parseAnthropic(page('anthropic.md'), '2026-10-23');
  assert.deepEqual(unknown, []);
  const opus = rows.find((r) => r.model === 'claude-opus-5-5');
  assert.deepEqual([opus.input, opus.output, opus.cache_read, opus.batch_input, opus.batch_output], [4, 20, 0.2, 2, 10]);
  assert.deepEqual(opus.cache_write, { '5m': 5, '1h': 8 });
  assert.deepEqual(opus.other_tiers.fast, { input: 8, output: 40 });
  assert.equal(rows.find((r) => r.model === 'claude-opus-4-20250514').status, 'retired');
  assert.ok(!rows.some((r) => /mythos/i.test(r.name)), 'models without a public id are skipped');
});

test('anthropic parser fails loudly when a model disappears', () => {
  const md = page('anthropic.md').replace(/\| Claude Opus 4\.8 .*\n/g, '');
  assert.throws(() => parseAnthropic(md, '2026-10-23'), ParseError);
});

test('anthropic parser reports new names it has no id for', () => {
  const md = page('anthropic.md').replace('| Claude Opus 5.5 ', '| Claude Opus 9 | $1 / MTok | $1 / MTok | $1 / MTok | $1 / MTok | $1 / MTok |\n| Claude Opus 5.5 ');
  assert.deepEqual(parseAnthropic(md, '2026-10-23').unknown, ['Claude Opus 9']);
});

test('openai parser reads short and long context, batch, flex and fast', () => {
  const { rows } = parseOpenAI(page('openai.md'), '2026-10-23');
  const sol = rows.find((r) => r.model === 'gpt-6.1-sol');
  assert.deepEqual([sol.input, sol.output, sol.cache_read], [2, 10, 0.1]);
  assert.deepEqual(sol.cache_write, { '30m': 2.5, '12h': 5 });
  assert.equal(sol.long_context.threshold_tokens, 272000);
  assert.deepEqual([sol.long_context.input, sol.long_context.output], [4, 15]);
  assert.deepEqual([sol.batch_input, sol.batch_output], [1, 5]);
  assert.equal(sol.other_tiers.fast.input, 4);
  const mini = rows.find((r) => r.model === 'gpt-4o-mini');
  assert.deepEqual([mini.input, mini.output, mini.cache_read, mini.batch_input], [0.15, 0.6, 0.075, 0.075]);
  assert.equal(rows.filter((r) => r.model === 'gpt-5.6-sol').length, 1, 'the cyber table repeat is not a second row');
  assert.ok(rows.find((r) => r.model === 'gpt-5.5-pro').cache_read === null);
});

test('gemini parser reads promos, tiers and the 200k threshold', () => {
  const { rows } = parseGemini(page('gemini.md'), '2026-10-23');
  const argon = rows.find((r) => r.model === 'gemini-4-argon');
  assert.deepEqual([argon.input, argon.output, argon.cache_read], [2, 10, 0.1]);
  assert.deepEqual(argon.scheduled, [{ effective: '2026-12-01', input: 4, output: 20, cache_read: 0.2, batch_input: 2, batch_output: 10 }]);
  const pro = rows.find((r) => r.model === 'gemini-2.5-pro');
  assert.equal(pro.long_context.threshold_tokens, 200000);
  assert.deepEqual([pro.long_context.input, pro.long_context.output, pro.long_context.cache_read], [2.5, 15, 0.25]);
  assert.equal(rows.find((r) => r.model === 'gemini-2.5-flash').input, 0.3, 'text price, not the audio price');
});

test('deepseek parser reads peak and off-peak', () => {
  const { rows } = parseDeepSeek(page('deepseek.html'), '2026-10-23');
  const flash = rows.find((r) => r.model === 'deepseek-flash');
  assert.deepEqual([flash.input, flash.output, flash.cache_read], [0.3, 1.2, 0.006]);
  assert.deepEqual(flash.other_tiers.offpeak, { input: 0.15, output: 0.6, cache_read: 0.003 });
  assert.deepEqual(flash.aliases, ['deepseek-v4-flash', 'deepseek-v4-flash-vision-exp']);
  assert.equal(rows.find((r) => r.model === 'deepseek-v4-pro').input, 1.32);
});

test('committed prices.json equals what the parsers produce from the saved pages', () => {
  const fresh = [
    ...parseAnthropic(page('anthropic.md'), 'X').rows,
    ...parseOpenAI(page('openai.md'), 'X').rows,
    ...parseGemini(page('gemini.md'), 'X').rows,
    ...parseDeepSeek(page('deepseek.html'), 'X').rows,
  ].map(({ checked, ...r }) => r);
  const committed = prices.models.map(({ checked, ...r }) => r);
  assert.deepEqual(committed, fresh);
});
