// Amazon Bedrock prices from AWS's own price list (the AWS Price List bulk API offer file for
// AmazonBedrock in us-east-1). Node standard library only.
//
// Each product in the offer file is one meter (a usage type such as
// "USE1-DeepSeek-R1-input-tokens" or "USE1-deepseek.v3.2-mantle-output-tokens-batch") with the
// model named by its `provider` and `model` attributes, and a price per 1K tokens in its
// OnDemand term. This reads the in-Region on-demand meters: standard input, output and cache
// read, Batch inference, and the flex and priority service tiers. Global cross-Region meters,
// latency-optimized meters, custom-model meters and provisioned throughput are left out.

import { ANTHROPIC_IDS, BEDROCK_ANTHROPIC_IDS, BEDROCK_ANTHROPIC_SKIP, BEDROCK_IDS, BEDROCK_SKIP, SOURCES } from '../meta.mjs';
import { ParseError, row } from './common.mjs';

const REGION = 'us-east-1';

/** "0.0013500000" USD per 1K tokens -> 1.35 USD per 1M, on the decimal text (no float error). */
export function per1kToPer1m(s) {
  const t = String(s).trim();
  if (!/^\d+(\.\d+)?$/.test(t)) throw new ParseError(`bedrock: price "${t}" is not a plain decimal`);
  let [int, frac = ''] = t.split('.');
  frac = frac.padEnd(3, '0');
  const whole = (int + frac.slice(0, 3)).replace(/^0+(?=\d)/, '');
  const rest = frac.slice(3).replace(/0+$/, '');
  return Number(rest ? `${whole}.${rest}` : whole);
}

// The meter a usage type measures, or null for meters this table does not carry.
function meter(usagetype, feature) {
  const u = usagetype.replace(/^[A-Z0-9]+-/, '').toLowerCase();
  const f = (feature || '').toLowerCase();
  if (/custom-model|customization|provisioned|cross-region|global|latency|long-context|reserved/.test(u)) return null;
  if (/customization|provisioned/.test(f)) return null;
  if (/audio|image|video|speech/.test(u)) return null;
  let kind = null;
  if (/cache-read/.test(u)) kind = 'cache_read';
  else if (/cache-write/.test(u)) return null;
  else if (/input-tokens?(-count)?(-|$)/.test(u)) kind = 'input';
  else if (/output-tokens?(-count)?(-|$)/.test(u)) kind = 'output';
  if (!kind) return null;
  let tier = 'standard';
  if (/-batch$/.test(u) || f === 'batch inference') tier = 'batch';
  else if (/-flex$/.test(u)) tier = 'flex';
  else if (/-priority$/.test(u)) tier = 'priority';
  else if (!/-standard$/.test(u) && !/-(input|output)-tokens?(-count)?$|-token-count$/.test(u)) return null;
  return { kind, tier, mantle: /-mantle-/.test(u) };
}

/**
 * Parses the AmazonBedrock offer file (JSON text). Returns { rows, unknown }: one row per model
 * with a standard input and output price, keyed through BEDROCK_IDS; `unknown` lists price-list
 * names that have token prices and no id mapping yet.
 */
