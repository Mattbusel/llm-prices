//! Dated, source-cited LLM API prices (Anthropic, OpenAI, Google Gemini, DeepSeek) with no
//! dependencies and no network access.
//!
//! Every row carries the provider's official pricing page and the date the price was read
//! from it. Prices are USD per 1 million tokens. The table is regenerated from
//! [`prices.json`](https://github.com/Mattbusel/llm-prices/blob/main/prices.json), which a daily
//! job re-checks against the provider pages.
//!
//! ```
//! let p = llm_prices::lookup("claude-sonnet-5-5").unwrap();
//! assert_eq!(p.input, Some(2.0));
//! assert_eq!(p.cache_read, Some(0.10));
//! // Dated ids, Bedrock ids and provider prefixes resolve to the same row.
//! assert!(llm_prices::lookup("anthropic/claude-sonnet-5-5").is_some());
//! assert!(llm_prices::lookup("claude-sonnet-4-5").is_some());
//!
//! // 10,000 input + 2,000 output tokens on Claude Haiku 5.5 (prompt under 100k tokens).
//! let cost = llm_prices::lookup("claude-haiku-5-5").unwrap().cost(10_000, 2_000).unwrap();
//! assert!((cost - 0.002).abs() < 1e-12);
//! ```
//!
//! # Cost of a real API response
//!
//! [`Price::cost_usage`] prices a whole response, including prompt-cache reads, cache writes
//! by lifetime, batch discounts and the long-context switch, and returns a [`CostBreakdown`].
//! Fill a [`Usage`] from the usage block the provider sent back.
//!
//! An Anthropic Messages call on Claude Sonnet 4.6 that reads a cached system prompt and
//! writes a new 5-minute cache entry. The response carried `"input_tokens": 2048,
//! "cache_read_input_tokens": 45000, "cache_creation_input_tokens": 3500,
//! "output_tokens": 1024`. Prices from the table: input $3, cache read $0.30, 5-minute cache
//! write $3.75 and output $15 per 1M tokens.
//!
//! ```
//! use llm_prices::{lookup, Usage};
//!
//! let usage = Usage {
//!     input: 2_048,          // usage.input_tokens (uncached part of the prompt)
//!     cache_read: 45_000,    // usage.cache_read_input_tokens
//!     cache_write_5m: 3_500, // usage.cache_creation_input_tokens (default 5m lifetime)
//!     output: 1_024,         // usage.output_tokens
//!     ..Usage::default()
//! };
//! let c = lookup("claude-sonnet-4-6").unwrap().cost_usage(&usage).unwrap();
//!
//! // 2,048 x $3 + 45,000 x $0.30 + 3,500 x $3.75 + 1,024 x $15, all per 1M tokens.
//! assert!((c.input - 0.006_144).abs() < 1e-12);
//! assert!((c.cache_read - 0.013_5).abs() < 1e-12);
//! assert!((c.cache_write - 0.013_125).abs() < 1e-12);
//! assert!((c.output - 0.015_36).abs() < 1e-12);
//! assert!((c.total - 0.048_129).abs() < 1e-12);
//! assert!(!c.long_context); // 50,548 prompt tokens; Sonnet 4.6 has no long-context tier
//! ```
//!
//! An OpenAI call on GPT-5.4 with a 300,000-token prompt, 200,000 of it served from the
//! prompt cache. The response carried `"prompt_tokens": 300000`, `"prompt_tokens_details":
//! {"cached_tokens": 200000}` and `"completion_tokens": 4000` (reasoning tokens are already
//! inside `completion_tokens` and bill as output). OpenAI's `prompt_tokens` includes the cached
//! tokens, so the uncached input is `300000 - 200000`. The whole prompt is over GPT-5.4's
//! 272,000-token threshold, so every token bills at the long-context prices: input $5, cache
//! read $0.50 and output $22.50 per 1M tokens.
//!
//! ```
//! use llm_prices::{lookup, Usage};
//!
//! let (prompt_tokens, cached_tokens, completion_tokens) = (300_000, 200_000, 4_000);
//! let usage = Usage {
//!     input: prompt_tokens - cached_tokens,
//!     cache_read: cached_tokens,
//!     output: completion_tokens,
//!     ..Usage::default()
//! };
//! let c = lookup("gpt-5.4").unwrap().cost_usage(&usage).unwrap();
//!
//! assert!(c.long_context); // 300,000 > 272,000
//! // 100,000 x $5 + 200,000 x $0.50 + 4,000 x $22.50, all per 1M tokens.
//! assert!((c.input - 0.5).abs() < 1e-12);
//! assert!((c.cache_read - 0.1).abs() < 1e-12);
//! assert!((c.output - 0.09).abs() < 1e-12);
//! assert!((c.total - 0.69).abs() < 1e-12);
//!
//! // `Price::cost` sees only the tokens you pass it. Passing just the uncached 100,000 keeps
//! // the request under the threshold and bills $0.31 instead of $0.69, so use `cost_usage`
//! // whenever the response reports cached tokens.
//! let naive = lookup("gpt-5.4").unwrap().cost(100_000, 4_000).unwrap();
//! assert!((naive - 0.31).abs() < 1e-12);
//! ```

