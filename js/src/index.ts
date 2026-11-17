/**
 * Dated, source-cited LLM API prices, with no dependencies and no network access.
 *
 * The same table as the Rust crate `llm-prices`, generated from
 * [prices.json](https://github.com/Mattbusel/llm-prices/blob/main/prices.json): Anthropic,
 * OpenAI, Google Gemini and DeepSeek (re-read from their pricing pages every day) plus Mistral,
 * xAI, Cohere, Groq, Together AI and Fireworks AI. Every row carries the provider's official
 * pricing page and the date the price was read from it. Prices are USD per 1M tokens.
 *
 * Data fields keep the snake_case names of prices.json (`cache_read`, `batch_input`,
 * `cache_write_5m`); functions and methods are camelCase.
 *
 * ```ts
 * import { lookup, lookupProvider } from '@mattbusel/llm-prices';
 *
 * const p = lookup('claude-sonnet-4-6')!;
 * p.costUsage({ input: 2048, cache_read: 45000, cache_write_5m: 3500, output: 1024 })!.total; // 0.048129
 * p.costUsageExact({ input: 2048, cache_read: 45000, cache_write_5m: 3500, output: 1024 })!.total; // '0.048129'
 * lookupProvider('groq', 'openai/gpt-oss-120b')!.provider; // 'groq'
 * ```
 *
 * @packageDocumentation
 */

import { GENERATED, ROWS, type RawTier } from './data.js';

export { GENERATED };

/** A price column, for {@link Price.rate}, {@link Price.rateStr} and {@link LongContext.rate}. */
export type RateName = 'input' | 'output' | 'cache_read' | 'cache_write' | 'batch_input' | 'batch_output';

/** A token count: a non-negative integer `number` or a `bigint`. */
export type Tokens = number | bigint;

/** One price tier: USD per 1M tokens, `null` when the page lists no price. */
export interface Tier {
  /** Uncached input price. */
  readonly input: number | null;
  /** Output price; reasoning tokens bill at this price. `null` for embedding models. */
  readonly output: number | null;
  /** Price of input tokens served from the prompt cache. */
  readonly cache_read: number | null;
  /** Cache write price per cache lifetime, for example `{ '5m': 2.5, '1h': 4 }`. */
  readonly cache_write: Readonly<Record<string, number>>;
  /** Batch API input price. */
  readonly batch_input: number | null;
  /** Batch API output price. */
  readonly batch_output: number | null;
}

/** Token counts from one API response, the input to {@link Price.costUsage}. Omitted fields are zero.
 *
 * | Field | Anthropic `usage` | OpenAI `usage` | Gemini `usageMetadata` |
 * |---|---|---|---|
 * | `input` | `input_tokens` | `prompt_tokens - prompt_tokens_details.cached_tokens` | `promptTokenCount - cachedContentTokenCount` |
 * | `cache_read` | `cache_read_input_tokens` | `prompt_tokens_details.cached_tokens` | `cachedContentTokenCount` |
 * | `cache_write_5m` | `cache_creation.ephemeral_5m_input_tokens` | 0 | 0 |
 * | `cache_write_1h` | `cache_creation.ephemeral_1h_input_tokens` | 0 | 0 |
 * | `output` | `output_tokens` | `completion_tokens` | `candidatesTokenCount + thoughtsTokenCount` |
 *
 * When an Anthropic response only reports the total `cache_creation_input_tokens`, those tokens
 * were written with the default 5-minute lifetime. Cache writes on other lifetimes (the OpenAI
 * rows' `30m` and `12h`) are not priced by `costUsage`, as in the Rust crate: add
 * `tokens * price.cacheWrite('30m') / 1e6` yourself.
 */
export interface Usage {
  /** Uncached input tokens, not counting cache reads or cache writes. */
  readonly input?: Tokens;
  /** Output tokens, reasoning or thinking tokens included. */
  readonly output?: Tokens;
  /** Input tokens served from the prompt cache. */
  readonly cache_read?: Tokens;
  /** Input tokens written to the cache with a 5-minute lifetime. */
  readonly cache_write_5m?: Tokens;
  /** Input tokens written to the cache with a 1-hour lifetime. */
  readonly cache_write_1h?: Tokens;
  /** The request went through the provider's Batch API. */
  readonly batch?: boolean;
}

