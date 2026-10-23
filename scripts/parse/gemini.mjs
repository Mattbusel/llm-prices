import { GEMINI_TEXT_MODELS, SOURCES } from '../meta.mjs';
import { ParseError, cells, row } from './common.mjs';

const MONTHS = { january: 1, february: 2, march: 3, april: 4, may: 5, june: 6, july: 7, august: 8, september: 9, october: 10, november: 11, december: 12 };

function isoDate(text) {
  const m = text.match(/([A-Z][a-z]+) ([0-9]{1,2}), ([0-9]{4})/);
  if (!m) return null;
  const mo = MONTHS[m[1].toLowerCase()];
  return `${m[3]}-${String(mo).padStart(2, '0')}-${m[2].padStart(2, '0')}`;
}

// Parses a "Paid Tier" cell. Returns { now, long, threshold, later, effective }.
// Shapes seen on the page:
//   "$2.00 through November 30, 2026. $4.00 starting December 1, 2026."
//   "$1.25, prompts \<= 200k tokens $2.50, prompts \> 200k tokens"
//   "$0.30 (text / image / video) $1.00 (audio)"   (text price is the first)
//   "$0.03 $1.00 / 1,000,000 tokens per hour (storage price)"
function paid(cell) {
  const c = cell.replace(/\\/g, '');
  const firstPart = c.split(/\$[0-9.]+ \/ 1,000,000 tokens per hour/)[0];
  const prices = [...firstPart.matchAll(/\$([0-9]+(?:\.[0-9]+)?)/g)].map((m) => Number(m[1]));
  if (!prices.length) return null;
  const out = { now: prices[0], long: null, threshold: null, later: null, effective: null };
  const tier = c.match(/prompts\s*<=\s*([0-9]+)k tokens/);
  if (tier) {
    out.threshold = Number(tier[1]) * 1000;
    out.long = prices[1] ?? null;
  }
  const sched = firstPart.match(/through ([A-Z][a-z]+ [0-9]{1,2}, [0-9]{4})\.\s*\$([0-9]+(?:\.[0-9]+)?)[^$]*?starting ([A-Z][a-z]+ [0-9]{1,2}, [0-9]{4})/);
  if (sched) {
    out.later = Number(sched[2]);
    out.effective = isoDate(sched[3]);
  }
  const storage = c.match(/\$([0-9.]+) \/ 1,000,000 tokens per hour/);
  out.storage = storage ? Number(storage[1]) : null;
  return out;
}

function tierTable(section, tierName) {
  const parts = section.split(/\n### /);
  const sub = parts.find((p) => p.split('\n')[0].trim() === tierName);
  if (!sub) return null;
  const res = {};
  for (const line of sub.split('\n')) {
    if (!line.startsWith('|')) continue;
    const c = cells(line);
    const label = c[0].toLowerCase();
    const val = c[c.length - 1];
    if (label.startsWith('input price')) res.input = paid(val);
    else if (label.startsWith('output price')) res.output = paid(val);
    else if (label.startsWith('context caching price')) res.cache = paid(val);
  }
  return res;
}

export function parseGemini(md, checked) {
  const src = SOURCES.google.page;
  const sections = md.split(/\n## /).slice(1);
  const found = new Map();
  const names = new Map();
  for (const s of sections) {
    const head = s.split('###')[0];
    names.set(s, head.split(/\r?\n/)[0].trim());
    const ids = [...head.matchAll(/\[`([^`]+)`\]/g)].map((m) => m[1]);
    for (const id of ids) if (GEMINI_TEXT_MODELS.includes(id)) found.set(id, s);
  }
  const rows = [];
  for (const id of GEMINI_TEXT_MODELS) {
    const s = found.get(id);
    if (!s) throw new ParseError(`google: ${id} is no longer on the pricing page`);
    const std = tierTable(s, 'Standard');
    if (!std || !std.input || !std.output) throw new ParseError(`google: ${id} standard table unreadable`);
    const bat = tierTable(s, 'Batch');
    const notes = ['prices are the Paid Tier text-token prices'];
    const scheduled = [];
    const later = {};
    let eff = null;
    for (const [k, v] of [['input', std.input], ['output', std.output], ['cache_read', std.cache], ['batch_input', bat?.input], ['batch_output', bat?.output]]) {
      if (v && v.later != null) {
        later[k] = v.later;
        eff = v.effective;
      }
    }
    if (eff) {
      scheduled.push({ effective: eff, ...later });
      notes.push(`current prices are promotional until the day before ${eff}; scheduled holds the published later prices`);
    }
    if (std.cache?.storage != null) notes.push(`explicit cache storage ${std.cache.storage} USD per 1M tokens per hour`);
    if (/Implicit caching only/.test(s)) notes.push('implicit caching only');
    if (/\(audio\)/.test(s)) notes.push('audio input is priced higher; see page');
    let long_context = null;
    const thr = std.input.threshold;
    if (thr) {
      long_context = {
        threshold_tokens: thr,
        applies_when: `prompt over ${thr / 1000}k tokens`,
        input: std.input.long,
        output: std.output.long,
        cache_write: null,
        cache_read: std.cache?.long ?? null,
        batch_input: bat?.input?.long ?? null,
        batch_output: bat?.output?.long ?? null,
      };
    }
    const other = {};
    for (const t of ['Flex', 'Priority']) {
      const x = tierTable(s, t);
      if (x && x.input && x.output) other[t.toLowerCase()] = { input: x.input.now, output: x.output.now, cache_read: x.cache?.now ?? null };
    }
    rows.push(
      row({
        provider: 'google',
        model: id,
        name: names.get(s),
        input: std.input.now,
        output: std.output.now,
        cache_write: null,
        cache_read: std.cache?.now ?? null,
        batch_input: bat?.input?.now ?? null,
        batch_output: bat?.output?.now ?? null,
        long_context,
        other_tiers: other,
        scheduled,
        source_url: src,
        checked,
        note: notes.join('; '),
      }),
    );
  }
  return { rows, unknown: [] };
}
