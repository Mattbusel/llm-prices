import { test } from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { mkdtempSync, writeFileSync, mkdirSync, readFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const cli = join(here, '..', 'src', 'cli.mjs');

function run(dir, args = [], env = {}) {
  const r = spawnSync(process.execPath, [cli, '--working-directory', dir, ...args], {
    encoding: 'utf8',
    env: { PATH: process.env.PATH, SYSTEMROOT: process.env.SYSTEMROOT, ...env },
  });
  return { code: r.status, out: r.stdout + r.stderr };
}

function repo(files) {
  const dir = mkdtempSync(join(tmpdir(), 'llm-prices-'));
  for (const [p, text] of Object.entries(files)) {
    mkdirSync(dirname(join(dir, p)), { recursive: true });
    writeFileSync(join(dir, p), text);
  }
  return dir;
}

test('fails on drift and prints file:line, both values and the source URL', () => {
  const dir = repo({ 'src/cost.py': 'PRICES = {\n    "claude-sonnet-5-5": {"input": 2.0, "output": 10.0, "cache_read": 0.20},\n}\n' });
  const r = run(dir);
  assert.equal(r.code, 1);
  assert.match(r.out, /src\/cost\.py:2\s+claude-sonnet-5-5\s+cache_read\s+0\.20 per 1M/);
  assert.match(r.out, /0\.1\s+https:\/\/platform\.claude\.com\/docs\/en\/about-claude\/pricing/);
});

test('passes when prices match', () => {
  const dir = repo({ 'cost.ts': 'export const P = { "gpt-4o": { input: 2.5, output: 10, cachedInput: 1.25 } };\n' });
  const r = run(dir);
  assert.equal(r.code, 0, r.out);
  assert.match(r.out, /No drift/);
});

test('fail-on never reports but exits 0', () => {
  const dir = repo({ 'a.py': 'P = {"gpt-4o": {"input": 5.0, "output": 15.0}}\n' });
  const r = run(dir, ['--fail-on', 'never']);
  assert.equal(r.code, 0);
  assert.match(r.out, /2 price\(s\) differ/);
});

test('fail-on missing also fails when a used model has no price', () => {
  const dir = repo({
    'prices.py': 'P = {"gpt-4o": {"input": 2.5, "output": 10.0}}\n',
    'app.py': 'client.responses.create(model="gpt-5.4-mini")\n',
  });
  assert.equal(run(dir, ['--fail-on', 'drift']).code, 0);
  const r = run(dir, ['--fail-on', 'missing']);
  assert.equal(r.code, 1);
  assert.match(r.out, /app\.py:1\s+gpt-5\.4-mini/);
});

test('paths, exclude and providers inputs', () => {
  const dir = repo({
    'src/a.py': 'P = {"gpt-4o": {"input": 5.0, "output": 15.0}}\n',
    'lib/b.py': 'P = {"claude-opus-5-5": {"input": 9.0, "output": 20.0}}\n',
  });
  assert.match(run(dir, ['--paths', 'src/**']).out, /2 price\(s\) differ/);
  assert.match(run(dir, ['--exclude', 'lib/**']).out, /2 price\(s\) differ/);
  const onlyAnthropic = run(dir, ['--providers', 'anthropic']);
  assert.match(onlyAnthropic.out, /1 price\(s\) differ/);
  assert.match(onlyAnthropic.out, /claude-opus-5-5/);
});

test('reads inputs from INPUT_* env vars and writes GitHub annotations, summary and outputs', () => {
  const dir = repo({ 'p.rs': 'match m { "claude-opus-5" => (5.0, 20.0), _ => (0.0, 0.0) }\n' });
  const summary = join(dir, 'summary.md');
  const output = join(dir, 'output.txt');
  writeFileSync(summary, '');
  writeFileSync(output, '');
  const r = run(dir, [], { GITHUB_ACTIONS: 'true', GITHUB_STEP_SUMMARY: summary, GITHUB_OUTPUT: output, INPUT_TOLERANCE: '0.01', INPUT_FAIL_ON: 'drift' });
  assert.equal(r.code, 1);
  assert.match(r.out, /::error file=p\.rs,line=1,title=LLM price drift::claude-opus-5 output is 20\.0/);
  assert.match(readFileSync(summary, 'utf8'), /\| `p\.rs:1` \| `claude-opus-5` \| output/);
  assert.match(readFileSync(output, 'utf8'), /drift-count=1/);
});

test('bad inputs exit 2 with a message', () => {
  const dir = repo({});
  const r = run(dir, ['--fail-on', 'sometimes']);
  assert.equal(r.code, 2);
  assert.match(r.out, /fail-on must be drift, missing or never/);
});