#![deny(missing_docs)]

mod data;

pub use data::GENERATED;

/// Prices that apply when a request is over a provider's long-context threshold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongContext {
    /// Requests whose prompt is over this many tokens pay these prices. The prompt counts
    /// uncached input, cache reads and cache writes together.
    pub threshold_tokens: u64,
    /// Uncached input price above the threshold, USD per 1M tokens.
    pub input: Option<f64>,
    /// Output price above the threshold (reasoning tokens included), USD per 1M tokens.
    pub output: Option<f64>,
    /// Price of input tokens served from the prompt cache above the threshold, USD per 1M
    /// tokens.
    pub cache_read: Option<f64>,
    /// Cache write price per cache lifetime, for example `("5m", 0.625)`.
    pub cache_write: &'static [(&'static str, f64)],
    /// Batch API input price above the threshold, USD per 1M tokens.
    pub batch_input: Option<f64>,
    /// Batch API output price above the threshold, USD per 1M tokens.
    pub batch_output: Option<f64>,
}

/// One model's prices in USD per 1M tokens. `None` means the page lists no price.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Price {
    /// `anthropic`, `openai`, `google` or `deepseek`.
    pub provider: &'static str,
    /// The API model id.
    pub model: &'static str,
    /// Other ids the provider documents for the same model.
    pub aliases: &'static [&'static str],
    /// `listed`, `retired` (still sold on some cloud platforms) or `limited`.
    pub status: &'static str,
    /// Uncached input price, USD per 1M tokens.
    pub input: Option<f64>,
    /// Output price, USD per 1M tokens. Reasoning tokens bill at this price. `None` for
    /// embedding models.
    pub output: Option<f64>,
    /// Price of input tokens served from the prompt cache, USD per 1M tokens. `None` when the
    /// page lists no cached-input price.
    pub cache_read: Option<f64>,
    /// Cache write price per cache lifetime, for example `("5m", 2.5)` and `("1h", 4.0)`.
    /// Empty when the provider does not charge separately for cache writes.
    pub cache_write: &'static [(&'static str, f64)],
    /// Batch API input price, USD per 1M tokens.
    pub batch_input: Option<f64>,
    /// Batch API output price, USD per 1M tokens.
    pub batch_output: Option<f64>,
    /// Prices for requests over a long-context threshold, when the model has one.
    pub long_context: Option<LongContext>,
    /// The provider's official pricing page this row was read from.
    pub source_url: &'static str,
    /// ISO date the row was checked against `source_url`.
    pub checked: &'static str,
}

