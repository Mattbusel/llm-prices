// OpenRouter prices from OpenRouter's own model list API (https://openrouter.ai/api/v1/models),
// the data https://openrouter.ai/models shows and bills by. Node standard library only.
//
// Prices there are USD per token as decimal strings ("0.0000025"). Each model becomes one row
// keyed by OpenRouter's id ("deepseek/deepseek-r1-0528"), so `openrouter/<id>` and
// lookup_provider("openrouter", "<id>") find OpenRouter's own price. A `:batch` variant becomes
// the batch price of its base model; `:free` variants, zero-priced and negative-priced (router)
// entries and models that do not output text are left out.

import { SOURCES } from '../meta.mjs';
import { ParseError, row } from './common.mjs';

/** "0.0000025" USD per token -> 2.5 USD per 1M tokens, on the decimal text (no float error). */
export function perTokenToPer1m(s) {
  const t = String(s).trim();
  if (!/^\d+(\.\d+)?$/.test(t)) return null;
  let [int, frac = ''] = t.split('.');
  frac = frac.padEnd(6, '0');
  const whole = (int + frac.slice(0, 6)).replace(/^0+(?=\d)/, '');
  const rest = frac.slice(6).replace(/0+$/, '');
  const v = Number(rest ? `${whole}.${rest}` : whole);
  return v > 0 ? v : null;
}

function prices(p) {
  return {
    input: perTokenToPer1m(p.prompt),
    output: perTokenToPer1m(p.completion),
    cache_read: p.input_cache_read === undefined ? null : perTokenToPer1m(p.input_cache_read),
  };
}

/**
 * Parses the /api/v1/models JSON text. Returns { rows, unknown } (unknown is always empty: every
 * entry carries its own id).
 */
export function parseOpenRouter(text, checked) {
  let doc;
  try {
    doc = JSON.parse(text);
  } catch (e) {
    throw new ParseError(`openrouter: model list is not JSON (${e.message})`);
  }
  if (!doc || !Array.isArray(doc.data)) throw new ParseError('openrouter: model list has no data array');
  const batch = new Map();
  for (const m of doc.data) {
    if (typeof m.id === 'string' && m.id.endsWith(':batch') && m.pricing) batch.set(m.id.slice(0, -6), prices(m.pricing));
  }
  const rows = [];
  const seen = new Set();
  for (const m of doc.data) {
    const id = m.id;
    if (typeof id !== 'string' || !m.pricing || id.includes(':') || seen.has(id)) continue;
    const outputs = (m.architecture && m.architecture.output_modalities) || ['text'];
    if (!outputs.includes('text')) continue;
    const base = prices(m.pricing);
    if (base.input === null || base.output === null) continue;
    seen.add(id);
    const b = batch.get(id);
    let longContext = null;
    const ov = (m.pricing.overrides || []).filter((o) => Number.isInteger(o.min_prompt_tokens) && o.min_prompt_tokens > 0);
    if (ov.length === 1) {
      const o = ov[0];
      const lc = prices({ prompt: o.prompt ?? m.pricing.prompt, completion: o.completion ?? m.pricing.completion, input_cache_read: o.input_cache_read });
      if (lc.input !== null && lc.output !== null) {
        longContext = {
          threshold_tokens: o.min_prompt_tokens - 1,
          applies_when: `prompts of ${o.min_prompt_tokens} tokens or more (OpenRouter pricing.overrides min_prompt_tokens)`,
          input: lc.input,
          output: lc.output,
          cache_write: null,
          cache_read: lc.cache_read,
          batch_input: null,
          batch_output: null,
        };
      }
    }
    const fees = ['request', 'web_search', 'image', 'internal_reasoning'].filter((k) => m.pricing[k] !== undefined && perTokenToPer1m(m.pricing[k]) !== null);
    rows.push(
      row({
        provider: 'openrouter',
        model: id,
        name: typeof m.name === 'string' ? m.name.replace(/\u2014/g, '-') : null,
        aliases: typeof m.canonical_slug === 'string' && m.canonical_slug !== id ? [m.canonical_slug] : [],
        input: base.input,
        output: base.output,
        cache_write: null,
        cache_read: base.cache_read,
        batch_input: b ? b.input : null,
        batch_output: b ? b.output : null,
        long_context: longContext,
        source_url: `https://openrouter.ai/${id}`,
        checked,
        note:
          `OpenRouter's price for this model (pricing field of ${SOURCES.openrouter.fetch})` +
          (b ? `; batch prices are the ${id}:batch entry` : '') +
          (ov.length > 1 ? '; the model has several prompt-length price overrides, only the base price is kept' : '') +
          (fees.length ? `; the API also lists ${fees.join(', ')} prices, which are not per-token and are left out` : ''),
      }),
    );
  }
  if (rows.length < 100) throw new ParseError(`openrouter: only ${rows.length} priced models; the API changed`);
  rows.sort((a, b) => (a.model < b.model ? -1 : a.model > b.model ? 1 : 0));
  return { rows, unknown: [] };
}

/** The model list cut down to the fields the parser reads, for test/pages. */
export function excerptOpenRouter(text) {
  const doc = JSON.parse(text);
  const data = doc.data.map((m) => ({
    id: m.id,
    canonical_slug: m.canonical_slug,
    name: m.name,
    architecture: { output_modalities: m.architecture && m.architecture.output_modalities },
    pricing: m.pricing,
  }));
  return JSON.stringify({ note: 'Excerpt of https://openrouter.ai/api/v1/models, kept only for parser tests.', data }, null, 1) + '\n';
}