export function parseBedrock(text, checked) {
  let doc;
  try {
    doc = JSON.parse(text);
  } catch (e) {
    throw new ParseError(`bedrock: offer file is not JSON (${e.message})`);
  }
  if (!doc || !doc.products || !doc.terms || !doc.terms.OnDemand) throw new ParseError('bedrock: offer file has no products or OnDemand terms');
  const byName = new Map();
  for (const [sku, p] of Object.entries(doc.products)) {
    const a = p.attributes || {};
    if (a.regionCode !== REGION || !a.usagetype) continue;
    const m = meter(a.usagetype, a.feature);
    if (!m) continue;
    const name = `${a.provider ?? ''}|${a.model ?? ''}`;
    for (const offer of Object.values(doc.terms.OnDemand[sku] || {})) {
      for (const d of Object.values(offer.priceDimensions || {})) {
        if (d.unit !== '1K tokens') continue;
        const usd = d.pricePerUnit && d.pricePerUnit.USD;
        if (usd === undefined) continue;
        const v = per1kToPer1m(usd);
        if (!(v > 0)) continue;
        if (!byName.has(name)) byName.set(name, {});
        const t = byName.get(name);
        const k = `${m.tier}.${m.kind}`;
        // The bedrock-runtime meter and the bedrock-mantle meter of a model carry the same
        // price; when they differ, the runtime meter (the id this table keys on) wins.
        if (t[k] === undefined || (t[k].mantle && !m.mantle)) t[k] = { v, mantle: m.mantle };
      }
    }
  }

  const rows = [];
  const unknown = [];
  const done = new Set();
  const src = SOURCES.bedrock.page;
  for (const [name, t] of [...byName.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
    const g = (k) => (t[k] ? t[k].v : null);
    if (g('standard.input') === null || g('standard.output') === null) continue;
    const id = BEDROCK_IDS[name];
    if (!id) {
      if (!BEDROCK_SKIP.includes(name)) unknown.push(name);
      continue;
    }
    if (done.has(id.model)) continue;
    done.add(id.model);
    const other = {};
    for (const tier of ['flex', 'priority']) {
      if (g(`${tier}.input`) !== null && g(`${tier}.output`) !== null) {
        other[tier] = { input: g(`${tier}.input`), output: g(`${tier}.output`) };
        if (g(`${tier}.cache_read`) !== null) other[tier].cache_read = g(`${tier}.cache_read`);
      }
    }
    rows.push(
      row({
        provider: 'bedrock',
        model: id.model,
        name: id.name,
        aliases: id.aliases,
        input: g('standard.input'),
        output: g('standard.output'),
        cache_write: null,
        cache_read: g('standard.cache_read'),
        batch_input: g('batch.input'),
        batch_output: g('batch.output'),
        other_tiers: other,
        source_url: src,
        checked,
        note:
          `on-demand price in us-east-1 from the AWS Price List API (price list name "${name}"); other Regions and global. inference profiles can bill differently` +
          (Object.keys(other).length ? `; other_tiers holds the ${Object.keys(other).join(' and ')} service tier prices` : '') +
          `; model id from https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-${id.card}.html`,
      }),
    );
  }
  if (rows.length < 20) throw new ParseError(`bedrock: only ${rows.length} models priced; the offer file layout changed`);
  rows.sort((a, b) => (a.model < b.model ? -1 : a.model > b.model ? 1 : 0));
  return { rows, unknown };
}

/** The offer file cut down to the us-east-1 token meters the parser reads, for test/pages. */
export function excerptBedrock(text) {
  const doc = JSON.parse(text);
  const products = {};
  const onDemand = {};
  for (const [sku, p] of Object.entries(doc.products)) {
    const a = p.attributes || {};
    if (a.regionCode !== REGION || !a.usagetype || !meter(a.usagetype, a.feature)) continue;
    products[sku] = { attributes: { provider: a.provider, model: a.model, usagetype: a.usagetype, feature: a.feature, regionCode: a.regionCode } };
    const terms = {};
    for (const [k, offer] of Object.entries(doc.terms.OnDemand[sku] || {})) {
      const dims = {};
      for (const [dk, d] of Object.entries(offer.priceDimensions || {})) dims[dk] = { unit: d.unit, pricePerUnit: { USD: d.pricePerUnit && d.pricePerUnit.USD } };
      terms[k] = { priceDimensions: dims };
    }
    onDemand[sku] = terms;
  }
  return JSON.stringify({ note: 'Excerpt of the AWS Price List offer file for AmazonBedrock (us-east-1), kept only for parser tests.', publicationDate: doc.publicationDate, products, terms: { OnDemand: onDemand } }) + '\n';
}

// -- Claude on Bedrock ---------------------------------------------------------------------------
//
// Claude is billed through the AmazonBedrockFoundationModels offer file (AWS Marketplace meters
// such as "USE1-MP:USE1_InputTokenCount_Global-Units" or
// "USE1-MP:USE1_input_tokens_long_ctx_standard-Units", priced per 1M tokens). Each Claude model
// has two sets of on-demand meters: global (the global. cross-Region inference profile) and
// regional (the in-Region id and the geographic profiles us., eu., apac., jp., au., ca.). For
// Claude Haiku 4.5, Sonnet 4.5, Opus 4.5 and later the regional meters are 10% above the global ones; Anthropic's
// pricing page says Bedrock sets these prices itself. One row per endpoint kind is emitted.

/** "3.3000000000" USD per 1M tokens -> 3.3, on the decimal text. */
function perMillion(s) {
  const t = String(s).trim();
  if (!/^\d+(\.\d+)?$/.test(t)) throw new ParseError(`bedrock: price "${t}" is not a plain decimal`);
  return Number(t);
}

// The Claude meter a usage type measures, or null for meters this table does not carry
// (latency-optimized, provisioned and reserved throughput).
function claudeMeter(usagetype) {
  const u = usagetype
    .replace(/^[A-Z0-9]+-(MP:)?[A-Z0-9]+_/, '')
    .replace(/-Units$/, '')
    .toLowerCase();
  if (/latency|reserved|tpm|provisioned|custom/.test(u)) return null;
  let kind = null;
  if (/^cacheread|^cache_read/.test(u)) kind = 'cache_read';
  else if (/^cachewrite1h|^cache_write_tokens_1h/.test(u)) kind = 'cache_write_1h';
  else if (/^cachewrite|^cache_write/.test(u)) kind = 'cache_write_5m';
  else if (/^input|^millionbatchinput/.test(u)) kind = 'input';
  else if (/^output|^millionbatchoutput/.test(u)) kind = 'output';
  if (!kind) return null;
  return { kind, scope: /global/.test(u) ? 'global' : 'regional', batch: /batch/.test(u), long: /long_ctx/.test(u) };
}

const LONG_CONTEXT_APPLIES = 'prompt (all input tokens incl. cache reads and writes) is over threshold_tokens';

function isClaudeMeter(a) {
  return (
    a.regionCode === REGION &&
    a.provider === 'Anthropic' &&
    !!a.usagetype &&
    !!a.model &&
    (a.feature === 'On-demand Inference' || a.feature === 'Batch Inference') &&
    claudeMeter(a.usagetype) !== null
  );
}

/**
 * Parses the AmazonBedrockFoundationModels offer file (JSON text) for Claude. `anthropicRows` are
 * the Anthropic rows of the same run: a Bedrock row with long-context meters takes the
 * long-context threshold from the Anthropic row of the same model (the price list has the
 * long-context prices but not the prompt size where they start). Returns { rows, unknown }.
 */
export function parseBedrockAnthropic(text, checked, anthropicRows = []) {
  let doc;
  try {
    doc = JSON.parse(text);
  } catch (e) {
    throw new ParseError(`bedrock: foundation-models offer file is not JSON (${e.message})`);
  }
  if (!doc || !doc.products || !doc.terms || !doc.terms.OnDemand) throw new ParseError('bedrock: foundation-models offer file has no products or OnDemand terms');
  const byName = new Map();
  for (const [sku, p] of Object.entries(doc.products)) {
    const a = p.attributes || {};
    // Skipped names are not read at all (retired models have meters this parser does not know).
    if (!isClaudeMeter(a) || BEDROCK_ANTHROPIC_SKIP.includes(a.model)) continue;
    const m = claudeMeter(a.usagetype);
    for (const offer of Object.values(doc.terms.OnDemand[sku] || {})) {
      for (const d of Object.values(offer.priceDimensions || {})) {
        if (d.unit !== '1M tokens') continue;
        const usd = d.pricePerUnit && d.pricePerUnit.USD;
        if (usd === undefined) continue;
        const v = perMillion(usd);
        if (!(v > 0)) continue;
        if (!byName.has(a.model)) byName.set(a.model, {});
        const t = byName.get(a.model);
        const k = `${m.scope}.${m.long ? 'long.' : ''}${m.batch ? 'batch_' : ''}${m.kind}`;
        if (t[k] !== undefined && t[k] !== v) throw new ParseError(`bedrock: ${a.model} has two prices for ${k} (${t[k]} and ${v}, ${a.usagetype})`);
        t[k] = v;
      }
    }
  }

  const rows = [];
  const unknown = [];
  const src = SOURCES.bedrock.page;
  for (const [name, t] of [...byName.entries()].sort(([a], [b]) => (a < b ? -1 : a > b ? 1 : 0))) {
    const id = BEDROCK_ANTHROPIC_IDS[name];
    if (!id) {
      if (t['regional.input'] !== undefined || t['global.input'] !== undefined) unknown.push(`Anthropic|${name}`);
      continue;
    }
    const profiles = id.aliases.filter((a) => !a.startsWith('anthropic.')).map((a) => a.split('.')[0] + '.');
    for (const scope of ['regional', 'global']) {
      const g = (k) => (t[`${scope}.${k}`] === undefined ? null : t[`${scope}.${k}`]);
      if (g('input') === null || g('output') === null) continue;
      const model = scope === 'global' ? id.global : id.model;
      if (!model) continue;
      const cw = (pre) => {
        const o = {};
        if (g(`${pre}cache_write_5m`) !== null) o['5m'] = g(`${pre}cache_write_5m`);
        if (g(`${pre}cache_write_1h`) !== null) o['1h'] = g(`${pre}cache_write_1h`);
        return Object.keys(o).length ? o : null;
      };
      let longContext = null;
      if (g('long.input') !== null && g('long.output') !== null) {
        const makerId = ANTHROPIC_IDS[name] && ANTHROPIC_IDS[name].model;
        const maker = anthropicRows.find((r) => r.provider === 'anthropic' && r.model === makerId);
        const threshold = maker && maker.long_context && maker.long_context.threshold_tokens;
        if (!threshold) throw new ParseError(`bedrock: ${name} has long-context prices but the Anthropic row gives no threshold`);
        longContext = {
          threshold_tokens: threshold,
          applies_when: maker.long_context.applies_when || LONG_CONTEXT_APPLIES,
          input: g('long.input'),
          output: g('long.output'),
          cache_write: cw('long.'),
          cache_read: g('long.cache_read'),
          batch_input: g('long.batch_input'),
          batch_output: g('long.batch_output'),
        };
      }
      const where =
        scope === 'global'
          ? `the global cross-Region inference profile (${id.global})`
          : `regional endpoints: the in-Region id and the geographic inference profiles${profiles.length ? ` (${profiles.join(', ')})` : ''}`;
      rows.push(
        row({
          provider: 'bedrock',
          model,
          name: `${name} (${scope})`,
          aliases: scope === 'global' ? [] : id.aliases,
          input: g('input'),
          output: g('output'),
          cache_write: cw(''),
          cache_read: g('cache_read'),
          batch_input: g('batch_input'),
          batch_output: g('batch_output'),
          long_context: longContext,
          source_url: src,
          checked,
          note:
            `Amazon Bedrock's price for ${where}, from the AWS Price List API (AmazonBedrockFoundationModels offer, us-east-1, price list name "${name}")` +
            (longContext ? '; long-context threshold from the Anthropic row of the same model' : '') +
            `; model ids from https://docs.aws.amazon.com/bedrock/latest/userguide/model-card-${id.card}.html`,
        }),
      );
    }
  }
  if (rows.length < 10) throw new ParseError(`bedrock: only ${rows.length} Claude rows priced; the foundation-models offer file layout changed`);
  rows.sort((a, b) => (a.model < b.model ? -1 : a.model > b.model ? 1 : 0));
  return { rows, unknown };
}

/** The foundation-models offer file cut down to the us-east-1 Claude token meters, for test/pages. */
export function excerptBedrockAnthropic(text) {
  const doc = JSON.parse(text);
  const products = {};
  const onDemand = {};
  for (const [sku, p] of Object.entries(doc.products)) {
    const a = p.attributes || {};
    if (!isClaudeMeter(a)) continue;
    products[sku] = { attributes: { provider: a.provider, model: a.model, usagetype: a.usagetype, feature: a.feature, regionCode: a.regionCode } };
    const terms = {};
    for (const [k, offer] of Object.entries(doc.terms.OnDemand[sku] || {})) {
      const dims = {};
      for (const [dk, d] of Object.entries(offer.priceDimensions || {})) dims[dk] = { unit: d.unit, pricePerUnit: { USD: d.pricePerUnit && d.pricePerUnit.USD } };
      terms[k] = { priceDimensions: dims };
    }
    onDemand[sku] = terms;
  }
  return (
    JSON.stringify({
      note: 'Excerpt of the AWS Price List offer file for AmazonBedrockFoundationModels (us-east-1, Claude token meters), kept only for parser tests.',
      publicationDate: doc.publicationDate,
      products,
      terms: { OnDemand: onDemand },
    }) + '\n'
  );
}
