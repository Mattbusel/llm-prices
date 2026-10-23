// Helpers shared by the page parsers. Node standard library only.

export class ParseError extends Error {}

/** "$12.50 / MTok<sup>1</sup>" -> 12.5, "-" -> null. Throws on anything else. */
export function money(cell, where) {
  const s = String(cell).replace(/<[^>]+>/g, '').trim();
  if (s === '-' || s === '' || s === '\u2014') return null;
  const m = s.match(/^\$\s*([0-9]+(?:\.[0-9]+)?)/);
  if (!m) throw new ParseError(`expected a $ price at ${where}, got "${s}"`);
  return Number(m[1]);
}

/** First "$x" in a free-text cell, or null when the cell has none. */
export function firstMoney(cell) {
  const m = String(cell).match(/\$\s*([0-9]+(?:\.[0-9]+)?)/);
  return m ? Number(m[1]) : null;
}

/** Splits a markdown table row into trimmed cells. */
export function cells(line) {
  return line.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((c) => c.trim());
}

/**
 * Returns every markdown table in `text` as { header: string[], rows: string[][], start: index }.
 * A table is a run of lines starting with "|" whose second line is the --- separator.
 */
export function tables(text) {
  const lines = text.split(/\r?\n/);
  const out = [];
  let offset = 0;
  const offsets = lines.map((l) => {
    const o = offset;
    offset += l.length + 1;
    return o;
  });
  for (let i = 0; i < lines.length - 1; i++) {
    if (!lines[i].trim().startsWith('|')) continue;
    if (!/^\s*\|\s*:?-{3,}/.test(lines[i + 1])) continue;
    const header = cells(lines[i]);
    const rows = [];
    let j = i + 2;
    while (j < lines.length && lines[j].trim().startsWith('|')) {
      rows.push(cells(lines[j]));
      j++;
    }
    out.push({ header, rows, start: offsets[i] });
    i = j - 1;
  }
  return out;
}

const FIELDS = ['input', 'output', 'cache_read', 'batch_input', 'batch_output'];

/** Builds a row with a fixed key order so diffs stay readable. */
export function row(r) {
  const o = {
    provider: r.provider,
    model: r.model,
    name: r.name ?? null,
    aliases: r.aliases ?? [],
    status: r.status ?? 'listed',
    unit: 'USD per 1M tokens',
  };
  for (const f of ['input', 'output']) o[f] = r[f] ?? null;
  o.cache_write = r.cache_write ?? null;
  o.cache_read = r.cache_read ?? null;
  o.batch_input = r.batch_input ?? null;
  o.batch_output = r.batch_output ?? null;
  o.long_context = r.long_context ?? null;
  o.other_tiers = r.other_tiers && Object.keys(r.other_tiers).length ? r.other_tiers : null;
  o.scheduled = r.scheduled && r.scheduled.length ? r.scheduled : null;
  o.source_url = r.source_url;
  o.checked = r.checked;
  const missing = ['input', 'output', 'cache_read', 'batch_input', 'batch_output'].filter((f) => o[f] === null);
  const parts = [r.note, missing.length ? `null means the page lists no price: ${missing.join(', ')}` : null].filter(Boolean);
  o.note = parts.length ? parts.join('; ') : null;
  return o;
}

export { FIELDS };
