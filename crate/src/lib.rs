//! Dated, source-cited LLM API prices with no required dependencies and no network access.
//!
//! The table covers Anthropic, OpenAI, Google Gemini and DeepSeek (re-read from their pricing
//! pages every day) plus Mistral, xAI, Cohere, Groq, Together AI and Fireworks AI (read by
//! hand; each row's [`Price::checked`] date says when). Every row carries the provider's
//! official pricing page and the date the price was read from it. Prices are USD per 1 million
//! tokens. The table is generated from
//! [`prices.json`](https://github.com/Mattbusel/llm-prices/blob/main/prices.json).
//!
//! ```
//! let p = llm_prices::lookup("claude-sonnet-5-5").unwrap();
//! assert_eq!(p.input, Some(2.0));
//! assert_eq!(p.cache_read, Some(0.10));
//! // Dated ids, Bedrock ids and provider prefixes resolve to the same row.
//! assert!(llm_prices::lookup("anthropic/claude-sonnet-5-5").is_some());
//! assert!(llm_prices::lookup("claude-sonnet-4-5").is_some());
//! // So do OpenAI snapshot ids and -latest aliases.
//! assert_eq!(llm_prices::lookup("gpt-4o-2024-08-06").unwrap().model, "gpt-4o");
//!
//! // 10,000 input + 2,000 output tokens on Claude Haiku 5.5 (prompt under 100k tokens).
//! let cost = llm_prices::lookup("claude-haiku-5-5").unwrap().cost(10_000, 2_000).unwrap();
//! assert!((cost - 0.002).abs() < 1e-12);
//! ```
//!
//! # Which provider served the request
//!
//! Open-weight models are sold by several hosts at different prices, so a gateway should look
//! a model up under the provider it called. [`lookup_provider`] (or [`Provider::lookup`]) only
//! returns that provider's row, and `None` when the table has no price for the model there:
//!
//! ```
//! use llm_prices::{lookup_provider, Provider};
//!
//! let groq = lookup_provider("groq", "openai/gpt-oss-120b").unwrap();
//! let fireworks = lookup_provider(Provider::Fireworks, "accounts/fireworks/models/gpt-oss-120b").unwrap();
//! assert_eq!(groq.cache_read, Some(0.075)); // 50% off input on Groq
//! assert_eq!(groq.batch_input, Some(0.075));
//! assert_eq!(fireworks.cache_read, Some(0.015));
//! // OpenAI does not sell gpt-oss through its API.
//! assert!(lookup_provider("openai", "gpt-oss-120b").is_none());
//! // LiteLLM-style ids name the provider, so plain `lookup` honors it.
//! let p = llm_prices::lookup("fireworks_ai/accounts/fireworks/models/gpt-oss-120b").unwrap();
//! assert_eq!(p.provider, "fireworks");
//! ```
//!
//! # Exact decimal prices
//!
//! [`Price::rate_str`] returns a price exactly as `prices.json` writes it, for billing code
//! that never touches `f64`. With the `rust_decimal` feature, `Price::cost_usage_decimal`
//! prices a whole response in `rust_decimal::Decimal`.
//!
//! ```
//! use llm_prices::{lookup, Rate};
//!
//! let p = lookup("gpt-4o-mini").unwrap();
//! assert_eq!(p.rate_str(Rate::Input), Some("0.15"));
//! assert_eq!(p.rate_str(Rate::CacheRead), Some("0.075"));
//! ```
//!
//! # Features
//!
//! - `rust_decimal`: `Price::cost_usage_decimal`, `Price::cost_decimal`,
//!   `Price::rate_decimal` and `DecimalCostBreakdown`, all in `rust_decimal::Decimal`.
//! - `serde`: `Serialize` for [`Price`], [`LongContext`], [`CostBreakdown`] and
//!   `DecimalCostBreakdown`, and `Serialize` plus `Deserialize` for [`Usage`] and [`Provider`].
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
#[cfg(feature = "rust_decimal")]
mod decimal;
mod provider;
#[cfg(feature = "serde")]
mod serde_impls;

