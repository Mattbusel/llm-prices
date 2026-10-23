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

mod data;

pub use data::GENERATED;

/// Prices that apply when a request is over a provider's long-context threshold.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LongContext {
    /// Requests whose prompt is over this many tokens pay these prices.
    pub threshold_tokens: u64,
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    /// Cache write price per cache lifetime, for example `("5m", 0.625)`.
    pub cache_write: &'static [(&'static str, f64)],
    pub batch_input: Option<f64>,
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
    pub input: Option<f64>,
    pub output: Option<f64>,
    pub cache_read: Option<f64>,
    /// Cache write price per cache lifetime, for example `("5m", 2.5)` and `("1h", 4.0)`.
    pub cache_write: &'static [(&'static str, f64)],
    pub batch_input: Option<f64>,
    pub batch_output: Option<f64>,
    pub long_context: Option<LongContext>,
    /// The provider's official pricing page this row was read from.
    pub source_url: &'static str,
    /// ISO date the row was checked against `source_url`.
    pub checked: &'static str,
}

impl Price {
    /// Cache write price for a cache lifetime such as `"5m"`, `"1h"`, `"30m"` or `"12h"`.
    pub fn cache_write(&self, ttl: &str) -> Option<f64> {
        self.cache_write.iter().find(|(k, _)| *k == ttl).map(|(_, v)| *v)
    }

    /// Standard (non-batch, uncached) cost in USD of one request. Uses the long-context
    /// prices when `input_tokens` is over the model's threshold. `None` when a needed price
    /// is not listed.
    pub fn cost(&self, input_tokens: u64, output_tokens: u64) -> Option<f64> {
        let (i, o) = match self.long_context {
            Some(lc) if input_tokens > lc.threshold_tokens => (lc.input?, lc.output?),
            _ => (self.input?, self.output?),
        };
        Some((input_tokens as f64 * i + output_tokens as f64 * o) / 1_000_000.0)
    }
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

    #[test]
    fn long_context_cost() {
        let h = lookup("claude-haiku-5-5").unwrap();
        assert!((h.cost(100_000, 0).unwrap() - 0.01).abs() < 1e-12);
        assert!((h.cost(100_001, 0).unwrap() - 100_001.0 * 0.5 / 1e6).abs() < 1e-12);
        assert_eq!(h.cache_write("1h"), Some(0.2));
    }
}
