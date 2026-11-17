// Ports of the Rust crate's cost tests (crate/src/lib.rs, crate/src/decimal.rs and the crate
// docs), plus checks that the generated table matches prices.json. Runs against the built ESM
// output.

import { test } from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { createHash } from 'node:crypto';
import { GENERATED, PRICES_VERSION, Price, all, longContextRate, lookup, promptTokens } from '../dist/esm/index.js';

const COLUMNS = ['input', 'output', 'cache_read', 'batch_input', 'batch_output'];
const close = (a, b, eps = 1e-12) => Math.abs(a - b) < eps;
const pricesJson = fileURLToPath(new URL('../../prices.json', import.meta.url));

// -- the generated table --------------------------------------------------------------------

test('PRICES_VERSION is the SHA-256 of prices.json', { skip: !existsSync(pricesJson) && 'prices.json is only in the repository' }, () => {
  const text = readFileSync(pricesJson, 'utf8').replace(/\r\n/g, '\n');
  assert.equal(PRICES_VERSION, `sha256:${createHash('sha256').update(text, 'utf8').digest('hex')}`);
});

test('the table matches prices.json', { skip: !existsSync(pricesJson) && 'prices.json is only in the repository' }, () => {
  const doc = JSON.parse(readFileSync(pricesJson, 'utf8'));
  assert.equal(GENERATED, doc.generated);
  assert.equal(all().length, doc.models.length);
  const sameTier = (p, m) => {
    for (const f of COLUMNS) assert.equal(p[f], m[f], `${m.model}.${f}`);
    assert.deepEqual({ ...p.cache_write }, Object.fromEntries(Object.entries(m.cache_write ?? {}).filter(([, v]) => v !== null)));
  };
  all().forEach((p, i) => {
    const m = doc.models[i];
    assert.deepEqual([p.provider, p.model, [...p.aliases], p.status], [m.provider, m.model, m.aliases, m.status]);
    assert.deepEqual([p.source_url, p.checked, p.name, p.note], [m.source_url, m.checked, m.name, m.note]);
    sameTier(p, m);
    if (m.long_context === null) assert.equal(p.long_context, null);
    else {
      assert.equal(p.long_context.threshold_tokens, m.long_context.threshold_tokens);
      sameTier(p.long_context, m.long_context);
    }
  });
});

test('exact text matches every price', () => {
  for (const p of all()) {
    for (const f of COLUMNS) {
      const value = p.rate(f);
      const text = p.rateStr(f);
      assert.equal(value === null, text === null, `${p.model} ${f}`);
      if (text !== null) {
        assert.equal(Number(text), value);
        assert.equal(String(value), text);
        assert.ok(!text.includes('e') && !text.startsWith('.'), text);
      }
    }
    for (const ttl of Object.keys(p.cache_write)) assert.equal(Number(p.rateStr('cache_write', ttl)), p.cacheWrite(ttl));
    const lc = p.long_context;
    if (lc === null) assert.equal(p.longContextRateStr('input'), null);
    else {
      for (const f of COLUMNS) {
        const text = p.longContextRateStr(f);
        assert.equal(longContextRate(lc, f) === null, text === null);
        if (text !== null) assert.equal(Number(text), longContextRate(lc, f));
      }
      for (const ttl of Object.keys(lc.cache_write)) assert.notEqual(p.longContextRateStr('cache_write', ttl), null);
    }
  }
});

test('rateStr needs an unchanged table row', () => {
  const p = lookup('gpt-4o');
  assert.equal(p.rateStr('input'), '2.5');
  assert.equal(p.rateStr('cache_write', '5m'), null);
  assert.equal(new Price({ ...p, model: 'my-model' }).rateStr('input'), null);
  const repriced = new Price({ ...p, output: 11 });
  assert.equal(repriced.rateStr('output'), null);
  assert.equal(repriced.rateStr('input'), '2.5');
  const h = lookup('claude-haiku-5-5');
  assert.equal(h.rateStr('input'), '0.1');
  assert.equal(h.rateStr('cache_write', '5m'), '0.125');
  assert.equal(h.longContextRateStr('input'), '0.5');
  assert.equal(lookup('gpt-4o-mini').rateStr('cache_read'), '0.075');
});

