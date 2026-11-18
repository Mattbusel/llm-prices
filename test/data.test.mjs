import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { parseAnthropic } from '../scripts/parse/anthropic.mjs';
import { parseOpenAI } from '../scripts/parse/openai.mjs';
import { parseGemini } from '../scripts/parse/gemini.mjs';
import { parseDeepSeek } from '../scripts/parse/deepseek.mjs';
import { parseBedrock, per1kToPer1m } from '../scripts/parse/bedrock.mjs';
import { parseOpenRouter, perTokenToPer1m } from '../scripts/parse/openrouter.mjs';
import { ParseError } from '../scripts/parse/common.mjs';
import { toToml } from '../scripts/toml.mjs';
import { toRust } from '../scripts/rust.mjs';
import { generatedFiles } from '../scripts/outputs.mjs';
import { pricesVersion } from '../scripts/version.mjs';
import { packageRows, toPython, toTs } from '../scripts/packages.mjs';
import { SOURCES, MANUAL_SOURCES } from '../scripts/meta.mjs';
import { mergeParsed, isDelisted } from '../scripts/merge.mjs';

const here = dirname(fileURLToPath(import.meta.url));
const root = join(here, '..');
const page = (f) => readFileSync(join(here, 'pages', f), 'utf8');
const prices = JSON.parse(readFileSync(join(root, 'prices.json'), 'utf8'));
const get = (id) => prices.models.find((m) => m.model === id);