/// Token counts from one API response, the input to [`Price::cost_usage`].
///
/// Build it with struct update syntax so fields you do not use stay zero:
///
/// ```
/// let u = llm_prices::Usage { input: 1_000, output: 200, ..Default::default() };
/// assert_eq!(u.prompt_tokens(), 1_000);
/// ```
///
/// Mapping from provider responses:
///
/// | Field | Anthropic `usage` | OpenAI `usage` | Gemini `usageMetadata` |
/// |---|---|---|---|
/// | `input` | `input_tokens` | `prompt_tokens - prompt_tokens_details.cached_tokens` | `promptTokenCount - cachedContentTokenCount` |
/// | `cache_read` | `cache_read_input_tokens` | `prompt_tokens_details.cached_tokens` | `cachedContentTokenCount` |
/// | `cache_write_5m` | `cache_creation.ephemeral_5m_input_tokens` | 0 (see below) | 0 |
/// | `cache_write_1h` | `cache_creation.ephemeral_1h_input_tokens` | 0 (see below) | 0 |
/// | `output` | `output_tokens` | `completion_tokens` | `candidatesTokenCount + thoughtsTokenCount` |
///
/// When an Anthropic response only reports the total `cache_creation_input_tokens`, those
/// tokens were written with the default 5-minute lifetime.
///
/// **Other cache lifetimes are not covered in 1.x.** `Usage` only has fields for the 5-minute
/// and 1-hour lifetimes. Some rows price cache writes on other lifetimes: the OpenAI rows
/// `gpt-6-astra`, `gpt-6-sol`, `gpt-6-luna`, `gpt-6.1-sol`, `gpt-5.6-sol`, `gpt-5.6-terra`,
/// `gpt-5.6-luna` and `gpt-5.6-cyber` list `"30m"` and `"12h"` prices, including in their
/// long-context tiers. [`Price::cost_usage`] cannot price those writes, so with the mapping
/// above it returns a total that leaves them out. Add them yourself with
/// [`Price::cache_write`]:
///
/// ```
/// use llm_prices::{lookup, Usage};
///
/// let p = lookup("gpt-6-sol").unwrap();
/// assert_eq!(p.cache_write("30m"), Some(2.5));
///
/// let written_30m: u64 = 50_000; // cache-write tokens your response reports for 30m
/// let c = p.cost_usage(&Usage { input: 10_000, output: 1_000, ..Default::default() }).unwrap();
/// let total = c.total + written_30m as f64 * p.cache_write("30m").unwrap() / 1_000_000.0;
/// // 10,000 x $2 + 1,000 x $10 + 50,000 x $2.50, all per 1M tokens.
/// assert!((total - 0.155).abs() < 1e-12);
/// ```
///
/// Those tokens also do not count toward the long-context threshold inside `cost_usage`.
/// When they push the prompt over [`LongContext::threshold_tokens`], take the rates from
/// [`Price::long_context`] instead. Fields for more cache lifetimes need a breaking change
/// (`Usage` is not `#[non_exhaustive]`) and are planned for 2.0.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Usage {
    /// Uncached input tokens. Do not include tokens counted in `cache_read` or the cache
    /// write fields.
    pub input: u64,
    /// Output tokens, reasoning or thinking tokens included.
    pub output: u64,
    /// Input tokens served from the prompt cache.
    pub cache_read: u64,
    /// Input tokens written to the cache with a 5-minute lifetime.
    pub cache_write_5m: u64,
    /// Input tokens written to the cache with a 1-hour lifetime.
    pub cache_write_1h: u64,
    /// The request went through the provider's Batch API.
    pub batch: bool,
}

impl Usage {
    /// Total prompt tokens: uncached input plus cache reads plus cache writes. This is the
    /// number compared against a model's long-context threshold.
    pub fn prompt_tokens(&self) -> u64 {
        self.input
            .saturating_add(self.cache_read)
            .saturating_add(self.cache_write_5m)
            .saturating_add(self.cache_write_1h)
    }
}

/// Cost in USD of one response, split by token kind. Returned by [`Price::cost_usage`].
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub struct CostBreakdown {
    /// Cost of the uncached input tokens.
    pub input: f64,
    /// Cost of the output tokens.
    pub output: f64,
    /// Cost of the input tokens served from the prompt cache.
    pub cache_read: f64,
    /// Cost of the cache writes, all lifetimes together.
    pub cache_write: f64,
    /// Sum of the other cost fields.
    pub total: f64,
    /// The prompt was over the model's long-context threshold, so long-context prices applied.
    pub long_context: bool,
}

impl Price {
    /// Cache write price for a cache lifetime such as `"5m"`, `"1h"`, `"30m"` or `"12h"`.
    pub fn cache_write(&self, ttl: &str) -> Option<f64> {
        self.cache_write.iter().find(|(k, _)| *k == ttl).map(|(_, v)| *v)
    }

    /// Standard (non-batch, uncached) cost in USD of one request. Uses the long-context
    /// prices when `input_tokens` is over the model's threshold. `None` when a needed price
    /// is not listed.
    ///
    /// Same as [`cost_usage`](Self::cost_usage) with only `input` and `output` set. When the
    /// response reports cached tokens, use `cost_usage` instead: cached tokens count toward
    /// the long-context threshold and bill at their own price.
    pub fn cost(&self, input_tokens: u64, output_tokens: u64) -> Option<f64> {
        let usage = Usage { input: input_tokens, output: output_tokens, ..Usage::default() };
        self.cost_usage(&usage).map(|c| c.total)
    }