test('rate names are checked', () => {
  const p = lookup('claude-sonnet-4-6');
  assert.equal(p.rate('output'), 15);
  assert.equal(p.rate('cache_write', '1h'), 6);
  assert.equal(p.cacheWrite('1h'), 6);
  assert.equal(p.cacheWrite('toString'), null);
  assert.throws(() => p.rate('cache_write'), RangeError);
  assert.throws(() => p.rate('input', '5m'), RangeError);
  assert.throws(() => p.rate('cached_input'), RangeError);
});

test('rows are frozen', () => {
  const p = lookup('gpt-4o');
  assert.ok(Object.isFrozen(p) && Object.isFrozen(p.cache_write) && Object.isFrozen(p.aliases) && Object.isFrozen(all()));
  assert.throws(() => {
    p.input = 1;
  }, TypeError);
  assert.equal(lookup('gpt-4o').input, 2.5);
});

// -- float costs (crate/src/lib.rs) ---------------------------------------------------------

test('crate doc example: Anthropic call with a cache read and write', () => {
  const usage = { input: 2048, cache_read: 45000, cache_write_5m: 3500, output: 1024 };
  const c = lookup('claude-sonnet-4-6').costUsage(usage);
  assert.ok(close(c.input, 0.006144));
  assert.ok(close(c.cache_read, 0.0135));
  assert.ok(close(c.cache_write, 0.013125));
  assert.ok(close(c.output, 0.01536));
  assert.ok(close(c.total, 0.048129));
  assert.equal(c.long_context, false);
});

test('crate doc example: OpenAI call over the long-context threshold', () => {
  const [promptTok, cached, completion] = [300000, 200000, 4000];
  const c = lookup('gpt-5.4').costUsage({ input: promptTok - cached, cache_read: cached, output: completion });
  assert.equal(c.long_context, true);
  assert.ok(close(c.input, 0.5));
  assert.ok(close(c.cache_read, 0.1));
  assert.ok(close(c.output, 0.09));
  assert.ok(close(c.total, 0.69));
  assert.ok(close(lookup('gpt-5.4').cost(100000, 4000), 0.31));
  assert.ok(close(lookup('claude-haiku-5-5').cost(10000, 2000), 0.002));
});

// The Rust crate's 1.0 `cost`, which `cost` must still equal bit for bit.
function cost10(p, i, o) {
  const lc = p.long_context;
  const [ri, ro] = lc && i > lc.threshold_tokens ? [lc.input, lc.output] : [p.input, p.output];
  if (ri === null || ro === null) return null;
  return (i * ri + o * ro) / 1e6;
}

test('cost agrees with costUsage and with the Rust 1.0 formula', () => {
  const counts = [0, 1, 999, 100000, 100001, 200000, 200001, 272000, 272001, 1000000, 12345678];
  for (const p of all()) {
    for (const i of counts) {
      for (const o of counts) {
        const total = p.costUsage({ input: i, output: o })?.total ?? null;
        assert.equal(p.cost(i, o), total, `${p.model} ${i} ${o}`);
        assert.equal(p.cost(i, o), cost10(p, i, o), `${p.model} ${i} ${o}`);
        assert.equal(p.cost(BigInt(i), BigInt(o)), total);
      }
    }
  }
});

test('long context counts cached prompt tokens', () => {
  const h = lookup('claude-haiku-5-5');
  const at = { input: 60000, cache_read: 40000 };
  let c = h.costUsage(at);
  assert.equal(c.long_context, false);
  assert.ok(close(c.total, (60000 * 0.1 + 40000 * 0.01) / 1e6, 1e-15));
  c = h.costUsage({ ...at, cache_write_1h: 1 });
  assert.equal(c.long_context, true);
  assert.ok(close(c.input, (60000 * 0.5) / 1e6, 1e-15));
  assert.ok(close(c.cache_read, (40000 * 0.05) / 1e6, 1e-15));
  assert.ok(close(c.cache_write, 1 / 1e6, 1e-15));
  assert.equal(promptTokens({ ...at, cache_write_1h: 1 }), 100001n);
});