/** Cost in USD of one response, split by token kind. Returned by {@link Price.costUsage}. */
export interface CostBreakdown {
  /** Cost of the uncached input tokens. */
  readonly input: number;
  /** Cost of the output tokens. */
  readonly output: number;
  /** Cost of the input tokens served from the prompt cache. */
  readonly cache_read: number;
  /** Cost of the cache writes, all lifetimes together. */
  readonly cache_write: number;
  /** Sum of the other cost fields. */
  readonly total: number;
  /** The prompt was over the model's long-context threshold, so long-context prices applied. */
  readonly long_context: boolean;
}

/**
 * {@link CostBreakdown} as exact decimal strings such as `'0.048129'`, for billing code that
 * keeps money in a decimal type (decimal.js, big.js, a database `NUMERIC`). Returned by
 * {@link Price.costUsageExact}.
 */
export interface ExactCostBreakdown {
  /** Cost of the uncached input tokens. */
  readonly input: string;
  /** Cost of the output tokens. */
  readonly output: string;
  /** Cost of the input tokens served from the prompt cache. */
  readonly cache_read: string;
  /** Cost of the cache writes, all lifetimes together. */
  readonly cache_write: string;
  /** Sum of the other cost fields. */
  readonly total: string;
  /** The prompt was over the model's long-context threshold, so long-context prices applied. */
  readonly long_context: boolean;
}

/** Prices that apply when a request is over a provider's long-context threshold. */
export interface LongContext extends Tier {
  /** Requests whose prompt (uncached input plus cache reads plus cache writes) is over this
   * many tokens pay these prices. */
  readonly threshold_tokens: number;
}

/** What {@link Price} is built from. Prices are USD per 1M tokens; omitted prices are `null`. */
export interface PriceInit {
  readonly provider: string;
  readonly model: string;
  readonly name?: string | null;
  readonly aliases?: readonly string[];
  readonly status?: string;
  readonly input?: number | null;
  readonly output?: number | null;
  readonly cache_read?: number | null;
  readonly cache_write?: Readonly<Record<string, number>>;
  readonly batch_input?: number | null;
  readonly batch_output?: number | null;
  readonly long_context?: LongContext | null;
  readonly source_url?: string;
  readonly checked?: string;
  readonly note?: string | null;
}

const RATES: readonly RateName[] = ['input', 'output', 'cache_read', 'cache_write', 'batch_input', 'batch_output'];

function pick<T>(tier: { readonly [K in Exclude<RateName, 'cache_write'>]: T | null } & { readonly cache_write: Readonly<Record<string, T>> }, rate: RateName, ttl?: string): T | null {
  if (!RATES.includes(rate)) throw new RangeError(`unknown price column ${JSON.stringify(rate)}; expected one of ${RATES.join(', ')}`);
  if (rate === 'cache_write') {
    if (ttl === undefined) throw new RangeError("rate('cache_write') needs a cache lifetime such as '5m' or '1h'");
    return Object.prototype.hasOwnProperty.call(tier.cache_write, ttl) ? (tier.cache_write[ttl] as T) : null;
  }
  if (ttl !== undefined) throw new RangeError(`only cache_write takes a cache lifetime, not ${JSON.stringify(rate)}`);
  return tier[rate];
}

function freezeTier<T extends Tier>(t: T): T {
  Object.freeze(t.cache_write);
  return Object.freeze(t);
}

/** The prices of one tier as plain numbers. */
function tierOf(t: Partial<Tier>): Tier {
  return {
    input: t.input ?? null,
    output: t.output ?? null,
    cache_read: t.cache_read ?? null,
    cache_write: { ...(t.cache_write ?? {}) },
    batch_input: t.batch_input ?? null,
    batch_output: t.batch_output ?? null,
  };
}

/** The long-context price for one column, USD per 1M tokens; `null` when it is not listed. */
export function longContextRate(lc: LongContext, rate: RateName, ttl?: string): number | null {
  return pick(lc, rate, ttl);
}

const M = 1_000_000;

/** One model's prices in USD per 1M tokens. `null` means the page lists no price. */
export class Price implements Tier {
  /** The provider that sells the model at these prices (a {@link Provider} value). */
  readonly provider: string;
  /** The API model id. */
  readonly model: string;
  /** The model's display name on the pricing page. */
  readonly name: string | null;
  /** Other ids the provider documents for the same model. */
  readonly aliases: readonly string[];
  /** `listed`, `retired` (still sold on some cloud platforms) or `limited`. */
  readonly status: string;
  readonly input: number | null;
  readonly output: number | null;
  readonly cache_read: number | null;
  readonly cache_write: Readonly<Record<string, number>>;
  readonly batch_input: number | null;
  readonly batch_output: number | null;
  /** Prices for requests over a long-context threshold, when the model has one. */
  readonly long_context: LongContext | null;
  /** The provider's official pricing page this row was read from. */
  readonly source_url: string;
  /** ISO date the row was checked against `source_url`. */
  readonly checked: string;
  /** Notes from the table (tiers, conditions, why a price is missing). */
  readonly note: string | null;

