#!/usr/bin/env node
// Fetches every provider pricing page, parses it, and rewrites prices.json, prices.toml and
// crate/src/data.rs when a price changed.
//
//   node scripts/update.mjs                  fetch live pages
//   node scripts/update.mjs --pages DIR      read saved pages (anthropic.md, openai.md, gemini.md, deepseek.html)
//   node scripts/update.mjs --force          rewrite files even when only dates changed
//
// Exit codes: 0 no price change, 10 prices changed (files rewritten), 2 fetch or parse failure.
// A markdown summary goes to $SUMMARY_FILE (default: update-summary.md) for the PR or issue body.

import { readFileSync, writeFileSync, existsSync } from 'node:fs';
import { join, dirname } from 'node:path';
import { fileURLToPath } from 'node:url';
import { SOURCES } from './meta.mjs';
import { parseAnthropic } from './parse/anthropic.mjs';
import { parseOpenAI } from './parse/openai.mjs';
import { parseGemini } from './parse/gemini.mjs';
import { parseDeepSeek } from './parse/deepseek.mjs';
import { toToml } from './toml.mjs';
import { toRust } from './rust.mjs';
import { excerpt } from './excerpt.mjs';

const root = join(dirname(fileURLToPath(import.meta.url)), '..');
const args = process.argv.slice(2);
const pagesDir = args.includes('--pages') ? args[args.indexOf('--pages') + 1] : null;
const force = args.includes('--force');
const today = process.env.LLM_PRICES_TODAY || new Date().toISOString().slice(0, 10);
const summaryFile = process.env.SUMMARY_FILE || join(root, 'update-summary.md');

const PROVIDERS = [
  ['anthropic', 'anthropic.md', parseAnthropic],
  ['openai', 'openai.md', parseOpenAI],
  ['google', 'gemini.md', parseGemini],
  ['deepseek', 'deepseek.html', parseDeepSeek],
];

async function load(provider, file) {
  if (pagesDir) return readFileSync(join(pagesDir, file), 'utf8');
  const url = SOURCES[provider].fetch;
  let last;
  for (let attempt = 1; attempt <= 3; attempt++) {
    try {
      const res = await fetch(url, { headers: { 'user-agent': 'llm-prices-updater (+https://github.com/Mattbusel/llm-prices)', accept: 'text/markdown,text/html;q=0.9,*/*;q=0.5' } });
      if (!res.ok) throw new Error(`HTTP ${res.status}`);
      return await res.text();
    } catch (e) {
      last = e;
      await new Promise((r) => setTimeout(r, 2000 * attempt));
    }
  }
  throw new Error(`fetch ${url} failed: ${last.message}`);
}

// Everything except the date, so a daily run that finds the same prices changes nothing.
function priceKey(m) {
  const { checked, ...rest } = m;
  return JSON.stringify(rest);
}

function fmt(v) {
  return v === null || v === undefined ? 'null' : typeof v === 'object' ? JSON.stringify(v) : String(v);
}

async function main() {
  const failures = [];
  const unknownNames = [];
  const fresh = [];
  const pages = {};
  for (const [provider, file, parse] of PROVIDERS) {
    try {
      const page = await load(provider, file);
      const { rows, unknown } = parse(page, today);
      if (!pagesDir) pages[file] = excerpt(provider, page, today);
      fresh.push(...rows);
      for (const u of unknown) unknownNames.push(`${provider}: "${u}"`);
    } catch (e) {
      failures.push({ provider, url: SOURCES[provider].page, error: e.message });
    }
  }

  const path = join(root, 'prices.json');
  const old = existsSync(path) ? JSON.parse(readFileSync(path, 'utf8')) : { models: [] };
  const oldBy = new Map(old.models.map((m) => [`${m.provider}/${m.model}`, m]));

  // Keep previous rows for providers whose page failed, so one broken page never deletes data.
  const failed = new Set(failures.map((f) => f.provider));
  const merged = [];
  for (const [provider] of PROVIDERS) {
    if (failed.has(provider)) merged.push(...old.models.filter((m) => m.provider === provider));
    else merged.push(...fresh.filter((m) => m.provider === provider));
  }

  const changes = [];
  const newBy = new Map(merged.map((m) => [`${m.provider}/${m.model}`, m]));
  for (const [k, m] of newBy) {
    const o = oldBy.get(k);
    if (!o) {
      changes.push(`- added \`${k}\`: input ${fmt(m.input)}, output ${fmt(m.output)}, cache read ${fmt(m.cache_read)} (${m.source_url})`);
      continue;
    }
    if (priceKey(o) === priceKey(m)) {
      m.checked = failed.has(m.provider) ? o.checked : today;
      continue;
    }
    for (const f of Object.keys(m)) {
      if (f === 'checked') continue;
      if (JSON.stringify(o[f]) !== JSON.stringify(m[f])) changes.push(`- \`${k}\` ${f}: ${fmt(o[f])} -> ${fmt(m[f])} (${m.source_url})`);
    }
  }
  for (const k of oldBy.keys()) if (!newBy.has(k)) changes.push(`- removed \`${k}\` (no longer on ${oldBy.get(k).source_url})`);

  const doc = {
    schema: 'https://github.com/Mattbusel/llm-prices#schema',
    unit: 'USD per 1M tokens',
    generated: changes.length || force || !old.generated ? today : old.generated,
    sources: Object.fromEntries(Object.entries(SOURCES).map(([k, v]) => [k, v.page])),
    models: merged,
  };

  const lines = [];
  if (changes.length) lines.push(`Prices changed on the official pages (checked ${today}):`, '', ...changes, '');
  if (unknownNames.length) lines.push('New model names on a pricing page with no API id mapping yet (add them to scripts/meta.mjs):', '', ...unknownNames.map((u) => `- ${u}`), '');
  if (failures.length) lines.push('Pages that could not be fetched or parsed (rows kept from the last good run):', '', ...failures.map((f) => `- ${f.provider} (${f.url}): ${f.error}`), '');
  writeFileSync(summaryFile, lines.join('\n') || 'No change.\n');

  if (changes.length || force) {
    writeFileSync(path, JSON.stringify(doc, null, 2) + '\n');
    writeFileSync(join(root, 'prices.toml'), toToml(doc));
    writeFileSync(join(root, 'crate', 'src', 'data.rs'), toRust(doc));
    writeFileSync(join(root, 'crate', 'prices.json'), JSON.stringify(doc, null, 2) + '\n');
    // Keep the parser tests on the same page text the new table came from.
    for (const [file, text] of Object.entries(pages)) writeFileSync(join(root, 'test', 'pages', file), text);
  }
  console.log(lines.join('\n') || 'No price changes.');
  if (failures.length || unknownNames.length) process.exitCode = 2;
  else if (changes.length) process.exitCode = 10;
}

main().catch((e) => {
  console.error(e);
  process.exitCode = 2;
});
