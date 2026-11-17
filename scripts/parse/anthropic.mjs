import { ANTHROPIC_IDS, ANTHROPIC_SKIP, SOURCES } from '../meta.mjs';
import { ParseError, money, row, tables } from './common.mjs';

// Splits "Claude Haiku 5.5 (for prompts over 100,000 tokens)" or
// "Claude Opus 4.1 ([retired, except on ...](...))" into name + qualifiers.
function splitName(cell) {
  const raw = cell.replace(/<[^>]+>/g, '').trim();
  const name = raw.replace(/\s*\(.*$/, '').trim();
  const status = /\(\[?retired/i.test(raw) ? 'retired' : /limited availability/i.test(raw) ? 'limited' : 'listed';
  let tier = 'base';
  let threshold = null;
  const over = raw.match(/for prompts over ([0-9,]+) tokens/i);
  const upTo = raw.match(/for prompts up to ([0-9,]+) tokens/i);
  if (over) {
    tier = 'long';
    threshold = Number(over[1].replace(/,/g, ''));
  } else if (upTo) {
    threshold = Number(upTo[1].replace(/,/g, ''));
  }
  return { name, status, tier, threshold };
}

export function parseAnthropic(md, checked) {
  const src = SOURCES.anthropic.page;
  const all = tables(md);
  const main = all.find((t) => t.header[1] === 'Base input tokens' && t.header.includes('5m cache writes'));
  const batch = all.find((t) => t.header[1] === 'Batch input' && t.header[2] === 'Batch output');
  if (!main) throw new ParseError('anthropic: model pricing table not found');
  if (!batch) throw new ParseError('anthropic: batch pricing table not found');
  const fastStart = md.indexOf('### Fast mode pricing');
  const fast = all.find((t) => t.start > fastStart && t.header[1] === 'Input' && t.header[2] === 'Output');

  // "### Data residency pricing": "For Claude 4.6 and later models, specifying US-only
  // inference through the `inference_geo` parameter incurs a 1.1x multiplier on all token
  // pricing categories". Read the first version and the multiplier from the page.
  const geoStart = md.indexOf('### Data residency pricing');
  if (geoStart < 0) throw new ParseError('anthropic: data residency pricing section not found');
  const geoEnd = md.indexOf('\n### ', geoStart + 5);
  const geoText = md.slice(geoStart, geoEnd > 0 ? geoEnd : undefined);
  const geoMatch = geoText.match(/For Claude ([0-9]+)\.([0-9]+) and later models, specifying US-only inference through the `inference_geo` parameter incurs a ([0-9.]+)x multiplier on all token pricing categories/);
  if (!geoMatch) throw new ParseError('anthropic: data residency multiplier not readable');
  const geoFrom = [Number(geoMatch[1]), Number(geoMatch[2])];
  const geoMultiplier = Number(geoMatch[3]);
  // "Claude Opus 4.6" -> [4, 6], "Claude Fable 5" -> [5, 0].
  const versionOf = (name) => {
    const v = name.match(/([0-9]+)(?:\.([0-9]+))?\s*$/);
    return v ? [Number(v[1]), Number(v[2] ?? 0)] : null;
  };
  const geoApplies = (name) => {
    const v = versionOf(name);
    return v !== null && (v[0] > geoFrom[0] || (v[0] === geoFrom[0] && v[1] >= geoFrom[1]));
  };

  // Footnotes under the main table: "*<sup>3 The $2/$10 ... will not occur.</sup>*". A price
  // cell that carries <sup>N</sup> gets footnote N copied into its row's note, so promotional,
  // cancelled or special cache prices are explained next to the number.
  const footnotes = new Map();
  for (const m of md.matchAll(/^\*<sup>([0-9]+) (.+?)<\/sup>\*\s*$/gm)) footnotes.set(m[1], m[2].replace(/<[^>]+>/g, '').trim().replace(/\.$/, ''));
  const cellNotes = (cells) => {
    const out = [];
    for (const c of cells) for (const m of c.matchAll(/<sup>([0-9]+)<\/sup>/g)) if (footnotes.has(m[1]) && !out.includes(footnotes.get(m[1]))) out.push(footnotes.get(m[1]));
    return out;
  };

  const byName = new Map();
  const unknown = [];
  for (const r of main.rows) {
    const n = splitName(r[0]);
    if (ANTHROPIC_SKIP.includes(n.name)) continue;
    if (!ANTHROPIC_IDS[n.name]) {
      unknown.push(n.name);
      continue;
    }
    const where = `anthropic ${n.name}`;
    const p = {
      input: money(r[1], where),
      cache_write: { '5m': money(r[2], where), '1h': money(r[3], where) },
      cache_read: money(r[4], where),
      output: money(r[5], where),
    };
    const e = byName.get(n.name) ?? { status: n.status, footnotes: [] };
    for (const f of cellNotes(r.slice(1))) if (!e.footnotes.includes(f)) e.footnotes.push(f);
    if (n.tier === 'long') e.long = { threshold: n.threshold, ...p };
    else e.base = p;
    byName.set(n.name, e);
  }
  for (const r of batch.rows) {
    const n = splitName(r[0]);
    const e = byName.get(n.name);
    if (!e) continue;
    const b = { batch_input: money(r[1], `batch ${n.name}`), batch_output: money(r[2], `batch ${n.name}`) };
    if (n.tier === 'long') Object.assign(e.long, b);
    else Object.assign(e.base, b);
  }
  const fastByName = new Map();
  if (fast) {
    for (const r of fast.rows) {
      for (const name of r[0].split('/').map((s) => s.trim())) {
        fastByName.set(name, { input: money(r[1].replace(/USD\s*/, ''), `fast ${name}`), output: money(r[2].replace(/USD\s*/, ''), `fast ${name}`) });
      }
    }
  }

  const rows = [];
  for (const [name, e] of byName) {
    if (!e.base) throw new ParseError(`anthropic: ${name} has no base price row`);
    const ids = ANTHROPIC_IDS[name];
    const notes = [];
    let long_context = null;
    if (e.long) {
      long_context = {
        threshold_tokens: e.long.threshold,
        applies_when: 'prompt (all input tokens incl. cache reads and writes) is over threshold_tokens',
        input: e.long.input,
        output: e.long.output,
        cache_write: e.long.cache_write,
        cache_read: e.long.cache_read,
        batch_input: e.long.batch_input ?? null,
        batch_output: e.long.batch_output ?? null,
      };
    }
    const other = {};
    if (fastByName.has(name)) {
      other.fast = fastByName.get(name);
      notes.push('fast mode (other_tiers.fast) is a separate premium tier; cache multipliers apply on top');
    }
    for (const f of e.footnotes) notes.push(`page footnote: ${f}`);
    if (e.status === 'retired') notes.push('page marks this model retired on the Claude API; still sold on some cloud platforms');
    const geo = geoApplies(name) ? { us: geoMultiplier } : null;
    if (geo) notes.push(`US-only inference (inference_geo "us") is ${geoMultiplier}x on every price (inference_geo field)`);
    else notes.push('the page says this model does not take the inference_geo parameter (data residency) and always uses standard pricing');
    rows.push(
      row({
        provider: 'anthropic',
        model: ids.model,
        name,
        aliases: ids.aliases,
        status: e.status,
        ...e.base,
        long_context,
        other_tiers: other,
        inference_geo: geo,
        source_url: src,
        checked,
        note: notes.join('; '),
      }),
    );
  }
  // A model that left the page keeps its last row (scripts/update.mjs marks it retired), but
  // most of the list missing means the page shape changed, not the models.
  const gone = Object.keys(ANTHROPIC_IDS).filter((name) => !byName.has(name));
  if (gone.length * 2 > Object.keys(ANTHROPIC_IDS).length) throw new ParseError(`anthropic: ${gone.join(', ')} not found on the pricing page`);
  return { rows, unknown };
}