pub use data::GENERATED;
#[cfg(feature = "rust_decimal")]
pub use decimal::DecimalCostBreakdown;
pub use provider::{lookup_provider, Provider, UnknownProvider};

/// Prices that apply when a request is over a provider's long-context threshold.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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
    #[cfg_attr(feature = "serde", serde(serialize_with = "crate::serde_impls::cache_write"))]
    pub cache_write: &'static [(&'static str, f64)],
    /// Batch API input price above the threshold, USD per 1M tokens.
    pub batch_input: Option<f64>,
    /// Batch API output price above the threshold, USD per 1M tokens.
    pub batch_output: Option<f64>,
}

/// One model's prices in USD per 1M tokens. `None` means the page lists no price.
///
/// With the `serde` feature, `Price` serializes with the field names of `prices.json` and
/// `cache_write` as a map from cache lifetime to price.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Price {
    /// The provider that sells the model at these prices: `anthropic`, `openai`, `google`,
    /// `deepseek`, `mistral`, `xai`, `cohere`, `groq`, `together` or `fireworks` (the
    /// [`Provider::as_str`] names). [`Price::provider_kind`] parses it.
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
    #[cfg_attr(feature = "serde", serde(serialize_with = "crate::serde_impls::cache_write"))]
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
///
/// With the `serde` feature, `Usage` serializes and deserializes with these field names, and
/// missing fields deserialize as zero (`false` for `batch`).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(default))]
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
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
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

/// One price column of a row, for [`Price::rate`], [`Price::rate_str`] and
/// [`LongContext::rate`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Rate<'a> {
    /// Uncached input ([`Price::input`]).
    Input,
    /// Output ([`Price::output`]).
    Output,
    /// Input served from the prompt cache ([`Price::cache_read`]).
    CacheRead,
    /// Cache write for one cache lifetime such as `"5m"`, `"1h"`, `"30m"` or `"12h"`
    /// ([`Price::cache_write`]).
    CacheWrite(&'a str),
    /// Batch API input ([`Price::batch_input`]).
    BatchInput,
    /// Batch API output ([`Price::batch_output`]).
    BatchOutput,
}

impl LongContext {
    /// The long-context price for one column, USD per 1M tokens. `None` when it is not listed.
    pub fn rate(&self, rate: Rate<'_>) -> Option<f64> {
        pick(rate, self.input, self.output, self.cache_read, self.cache_write, self.batch_input, self.batch_output)
    }
}

fn pick<T: Copy>(
    rate: Rate<'_>,
    input: Option<T>,
    output: Option<T>,
    cache_read: Option<T>,
    cache_write: &[(&str, T)],
    batch_input: Option<T>,
    batch_output: Option<T>,
) -> Option<T> {
    match rate {
        Rate::Input => input,
        Rate::Output => output,
        Rate::CacheRead => cache_read,
        Rate::CacheWrite(ttl) => cache_write.iter().find(|(k, _)| *k == ttl).map(|(_, v)| *v),
        Rate::BatchInput => batch_input,
        Rate::BatchOutput => batch_output,
    }
}

/// The exact decimal text of one tier of a row, generated next to `data::PRICES`.
pub(crate) struct ExactRates {
    pub(crate) input: Option<&'static str>,
    pub(crate) output: Option<&'static str>,
    pub(crate) cache_read: Option<&'static str>,
    pub(crate) cache_write: &'static [(&'static str, &'static str)],
    pub(crate) batch_input: Option<&'static str>,
    pub(crate) batch_output: Option<&'static str>,
}

