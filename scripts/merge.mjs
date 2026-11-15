// How a daily refresh combines the freshly parsed rows with the previous prices.json.
// Node standard library only.

const DELISTED = /no longer on the pricing page/;

/**
 * Rows for the parsed providers, in order: for a provider whose page failed, its previous rows
 * unchanged (one broken page never deletes data); otherwise the fresh rows, followed by every
 * previous row the page no longer lists, kept with its last listed prices and marked retired.
 * Code that still calls a delisted model (or prices old logs) gets the price the provider last
 * published, and the note says since when the page no longer lists it.
 *
 * Returns { merged, delisted } where `delisted` holds the "provider/model" keys that left their
 * page in this run. Rows carried over from an earlier run are returned as the same objects.
 */
export function mergeParsed(providers, oldRows, fresh, failed, today) {
  const merged = [];
  const delisted = [];
  for (const provider of providers) {
    if (failed.has(provider)) {
      merged.push(...oldRows.filter((m) => m.provider === provider));
      continue;
    }
    const rows = fresh.filter((m) => m.provider === provider);
    merged.push(...rows);
    const now = new Set(rows.map((m) => m.model));
    for (const m of oldRows) {
      if (m.provider !== provider || now.has(m.model)) continue;
      if (m.status === 'retired' && DELISTED.test(m.note || '')) {
        merged.push(m);
        continue;
      }
      const note = `no longer on the pricing page since ${today}; prices as last listed (checked ${m.checked})`;
      merged.push({ ...m, status: 'retired', note: m.note ? `${note}; ${m.note}` : note });
      delisted.push(`${provider}/${m.model}`);
    }
  }
  return { merged, delisted };
}

/** True for a row kept by `mergeParsed` after its model left the provider's page. */
export function isDelisted(row) {
  return row.status === 'retired' && DELISTED.test(row.note || '');
}