  /** A price row of your own; rows of the table come from {@link lookup} and {@link all}. */
  constructor(init: PriceInit) {
    this.provider = init.provider;
    this.model = init.model;
    this.name = init.name ?? null;
    this.aliases = Object.freeze([...(init.aliases ?? [])]);
    this.status = init.status ?? 'listed';
    const t = tierOf(init);
    this.input = t.input;
    this.output = t.output;
    this.cache_read = t.cache_read;
    this.cache_write = Object.freeze(t.cache_write);
    this.batch_input = t.batch_input;
    this.batch_output = t.batch_output;
    this.long_context = init.long_context ? freezeTier({ threshold_tokens: init.long_context.threshold_tokens, ...tierOf(init.long_context) }) : null;
    this.source_url = init.source_url ?? '';
    this.checked = init.checked ?? '';
    this.note = init.note ?? null;
    Object.freeze(this);
  }

  /** Cache write price for a cache lifetime such as `'5m'`, `'1h'`, `'30m'` or `'12h'`. */
  cacheWrite(ttl: string): number | null {
    return pick(this, 'cache_write', ttl);
  }

  /**
   * The standard price for one column, USD per 1M tokens; `null` when it is not listed.
   * `rate('cache_write', '1h')` takes the cache lifetime as `ttl`.
   */
  rate(rate: RateName, ttl?: string): number | null {
    return pick(this, rate, ttl);
  }

  /**
   * The standard price for one column exactly as prices.json writes it, for example `'0.075'`
   * or `'15'`. `null` when the price is not listed, or when this is not a row of the table with
   * that price unchanged.
   */
  rateStr(rate: RateName, ttl?: string): string | null {
    const row = EXACT.get(key(this.provider, this.model));
    return row ? checkedText(pick(row.base, rate, ttl), this.rate(rate, ttl)) : null;
  }

  /** The long-context price for one column exactly as prices.json writes it, or `null`. */
  longContextRateStr(rate: RateName, ttl?: string): string | null {
    const row = EXACT.get(key(this.provider, this.model));
    if (!row || !row.long_context || !this.long_context) return null;
    return checkedText(pick(row.long_context, rate, ttl), pick(this.long_context, rate, ttl));
  }

  /** {@link Price.provider} as a {@link Provider}; `null` for a provider name the package does not know. */
  providerKind(): Provider | null {
    return parseProvider(this.provider);
  }

  /**
   * Standard (non-batch, uncached) cost in USD of one request, with the long-context prices
   * when `inputTokens` is over the model's threshold. `null` when a needed price is not listed.
   * Same as {@link Price.costUsage} with only `input` and `output` set.
   */
  cost(inputTokens: Tokens, outputTokens: Tokens): number | null {
    return this.costUsage({ input: inputTokens, output: outputTokens })?.total ?? null;
  }

  /** {@link Price.cost} as an exact decimal string. */
  costExact(inputTokens: Tokens, outputTokens: Tokens): string | null {
    return this.costUsageExact({ input: inputTokens, output: outputTokens })?.total ?? null;
  }

  /**
   * Cost in USD of one response from its token counts, split by token kind. The rules of the
   * Rust crate's `Price::cost_usage`, with the same floating-point results:
   *
   * - **Long context.** When the model has a long-context tier and the prompt (input plus cache
   *   reads plus cache writes) is strictly over its threshold, every token bills at the
   *   long-context prices.
   * - **Batch.** With `batch: true`, input and output bill at the batch prices; cache reads and
   *   writes get the same discount as input (batch input price over standard input price).
   * - **Cache reads** bill at the cache-read price, or as ordinary input when none is listed.
   * - **Cache writes** bill at `cacheWrite('5m')` and `cacheWrite('1h')`.
   *
   * Returns `null` when a price it needs is not listed: input or output (batch prices with
   * `batch`, long-context prices above the threshold), or the cache-write price for a lifetime
   * with a non-zero token count. Throws a `RangeError` for a negative or fractional token count.
   */
  costUsage(usage: Usage): CostBreakdown | null {
    const u = counts(usage);
    const [r, lc] = this.ratesFor(u);
    const standardInput = r.input;
    if (standardInput === null) return null;
    let inRate: number, outRate: number, discount: number;
    if (u.batch) {
      if (r.batch_input === null || r.batch_output === null) return null;
      inRate = r.batch_input;
      outRate = r.batch_output;
      discount = standardInput > 0 ? r.batch_input / standardInput : 1;
    } else {
      if (r.output === null) return null;
      inRate = standardInput;
      outRate = r.output;
      discount = 1;
    }
    const readRate = r.cache_read === null ? inRate : r.cache_read * discount;
    const write = (ttl: string, tokens: bigint): number | null => {
      if (tokens === 0n) return 0;
      const rate = pick(r, 'cache_write', ttl);
      return rate === null ? null : Number(tokens) * rate * discount;
    };
    const w5 = write('5m', u.cache_write_5m);
    const w1 = write('1h', u.cache_write_1h);
    if (w5 === null || w1 === null) return null;
    // Same products and the same order of additions as the Rust crate, so the results are
    // bit-for-bit equal.
    const pInput = Number(u.input) * inRate;
    const pRead = Number(u.cache_read) * readRate;
    const pWrite = w5 + w1;
    const pOutput = Number(u.output) * outRate;
    return {
      input: pInput / M,
      output: pOutput / M,
      cache_read: pRead / M,
      cache_write: pWrite / M,
      total: (pInput + pRead + pWrite + pOutput) / M,
      long_context: lc,
    };
  }