test('every row has a source URL, an ISO date, a unit, and numbers or null', () => {
  assert.ok(prices.models.length >= 70);
  // Hosts sell the same open-weight id (openai/gpt-oss-120b), so ids are unique per provider.
  const ids = new Set();
  for (const m of prices.models) {
    const key = `${m.provider}/${m.model}`;
    assert.ok(!ids.has(key), `duplicate ${key}`);
    ids.add(key);
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

test('anthropic parser drops a model that left the page, and fails loudly when most did', () => {
  const md = page('anthropic.md').replace(/\| Claude Opus 4\.8 .*\n/g, '');
  assert.ok(!parseAnthropic(md, '2026-10-23').rows.some((r) => r.model === 'claude-opus-4-8'));
  const gutted = page('anthropic.md').replace(/\| Claude (Opus|Sonnet|Haiku) .*\n/g, '');
  assert.throws(() => parseAnthropic(gutted, '2026-10-23'), ParseError);
});

test('a model that leaves its page is kept with its last prices, marked retired, once', () => {
  const old = [
    { provider: 'openai', model: 'gpt-4o', status: 'listed', input: 2.5, checked: '2026-10-23', note: null },
    { provider: 'openai', model: 'o1', status: 'listed', input: 15, checked: '2026-10-23', note: 'n' },
    { provider: 'google', model: 'gemini-x', status: 'listed', input: 1, checked: '2026-10-23', note: null },
  ];
  const fresh = [{ provider: 'openai', model: 'gpt-4o', status: 'listed', input: 2.5, checked: 'T', note: null }];
  const first = mergeParsed(['openai', 'google'], old, fresh, new Set(['google']), '2026-11-15');
  assert.deepEqual(first.delisted, ['openai/o1']);
  const o1 = first.merged.find((m) => m.model === 'o1');
  assert.equal(o1.status, 'retired');
  assert.equal(o1.input, 15);
  assert.equal(o1.checked, '2026-10-23');
  assert.equal(o1.note, 'no longer on the pricing page since 2026-11-15; prices as last listed (checked 2026-10-23); n');
  assert.ok(isDelisted(o1));
  assert.equal(first.merged.find((m) => m.model === 'gemini-x'), old[2], 'a failed page keeps its rows as they were');
  // The next run carries the retired row over unchanged.
  const second = mergeParsed(['openai', 'google'], first.merged, fresh, new Set(['google']), '2026-11-16');
  assert.deepEqual(second.delisted, []);
  assert.equal(second.merged.find((m) => m.model === 'o1'), o1);
});

test('the rows that left the provider pages are still in the table', () => {
  for (const id of ['claude-3-5-haiku-20241022', 'gpt-4o-2024-05-13', 'gpt-3.5-turbo', 'o1', 'o3-mini', 'gpt-4.1-nano']) {
    const m = get(id);
    assert.ok(m && isDelisted(m), id);
  }
  assert.deepEqual([get('gpt-4o-2024-05-13').input, get('gpt-4o-2024-05-13').output], [5, 15]);
  assert.deepEqual([get('gpt-3.5-turbo').input, get('gpt-3.5-turbo').output], [0.5, 1.5]);
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
  // Cyber and Life Sciences rows now sit in the Specialized table, as short and long context rows.
  const cyber = rows.find((r) => r.model === 'gpt-6-cyber');
  assert.deepEqual([cyber.input, cyber.output, cyber.cache_read], [20, 100, 2]);
  assert.deepEqual(cyber.cache_write, { '30m': 25, '12h': 50 });
  assert.deepEqual([cyber.long_context.input, cyber.long_context.output, cyber.long_context.cache_read], [40, 150, 4]);
  assert.equal(rows.find((r) => r.model === 'gpt-5.6-cyber').long_context, null, 'a long context row of dashes is no tier');
  assert.equal(rows.find((r) => r.model === 'chat-latest').cache_write, null);
  assert.ok(!rows.some((r) => r.model === 'omni-moderation-latest'), 'free rows are skipped');
});

test('openai parser reads both tier heading spellings and the Ultrafast promotion and list tables', () => {
  // 2026-11-17: "#### Batch" / "#### Flex" became "### Batch pricing data" / "### Flex pricing data".
  // Missing them nulled every OpenAI batch and flex price.
  const md = page('openai.md');
  assert.match(md, /^### Batch pricing data$/m);
  const old = md.replace('### Batch pricing data', '#### Batch').replace('### Flex pricing data', '#### Flex');
  for (const text of [md, old]) {
    const { rows } = parseOpenAI(text, 'X');
    const sol = rows.find((r) => r.model === 'gpt-6.1-sol');
    assert.deepEqual([sol.batch_input, sol.batch_output], [1, 5]);
    assert.deepEqual([sol.long_context.batch_input, sol.long_context.batch_output], [2, 7.5]);
    assert.deepEqual(sol.other_tiers.flex, { input: 1, output: 5, cache_read: 0.05 });
    assert.deepEqual([rows.find((r) => r.model === 'gpt-4o').batch_input, rows.find((r) => r.model === 'gpt-4o').batch_output], [1.25, 5]);
  }
  const { rows } = parseOpenAI(md, 'X');
  const astra = rows.find((r) => r.model === 'gpt-6.2-astra');
  assert.deepEqual([astra.input, astra.output, astra.cache_read, astra.batch_input, astra.batch_output], [8, 40, 0.4, 4, 20]);
  // The promotion table is the Ultrafast price in force; the list price goes in the note with its date.
  assert.deepEqual(astra.other_tiers.ultrafast, { input: 24, output: 120, cache_read: 1.2 });
  assert.match(astra.note, /from 2027-03-01 the page lists ultrafast at input 48, output 240, cached input 2\.4/);
  assert.deepEqual(rows.find((r) => r.model === 'gpt-6-astra').other_tiers.ultrafast, { input: 60, output: 300, cache_read: 6 });
  assert.throws(() => parseOpenAI(md.replace(/starting March 1, 2027/, 'later'), 'X'), /names no start date/);
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
  // The page source is Markdoc now ({% tab %}, {% table %}); names lose their {% id %} tag.
  const hydrogen = rows.find((r) => r.model === 'gemini-4-hydrogen');
  assert.equal(hydrogen.name, 'Gemini 4 Hydrogen');
  assert.deepEqual([hydrogen.input, hydrogen.output, hydrogen.cache_read], [1.5, 7.5, 0.075]);
});

test('deepseek parser reads peak and off-peak', () => {
  const { rows } = parseDeepSeek(page('deepseek.html'), '2026-10-23');
  const flash = rows.find((r) => r.model === 'deepseek-flash');
  assert.deepEqual([flash.input, flash.output, flash.cache_read], [0.3, 1.2, 0.006]);
  assert.deepEqual(flash.other_tiers.offpeak, { input: 0.15, output: 0.6, cache_read: 0.003 });
  assert.deepEqual(flash.aliases, ['deepseek-v4-flash', 'deepseek-v4-flash-vision-exp', 'deepseek-chat', 'deepseek-reasoner']);
  assert.match(flash.note, /api-docs\.deepseek\.com\/updates/);
  assert.equal(rows.find((r) => r.model === 'deepseek-v4-pro').input, 1.32);
});

test('bedrock parser reads the AWS price list: runtime ids, US geo profiles, batch, flex and priority', () => {
  const { rows, unknown } = parseBedrock(page('bedrock.json'), '2026-11-17');
  assert.deepEqual(unknown, []);
  const at = (id) => rows.find((r) => r.model === id);
  // DeepSeek-R1 on Bedrock: $1.35 / $5.40 per 1M (0.00135 / 0.0054 per 1K in the price list).
  assert.deepEqual([at('deepseek.r1-v1:0').input, at('deepseek.r1-v1:0').output], [1.35, 5.4]);
  assert.deepEqual(at('deepseek.r1-v1:0').aliases, ['us.deepseek.r1-v1:0']);
  const nova = at('amazon.nova-pro-v1:0');
  assert.deepEqual([nova.input, nova.output, nova.cache_read, nova.batch_input, nova.batch_output], [0.8, 3.2, 0.2, 0.4, 1.6]);
  assert.deepEqual(nova.other_tiers.priority, { input: 1.4, output: 5.6, cache_read: 0.35 });
  // The runtime id is the row id and the mantle id an alias.
  assert.deepEqual(at('openai.gpt-oss-120b-1:0').aliases, ['openai.gpt-oss-120b']);
  // Global inference profiles bill at their own (lower) price and are not aliases.
  assert.ok(!at('moonshotai.kimi-k3').aliases.some((a) => a.startsWith('global.')));
  assert.equal(at('moonshotai.kimi-k3').input, 3.3);
  assert.ok(rows.every((r) => r.provider === 'bedrock' && r.source_url === 'https://aws.amazon.com/bedrock/pricing/'));
  assert.equal(per1kToPer1m('0.0000350000'), 0.035);
  assert.equal(per1kToPer1m('0.0288750000'), 28.875);
});

test('openrouter parser reads the model list: per-token prices, overrides, batch variants', () => {
  const { rows } = parseOpenRouter(page('openrouter.json'), '2026-11-17');
  const at = (id) => rows.find((r) => r.model === id);
  assert.ok(rows.length > 300);
  assert.deepEqual([at('perplexity/sonar-pro').input, at('perplexity/sonar-pro').output], [3, 15]);
  const r1 = at('deepseek/deepseek-r1-0528');
  assert.deepEqual([r1.input, r1.output, r1.cache_read], [0.5, 2.15, 0.35]);
  const terra = at('openai/gpt-6.1-terra-pro');
  assert.deepEqual([terra.batch_input, terra.batch_output], [0.5, 2.5]);
  assert.equal(terra.long_context.threshold_tokens, 271999);
  assert.deepEqual([terra.long_context.input, terra.long_context.output], [2, 7.5]);
  // No :free, :batch or router entries, and every price is positive.
  assert.ok(rows.every((r) => !r.model.includes(':') && r.input > 0 && r.output > 0));
  assert.ok(!at('openrouter/auto'));
  assert.equal(perTokenToPer1m('0.000000035'), 0.035);
  assert.equal(perTokenToPer1m('0'), null);
  assert.equal(perTokenToPer1m('-1'), null);
});

test('committed prices.json equals what the parsers produce from the saved pages', () => {
  const fresh = [
    ...parseAnthropic(page('anthropic.md'), 'X').rows,
    ...parseOpenAI(page('openai.md'), 'X').rows,
    ...parseGemini(page('gemini.md'), 'X').rows,
    ...parseDeepSeek(page('deepseek.html'), 'X').rows,
    ...parseBedrock(page('bedrock.json'), 'X').rows,
    ...parseOpenRouter(page('openrouter.json'), 'X').rows,
  ].map(({ checked, ...r }) => r);
  const committed = prices.models.filter((m) => m.provider in SOURCES && !isDelisted(m)).map(({ checked, ...r }) => r);
  assert.deepEqual(committed, fresh);
});

test('hand-checked rows cite their provider page, come after the parsed rows and say so', () => {
  const manual = prices.models.filter((m) => !(m.provider in SOURCES));
  assert.ok(manual.length >= 50);
  const firstManual = prices.models.findIndex((m) => !(m.provider in SOURCES));
  assert.ok(prices.models.slice(firstManual).every((m) => !(m.provider in SOURCES)), 'parsed rows come first');
  for (const m of manual) {
    assert.equal(m.source_url, MANUAL_SOURCES[m.provider], `${m.model} cites ${m.source_url}`);
    assert.match(m.note, /checked by hand/, m.model);
  }
  assert.deepEqual(prices.sources, { ...Object.fromEntries(Object.entries(SOURCES).map(([k, v]) => [k, v.page])), ...MANUAL_SOURCES });
  // Spot checks against the pages (2026-11-14).
  const at = (provider, model) => prices.models.find((m) => m.provider === provider && m.model === model);
  assert.deepEqual([at('xai', 'grok-4.7').input, at('xai', 'grok-4.7').cache_read, at('xai', 'grok-4.7').output], [2, 0.5, 6]);
  assert.equal(at('xai', 'grok-4.7').long_context.threshold_tokens, 199999);
  assert.deepEqual([at('mistral', 'mistral-large-4').input, at('mistral', 'mistral-large-4').output], [0.7, 2.1]);
  assert.deepEqual(at('mistral', 'mistral-large-4').aliases, ['mistral-large-4-0', 'mistral-large-latest']);
  assert.deepEqual([at('groq', 'openai/gpt-oss-120b').input, at('groq', 'openai/gpt-oss-120b').output], [0.15, 0.6]);
  assert.equal(at('together', 'openai/gpt-oss-120b').cache_read, null);
  assert.equal(at('fireworks', 'accounts/fireworks/models/gpt-oss-120b').cache_read, 0.015);
  assert.deepEqual(at('fireworks', 'accounts/fireworks/models/kimi-k3').other_tiers.priority, { input: 3.75, output: 18.75, cache_read: 0.375 });
  assert.deepEqual([at('cohere', 'command-r-plus-08-2024').input, at('cohere', 'command-r-plus-08-2024').output], [2.5, 10]);
  // Perplexity Agent API rates (2026-11-17).
  assert.deepEqual([at('perplexity', 'perplexity/sonar').input, at('perplexity', 'perplexity/sonar').output, at('perplexity', 'perplexity/sonar').cache_read], [0.25, 2.5, 0.0625]);
  assert.deepEqual([at('perplexity', 'perplexity/kimi-k3').input, at('perplexity', 'perplexity/kimi-k3').output], [3, 15]);
});

test('crate data.rs, crate/prices.json and prices.toml are regenerated from prices.json', () => {
  const version = pricesVersion(readFileSync(join(root, 'prices.json'), 'utf8'));
  assert.match(version, /^sha256:[0-9a-f]{64}$/);
  assert.equal(readFileSync(join(root, 'crate', 'src', 'data.rs'), 'utf8'), toRust(prices, version));
  assert.equal(readFileSync(join(root, 'crate', 'prices.json'), 'utf8'), readFileSync(join(root, 'prices.json'), 'utf8'));
});

test('the Python and npm package data are regenerated from prices.json', () => {
  const text = readFileSync(join(root, 'prices.json'), 'utf8');
  for (const [path, content] of Object.entries(generatedFiles(prices, text))) {
    assert.equal(readFileSync(join(root, path), 'utf8'), content, `${path} is stale: run node scripts/build.mjs`);
  }
  // Prices travel as their prices.json text, so no language re-prints a float.
  const rows = packageRows(prices);
  assert.equal(rows.length, prices.models.length);
  assert.equal(rows.find((r) => r.model === 'gpt-4o-mini').cache_read, '0.075');
  assert.throws(() => toPython(prices), /needs the prices version/);
  assert.match(toPython(prices, 'sha256:0'), /^ROWS: Tuple\[Dict\[str, Any\], \.\.\.\] = \($/m);
  assert.match(toTs(prices, 'sha256:0'), /^export const ROWS: readonly RawRow\[\] = \[$/m);
  assert.throws(() => packageRows({ models: [{ ...prices.models[0], input: 1e-7 }] }), /not a plain decimal/);
});