test('cache reads fall back to the input price', () => {
  const p = lookup('gpt-5.5-pro');
  assert.equal(p.cache_read, null);
  let c = p.costUsage({ cache_read: 100000 });
  assert.equal(c.cache_read, 3);
  assert.equal(c.input, 0);
  c = p.costUsage({ cache_read: 1000000 });
  assert.equal(c.long_context, true);
  assert.equal(c.cache_read, 60);
});

test('batch and cache writes', () => {
  const s = lookup('claude-sonnet-4-6');
  const u = { cache_write_5m: 1000000, cache_write_1h: 1000000 };
  assert.equal(s.costUsage(u).cache_write, 3.75 + 6);
  assert.equal(s.costUsage({ ...u, batch: true }).cache_write, (3.75 + 6) / 2);
  const pro = lookup('gpt-5.5-pro');
  assert.equal(pro.costUsage({ input: 272001, batch: true }), null);
  assert.notEqual(pro.costUsage({ input: 272000, batch: true }), null);
  assert.equal(lookup('text-embedding-3-small').costUsage({}), null);
  const haiku = lookup('claude-haiku-4-5');
  const b = { input: 10000, cache_read: 90000, output: 1000, batch: true };
  const c = haiku.costUsage(b);
  assert.ok(close(c.input, 0.005) && close(c.cache_read, 0.0045) && close(c.output, 0.0025));
  assert.equal(lookup('gpt-3.5-turbo').costUsage(b), null);
  assert.equal(lookup('gpt-6-sol').costUsage({ input: 1, cache_write_5m: 1 }), null);
});

test('other cache lifetimes are added by hand', () => {
  const p = lookup('gpt-6-sol');
  assert.equal(p.cacheWrite('30m'), 2.5);
  const total = p.costUsage({ input: 10000, output: 1000 }).total + (50000 * p.cacheWrite('30m')) / 1e6;
  assert.ok(close(total, 0.155));
});

test('long-context cost and the xAI 200k threshold', () => {
  const h = lookup('claude-haiku-5-5');
  assert.ok(close(h.cost(100000, 0), 0.01));
  assert.ok(close(h.cost(100001, 0), (100001 * 0.5) / 1e6));
  assert.equal(h.cacheWrite('1h'), 0.2);
  const g = lookup('grok-4.7');
  assert.equal(g.long_context.threshold_tokens, 199999);
  assert.equal(g.costUsage({ input: 200000 }).long_context, true);
  assert.equal(g.costUsage({ input: 199999 }).long_context, false);
});

test('token counts are validated', () => {
  const p = lookup('gpt-4o');
  for (const bad of [-1, 1.5, NaN, Infinity, -1n, '10']) assert.throws(() => p.costUsage({ input: bad }), RangeError, String(bad));
  assert.throws(() => p.costUsage(null), TypeError);
  assert.equal(p.cost(2n ** 64n - 1n, 0) > 0, true);
});

test('a hand-built Price', () => {
  const p = new Price({ provider: 'acme', model: 'm1', input: 1, output: 2, cache_read: 0.1, cache_write: { '5m': 1.25 } });
  assert.equal(p.providerKind(), null);
  assert.equal(p.rateStr('input'), null);
  const u = { input: 1000000, output: 1000000, cache_read: 1000000, cache_write_5m: 1000000 };
  assert.equal(p.costUsage(u).total, 1 + 2 + 0.1 + 1.25);
  assert.equal(p.costUsageExact(u).total, '4.35');
  assert.equal(p.costExact(1000000, 0), '1');
});

// -- exact decimal costs (crate/src/decimal.rs) ---------------------------------------------

test('exact doc examples', () => {
  assert.equal(lookup('claude-haiku-5-5').costExact(10000, 2000), '0.002');
  const c = lookup('claude-sonnet-4-6').costUsageExact({ input: 2048, cache_read: 45000, cache_write_5m: 3500, output: 1024 });
  assert.deepEqual(c, { input: '0.006144', output: '0.01536', cache_read: '0.0135', cache_write: '0.013125', total: '0.048129', long_context: false });
  assert.equal(lookup('claude-opus-5-5').costExact(1000000, 1000000), '24');
  assert.equal(lookup('claude-opus-5-5').costExact(0, 0), '0');
});

