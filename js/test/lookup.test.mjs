// Ports of the Rust crate's lookup tests (crate/tests/lookup.rs and the unit tests in
// crate/src/lib.rs and crate/src/provider.rs): every id form the Rust `lookup` resolves must
// resolve to the same row here. Runs against the built ESM output.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { all, lookup, lookupProvider, parseProvider, providerRows, fallbackProviders, Provider, PROVIDERS } from '../dist/esm/index.js';

const ids = (p) => [p.model, ...p.aliases];
const undated = (id) => id.match(/^(.*)-(20\d{6})$/)?.[1] ?? id;
const sharedId = (p, id) => all().some((q) => q.provider !== p.provider && ids(q).some((m) => m.toLowerCase() === id.toLowerCase()));

function assertResolves(form, p) {
  const found = lookup(form);
  assert.ok(found, `${JSON.stringify(form)} did not resolve (expected ${p.model})`);
  assert.equal(found.model, p.model, `${JSON.stringify(form)} resolved to the wrong row`);
}

function assertResolvesUnderProvider(form, p) {
  const found = lookupProvider(p.provider, form);
  assert.ok(found, `${JSON.stringify(form)} did not resolve under ${p.provider}`);
  assert.equal(found, p, `${JSON.stringify(form)} under ${p.provider} resolved to ${found.provider}/${found.model}`);
}