    /// Cost in USD of one response from its token counts, split by token kind.
    ///
    /// Rules:
    ///
    /// - **Long context.** When the model has a [`LongContext`] tier and
    ///   [`Usage::prompt_tokens`] (input plus cache reads plus cache writes) is strictly
    ///   greater than its threshold, every token bills at the long-context prices.
    /// - **Batch.** When `usage.batch` is set, input and output bill at the batch prices. The
    ///   table lists no separate batch cache prices, so cache reads and writes get the same
    ///   discount as input (the batch input price divided by the standard input price, 50% on
    ///   every current row).
    /// - **Cache reads** bill at the cache-read price. A model with no listed cache-read
    ///   price bills them as ordinary input.
    /// - **Cache writes** bill at [`cache_write("5m")`](Self::cache_write) and
    ///   `cache_write("1h")`.
    /// - **Other cache lifetimes are not priced.** Rows that price cache writes on other
    ///   lifetimes (OpenAI `"30m"` and `"12h"`) cannot have those writes priced through
    ///   [`Usage`] in 1.x. Add `tokens * price.cache_write("30m")` (or `"12h"`), divided by
    ///   1M, to the total yourself; see [`Usage`] for an example. Setting `cache_write_5m` or
    ///   `cache_write_1h` on such a row returns `None`.
    ///
    /// Returns `None` when a price it needs is not listed: the input or output price (batch
    /// prices when `usage.batch` is set, long-context prices above the threshold), or the
    /// cache-write price for a lifetime with a non-zero token count. The input and output
    /// prices are required even when their token count is zero, as [`cost`](Self::cost)
    /// has always done.
    ///
    /// ```
    /// use llm_prices::{lookup, Usage};
    ///
    /// let haiku = lookup("claude-haiku-4-5").unwrap();
    /// // A batch request: $0.50 input and $2.50 output per 1M tokens, cache reads at half
    /// // of $0.10.
    /// let u = Usage {
    ///     input: 10_000,
    ///     cache_read: 90_000,
    ///     output: 1_000,
    ///     batch: true,
    ///     ..Default::default()
    /// };
    /// let c = haiku.cost_usage(&u).unwrap();
    /// assert!((c.input - 0.005).abs() < 1e-12);
    /// assert!((c.cache_read - 0.0045).abs() < 1e-12);
    /// assert!((c.output - 0.0025).abs() < 1e-12);
    ///
    /// // gpt-3.5-turbo lists no batch price.
    /// assert!(lookup("gpt-3.5-turbo").unwrap().cost_usage(&u).is_none());
    /// // OpenAI rows list 30m and 12h cache lifetimes, not 5m.
    /// let w = Usage { input: 1, cache_write_5m: 1, ..Default::default() };
    /// assert!(lookup("gpt-6-sol").unwrap().cost_usage(&w).is_none());
    /// ```
    pub fn cost_usage(&self, usage: &Usage) -> Option<CostBreakdown> {
        let lc = self.long_context.filter(|lc| usage.prompt_tokens() > lc.threshold_tokens);
        let rates = match lc {
            Some(lc) => Rates {
                input: lc.input,
                output: lc.output,
                cache_read: lc.cache_read,
                cache_write: lc.cache_write,
                batch_input: lc.batch_input,
                batch_output: lc.batch_output,
            },
            None => Rates {
                input: self.input,
                output: self.output,
                cache_read: self.cache_read,
                cache_write: self.cache_write,
                batch_input: self.batch_input,
                batch_output: self.batch_output,
            },
        };
        let standard_input = rates.input?;
        let (in_rate, out_rate, discount) = if usage.batch {
            let batch_input = rates.batch_input?;
            let discount = if standard_input > 0.0 { batch_input / standard_input } else { 1.0 };
            (batch_input, rates.batch_output?, discount)
        } else {
            (standard_input, rates.output?, 1.0)
        };
        let read_rate = rates.cache_read.map_or(in_rate, |r| r * discount);
        let write = |ttl: &str, tokens: u64| -> Option<f64> {
            if tokens == 0 {
                return Some(0.0);
            }
            let rate = rates.cache_write.iter().find(|(k, _)| *k == ttl).map(|(_, v)| *v)?;
            Some(tokens as f64 * rate * discount)
        };

        // Each product is tokens x USD-per-1M. The total adds the products before dividing
        // once, so `cost()` returns bit-for-bit what it returned in 1.0.
        let p_input = usage.input as f64 * in_rate;
        let p_read = usage.cache_read as f64 * read_rate;
        let p_write = write("5m", usage.cache_write_5m)? + write("1h", usage.cache_write_1h)?;
        let p_output = usage.output as f64 * out_rate;
        const M: f64 = 1_000_000.0;
        Some(CostBreakdown {
            input: p_input / M,
            output: p_output / M,
            cache_read: p_read / M,
            cache_write: p_write / M,
            total: (p_input + p_read + p_write + p_output) / M,
            long_context: lc.is_some(),
        })
    }
}

