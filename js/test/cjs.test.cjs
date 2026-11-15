// The CommonJS build loads with require() and gives the same answers as the ES module build.

const { test } = require('node:test');
const assert = require('node:assert/strict');
const cjs = require('../dist/cjs/index.js');

test('require() loads the CommonJS build', () => {
  assert.equal(typeof cjs.lookup, 'function');
  assert.equal(cjs.lookup('gpt-4o-2024-08-06').model, 'gpt-4o');
  assert.equal(cjs.lookupProvider('groq', 'openai/gpt-oss-120b').provider, 'groq');
  assert.equal(cjs.lookup('claude-haiku-5-5').costExact(10000, 2000), '0.002');
});

test('the package name resolves to both builds', async () => {
  const self = require('../package.json').name;
  // Self-reference through package.json "exports".
  assert.equal(require(self).lookup, cjs.lookup);
  const esm = await import(self);
  assert.equal(esm.all().length, cjs.all().length);
  assert.equal(esm.GENERATED, cjs.GENERATED);
  for (const [a, b] of esm.all().map((p, i) => [p, cjs.all()[i]])) {
    assert.equal(a.model, b.model);
    assert.equal(a.cost(123456, 7890), b.cost(123456, 7890));
  }
});