  /**
   * {@link Price.costUsage} in exact decimals, returned as strings such as `'0.048129'`.
   *
   * The prices are the decimals prices.json lists and the rules are those of `costUsage`, so it
   * returns `null` in exactly the same cases. Each component is `tokens x price / 1,000,000`
   * computed with `bigint`, with no rounding. The only division is the batch discount on cache
   * tokens; it is exact whenever the ratio terminates (50% or 80% on every current row) and
   * otherwise rounds half-to-even at 28 significant digits. Strings have no trailing zeros and
   * no exponent.
   */
  costUsageExact(usage: Usage): ExactCostBreakdown | null {
    const u = counts(usage);
    const [r, lc] = this.ratesFor(u);
    const standardInput = dec(r.input);
    if (standardInput === null) return null;
    let inRate: Dec, outRate: Dec, num: Dec, den: Dec;
    if (u.batch) {
      const bi = dec(r.batch_input);
      const bo = dec(r.batch_output);
      if (bi === null || bo === null) return null;
      inRate = bi;
      outRate = bo;
      [num, den] = standardInput.n === 0n ? [ONE, ONE] : [bi, standardInput];
    } else {
      const o = dec(r.output);
      if (o === null) return null;
      [inRate, outRate, num, den] = [standardInput, o, ONE, ONE];
    }
    const same = cmp(num, den) === 0;
    const times = (tokens: bigint, rate: Dec): Dec => mul({ n: tokens, s: 0 }, rate);
    const discounted = (tokens: bigint, rate: Dec): Dec => {
      const p = times(tokens, rate);
      return same ? p : div(mul(p, num), den);
    };
    const cacheRead = dec(r.cache_read);
    const pRead = cacheRead === null ? times(u.cache_read, inRate) : discounted(u.cache_read, cacheRead);
    const write = (ttl: string, tokens: bigint): Dec | null => {
      if (tokens === 0n) return ZERO;
      const rate = dec(pick(r, 'cache_write', ttl));
      return rate === null ? null : discounted(tokens, rate);
    };
    const w5 = write('5m', u.cache_write_5m);
    const w1 = write('1h', u.cache_write_1h);
    if (w5 === null || w1 === null) return null;
    const pInput = times(u.input, inRate);
    const pWrite = add(w5, w1);
    const pOutput = times(u.output, outRate);
    const total = add(add(add(pInput, pRead), pWrite), pOutput);
    const perM = (v: Dec): string => format({ n: v.n, s: v.s + 6 });
    return {
      input: perM(pInput),
      output: perM(pOutput),
      cache_read: perM(pRead),
      cache_write: perM(pWrite),
      total: perM(total),
      long_context: lc,
    };
  }

  private ratesFor(u: Counts): [Tier, boolean] {
    const lc = this.long_context;
    if (lc && u.input + u.cache_read + u.cache_write_5m + u.cache_write_1h > BigInt(lc.threshold_tokens)) return [lc, true];
    return [this, false];
  }
}

// -- token counts ---------------------------------------------------------------------------

interface Counts {
  input: bigint;
  output: bigint;
  cache_read: bigint;
  cache_write_5m: bigint;
  cache_write_1h: bigint;
  batch: boolean;
}