impl ExactRates {
    fn get(&self, rate: Rate<'_>) -> Option<&'static str> {
        pick(rate, self.input, self.output, self.cache_read, self.cache_write, self.batch_input, self.batch_output)
    }
}

/// The exact decimal text of a row's standard and long-context prices.
pub(crate) struct ExactRow {
    pub(crate) base: ExactRates,
    pub(crate) long_context: Option<ExactRates>,
}

/// `text` when it is the decimal form of `value`, so a `Price` whose number was edited never
/// reports the table's text.
fn checked_text(text: Option<&'static str>, value: Option<f64>) -> Option<&'static str> {
    let (text, value) = (text?, value?);
    (text.parse::<f64>().ok()? == value).then_some(text)
}

impl Price {
    /// Cache write price for a cache lifetime such as `"5m"`, `"1h"`, `"30m"` or `"12h"`.
    pub fn cache_write(&self, ttl: &str) -> Option<f64> {
        self.cache_write.iter().find(|(k, _)| *k == ttl).map(|(_, v)| *v)
    }

    /// The standard price for one column, USD per 1M tokens. `None` when it is not listed.
    ///
    /// ```
    /// use llm_prices::{lookup, Rate};
    ///
    /// let p = lookup("claude-sonnet-4-6").unwrap();
    /// assert_eq!(p.rate(Rate::Output), Some(15.0));
    /// assert_eq!(p.rate(Rate::CacheWrite("1h")), Some(6.0));
    /// ```
    pub fn rate(&self, rate: Rate<'_>) -> Option<f64> {
        pick(rate, self.input, self.output, self.cache_read, self.cache_write, self.batch_input, self.batch_output)
    }

    /// The standard price for one column exactly as `prices.json` writes it, for example
    /// `"0.075"` or `"15"`, USD per 1M tokens. Parse it into a decimal type to bill without
    /// floating point.
    ///
    /// `None` when the price is not listed, or when this `Price` is not a row of the table
    /// with that price unchanged (a value built or edited by hand has no source text).
    ///
    /// ```
    /// use llm_prices::{lookup, Rate};
    ///
    /// let p = lookup("claude-haiku-5-5").unwrap();
    /// assert_eq!(p.rate_str(Rate::Input), Some("0.1"));
    /// assert_eq!(p.rate_str(Rate::CacheWrite("5m")), Some("0.125"));
    /// assert_eq!(p.long_context_rate_str(Rate::Input), Some("0.5"));
    ///
    /// let mut edited = *p;
    /// edited.input = Some(0.2);
    /// assert_eq!(edited.rate_str(Rate::Input), None);
    /// ```
    pub fn rate_str(&self, rate: Rate<'_>) -> Option<&'static str> {
        checked_text(self.exact()?.base.get(rate), self.rate(rate))
    }

    /// The long-context price for one column exactly as `prices.json` writes it. `None` when
    /// the model has no long-context tier, the price is not listed, or this `Price` is not an
    /// unchanged row of the table.
    pub fn long_context_rate_str(&self, rate: Rate<'_>) -> Option<&'static str> {
        let text = self.exact()?.long_context.as_ref()?.get(rate);
        checked_text(text, self.long_context?.rate(rate))
    }

    /// The exact text of this row, found by provider and model id.
    fn exact(&self) -> Option<&'static ExactRow> {
        let i = data::PRICES.iter().position(|p| p.provider == self.provider && p.model == self.model)?;
        data::EXACT.get(i)
    }

    /// The index of this row in [`all`], found by provider and model id.
    fn index(&self) -> Option<usize> {
        data::PRICES.iter().position(|p| p.provider == self.provider && p.model == self.model)
    }

    /// The prices of one other service tier of this model (`"flex"`, `"priority"`, `"fast"`,
    /// `"ultrafast"`, `"offpeak"`, ...), as a [`Price`] with the same provider and model.
    /// `"standard"` returns the table row itself. `None` when the provider's page lists no
    /// such tier for the model, or when this `Price` is not a row of the table.
    ///
    /// Tier prices follow the rules of [`cost_usage`](Self::cost_usage): a tier that lists no
    /// cache-read price bills cache reads as its input, and tiers have no batch prices.
    /// Gemini's Flex and Priority tiers carry their own long-context prices.
    ///
    /// ```
    /// use llm_prices::{lookup, Usage};
    ///
    /// let pro = lookup("gemini-2.5-pro").unwrap();
    /// let priority = pro.tier("priority").unwrap();
    /// assert_eq!(priority.input, Some(2.25));
    /// assert_eq!(priority.long_context.unwrap().output, Some(27.0));
    ///
    /// let u = Usage { input: 1_000, output: 1_000, ..Default::default() };
    /// let fast = lookup("claude-opus-5-5").unwrap().tier("fast").unwrap();
    /// assert!((fast.cost_usage(&u).unwrap().total - 0.048).abs() < 1e-12);
    /// assert!(pro.tier("fast").is_none());
    /// ```
    pub fn tier(&self, name: &str) -> Option<&'static Price> {
        let i = self.index()?;
        if name == "standard" {
            return data::PRICES.get(i);
        }
        data::TIERS.iter().find(|(row, n, _)| *row == i && *n == name).map(|(_, _, p)| p)
    }

    /// The names of the other service tiers [`tier`](Self::tier) can return for this model.
    ///
    /// ```
    /// let names: Vec<&str> = llm_prices::lookup("gemini-2.5-flash").unwrap().tiers().collect();
    /// assert_eq!(names, ["flex", "priority"]);
    /// ```
    pub fn tiers(&self) -> impl Iterator<Item = &'static str> {
        let i = self.index();
        data::TIERS.iter().filter(move |(row, _, _)| Some(*row) == i).map(|(_, n, _)| *n)
    }

    /// The data residency multiplier for `geo` on this model, applied to every price (tiers
    /// and long-context prices included): `Some(1.1)` for `"us"` on Claude 4.6 and later
    /// (`inference_geo: "us"`). `Some(1.0)` for `"global"`. `None` when the table lists no
    /// multiplier for `geo` on this model (Anthropic models before Claude 4.6 reject the
    /// parameter).
    ///
    /// ```
    /// use llm_prices::lookup;
    ///
    /// assert_eq!(lookup("claude-sonnet-4-6").unwrap().inference_geo("us"), Some(1.1));
    /// assert_eq!(lookup("claude-haiku-4-5").unwrap().inference_geo("us"), None);
    /// ```
    pub fn inference_geo(&self, geo: &str) -> Option<f64> {
        self.inference_geo_str(geo)?.parse().ok()
    }

    /// [`inference_geo`](Self::inference_geo) as the decimal text in `prices.json`.
    pub fn inference_geo_str(&self, geo: &str) -> Option<&'static str> {
        if geo == "global" {
            return Some("1");
        }
        let i = self.index()?;
        data::INFERENCE_GEO.iter().find(|(row, g, _)| *row == i && *g == geo).map(|(_, _, m)| *m)
    }

