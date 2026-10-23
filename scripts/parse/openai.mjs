import { SOURCES } from '../meta.mjs';
import { ParseError, money, row, tables } from './common.mjs';

const TIER_HEADINGS = [
  ['### Standard pricing data', 'standard'],
  ['#### Batch', 'batch'],
  ['#### Flex', 'flex'],
  ['### Fast pricing data', 'fast'],
  ['### Ultrafast pricing data', 'ultrafast'],
];

function cleanId(cell) {
  return cell.replace(/\(.*?\)/g, '').replace(/`/g, '').trim();
}

// Reads one table into { id: prices } for a tier. Handles the 10-column short/long table,
// the 4-column Input/Output/Cached table and the 5-column Category table.
function readTable(t, tier, out) {
  const h = t.header.map((x) => x.toLowerCase());
  const mi = h.indexOf('model');
  if (mi < 0) return;
  for (const r of t.rows) {
    const id = cleanId(r[mi]);
    const where = `openai ${tier} ${id}`;
    let p;
    if (h[mi + 1] === 'short context input') {
      p = {
        input: money(r[mi + 1], where),
        output: money(r[mi + 2], where),
        cache_read: money(r[mi + 3], where),
        cache_write_30m: money(r[mi + 4], where),
        cache_write_12h: money(r[mi + 5], where),
        long: {
          input: money(r[mi + 6], where),
          output: money(r[mi + 7], where),
          cache_read: money(r[mi + 8], where),
          cache_write_30m: money(r[mi + 9], where),
          cache_write_12h: money(r[mi + 10], where),
        },
      };
    } else if (h[mi + 1] === 'input' && h[mi + 2] === 'output' && h[mi + 3] === 'cached input') {
      if (/free/i.test(r[mi + 1])) continue;
      p = { input: money(r[mi + 1], where), output: money(r[mi + 2], where), cache_read: money(r[mi + 3], where) };
    } else {
      continue;
    }
    const bucket = (out[id] ??= {});
    if (!bucket[tier]) bucket[tier] = p;
  }
}

function cw(p) {
  if (!p || (p.cache_write_30m == null && p.cache_write_12h == null)) return null;
  return { '30m': p.cache_write_30m, '12h': p.cache_write_12h };
}

function hasAny(o) {
  return o && Object.values(o).some((v) => v !== null && v !== undefined);
}

export function parseOpenAI(md, checked) {
  const src = SOURCES.openai.page;
  const cut = (label) => {
    const i = md.search(new RegExp(`^\\s*${label}\\s*$`, 'm'));
    return i;
  };
  const regions = [
    ['Flagship models', 'Cyber models'],
    ['Cyber models', 'Life sciences models'],
    ['Specialized models', 'Finetuning'],
  ];
  const thr = md.match(/Short context:\s*≤\s*([0-9]+)K input tokens/);
  if (!thr) throw new ParseError('openai: short/long context threshold sentence not found');
  const threshold = Number(thr[1]) * 1000;

  const models = {};
  let order = [];
  for (const [from, to] of regions) {
    const a = cut(from);
    const b = cut(to);
    if (a < 0 || b < 0 || b < a) throw new ParseError(`openai: section "${from}" not found`);
    const region = md.slice(a, b);
    // Split region by tier headings; "Grouped Pricing Table data" sections in Specialized
    // are Standard first, then Fast.
    const marks = [];
    for (const [hd, tier] of TIER_HEADINGS) {
      let i = region.indexOf(hd);
      while (i >= 0) {
        marks.push([i, tier]);
        i = region.indexOf(hd, i + 1);
      }
    }
    if (from === 'Specialized models') {
      const std = region.search(/^\s*Standard\s*$/m);
      const fast = region.search(/^\s*Fast\s*$/m);
      if (std >= 0) marks.push([std, 'standard']);
      if (fast >= 0) marks.push([fast, 'fast']);
    }
    marks.sort((x, y) => x[0] - y[0]);
    for (let k = 0; k < marks.length; k++) {
      const [start, tier] = marks[k];
      const end = k + 1 < marks.length ? marks[k + 1][0] : region.length;
      const before = Object.keys(models).length;
      for (const t of tables(region.slice(start, end))) readTable(t, tier, models);
      if (Object.keys(models).length > before) order = Object.keys(models);
    }
  }
  if (!models['gpt-4o'] || !models['gpt-4o'].standard) throw new ParseError('openai: gpt-4o standard row missing, page shape changed');

  const rows = [];
  for (const id of order) {
    const m = models[id];
    const s = m.standard;
    if (!s) continue;
    const b = m.batch;
    const notes = [];
    let long_context = null;
    if (s.long && hasAny(s.long)) {
      long_context = {
        threshold_tokens: threshold,
        applies_when: `input tokens over ${thr[1]}K (page: short context is up to ${thr[1]}K input tokens)`,
        input: s.long.input,
        output: s.long.output,
        cache_write: cw(s.long),
        cache_read: s.long.cache_read,
        batch_input: b?.long?.input ?? null,
        batch_output: b?.long?.output ?? null,
      };
    }
    const other = {};
    for (const tier of ['flex', 'fast', 'ultrafast']) {
      if (m[tier]) other[tier] = { input: m[tier].input, output: m[tier].output, cache_read: m[tier].cache_read };
    }
    if (b && b.cache_read != null) notes.push(`batch cached input ${b.cache_read}`);
    notes.push('regional processing (data residency) adds 10% for models released on or after 2026-03-05');
    rows.push(
      row({
        provider: 'openai',
        model: id,
        input: s.input,
        output: s.output,
        cache_write: cw(s),
        cache_read: s.cache_read,
        batch_input: b?.input ?? null,
        batch_output: b?.output ?? null,
        long_context,
        other_tiers: other,
        source_url: src,
        checked,
        note: notes.join('; '),
      }),
    );
  }
  return { rows, unknown: [] };
}
