#!/usr/bin/env node
// llm-prices check: fails CI when prices hard-coded in a repo drift from the provider pages.
//
// As a GitHub Action the inputs arrive as INPUT_* environment variables (see action.yml).
// Locally:  node src/cli.mjs [--paths "src/**"] [--exclude "tests/**"] [--providers anthropic,openai]
//                            [--fail-on drift|missing|never] [--tolerance 0.01] [--prices prices.json] [--all]
//                            [--include-tests true]

import { readFileSync, appendFileSync } from 'node:fs';
import { join, dirname, resolve } from 'node:path';
import { fileURLToPath } from 'node:url';
import { buildIndex, scanText, scannable, missingPrices, isTestFile } from './scan.mjs';
import { listFiles, splitList } from './files.mjs';

const here = dirname(fileURLToPath(import.meta.url));

function arg(name) {
  const i = process.argv.indexOf(`--${name}`);
  if (i >= 0 && i + 1 < process.argv.length) return process.argv[i + 1];
  const env = process.env[`INPUT_${name.toUpperCase().replace(/-/g, '_')}`];
  return env === undefined || env === '' ? undefined : env;
}

function fmtNum(n) {
  if (n === null || n === undefined) return '-';
  return String(Math.round(n * 1e6) / 1e6);
}

function pad(rows) {
  const w = rows[0].map((_, c) => Math.max(...rows.map((r) => String(r[c]).length)));
  return rows.map((r) => r.map((x, c) => String(x).padEnd(w[c])).join('  ').trimEnd()).join('\n');
}

const UNIT_LABEL = { M: 'per 1M', K: 'per 1K', T: 'per token' };

export function run() {
  const root = resolve(arg('working-directory') || process.env.GITHUB_WORKSPACE || process.cwd());
  const pricesPath = arg('prices') || join(here, '..', 'prices.json');
  const prices = JSON.parse(readFileSync(pricesPath, 'utf8'));
  const providers = splitList(arg('providers') || '').map((s) => s.toLowerCase()).filter((p) => p !== 'all');
  const failOn = (arg('fail-on') || 'drift').toLowerCase();
  if (!['drift', 'missing', 'never'].includes(failOn)) throw new Error(`fail-on must be drift, missing or never (got "${failOn}")`);
  const tolerance = Number(arg('tolerance') ?? '0.01');
  if (!(tolerance >= 0)) throw new Error('tolerance must be a number >= 0, for example 0.01 for 1%');
  const include = splitList(arg('paths') || '**');
  const exclude = splitList(arg('exclude') || '');
  const includeTests = /^(true|1|yes)$/i.test(arg('include-tests') || '');
  const showAll = process.argv.includes('--all') || /^(true|1|yes)$/i.test(arg('show-matches') || '');

  const index = buildIndex(prices, providers);
  const files = listFiles(root, include, exclude).filter((f) => scannable(f) && (includeTests || !isTestFile(f)));
  const findings = [];
  const seen = new Map();
  const priced = new Set();
  for (const f of files) {
    let text;
    try {
      text = readFileSync(join(root, f), 'utf8');
    } catch {
      continue;
    }
    if (text.includes('\u0000')) continue;
    const r = scanText(f, text, index, { tolerance, includeTests });
    findings.push(...r.findings);
    for (const [k, v] of r.seen) if (!seen.has(k)) seen.set(k, v);
    for (const p of r.priced) priced.add(p);
  }
  const drift = findings.filter((x) => !x.ok);
  const okCount = findings.length - drift.length;
  const pricedProviders = new Set(findings.map((x) => x.provider));
  const missing = missingPrices(seen, priced, pricedProviders);

  const out = [];
  out.push(`llm-prices: scanned ${files.length} files, found ${findings.length} hard-coded prices for ${priced.size} known models (table generated ${prices.generated}).`);
  if (drift.length) {
    out.push('', `${drift.length} price(s) differ from the provider's official page:`, '');
    const rows = [['file:line', 'model', 'field', 'yours', 'real (USD/1M)', 'source']];
    for (const d of drift) rows.push([`${d.file}:${d.line}`, d.model, d.field, `${d.written} ${UNIT_LABEL[d.unit]} (= ${fmtNum(d.perM)}/1M)`, fmtNum(d.expected), d.source_url]);
    out.push(pad(rows));
  } else {
    out.push('', 'No drift: every hard-coded price found matches the official pages.');
  }
  if (showAll && okCount) {
    out.push('', 'Matching prices:', '');
    const rows = [['file:line', 'model', 'field', 'yours']];
    for (const d of findings.filter((x) => x.ok)) rows.push([`${d.file}:${d.line}`, d.model, d.field, `${d.written} ${UNIT_LABEL[d.unit]}`]);
    out.push(pad(rows));
  }
  if (missing.length) {
    out.push('', `${missing.length} model(s) are used in quotes but have no price in the scanned files:`, '');
    const rows = [['file:line', 'model', 'source']];
    for (const m of missing) rows.push([`${m.file}:${m.line}`, m.model, m.source_url]);
    out.push(pad(rows));
  }
  console.log(out.join('\n'));

  if (process.env.GITHUB_ACTIONS === 'true') {
    const annotate = !/^(false|0|no)$/i.test(arg('annotations') || 'true');
    for (const d of annotate ? drift : []) {
      console.log(`::error file=${d.file},line=${d.line},title=LLM price drift::${d.model} ${d.field} is ${d.written} ${UNIT_LABEL[d.unit]} here; the provider page says ${fmtNum(d.expected)} per 1M tokens (${d.source_url}, checked ${d.checked})`);
    }
    if (annotate && failOn === 'missing') for (const m of missing) console.log(`::warning file=${m.file},line=${m.line},title=LLM price missing::${m.model} is used but no price for it was found (${m.source_url})`);
    if (process.env.GITHUB_STEP_SUMMARY) {
      const md = ['### llm-prices', '', `Scanned ${files.length} files. ${findings.length} hard-coded prices found, ${drift.length} drifted, ${missing.length} models without a price.`, ''];
      if (drift.length) {
        md.push('| File | Model | Field | Yours | Real (USD / 1M) | Source |', '| --- | --- | --- | --- | --- | --- |');
        for (const d of drift) md.push(`| \`${d.file}:${d.line}\` | \`${d.model}\` | ${d.field} | ${d.written} ${UNIT_LABEL[d.unit]} | ${fmtNum(d.expected)} | [page](${d.source_url}) |`);
      }
      try {
        appendFileSync(process.env.GITHUB_STEP_SUMMARY, md.join('\n') + '\n');
      } catch {}
    }
    if (process.env.GITHUB_OUTPUT) {
      try {
        appendFileSync(process.env.GITHUB_OUTPUT, `drift-count=${drift.length}\nmissing-count=${missing.length}\nprices-found=${findings.length}\n`);
      } catch {}
    }
  }

  if (failOn === 'never') return 0;
  if (drift.length) return 1;
  if (failOn === 'missing' && missing.length) return 1;
  return 0;
}

if (process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url)) {
  try {
    process.exitCode = run();
  } catch (e) {
    console.error(`llm-prices: ${e.message}`);
    process.exitCode = 2;
  }
}