function tokens(v: Tokens | undefined, name: string): bigint {
  if (v === undefined) return 0n;
  if (typeof v === 'bigint') {
    if (v < 0n) throw new RangeError(`usage.${name} must not be negative (${v})`);
    return v;
  }
  if (typeof v !== 'number' || !Number.isInteger(v) || v < 0) throw new RangeError(`usage.${name} must be a non-negative integer, not ${String(v)}`);
  return BigInt(v);
}

function counts(u: Usage): Counts {
  if (u === null || typeof u !== 'object') throw new TypeError('usage must be an object such as { input: 1000, output: 200 }');
  return {
    input: tokens(u.input, 'input'),
    output: tokens(u.output, 'output'),
    cache_read: tokens(u.cache_read, 'cache_read'),
    cache_write_5m: tokens(u.cache_write_5m, 'cache_write_5m'),
    cache_write_1h: tokens(u.cache_write_1h, 'cache_write_1h'),
    batch: Boolean(u.batch),
  };
}

/** Total prompt tokens: uncached input plus cache reads plus cache writes, the number compared
 * against a model's long-context threshold. */
export function promptTokens(usage: Usage): bigint {
  const u = counts(usage);
  return u.input + u.cache_read + u.cache_write_5m + u.cache_write_1h;
}

// -- exact decimals: n / 10^s with bigint n and s >= 0 ---------------------------------------

interface Dec {
  readonly n: bigint;
  readonly s: number;
}

const ZERO: Dec = { n: 0n, s: 0 };
const ONE: Dec = { n: 1n, s: 0 };
const pow10 = (k: number): bigint => 10n ** BigInt(k);

/** The decimal a price was written as: `String(number)` is the shortest text that reads back
 * to the same number, which for every price in the table is the text in prices.json. */
function dec(v: number | null): Dec | null {
  if (v === null || !Number.isFinite(v)) return null;
  const m = /^(-?)(\d+)(?:\.(\d+))?(?:e([+-]?\d+))?$/i.exec(String(v));
  if (!m) return null;
  const frac = m[3] ?? '';
  let n = BigInt((m[2] ?? '0') + frac);
  let s = frac.length - Number(m[4] ?? 0);
  if (s < 0) {
    n *= pow10(-s);
    s = 0;
  }
  return { n: m[1] ? -n : n, s };
}

function align(a: Dec, b: Dec): [bigint, bigint, number] {
  const s = Math.max(a.s, b.s);
  return [a.n * pow10(s - a.s), b.n * pow10(s - b.s), s];
}

function add(a: Dec, b: Dec): Dec {
  const [x, y, s] = align(a, b);
  return { n: x + y, s };
}

function cmp(a: Dec, b: Dec): number {
  const [x, y] = align(a, b);
  return x === y ? 0 : x < y ? -1 : 1;
}

function mul(a: Dec, b: Dec): Dec {
  return { n: a.n * b.n, s: a.s + b.s };
}

/** a / b (both non-negative, b non-zero): exact when it terminates, else rounded half-to-even
 * at 28 significant digits. */
function div(a: Dec, b: Dec): Dec {
  const num = a.n * pow10(b.s);
  const den = b.n * pow10(a.s);
  if (num === 0n) return ZERO;
  for (let k = 0; ; k++) {
    const scaled = num * pow10(k);
    let q = scaled / den;
    const rem = scaled % den;
    if (rem === 0n) return { n: q, s: k };
    if (q.toString().length >= 28) {
      const twice = 2n * rem;
      if (twice > den || (twice === den && q % 2n === 1n)) q += 1n;
      return { n: q, s: k };
    }
  }
}

/** Plain decimal text without trailing zeros or exponent. */
function format(d: Dec): string {
  const neg = d.n < 0n;
  let digits = (neg ? -d.n : d.n).toString();
  if (d.s > 0) {
    digits = digits.padStart(d.s + 1, '0');
    const int = digits.slice(0, digits.length - d.s);
    const frac = digits.slice(digits.length - d.s).replace(/0+$/, '');
    digits = frac ? `${int}.${frac}` : int;
  }
  return digits === '0' ? '0' : (neg ? '-' : '') + digits;
}

// -- the table ------------------------------------------------------------------------------

function numberTier(t: RawTier): Tier {
  const num = (s: string | null): number | null => (s === null ? null : Number(s));
  return {
    input: num(t.input),
    output: num(t.output),
    cache_read: num(t.cache_read),
    cache_write: Object.fromEntries(Object.entries(t.cache_write).map(([k, v]) => [k, Number(v)])),
    batch_input: num(t.batch_input),
    batch_output: num(t.batch_output),
  };
}

const key = (provider: string, model: string): string => `${provider}\u0000${model}`;