/// The price set that applies to one request (standard or long-context).
struct Rates {
    input: Option<f64>,
    output: Option<f64>,
    cache_read: Option<f64>,
    cache_write: &'static [(&'static str, f64)],
    batch_input: Option<f64>,
    batch_output: Option<f64>,
}

/// Every row in the table.
pub fn all() -> &'static [Price] {
    data::PRICES
}

/// Raw `prices.json` this crate was generated from.
pub const PRICES_JSON: &str = include_str!("../prices.json");

fn strip_prefix(id: &str) -> &str {
    let mut s = id;
    loop {
        let before = s;
        for p in ["models/", "anthropic/", "openai/", "google/", "gemini/", "vertex_ai/", "bedrock/", "azure/", "openrouter/", "deepseek/"] {
            if let Some(rest) = s.strip_prefix(p) {
                s = rest;
            }
        }
        for p in ["us.anthropic.", "eu.anthropic.", "apac.anthropic.", "global.anthropic.", "anthropic."] {
            if let Some(rest) = s.strip_prefix(p) {
                s = rest;
            }
        }
        if s == before {
            return s;
        }
    }
}

fn strip_date(id: &str) -> Option<&str> {
    let (head, tail) = id.rsplit_once('-')?;
    (tail.len() == 8 && tail.starts_with("20") && tail.bytes().all(|b| b.is_ascii_digit())).then_some(head)
}