    /// [`cost_usage`](Self::cost_usage) with the data residency multiplier for `geo`
    /// applied to every component. `None` when `cost_usage` is `None` or the table lists no
    /// multiplier for `geo` on this model.
    ///
    /// ```
    /// use llm_prices::{lookup, Usage};
    ///
    /// let u = Usage { input: 1_000_000, output: 100_000, ..Default::default() };
    /// let c = lookup("claude-sonnet-4-6").unwrap().cost_usage_geo(&u, "us").unwrap();
    /// assert!((c.total - 4.95).abs() < 1e-9); // (3.00 + 1.50) x 1.1
    /// ```
    pub fn cost_usage_geo(&self, usage: &Usage, geo: &str) -> Option<CostBreakdown> {
        let m = self.inference_geo(geo)?;
        let c = self.cost_usage(usage)?;
        Some(CostBreakdown {
            input: c.input * m,
            output: c.output * m,
            cache_read: c.cache_read * m,
            cache_write: c.cache_write * m,
            total: c.total * m,
            long_context: c.long_context,
        })
    }

    /// [`Price::provider`] as a [`Provider`]. `None` only for a `Price` built by hand with a
    /// provider name this crate does not know.
    pub fn provider_kind(&self) -> Option<Provider> {
        Provider::parse(self.provider)
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
        let (rates, lc) = self.rates_for(usage);
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
            long_context: lc,
        })
    }

    /// The price set that applies to `usage` (standard or long-context), and whether it is
    /// the long-context one.
    pub(crate) fn rates_for(&self, usage: &Usage) -> (Rates, bool) {
        match self.long_context.filter(|lc| usage.prompt_tokens() > lc.threshold_tokens) {
            Some(lc) => (
                Rates {
                    input: lc.input,
                    output: lc.output,
                    cache_read: lc.cache_read,
                    cache_write: lc.cache_write,
                    batch_input: lc.batch_input,
                    batch_output: lc.batch_output,
                },
                true,
            ),
            None => (
                Rates {
                    input: self.input,
                    output: self.output,
                    cache_read: self.cache_read,
                    cache_write: self.cache_write,
                    batch_input: self.batch_input,
                    batch_output: self.batch_output,
                },
                false,
            ),
        }
    }
}