function checkedText(text: string | null, value: number | null): string | null {
  return text === null || value === null || Number(text) !== value ? null : text;
}

const PRICES: readonly Price[] = Object.freeze(
  ROWS.map(
    (r) =>
      new Price({
        provider: r.provider,
        model: r.model,
        name: r.name,
        aliases: r.aliases,
        status: r.status,
        ...numberTier(r),
        long_context: r.long_context ? { threshold_tokens: r.long_context.threshold_tokens, ...numberTier(r.long_context) } : null,
        source_url: r.source_url,
        checked: r.checked,
        note: r.note,
      }),
  ),
);

const EXACT: ReadonlyMap<string, { base: RawTier; long_context: RawTier | null }> = new Map(
  ROWS.map((r) => [key(r.provider, r.model), { base: r, long_context: r.long_context }]),
);

/** Every row in the table, in prices.json order: Anthropic, OpenAI, Google and DeepSeek first,
 * then Mistral, xAI, Cohere, Groq, Together AI and Fireworks AI. */
export function all(): readonly Price[] {
  return PRICES;
}

// -- providers ------------------------------------------------------------------------------

/** The canonical provider names. Rows of the table carry one of the first ten as
 * {@link Price.provider}; Vertex, Bedrock, Azure and OpenRouter have no rows of their own and
 * fall back to the model makers they resell (see {@link fallbackProviders}). */
export const Provider = Object.freeze({
  Anthropic: 'anthropic',
  OpenAi: 'openai',
  Google: 'google',
  Vertex: 'vertex',
  Bedrock: 'bedrock',
  Azure: 'azure',
  DeepSeek: 'deepseek',
  Mistral: 'mistral',
  Groq: 'groq',
  Together: 'together',
  Fireworks: 'fireworks',
  Xai: 'xai',
  Cohere: 'cohere',
  OpenRouter: 'openrouter',
} as const);

/** A canonical provider name. */
export type Provider = (typeof Provider)[keyof typeof Provider];

/** Every provider, in declaration order. */
export const PROVIDERS: readonly Provider[] = Object.freeze(Object.values(Provider));

const PROVIDER_NAMES: ReadonlyMap<string, Provider> = new Map(
  (
    [
      ['anthropic', ['anthropic', 'claude']],
      ['openai', ['openai', 'open-ai']],
      ['google', ['google', 'gemini', 'google-ai-studio', 'google-gemini', 'ai-studio', 'google-genai', 'google-generativeai', 'genai', 'google-ai', 'googleai']],
      ['vertex', ['vertex', 'vertex-ai', 'vertexai', 'google-vertex', 'google-vertex-ai', 'vertex-ai-beta', 'google-vertexai']],
      ['bedrock', ['bedrock', 'aws', 'aws-bedrock', 'amazon-bedrock', 'bedrock-converse', 'bedrock-runtime', 'amazon']],
      ['azure', ['azure', 'azure-openai', 'azure-ai', 'azure-foundry', 'microsoft-foundry', 'azure-ai-foundry']],
      ['deepseek', ['deepseek']],
      ['mistral', ['mistral', 'mistralai', 'mistral-ai']],
      ['groq', ['groq', 'groqcloud', 'groq-cloud']],
      ['together', ['together', 'together-ai', 'togetherai']],
      ['fireworks', ['fireworks', 'fireworks-ai', 'fireworksai']],
      ['xai', ['xai', 'x-ai', 'grok']],
      ['cohere', ['cohere', 'cohere-chat', 'cohere-ai']],
      ['openrouter', ['openrouter', 'open-router']],
    ] as const
  ).flatMap(([p, names]) => names.map((n) => [n, p] as const)),
);

const FALLBACKS: Readonly<Partial<Record<Provider, readonly Provider[]>>> = {
  vertex: ['google', 'anthropic'],
  bedrock: ['anthropic'],
  azure: ['openai'],
  openrouter: ['anthropic', 'openai', 'google', 'deepseek', 'mistral', 'xai', 'cohere'],
};

const PLATFORMS: ReadonlySet<string> = new Set(['vertex', 'bedrock', 'azure', 'openrouter']);

/** Providers that sell other companies' models at their own prices: the cloud platforms and the
 * open-weight hosts. An id that starts with one of these is only priced at that provider. */
const HOSTS: ReadonlySet<string> = new Set(['vertex', 'bedrock', 'azure', 'openrouter', 'groq', 'together', 'fireworks']);

/** The provider an id (already ASCII-lowercased) names at its start (`groq/...`,
 * `accounts/fireworks/...`) and the rest of the id. */
