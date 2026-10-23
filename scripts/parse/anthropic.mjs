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
    const e = byName.get(n.name) ?? { status: n.status };
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
    if (e.status === 'retired') notes.push('page marks this model retired on the Claude API; still sold on some cloud platforms');
    notes.push('US-only inference (inference_geo "us") is 1.1x on Claude 4.6 and later');
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
        source_url: src,
        checked,
        note: notes.join('; '),
      }),
    );
  }
  for (const name of Object.keys(ANTHROPIC_IDS)) {
    if (!byName.has(name)) throw new ParseError(`anthropic: ${name} is no longer on the pricing page`);
  }
  return { rows, unknown };
}