/// Finds a model by id. Accepts the documented id or alias, provider prefixes
/// (`anthropic/`, `models/`), Bedrock ids (`us.anthropic.claude-...-v1:0`), Vertex `@date`
/// suffixes, and the undated form of dated Anthropic ids.
pub fn lookup(id: &str) -> Option<&'static Price> {
    let lower = id.trim().to_ascii_lowercase();
    let exact = |s: &str| all().iter().find(|p| p.model == s || p.aliases.contains(&s));
    if let Some(p) = exact(&lower) {
        return Some(p);
    }
    let mut s = strip_prefix(&lower).to_string();
    if let Some(i) = s.find("-v") {
        if s[i + 2..].chars().next().is_some_and(|c| c.is_ascii_digit()) && !s[i + 2..].contains('-') {
            s.truncate(i);
        }
    }
    if let Some(i) = s.find('@') {
        s.replace_range(i..i + 1, "-");
    }
    if let Some(p) = exact(&s) {
        return Some(p);
    }
    // Undated form of a dated Anthropic id, or a dated form of an undated one.
    let undated = strip_date(&s).unwrap_or(&s).to_string();
    all().iter().find(|p| {
        p.provider == "anthropic"
            && std::iter::once(p.model)
                .chain(p.aliases.iter().copied())
                .any(|m| strip_date(m).unwrap_or(m) == undated)
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_row_has_a_source_and_date() {
        assert!(all().len() > 50);
        for p in all() {
            assert!(p.source_url.starts_with("https://"), "{}", p.model);
            assert_eq!(p.checked.len(), 10, "{}", p.model);
        }
    }

    #[test]
    fn lookups() {
        assert_eq!(lookup("claude-opus-5-5").unwrap().input, Some(4.0));
        assert_eq!(lookup("us.anthropic.claude-sonnet-4-20250514-v1:0").unwrap().model, "claude-sonnet-4-20250514");
        assert_eq!(lookup("claude-opus-4-1@20250805").unwrap().model, "claude-opus-4-1-20250805");
        assert_eq!(lookup("claude-haiku-4-5-20251001").unwrap().model, "claude-haiku-4-5");
        assert_eq!(lookup("models/gemini-2.5-pro").unwrap().output, Some(10.0));
        assert_eq!(lookup("deepseek-v4-flash").unwrap().model, "deepseek-flash");
        assert!(lookup("gpt-4o-2024-08-06").is_none());
        assert!(lookup("not-a-model").is_none());
    }

    /// The 1.0 implementation of `cost`, kept to prove the new one is unchanged.
    fn cost_1_0(p: &Price, input_tokens: u64, output_tokens: u64) -> Option<f64> {
        let (i, o) = match p.long_context {
            Some(lc) if input_tokens > lc.threshold_tokens => (lc.input?, lc.output?),
            _ => (p.input?, p.output?),
        };
        Some((input_tokens as f64 * i + output_tokens as f64 * o) / 1_000_000.0)
    }

    #[test]
    fn cost_agrees_with_cost_usage_and_1_0() {
        let counts = [0, 1, 999, 100_000, 100_001, 200_000, 200_001, 272_000, 272_001, 1_000_000, 12_345_678];
        for p in all() {
            for &i in &counts {
                for &o in &counts {
                    let u = Usage { input: i, output: o, ..Usage::default() };
                    let total = p.cost_usage(&u).map(|c| c.total);
                    assert_eq!(p.cost(i, o), total, "{} {i} {o}", p.model);
                    // Bit-for-bit equal to 1.0 (NaN is impossible: all prices are finite).
                    assert_eq!(p.cost(i, o), cost_1_0(p, i, o), "{} {i} {o}", p.model);
                }
            }
        }
    }

    #[test]
    fn long_context_counts_cached_prompt_tokens() {
        let h = lookup("claude-haiku-5-5").unwrap();
        // 60k uncached + 40k cache read = exactly the 100k threshold: standard prices.
        let at = Usage { input: 60_000, cache_read: 40_000, ..Usage::default() };
        let c = h.cost_usage(&at).unwrap();
        assert!(!c.long_context);
        assert!((c.total - (60_000.0 * 0.1 + 40_000.0 * 0.01) / 1e6).abs() < 1e-15);
        // One cache-write token more crosses it: everything at long-context prices.
        let over = Usage { cache_write_1h: 1, ..at };
        let c = h.cost_usage(&over).unwrap();
        assert!(c.long_context);
        assert!((c.input - 60_000.0 * 0.5 / 1e6).abs() < 1e-15);
        assert!((c.cache_read - 40_000.0 * 0.05 / 1e6).abs() < 1e-15);
        assert!((c.cache_write - 1.0 / 1e6).abs() < 1e-15);
        assert!((c.total - (c.input + c.cache_read + c.cache_write + c.output)).abs() < 1e-15);
    }

    #[test]
    fn cache_read_falls_back_to_input_price() {
        // gpt-5.5-pro lists no cached-input price.
        let p = lookup("gpt-5.5-pro").unwrap();
        assert_eq!(p.cache_read, None);
        let c = p.cost_usage(&Usage { cache_read: 100_000, ..Usage::default() }).unwrap();
        assert_eq!(c.cache_read, 3.0); // 100k x $30
        assert_eq!(c.input, 0.0);
        // Over the 272k threshold the fallback is the long-context input price, $60.
        let c = p.cost_usage(&Usage { cache_read: 1_000_000, ..Usage::default() }).unwrap();
        assert!(c.long_context);
        assert_eq!(c.cache_read, 60.0);
    }

    #[test]
    fn batch_and_cache_writes() {
        let s = lookup("claude-sonnet-4-6").unwrap();
        let u = Usage { cache_write_5m: 1_000_000, cache_write_1h: 1_000_000, ..Usage::default() };
        assert_eq!(s.cost_usage(&u).unwrap().cache_write, 3.75 + 6.0);
        let c = s.cost_usage(&Usage { batch: true, ..u }).unwrap();
        assert_eq!(c.cache_write, (3.75 + 6.0) / 2.0);
        // Long-context batch prices missing on gpt-5.5-pro.
        let pro = lookup("gpt-5.5-pro").unwrap();
        assert!(pro.cost_usage(&Usage { input: 272_001, batch: true, ..Usage::default() }).is_none());
        assert!(pro.cost_usage(&Usage { input: 272_000, batch: true, ..Usage::default() }).is_some());
        // Embedding rows list no output price, so they have no cost, as in 1.0.
        assert!(lookup("text-embedding-3-small").unwrap().cost_usage(&Usage::default()).is_none());
    }

    #[test]
    fn long_context_cost() {
        let h = lookup("claude-haiku-5-5").unwrap();
        assert!((h.cost(100_000, 0).unwrap() - 0.01).abs() < 1e-12);
        assert!((h.cost(100_001, 0).unwrap() - 100_001.0 * 0.5 / 1e6).abs() < 1e-12);
        assert_eq!(h.cache_write("1h"), Some(0.2));
    }
}