function idHint(lower: string): [Provider, string] | null {
  if (lower.startsWith('accounts/fireworks/')) return ['fireworks', lower];
  const i = lower.indexOf('/');
  if (i < 0) return null;
  const p = parseProvider(lower.slice(0, i));
  return p === null ? null : [p, lower.slice(i + 1)];
}

/** Length of `s` in UTF-8 bytes (the Rust crate limits provider names to 24 bytes). */
function utf8Length(s: string): number {
  let n = 0;
  for (const ch of s) {
    const c = ch.codePointAt(0) ?? 0;
    n += c < 0x80 ? 1 : c < 0x800 ? 2 : c < 0x10000 ? 3 : 4;
  }
  return n;
}

/**
 * Parses a provider name, ignoring ASCII case and treating `_`, `-` and spaces alike. Accepts
 * the canonical names and the spellings LiteLLM, LangChain, OpenRouter and the provider SDKs
 * use (`gemini`, `google_genai`, `vertex_ai`, `google_vertexai`, `bedrock_converse`,
 * `together_ai`, `x-ai`, ...). `null` for anything else.
 */
export function parseProvider(name: string): Provider | null {
  const trimmed = String(name).trim();
  if (utf8Length(trimmed) > 24) return null;
  return PROVIDER_NAMES.get(asciiLower(trimmed).replace(/[_ ]/g, '-')) ?? null;
}

/** The model makers whose list prices {@link Provider} lookups fall back to: Vertex AI to
 * Google and Anthropic, Bedrock to Anthropic, Azure to OpenAI, OpenRouter to every model maker
 * in the table. Empty for every other provider. */
export function fallbackProviders(provider: Provider): readonly Provider[] {
  return FALLBACKS[provider] ?? [];
}

/** The rows a provider sells, in table order. */
export function providerRows(provider: Provider | string): Price[] {
  const p = parseProvider(provider);
  return p === null ? [] : PRICES.filter((row) => row.provider === p);
}

/**
 * Finds `model` as sold by `provider` (a {@link Provider} or a name such as `'groq'`,
 * `'together_ai'` or `'vertex_ai'`), with the same id handling as {@link lookup}. When the
 * provider has no row for it, the rows of its {@link fallbackProviders} are tried, so Claude on
 * Bedrock gets Anthropic's row. `null` when the name is not a known provider or the table has
 * no price for the model there.
 */
export function lookupProvider(provider: Provider | string, model: string): Price | null {
  const p = parseProvider(provider);
  if (p === null) return null;
  const host = idHint(asciiLower(String(model).trim()))?.[0];
  // The id names another host or platform, which sells at its own price.
  if (host !== undefined && HOSTS.has(host) && host !== p && !fallbackProviders(host).includes(p)) return null;
  const found = resolve(model, (row) => row.provider === p);
  if (found) return found;
  const makers = fallbackProviders(p);
  return makers.length ? resolve(model, (row) => (makers as readonly string[]).includes(row.provider)) : null;
}

// -- model ids ------------------------------------------------------------------------------

const PREFIXES = [
  'models/',
  'anthropic/',
  'openai/',
  'google/',
  'gemini/',
  'vertex_ai/',
  'bedrock/',
  'azure/',
  'openrouter/',
  'deepseek/',
  'mistral/',
  'mistralai/',
  'xai/',
  'x-ai/',
  'cohere/',
  'groq/',
  'together/',
  'together_ai/',
  'fireworks/',
  'fireworks_ai/',
  'accounts/fireworks/models/',
  // Bedrock's vendor prefix with an optional cross-region inference profile.
  'us.anthropic.',
  'eu.anthropic.',
  'apac.anthropic.',
  'global.anthropic.',
  'anthropic.',
];

const asciiLower = (s: string): string => s.replace(/[A-Z]/g, (c) => String.fromCharCode(c.charCodeAt(0) + 32));
const eqCi = (a: string, b: string): boolean => a.length === b.length && asciiLower(a) === asciiLower(b);
const isDigits = (s: string): boolean => /^[0-9]*$/.test(s);

function rsplit(s: string): [string, string] | null {
  const i = s.lastIndexOf('-');
  return i < 0 ? null : [s.slice(0, i), s.slice(i + 1)];
}

function stripPrefix(id: string): string {
  let s = id;
  for (;;) {
    const before = s;
    for (const p of PREFIXES) if (s.length >= p.length && eqCi(s.slice(0, p.length), p)) s = s.slice(p.length);
    if (s === before) return s;
  }
}