test('every row has a source and a date', () => {
  assert.ok(all().length > 50);
  for (const p of all()) {
    assert.match(p.source_url, /^https:\/\//, p.model);
    assert.match(p.checked, /^\d{4}-\d{2}-\d{2}$/, p.model);
  }
});

test('lookups', () => {
  assert.equal(lookup('claude-opus-5-5').input, 4);
  assert.equal(lookup('us.anthropic.claude-sonnet-4-20250514-v1:0').model, 'claude-sonnet-4-20250514');
  assert.equal(lookup('claude-opus-4-1@20250805').model, 'claude-opus-4-1-20250805');
  assert.equal(lookup('claude-haiku-4-5-20251001').model, 'claude-haiku-4-5');
  assert.equal(lookup('models/gemini-2.5-pro').output, 10);
  assert.equal(lookup('deepseek-v4-flash').model, 'deepseek-flash');
  assert.equal(lookup('gpt-4o-2024-08-06').model, 'gpt-4o');
  assert.equal(lookup('gpt-4o-2024-05-13').model, 'gpt-4o-2024-05-13');
  assert.equal(lookup('not-a-model'), null);
});

test('crate doc examples', () => {
  const p = lookup('claude-sonnet-5-5');
  assert.deepEqual([p.input, p.cache_read], [2, 0.1]);
  assert.ok(lookup('anthropic/claude-sonnet-5-5'));
  assert.ok(lookup('claude-sonnet-4-5'));
  assert.equal(lookup('mistral-large-latest').model, 'mistral-large-4');
  assert.equal(lookup('xai/grok-4.7-latest').model, 'grok-4.7');
  assert.equal(lookup('vertex_ai/claude-opus-4-1@20250805').model, 'claude-opus-4-1-20250805');
  assert.equal(lookup('gpt-4o-2024-05-13').input, 5);
  assert.equal(lookup('fireworks_ai/accounts/fireworks/models/gpt-oss-120b').provider, 'fireworks');
});

test('snapshot and -latest suffixes', () => {
  assert.equal(lookup('chat-latest').model, 'chat-latest');
  assert.equal(lookup('claude-3-5-haiku-latest').model, 'claude-3-5-haiku-20241022');
  assert.equal(lookup('gpt-4-turbo').model, 'gpt-4-turbo-2024-04-09');
  // `-latest` alone and a bare date are not model ids.
  assert.equal(lookup('-latest'), null);
  assert.equal(lookup('2024-08-06'), null);
});

// Bedrock and OpenRouter resell other makers' models under their own ids (OpenRouter's
// anthropic/claude-fable-5 is also Anthropic's id), so their rows are reached by naming them.
const platformRow = (p) => p.provider === 'bedrock' || p.provider === 'openrouter';

test('ids and aliases resolve exactly', () => {
  for (const p of all()) {
    if (platformRow(p)) {
      for (const id of ids(p)) {
        for (const form of [id, id.toUpperCase(), `  ${id}\n`]) {
          assertResolvesUnderProvider(form, p);
          assert.equal(lookup(`${p.provider}/${form.trim()}`), p, form);
        }
      }
      continue;
    }
    for (const id of ids(p)) {
      for (const form of [id, id.toUpperCase(), `  ${id}\n`]) {
        assertResolves(form, p);
        assertResolvesUnderProvider(form, p);
      }
      if (!sharedId(p, id)) assert.equal(lookup(id), p, id);
    }
  }
});

test('ids are unique per provider', () => {
  const seen = new Set();
  for (const p of all()) {
    for (const id of ids(p)) {
      const key = `${p.provider}/${id.toLowerCase()}`;
      assert.ok(!seen.has(key), `${id} appears on two ${p.provider} rows`);
      seen.add(key);
    }
  }
});

const PREFIXES = {
  anthropic: ['anthropic/', 'openrouter/', 'bedrock/', 'us.anthropic.', 'bedrock/us.anthropic.', 'vertex_ai/'],
  openai: ['openai/', 'openrouter/', 'azure/'],
  google: ['google/', 'openrouter/', 'models/', 'gemini/', 'vertex_ai/'],
  deepseek: ['deepseek/', 'openrouter/'],
  mistral: ['mistral/', 'openrouter/', 'mistralai/'],
  xai: ['xai/', 'openrouter/', 'x-ai/'],
  cohere: ['cohere/', 'openrouter/'],
  groq: ['groq/'],
  together: ['together_ai/', 'together/'],
  fireworks: ['fireworks_ai/', 'fireworks/'],
  bedrock: ['bedrock/'],
  openrouter: ['openrouter/'],
  perplexity: [''],
};

test('provider prefixes resolve', () => {
  for (const p of all()) {
    assert.ok(PREFIXES[p.provider], `new provider ${p.provider}: add its prefixes here`);
    for (const id of ids(p)) {
      for (const prefix of PREFIXES[p.provider]) {
        const form = prefix + id;
        if (prefix === 'openrouter/' && p.provider !== 'openrouter') {
          // OpenRouter's own row for the same model wins when it lists one.
          const found = lookup(form);
          assert.ok(found === p || found?.provider === 'openrouter', form);
          continue;
        }
        assertResolves(form, p);
        if (!['openrouter/', 'us.anthropic.', 'models/'].includes(prefix)) assert.equal(lookup(form), p, form);
      }
    }
  }
});

test('Bedrock ids resolve for Anthropic rows', () => {
  for (const p of all().filter((p) => p.provider === 'anthropic')) {
    for (const id of ids(p)) {
      for (const region of ['', 'us.', 'eu.', 'apac.', 'global.']) {
        assertResolves(`${region}anthropic.${id}-v1:0`, p);
        assertResolves(`${region}anthropic.${id}-v2:0`, p);
        assert.equal(lookupProvider(Provider.Bedrock, `${region}anthropic.${id}-v1:0`), p);
      }
    }
  }
});

test('Vertex @date resolves for Anthropic rows', () => {
  for (const p of all().filter((p) => p.provider === 'anthropic')) {
    for (const id of ids(p)) {
      const base = undated(id);
      const date = base === id ? '20990101' : id.slice(base.length + 1);
      assertResolves(`${base}@${date}`, p);
      assertResolves(`vertex_ai/${base}@${date}`, p);
      assert.equal(lookupProvider('vertex_ai', `${base}@${date}`), p);
    }
  }
});

test('dated and undated Anthropic ids resolve', () => {
  for (const p of all().filter((p) => p.provider === 'anthropic')) {
    for (const id of ids(p)) {
      const base = undated(id);
      assertResolves(base, p);
      assertResolves(`${base}-20990101`, p);
      assertResolves(`anthropic/${base}-20990101`, p);
      assertResolves(`${base}-latest`, p);
    }
  }
});

test('dated OpenAI ids resolve to the base row', () => {
  for (const [id, base] of [
    ['gpt-4o-2024-08-06', 'gpt-4o'],
    ['gpt-4o-2024-11-20', 'gpt-4o'],
    ['openai/gpt-4o-2024-11-20', 'gpt-4o'],
    ['azure/gpt-4.1-2025-04-14', 'gpt-4.1'],
    ['o3-2025-04-16', 'o3'],
    ['gpt-4o-mini-2024-07-18', 'gpt-4o-mini'],
    ['gpt-5-2025-08-07', 'gpt-5'],
  ]) {
    assert.equal(lookup(id)?.model, base, id);
    assert.equal(lookupProvider('openai', id)?.model, base, id);
  }
  assert.equal(lookup('gpt-4o-2024-05-13').input, 5);
  assert.equal(lookup('gpt-4o').input, 2.5);
  for (const p of all()) {
    for (const id of ids(p)) {
      if (platformRow(p)) assert.equal(lookup(`${p.provider}/${id}-2099-01-01`), p, id);
      else assertResolves(`${id}-2099-01-01`, p);
    }
  }
});

test('-latest aliases resolve', () => {
  for (const [id, model] of [
    ['mistral-large-latest', 'mistral-large-4'],
    ['mistral-small-latest', 'mistral-small-2603'],
    ['codestral-latest', 'codestral-2508'],
    ['grok-4.7-latest', 'grok-4.7'],
    ['xai/grok-4.3-latest', 'grok-4.3'],
    ['claude-3-5-haiku-latest', 'claude-3-5-haiku-20241022'],
    ['claude-opus-5-5-latest', 'claude-opus-5-5'],
  ]) {
    assert.equal(lookup(id)?.model, model, id);
  }
});

test('provider lookup keeps hosts apart', () => {
  const groq = lookupProvider('groq', 'openai/gpt-oss-120b');
  const together = lookupProvider('together_ai', 'openai/gpt-oss-120b');
  const fireworks = lookupProvider('fireworks', 'accounts/fireworks/models/gpt-oss-120b');
  assert.deepEqual([groq.provider, together.provider, fireworks.provider], ['groq', 'together', 'fireworks']);
  assert.equal(groq.cache_read, 0.075);
  assert.equal(fireworks.cache_read, 0.015);
  assert.equal(together.cache_read, null);
  assert.equal(lookupProvider('openai', 'openai/gpt-oss-120b'), null);
  assert.equal(lookupProvider('openai', 'gpt-oss-120b'), null);
  assert.equal(lookupProvider('anthropic', 'gpt-4o'), null);
  assert.equal(lookupProvider('groq', 'claude-sonnet-4-6'), null);
  assert.equal(lookupProvider('together', 'gpt-4o'), null);
  assert.equal(lookupProvider('fireworks', 'gpt-oss-120b'), fireworks);
  assert.equal(lookupProvider('fireworks_ai', 'fireworks_ai/accounts/fireworks/models/gpt-oss-120b'), fireworks);
  assert.equal(lookupProvider('bedrock', 'claude-sonnet-4-6').provider, 'anthropic');
  assert.equal(lookupProvider('vertex', 'gemini-2.5-pro').provider, 'google');
  assert.equal(lookupProvider('azure', 'gpt-4o').provider, 'openai');
  // OpenRouter's own rows (its model list) come first; models it does not list fall back to the
  // maker's list price.
  for (const [id, maker] of [['mistralai/mistral-large-4', 'mistral'], ['x-ai/grok-4.7', 'xai']]) {
    assert.ok(['openrouter', maker].includes(lookupProvider('openrouter', id).provider), id);
  }
  assert.equal(lookupProvider('bedrock', 'gpt-4o'), null);
  assert.equal(lookupProvider('azure', 'claude-sonnet-4-6'), null);
  assert.equal(lookupProvider('vertex', 'mistral-large-4'), null);
  assert.equal(lookupProvider('bedrock', 'deepseek-flash'), null);
  assert.equal(lookupProvider('acme', 'gpt-4o'), null);
  const mistral = lookupProvider('mistral', 'mistral-large-latest');
  assert.equal(mistral.input, 0.7);
  assert.equal(lookupProvider(Provider.Mistral, 'mistral/mistral-large-4'), mistral);
});

test('provider doc examples', () => {
  const p = lookupProvider(Provider.Together, 'moonshotai/Kimi-K3');
  assert.deepEqual([p.provider, p.input], ['together', 3]);
  assert.ok(lookupProvider(Provider.Fireworks, 'accounts/fireworks/models/kimi-k3'));
  assert.equal(lookupProvider(Provider.Bedrock, 'global.anthropic.claude-opus-4-5-20251101-v1:0').provider, 'anthropic');
  assert.equal(lookupProvider(Provider.Groq, 'claude-opus-4-5'), null);
  assert.ok(providerRows('groq').every((p) => p.provider === 'groq'));
  assert.ok(providerRows('groq').some((p) => p.model === 'openai/gpt-oss-120b'));
});

test('provider rows cover the table', () => {
  assert.equal(PROVIDERS.reduce((n, p) => n + providerRows(p).length, 0), all().length);
  for (const name of ['mistral', 'xai', 'cohere', 'groq', 'together', 'fireworks']) {
    assert.ok(providerRows(name).length > 0, name);
  }
  assert.deepEqual(providerRows('not-a-provider'), []);
  assert.deepEqual(fallbackProviders('bedrock'), ['anthropic']);
  assert.deepEqual(fallbackProviders('groq'), []);
});

test('unknown ids do not resolve', () => {
  for (const id of ['', '   ', 'not-a-model', 'anthropic/', 'claude', 'gpt', 'models/', 'groq/', '-latest', 'latest', '2024-08-06']) {
    assert.equal(lookup(id), null, JSON.stringify(id));
  }
});

test('provider names round-trip', () => {
  assert.equal(PROVIDERS.length, 15);
  for (const p of PROVIDERS) {
    assert.equal(parseProvider(p), p);
    assert.equal(parseProvider(p.toUpperCase()), p);
  }
  for (const p of all()) {
    assert.equal(p.providerKind(), p.provider);
    // Bedrock and OpenRouter have rows of their own; Vertex and Azure do not yet.
    assert.ok(!['vertex', 'azure'].includes(p.provider));
  }
});

test('provider aliases', () => {
  for (const [name, want] of [
    ['gemini', 'google'],
    ['vertex_ai', 'vertex'],
    ['Vertex AI', 'vertex'],
    ['aws', 'bedrock'],
    ['azure_openai', 'azure'],
    ['mistralai', 'mistral'],
    ['together_ai', 'together'],
    ['Together_AI', 'together'],
    ['fireworks_ai', 'fireworks'],
    ['x-ai', 'xai'],
    ['open_router', 'openrouter'],
    ['google_genai', 'google'],
    ['google-generativeai', 'google'],
    ['google_vertexai', 'vertex'],
    ['bedrock_converse', 'bedrock'],
    ['azure_ai_foundry', 'azure'],
    ['GroqCloud', 'groq'],
    ['cohere_ai', 'cohere'],
  ]) {
    assert.equal(parseProvider(name), want, name);
  }
  for (const bad of ['', 'deepseek-ai', 'meta-llama', 'qwen', 'ollama', 'huggingface', 'a-very-long-provider-name-that-is-not-real']) {
    assert.equal(parseProvider(bad), null, bad);
    assert.equal(lookupProvider(bad, 'gpt-4o'), null, bad);
  }
});

test('legacy DeepSeek names and delisted rows still price', () => {
  for (const id of ['deepseek-chat', 'deepseek-reasoner']) {
    assert.equal(lookupProvider('deepseek', id).model, 'deepseek-flash', id);
  }
  assert.equal(lookupProvider('google_genai', 'gemini-2.5-flash').model, 'gemini-2.5-flash');
  for (const id of ['gpt-3.5-turbo', 'o1', 'claude-3-5-haiku-20241022']) {
    const p = lookup(id);
    assert.ok(p, id);
    assert.equal(p.status, 'retired', id);
  }
});

// A host or platform path is priced at that host's own row, or not at all: never with the
// model maker's first-party row (`accounts/fireworks/models/deepseek-r1` is not DeepSeek's
// $0.30 / $1.20).
test('host paths never take the maker price', () => {
  for (const id of [
    'accounts/fireworks/models/deepseek-r1',
    'accounts/fireworks/models/deepseek-chat',
    'fireworks_ai/accounts/fireworks/models/deepseek-r1',
    'fireworks_ai/deepseek-chat',
    'fireworks/gpt-4o',
    'together_ai/deepseek-chat',
    'together_ai/deepseek-ai/DeepSeek-R1',
    'together/claude-sonnet-4-6',
    'together_ai/openai/gpt-4o',
    'groq/gpt-4o',
    'groq/deepseek-chat',
    'bedrock/deepseek-r1',
    'bedrock/deepseek-chat',
    'bedrock/gpt-4o',
    'vertex_ai/deepseek-chat',
    'vertex_ai/gpt-4o',
    'azure/deepseek-r1',
    'azure/claude-sonnet-4-6',
    'openrouter/acme/not-a-model',
  ]) {
    const p = lookup(id);
    assert.equal(p, null, `${id} resolved to ${p?.provider}/${p?.model}`);
  }
});

test('host paths resolve to the host own row', () => {
  for (const [id, provider, model] of [
    ['accounts/fireworks/models/gpt-oss-120b', 'fireworks', 'accounts/fireworks/models/gpt-oss-120b'],
    ['fireworks_ai/accounts/fireworks/models/kimi-k3', 'fireworks', 'accounts/fireworks/models/kimi-k3'],
    ['fireworks_ai/deepseek-v4p1-flash', 'fireworks', 'accounts/fireworks/models/deepseek-v4p1-flash'],
    ['together_ai/openai/gpt-oss-120b', 'together', 'openai/gpt-oss-120b'],
    ['together_ai/moonshotai/Kimi-K3', 'together', 'moonshotai/Kimi-K3'],
    ['together/deepseek-ai/DeepSeek-V4.1-Flash', 'together', 'deepseek-ai/DeepSeek-V4.1-Flash'],
    ['groq/openai/gpt-oss-120b', 'groq', 'openai/gpt-oss-120b'],
    ['groq/qwen/qwen3.8-27b', 'groq', 'qwen/qwen3.8-27b'],
    // Bedrock's own rows (AWS price list): runtime id, US geo profile, mantle id.
    ['bedrock/us.deepseek.r1-v1:0', 'bedrock', 'deepseek.r1-v1:0'],
    ['bedrock/openai.gpt-oss-120b', 'bedrock', 'openai.gpt-oss-120b-1:0'],
    ['deepseek.r1-v1:0', 'bedrock', 'deepseek.r1-v1:0'],
    // OpenRouter's own rows, also under a host's name it lists models as.
    ['openrouter/deepseek/deepseek-r1-0528', 'openrouter', 'deepseek/deepseek-r1-0528'],
    ['openrouter/perplexity/sonar-pro', 'openrouter', 'perplexity/sonar-pro'],
    ['meta-llama/llama-3.3-70b-instruct', 'openrouter', 'meta-llama/llama-3.3-70b-instruct'],
    // Perplexity's Agent API rows.
    ['perplexity/sonar', 'perplexity', 'perplexity/sonar'],
    ['sonar', 'perplexity', 'perplexity/sonar'],
    ['perplexity/kimi-k3', 'perplexity', 'perplexity/kimi-k3'],
  ]) {
    const p = lookup(id);
    assert.deepEqual([p?.provider, p?.model], [provider, model], id);
  }
});

test('first-party prefixes still resolve', () => {
  for (const [id, provider] of [
    ['anthropic/claude-sonnet-4-6', 'anthropic'],
    ['openai/gpt-4o', 'openai'],
    ['openai/gpt-4o-2024-11-20', 'openai'],
    ['gemini/gemini-2.5-flash', 'google'],
    ['models/gemini-2.5-flash', 'google'],
    ['deepseek/deepseek-chat', 'deepseek'],
    ['mistral/mistral-large-latest', 'mistral'],
    ['xai/grok-4.7', 'xai'],
    ['bedrock/us.anthropic.claude-sonnet-4-20250514-v1:0', 'anthropic'],
    ['vertex_ai/gemini-2.5-pro', 'google'],
    ['azure/gpt-4o', 'openai'],
  ]) {
    assert.equal(lookup(id)?.provider, provider, id);
  }
  // OpenRouter's own row when it lists the model, Anthropic's otherwise.
  assert.ok(['openrouter', 'anthropic'].includes(lookup('openrouter/anthropic/claude-sonnet-4-6')?.provider));
  // A bare id never takes a platform's row unless it is that row's exact id.
  const gpt4 = lookup('gpt-4');
  assert.ok(gpt4 === null || gpt4.provider === 'openai');
  assert.equal(lookup('openai/gpt-oss-120b')?.provider, 'groq');
  for (const id of ['anthropic/gpt-4o', 'openai/claude-sonnet-4-6', 'openai/deepseek-chat', 'huggingface/deepseek-r1', 'deepseek-ai/DeepSeek-R1', 'accounts/fireworks/models/not-a-model']) {
    assert.equal(lookup(id), null, id);
  }
});

test('lookupProvider does not cross hosts', () => {
  assert.equal(lookupProvider('deepseek', 'accounts/fireworks/models/deepseek-chat'), null);
  assert.equal(lookupProvider('deepseek', 'together_ai/deepseek-chat'), null);
  assert.equal(lookupProvider('groq', 'fireworks_ai/accounts/fireworks/models/gpt-oss-120b'), null);
  assert.equal(lookupProvider('openrouter', 'groq/openai/gpt-oss-120b'), null);
  assert.equal(lookupProvider('openai', 'azure/gpt-4.1')?.provider, 'openai');
});