/// The price set that applies to one request (standard or long-context).
pub(crate) struct Rates {
    pub(crate) input: Option<f64>,
    pub(crate) output: Option<f64>,
    pub(crate) cache_read: Option<f64>,
    pub(crate) cache_write: &'static [(&'static str, f64)],
    pub(crate) batch_input: Option<f64>,
    pub(crate) batch_output: Option<f64>,
}

/// Every row in the table, in `prices.json` order: Anthropic, OpenAI, Google and DeepSeek
/// first, then Mistral, xAI, Cohere, Groq, Together AI and Fireworks AI.
pub fn all() -> &'static [Price] {
    data::PRICES
}

/// An iterator over every row in the table, in the order of [`all`].
///
/// ```
/// let groq: Vec<&str> = llm_prices::iter().filter(|p| p.provider == "groq").map(|p| p.model).collect();
/// assert!(groq.contains(&"openai/gpt-oss-120b"));
/// ```
pub fn iter() -> core::slice::Iter<'static, Price> {
    data::PRICES.iter()
}

/// Raw `prices.json` this crate was generated from.
pub const PRICES_JSON: &str = include_str!("../prices.json");

/// Provider prefixes that routers and SDKs put in front of a model id (`anthropic/`,
/// `vertex_ai/`, LiteLLM's `together_ai/`, Fireworks' `accounts/fireworks/models/`).
const PREFIXES: &[&str] = &[
    "models/",
    "anthropic/",
    "openai/",
    "google/",
    "gemini/",
    "vertex_ai/",
    "bedrock/",
    "azure/",
    "openrouter/",
    "deepseek/",
    "mistral/",
    "mistralai/",
    "xai/",
    "x-ai/",
    "cohere/",
    "groq/",
    "together/",
    "together_ai/",
    "fireworks/",
    "fireworks_ai/",
    "accounts/fireworks/models/",
    "perplexity/",
];

/// Bedrock's vendor prefix with an optional cross-region inference profile.
const BEDROCK_PREFIXES: &[&str] = &["us.anthropic.", "eu.anthropic.", "apac.anthropic.", "global.anthropic.", "anthropic."];

/// `s` without a leading `prefix`, ignoring ASCII case.
fn strip_ci<'a>(s: &'a str, prefix: &str) -> Option<&'a str> {
    let head = s.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix).then(|| &s[prefix.len()..])
}

fn strip_prefix(id: &str) -> &str {
    let mut s = id;
    loop {
        let before = s;
        for p in PREFIXES.iter().chain(BEDROCK_PREFIXES) {
            if let Some(rest) = strip_ci(s, p) {
                s = rest;
            }
        }
        if s == before {
            return s;
        }
    }
}

/// `id` without a trailing snapshot date: `-YYYYMMDD` (Anthropic) or `-YYYY-MM-DD` (OpenAI).
fn strip_date(id: &str) -> Option<&str> {
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let (head, tail) = id.rsplit_once('-')?;
    if tail.len() == 8 && tail.starts_with("20") && digits(tail) {
        return Some(head);
    }
    // -YYYY-MM-DD: the last three dash-separated parts are 4, 2 and 2 digits.
    let (rest, month) = head.rsplit_once('-')?;
    let (base, year) = rest.rsplit_once('-')?;
    let shape = tail.len() == 2 && month.len() == 2 && year.len() == 4 && year.starts_with("20");
    (shape && digits(tail) && digits(month) && digits(year) && !base.is_empty()).then_some(base)
}