/** `id` without a trailing `-YYYYMMDD` (Anthropic) or `-YYYY-MM-DD` (OpenAI). */
function stripDate(id: string): string | null {
  const a = rsplit(id);
  if (!a) return null;
  const [head, tail] = a;
  if (tail.length === 8 && tail.startsWith('20') && isDigits(tail)) return head;
  const b = rsplit(head);
  if (!b) return null;
  const [rest, month] = b;
  const c = rsplit(rest);
  if (!c) return null;
  const [base, year] = c;
  const shape = tail.length === 2 && month.length === 2 && year.length === 4 && year.startsWith('20');
  return shape && isDigits(tail) && isDigits(month) && isDigits(year) && base ? base : null;
}

/** `id` without a trailing `-latest`, then without a trailing snapshot date. */
function stripVersion(id: string): string {
  const s = id.length > 7 && eqCi(id.slice(-7), '-latest') ? id.slice(0, -7) : id;
  return stripDate(s) ?? s;
}

function ids(p: Price): string[] {
  return [p.model, ...p.aliases];
}

function normalize(lower: string): string {
  let s = stripPrefix(lower);
  const i = s.indexOf('-v');
  if (i >= 0) {
    const rest = s.slice(i + 2);
    if (/^[0-9]/.test(rest) && !rest.includes('-')) s = s.slice(0, i);
  }
  return s.replace('@', '-');
}

function resolve(id: string, keep: (p: Price) => boolean): Price | null {
  const rows = PRICES.filter(keep);
  const lower = asciiLower(String(id).trim());
  const exact = rows.find((p) => ids(p).some((m) => eqCi(m, lower)));
  if (exact) return exact;
  const s = normalize(lower);
  if (!s) return null;
  const strippedEq = (want: string): Price | undefined => rows.find((p) => ids(p).some((m) => eqCi(stripPrefix(m), want)));
  const found = strippedEq(s);
  if (found) return found;
  const base = stripVersion(s);
  if (base !== s) {
    const b = strippedEq(base);
    if (b) return b;
  }
  return (
    rows.find((p) =>
      ids(p).some((m) => {
        const u = stripDate(stripPrefix(m));
        return u !== null && eqCi(u, base);
      }),
    ) ?? null
  );
}

/**
 * Finds a model by id across every provider in the table.
 *
 * Accepts the documented id or an alias in any ASCII case, provider prefixes (`anthropic/`,
 * `models/`, `openrouter/`, LiteLLM's `together_ai/`, ...), Fireworks'
 * `accounts/fireworks/models/` path, Bedrock ids (`us.anthropic.claude-...-v1:0`), Vertex
 * `@date` suffixes, dated snapshot ids (`gpt-4o-2024-08-06` finds `gpt-4o`), `-latest` ids,
 * and the undated form of a dated id.
 *
 * When the id starts with a provider name and a slash (`groq/`, `together_ai/`, ...), that
 * provider is tried first. A host or platform path (`groq/`, `together_ai/`, `fireworks_ai/`,
 * `accounts/fireworks/models/`, `bedrock/`, `vertex_ai/`, `azure/`, `openrouter/`) is only
 * priced at that host, or at the model maker's list price where the platform charges it
 * ({@link fallbackProviders}); otherwise the result is `null`, never another provider's row,
 * so `accounts/fireworks/models/deepseek-r1` does not get DeepSeek's own price. A model maker's
 * prefix (`anthropic/`, `openai/`, ...) on a model that maker does not sell only matches rows
 * whose own id carries the same prefix (Groq's `openai/gpt-oss-120b`). Without a provider
 * prefix the first matching row in table order wins: model makers before the hosts that resell
 * open-weight models. Use {@link lookupProvider} whenever you know which provider served the
 * request.
 */
export function lookup(model: string): Price | null {
  const lower = asciiLower(String(model).trim());
  const hint = idHint(lower);
  if (hint) {
    const found = lookupProvider(hint[0], hint[1]);
    if (found) return found;
    // A host or platform path: that host's price or nothing. Its price for a model is not the
    // model maker's, so falling back to the maker's row would be wrong.
    if (HOSTS.has(hint[0])) return null;
    // A model maker's prefix on a model it does not sell (`openai/gpt-oss-120b`) only matches
    // rows whose own id starts with that prefix, such as a host's open-weight row.
    const prefix = lower.slice(0, lower.indexOf('/') + 1);
    return resolve(lower, (p) => ids(p).some((m) => m.length >= prefix.length && eqCi(m.slice(0, prefix.length), prefix)));
  }
  return resolve(lower, (p) => !PLATFORMS.has(p.provider)) ?? resolve(lower, (p) => PLATFORMS.has(p.provider));
}