// Exact decimal sum of decimal strings, for checking total = sum of parts.
function sum(...xs) {
  const scale = Math.max(...xs.map((x) => (x.split('.')[1] ?? '').length));
  const big = xs.reduce((acc, x) => {
    const [i, f = ''] = x.split('.');
    return acc + BigInt(i + f.padEnd(scale, '0'));
  }, 0n);
  return Number(big) / 10 ** scale;
}

test('exact costs agree with float costs', () => {
  const counts = [0, 1, 999, 60000, 100001, 199999, 200000, 272001, 1000000, 12345678];
  for (const p of all()) {
    for (const i of counts) {
      for (const r of [0, 40000, 250000]) {
        for (const batch of [false, true]) {
          const u = { input: i, cache_read: r, output: Math.floor(i / 3), batch };
          const f = p.costUsage(u);
          const d = p.costUsageExact(u);
          assert.equal(f === null, d === null, `${p.model} ${JSON.stringify(u)}`);
          if (f && d) {
            assert.equal(f.long_context, d.long_context);
            assert.ok(Math.abs(f.total - Number(d.total)) <= 1e-9 * Math.max(Math.abs(f.total), 1), `${p.model} ${f.total} vs ${d.total}`);
            assert.ok(Math.abs(sum(d.input, d.output, d.cache_read, d.cache_write) - Number(d.total)) <= 1e-12 * Math.max(Number(d.total), 1));
            assert.match(d.total, /^\d+(\.\d*[1-9])?$/);
          }
        }
      }
    }
  }
});

test('exact where float is not', () => {
  const p = new Price({ ...lookup('gpt-4.1-nano'), input: 0.1, output: 0.2 });
  assert.notEqual(p.cost(3, 3), 9e-7); // 9.000000000000001e-7 in floating point
  assert.equal(p.costExact(3, 3), '0.0000009');
  const h = lookup('claude-haiku-4-5');
  const c = h.costUsageExact({ input: 10000, cache_read: 90000, output: 1000, batch: true });
  assert.deepEqual([c.input, c.cache_read, c.output, c.total], ['0.005', '0.0045', '0.0025', '0.012']);
  const g = lookup('grok-4.7');
  assert.equal(g.costUsageExact({ cache_read: 100000, batch: true }).cache_read, '0.04');
  const lc = g.costUsageExact({ cache_read: 1000000, batch: true });
  assert.equal(lc.long_context, true);
  assert.equal(lc.cache_read, '0.8');
});

test('exact long context, missing prices and huge counts', () => {
  const c = lookup('claude-haiku-5-5').costUsageExact({ input: 100001 });
  assert.equal(c.long_context, true);
  assert.equal(c.input, '0.0500005');
  assert.equal(lookup('gpt-6-sol').costUsageExact({ input: 1, cache_write_5m: 1 }), null);
  assert.equal(lookup('text-embedding-3-small').costExact(1, 0), null);
  const big = 2n ** 64n - 1n;
  const d = lookup('o1-pro').costUsageExact({ input: big, output: big, cache_read: big });
  assert.ok(d && /^\d+(\.\d+)?$/.test(d.total));
});

test('a non-terminating batch discount rounds at 28 significant digits', () => {
  // Batch input at a third of the input price: cache reads get 1/3 off ... with 0.1/0.3.
  const p = new Price({ provider: 'acme', model: 'third', input: 0.3, output: 1, batch_input: 0.1, batch_output: 0.5, cache_read: 0.3 });
  const c = p.costUsageExact({ cache_read: 1000000, batch: true });
  assert.equal(c.cache_read, '0.1');
  const q = new Price({ provider: 'acme', model: 'seventh', input: 0.7, output: 1, batch_input: 0.1, batch_output: 0.5, cache_read: 1 });
  // 1,000,000 x $1 x 0.1 / 0.7 per 1M = 1/7 = 0.1428571428571428571428571429 (28 digits).
  assert.equal(q.costUsageExact({ cache_read: 1000000, batch: true }).cache_read, '0.1428571428571428571428571429');
});