/// `id` without a trailing `-latest`, then without a trailing snapshot date.
fn strip_version(id: &str) -> &str {
    let s = match id.len().checked_sub("-latest".len()) {
        Some(i) if i > 0 && id.get(i..).is_some_and(|t| t.eq_ignore_ascii_case("-latest")) => &id[..i],
        _ => id,
    };
    strip_date(s).unwrap_or(s)
}

/// Every id the table documents for a row: the model id and its aliases.
fn ids(p: &'static Price) -> impl Iterator<Item = &'static str> {
    core::iter::once(p.model).chain(p.aliases.iter().copied())
}

/// The id without provider prefixes, without Bedrock's `-v1:0` version suffix, and with
/// Vertex's `@` turned into `-`.
fn normalize(lower: &str) -> String {
    let mut s = strip_prefix(lower).to_string();
    if let Some(i) = s.find("-v") {
        if s[i + 2..].chars().next().is_some_and(|c| c.is_ascii_digit()) && !s[i + 2..].contains('-') {
            s.truncate(i);
        }
    }
    if let Some(i) = s.find('@') {
        s.replace_range(i..i + 1, "-");
    }
    s
}

/// Whether the row's id or an alias starts with `prefix` (`openai/`), ignoring ASCII case.
pub(crate) fn carries(p: &Price, prefix: &str) -> bool {
    core::iter::once(p.model).chain(p.aliases.iter().copied()).any(|m| strip_ci(m, prefix).is_some())
}

/// The first row `keep` accepts whose id or an alias is `lower`, ignoring ASCII case.
fn exact(lower: &str, keep: &dyn Fn(&Price) -> bool) -> Option<&'static Price> {
    all().iter().filter(|p| keep(p)).find(|p| ids(p).any(|m| m.eq_ignore_ascii_case(lower)))
}

/// Resolves `id` against the rows `keep` accepts, each step in table order:
///
/// 1. the id or an alias, ignoring ASCII case;
/// 2. the same after [`normalize`]; provider prefixes are removed from the table's ids too,
///    so `groq/openai/gpt-oss-120b` finds `openai/gpt-oss-120b`;
/// 3. the base row of a `-latest`, `-YYYYMMDD` or `-YYYY-MM-DD` id;
/// 4. a dated row whose undated form is the id (`claude-sonnet-4-5` finds
///    `claude-sonnet-4-5-20250929`).
pub(crate) fn resolve(id: &str, keep: &dyn Fn(&Price) -> bool) -> Option<&'static Price> {
    let rows = || all().iter().filter(|p| keep(p));
    let lower = id.trim().to_ascii_lowercase();
    if let Some(p) = rows().find(|p| ids(p).any(|m| m.eq_ignore_ascii_case(&lower))) {
        return Some(p);
    }
    let s = normalize(&lower);
    if s.is_empty() {
        return None;
    }
    let stripped_eq = |want: &str| rows().find(|p| ids(p).any(|m| strip_prefix(m).eq_ignore_ascii_case(want)));
    if let Some(p) = stripped_eq(&s) {
        return Some(p);
    }
    let base = strip_version(&s);
    if base != s {
        if let Some(p) = stripped_eq(base) {
            return Some(p);
        }
    }
    rows().find(|p| ids(p).any(|m| strip_date(strip_prefix(m)).is_some_and(|u| u.eq_ignore_ascii_case(base))))
}

/// Finds a model by id across every provider in the table.
///
/// Accepts:
///
/// - the documented id or an alias, in any ASCII case (`gpt-4o`, `mistral-large-latest`);
/// - provider prefixes (`anthropic/`, `models/`, `openrouter/`, LiteLLM's `together_ai/` and
///   `fireworks_ai/`, and the like) and Fireworks' `accounts/fireworks/models/` path;
/// - Bedrock ids (`us.anthropic.claude-...-v1:0`) and Vertex `@date` suffixes;
/// - dated snapshot ids, which resolve to their base row: `gpt-4o-2024-08-06` finds `gpt-4o`
///   and `claude-opus-5-5-20990101` finds `claude-opus-5-5`. A snapshot the table prices on
///   its own row (`gpt-4o-2024-05-13`) still finds that row;
/// - `-latest` ids (`claude-3-5-haiku-latest`, `grok-4.7-latest`), which resolve to the row of
///   the same model;
/// - the undated form of a dated id (`claude-sonnet-4-5` finds `claude-sonnet-4-5-20250929`).
///
/// When the id starts with a provider name and a slash (`groq/`, `together_ai/`,
/// `vertex_ai/`, ...), that provider is tried first, as [`lookup_provider`] does. A host or
/// platform path (`groq/`, `together_ai/`, `fireworks_ai/`, `accounts/fireworks/models/`,
/// `perplexity/`, `bedrock/`, `vertex_ai/`, `azure/`, `openrouter/`) is priced at that host's
/// own row (`bedrock/us.deepseek.r1-v1:0`, `openrouter/meta-llama/llama-3.3-70b-instruct`), or
/// at the model maker's list price where the platform charges it
/// ([`Provider::fallback_providers`]); otherwise the result is `None`, never another
/// provider's row, so `accounts/fireworks/models/deepseek-r1` does not get DeepSeek's own
/// price. A model maker's prefix (`anthropic/`, `openai/`, ...) on a model that maker does not
/// sell only matches rows whose own id carries the same prefix (Groq's
/// `openai/gpt-oss-120b`, then OpenRouter's `x-ai/grok-4.1-fast` by its exact id). Without a
/// provider prefix the first matching row in table order wins: model makers come before the
/// hosts that resell open-weight models, and the hosts are ordered Groq, Together AI,
/// Fireworks AI, Perplexity; Bedrock and OpenRouter rows only match their exact id there. Use
/// [`lookup_provider`] whenever you know which provider served the request, since several
/// hosts sell the same open-weight model at different prices.
///
/// A dated id resolves to the base row's price. Providers sometimes price an old snapshot
/// differently from the current alias; the table lists such a snapshot on its own row when
/// the pricing page does.
///
/// ```
/// use llm_prices::lookup;
///
/// assert_eq!(lookup("gpt-4o-2024-08-06").unwrap().model, "gpt-4o");
/// assert_eq!(lookup("gpt-4o-2024-05-13").unwrap().input, Some(5.0)); // its own row
/// assert_eq!(lookup("mistral-large-latest").unwrap().model, "mistral-large-4");
/// assert_eq!(lookup("xai/grok-4.7-latest").unwrap().model, "grok-4.7");
/// assert_eq!(lookup("vertex_ai/claude-opus-4-1@20250805").unwrap().model, "claude-opus-4-1-20250805");
/// // Fireworks' price for DeepSeek R1 is not DeepSeek's first-party price.
/// assert!(lookup("accounts/fireworks/models/deepseek-r1").is_none());
/// // Bedrock's own price for DeepSeek-R1 (AWS price list, us-east-1).
/// assert_eq!(lookup("bedrock/us.deepseek.r1-v1:0").unwrap().input, Some(1.35));
/// ```
pub fn lookup(id: &str) -> Option<&'static Price> {
    let lower = id.trim().to_ascii_lowercase();
    if let Some((provider, rest)) = provider::id_hint(&lower) {
        if let Some(p) = provider.lookup(rest) {
            return Some(p);
        }
        if provider.is_host() {
            // A host or platform path: that host's price or nothing. Its price for a model is
            // not the model maker's, so falling back to the maker's row would be wrong.
            return None;
        }
        // A model maker's prefix on a model it does not sell (`openai/gpt-oss-120b`) only
        // matches rows whose own id starts with that prefix, such as a host's open-weight row;
        // a platform's own rows (OpenRouter's `x-ai/grok-4.1-fast`) only by their exact id.
        let prefix = &lower[..=lower.find('/').unwrap_or(0)];
        return resolve(&lower, &|p| !provider::is_platform(p.provider) && carries(p, prefix))
            .or_else(|| exact(&lower, &|p| provider::is_platform(p.provider)));
    }
    // Without a provider path, Bedrock and OpenRouter rows, which resell other makers' models,
    // are only found by their exact id, so `gpt-4` never takes OpenRouter's `openai/gpt-4`.
    resolve(&lower, &|p| !provider::is_platform(p.provider)).or_else(|| exact(&lower, &|p| provider::is_platform(p.provider)))
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
        // 1.2: OpenAI snapshot ids resolve to the base row unless the table prices them.
        assert_eq!(lookup("gpt-4o-2024-08-06").unwrap().model, "gpt-4o");
        assert_eq!(lookup("gpt-4o-2024-05-13").unwrap().model, "gpt-4o-2024-05-13");
        assert!(lookup("not-a-model").is_none());
    }

    #[test]
    fn exact_table_matches_prices() {
        assert_eq!(data::EXACT.len(), data::PRICES.len());
        let rates = |p: &Price| -> Vec<Rate<'static>> {
            let mut v = vec![Rate::Input, Rate::Output, Rate::CacheRead, Rate::BatchInput, Rate::BatchOutput];
            v.extend(p.cache_write.iter().map(|(ttl, _)| Rate::CacheWrite(ttl)));
            v
        };
        for p in all() {
            for rate in rates(p) {
                let value = p.rate(rate);
                let text = p.rate_str(rate);
                assert_eq!(value.is_some(), text.is_some(), "{} {rate:?}", p.model);
                if let (Some(v), Some(t)) = (value, text) {
                    // The f64 prints back as the source text, so decimal billing built from
                    // either agrees.
                    assert_eq!(format!("{v}"), t, "{} {rate:?}", p.model);
                    assert!(!t.contains('e') && !t.starts_with('.'), "{t}");
                }
            }
            match p.long_context {
                Some(lc) => {
                    for (ttl, _) in lc.cache_write {
                        assert!(p.long_context_rate_str(Rate::CacheWrite(ttl)).is_some());
                    }
                    for rate in [Rate::Input, Rate::Output, Rate::CacheRead, Rate::BatchInput, Rate::BatchOutput] {
                        let text = p.long_context_rate_str(rate);
                        assert_eq!(lc.rate(rate).map(|v| format!("{v}")).as_deref(), text, "{} lc {rate:?}", p.model);
                    }
                }
                None => assert_eq!(p.long_context_rate_str(Rate::Input), None),
            }
        }
    }

    #[test]
    fn rate_str_needs_an_unchanged_table_row() {
        let p = *lookup("gpt-4o").unwrap();
        assert_eq!(p.rate_str(Rate::Input), Some("2.5"));
        assert_eq!(p.rate_str(Rate::CacheWrite("5m")), None);
        let renamed = Price { model: "my-model", ..p };
        assert_eq!(renamed.rate_str(Rate::Input), None);
        let repriced = Price { output: Some(11.0), ..p };
        assert_eq!(repriced.rate_str(Rate::Output), None);
        assert_eq!(repriced.rate_str(Rate::Input), Some("2.5"));
    }

    #[test]
    fn iter_and_all_agree() {
        assert_eq!(iter().len(), all().len());
        assert!(iter().zip(all()).all(|(a, b)| core::ptr::eq(a, b)));
    }

    #[test]
    fn snapshot_and_latest_suffixes() {
        assert_eq!(strip_date("gpt-4o-2024-08-06"), Some("gpt-4o"));
        assert_eq!(strip_date("claude-opus-4-1-20250805"), Some("claude-opus-4-1"));
        assert_eq!(strip_date("gpt-4-0613"), None);
        assert_eq!(strip_date("grok-4.20-0309-reasoning"), None);
        assert_eq!(strip_date("-2024-08-06"), None);
        assert_eq!(strip_version("grok-4.7-latest"), "grok-4.7");
        assert_eq!(strip_version("chat-latest"), "chat");
        assert_eq!(strip_version("-latest"), "-latest");
        assert_eq!(strip_version("claude-3-5-haiku-latest"), "claude-3-5-haiku");
        // chat-latest is its own OpenAI row, so the exact match wins over stripping.
        assert_eq!(lookup("chat-latest").unwrap().model, "chat-latest");
        assert_eq!(lookup("claude-3-5-haiku-latest").unwrap().model, "claude-3-5-haiku-20241022");
        assert_eq!(lookup("gpt-4-turbo").unwrap().model, "gpt-4-turbo-2024-04-09");
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
