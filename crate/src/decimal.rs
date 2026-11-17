//! Exact decimal costs (the `rust_decimal` feature).

use core::str::FromStr;

use rust_decimal::Decimal;

use crate::{Price, Rate, Usage};

/// Cost in USD of one response as exact decimals, split by token kind. Returned by
/// [`Price::cost_usage_decimal`]; the fields mean the same as in [`CostBreakdown`](crate::CostBreakdown).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
#[non_exhaustive]
pub struct DecimalCostBreakdown {
    /// Cost of the uncached input tokens.
    pub input: Decimal,
    /// Cost of the output tokens.
    pub output: Decimal,
    /// Cost of the input tokens served from the prompt cache.
    pub cache_read: Decimal,
    /// Cost of the cache writes, all lifetimes together.
    pub cache_write: Decimal,
    /// Sum of the other cost fields.
    pub total: Decimal,
    /// The prompt was over the model's long-context threshold, so long-context prices applied.
    pub long_context: bool,
}

/// The decimal a price was written as. Every price in the table is a short decimal, and Rust
/// prints an `f64` as the shortest text that reads back to the same value, so this is the
/// number in `prices.json` (a test checks every row against [`Price::rate_str`]).
fn exact(v: f64) -> Option<Decimal> {
    if !v.is_finite() {
        return None;
    }
    Decimal::from_str(&format!("{v}")).ok()
}

impl Price {
    /// The standard price for one column as an exact [`Decimal`], USD per 1M tokens. `None`
    /// when it is not listed. Same number as [`Price::rate_str`], also for a `Price` built by
    /// hand.
    ///
    /// ```
    /// use llm_prices::{lookup, Rate};
    /// use rust_decimal::Decimal;
    ///
    /// let p = lookup("gpt-4o-mini").unwrap();
    /// assert_eq!(p.rate_decimal(Rate::CacheRead), Some(Decimal::new(75, 3))); // 0.075
    /// ```
    pub fn rate_decimal(&self, rate: Rate<'_>) -> Option<Decimal> {
        exact(self.rate(rate)?)
    }

    /// [`cost`](Self::cost) in exact decimals: the standard (non-batch, uncached) cost in USD
    /// of one request.
    ///
    /// ```
    /// use llm_prices::lookup;
    /// use rust_decimal::Decimal;
    ///
    /// // 10,000 x $0.10 + 2,000 x $0.50, per 1M tokens.
    /// let c = lookup("claude-haiku-5-5").unwrap().cost_decimal(10_000, 2_000).unwrap();
    /// assert_eq!(c, Decimal::new(2, 3)); // exactly 0.002
    /// ```
    pub fn cost_decimal(&self, input_tokens: u64, output_tokens: u64) -> Option<Decimal> {
        let usage = Usage { input: input_tokens, output: output_tokens, ..Usage::default() };
        self.cost_usage_decimal(&usage).map(|c| c.total)
    }

    /// [`cost_usage`](Self::cost_usage) in exact decimals, for billing code that keeps money
    /// in [`Decimal`] and never in `f64`.
    ///
    /// The prices are the decimals `prices.json` lists ([`Price::rate_decimal`]), and the
    /// rules are the ones `cost_usage` documents, so it returns `None` in exactly the same
    /// cases. Each component is `tokens x price / 1,000,000` with no rounding. The only
    /// division by a price is the batch discount on cache tokens (batch input price over
    /// standard input price); it is exact whenever that ratio is a terminating decimal, which
    /// it is on every current row (50% or 80%), and otherwise rounds at the 28th significant
    /// digit. `None` also when a product overflows `Decimal`, which takes token counts in the
    /// trillions of trillions. Results are [normalized](Decimal::normalize), so `0.0021` and not
    /// `0.0021000`.
    ///
    /// ```
    /// use llm_prices::{lookup, Usage};
    /// use rust_decimal::Decimal;
    /// use std::str::FromStr;
    ///
    /// // The Anthropic example from the crate docs: 2,048 x $3 + 45,000 x $0.30
    /// // + 3,500 x $3.75 + 1,024 x $15, per 1M tokens.
    /// let usage = Usage { input: 2_048, cache_read: 45_000, cache_write_5m: 3_500, output: 1_024, ..Usage::default() };
    /// let c = lookup("claude-sonnet-4-6").unwrap().cost_usage_decimal(&usage).unwrap();
    /// assert_eq!(c.total, Decimal::from_str("0.048129").unwrap());
    /// assert_eq!(c.cache_write, Decimal::from_str("0.013125").unwrap());
    /// ```
    pub fn cost_usage_decimal(&self, usage: &Usage) -> Option<DecimalCostBreakdown> {
        let (rates, lc) = self.rates_for(usage);
        let dec = |v: Option<f64>| -> Option<Decimal> { exact(v?) };
        let standard_input = dec(rates.input)?;
        // The batch discount as numerator / denominator, applied to cache reads and writes.
        let (in_rate, out_rate, num, den) = if usage.batch {
            let batch_input = dec(rates.batch_input)?;
            let batch_output = dec(rates.batch_output)?;
            if standard_input.is_zero() {
                (batch_input, batch_output, Decimal::ONE, Decimal::ONE)
            } else {
                (batch_input, batch_output, batch_input, standard_input)
            }
        } else {
            (standard_input, dec(rates.output)?, Decimal::ONE, Decimal::ONE)
        };
        let times = |tokens: u64, rate: Decimal| Decimal::from(tokens).checked_mul(rate);
        let discounted = |tokens: u64, rate: Decimal| -> Option<Decimal> {
            let p = times(tokens, rate)?;
            if num == den {
                Some(p)
            } else {
                p.checked_mul(num)?.checked_div(den)
            }
        };
        let p_input = times(usage.input, in_rate)?;
        let p_read = match rates.cache_read {
            Some(r) => discounted(usage.cache_read, exact(r)?)?,
            None => times(usage.cache_read, in_rate)?,
        };
        let write = |ttl: &str, tokens: u64| -> Option<Decimal> {
            if tokens == 0 {
                return Some(Decimal::ZERO);
            }
            let rate = rates.cache_write.iter().find(|(k, _)| *k == ttl).map(|(_, v)| *v)?;
            discounted(tokens, exact(rate)?)
        };
        let p_write = write("5m", usage.cache_write_5m)?.checked_add(write("1h", usage.cache_write_1h)?)?;
        let p_output = times(usage.output, out_rate)?;
        let per_m = |v: Decimal| -> Option<Decimal> { v.checked_mul(Decimal::new(1, 6)).map(|d| d.normalize()) };
        let total = p_input.checked_add(p_read)?.checked_add(p_write)?.checked_add(p_output)?;
        Some(DecimalCostBreakdown {
            input: per_m(p_input)?,
            output: per_m(p_output)?,
            cache_read: per_m(p_read)?,
            cache_write: per_m(p_write)?,
            total: per_m(total)?,
            long_context: lc,
        })
    }

    /// [`cost_usage_decimal`](Self::cost_usage_decimal) with the data residency multiplier
    /// for `geo` ([`Price::inference_geo_str`]) applied to every component, in exact
    /// decimals. `None` when `cost_usage_decimal` is `None` or the table lists no multiplier
    /// for `geo` on this model.
    ///
    /// ```
    /// use llm_prices::{lookup, Usage};
    /// use rust_decimal::Decimal;
    ///
    /// let u = Usage { input: 1_000_000, output: 100_000, ..Default::default() };
    /// let c = lookup("claude-sonnet-4-6").unwrap().cost_usage_decimal_geo(&u, "us").unwrap();
    /// assert_eq!(c.total, Decimal::new(495, 2)); // (3.00 + 1.50) x 1.1
    /// ```
    pub fn cost_usage_decimal_geo(&self, usage: &Usage, geo: &str) -> Option<DecimalCostBreakdown> {
        let m = Decimal::from_str(self.inference_geo_str(geo)?).ok()?;
        let c = self.cost_usage_decimal(usage)?;
        let x = |v: Decimal| v.checked_mul(m).map(|d| d.normalize());
        Some(DecimalCostBreakdown {
            input: x(c.input)?,
            output: x(c.output)?,
            cache_read: x(c.cache_read)?,
            cache_write: x(c.cache_write)?,
            total: x(c.total)?,
            long_context: c.long_context,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{all, lookup};

    #[test]
    fn rate_decimal_is_the_source_text() {
        for p in all() {
            for rate in [Rate::Input, Rate::Output, Rate::CacheRead, Rate::BatchInput, Rate::BatchOutput] {
                let want = p.rate_str(rate).map(|s| Decimal::from_str(s).unwrap());
                assert_eq!(p.rate_decimal(rate), want, "{} {rate:?}", p.model);
            }
            for (ttl, _) in p.cache_write {
                let s = p.rate_str(Rate::CacheWrite(ttl)).unwrap();
                assert_eq!(p.rate_decimal(Rate::CacheWrite(ttl)), Some(Decimal::from_str(s).unwrap()));
            }
        }
    }

    /// The decimal cost agrees with the f64 cost on every row and returns `None` in the same
    /// cases.
    #[test]
    fn agrees_with_cost_usage() {
        let counts = [0u64, 1, 999, 60_000, 100_001, 199_999, 200_000, 272_001, 1_000_000, 12_345_678];
        for p in all() {
            for &i in &counts {
                for &r in &[0u64, 40_000, 250_000] {
                    for batch in [false, true] {
                        let u = Usage { input: i, cache_read: r, output: i / 3, batch, ..Usage::default() };
                        let f = p.cost_usage(&u);
                        let d = p.cost_usage_decimal(&u);
                        assert_eq!(f.is_some(), d.is_some(), "{} {u:?}", p.model);
                        if let (Some(f), Some(d)) = (f, d) {
                            assert_eq!(f.long_context, d.long_context);
                            let dt: f64 = d.total.to_string().parse().unwrap();
                            assert!((f.total - dt).abs() <= 1e-9 * f.total.abs().max(1.0), "{} {u:?}: {} vs {}", p.model, f.total, d.total);
                            assert_eq!(d.total, d.input + d.output + d.cache_read + d.cache_write);
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn exact_where_f64_is_not() {
        // 3 x $0.1 + 3 x $0.2 per 1M is 0.0000009 exactly; f64 gets 9.000000000000001e-7.
        let p = Price { input: Some(0.1), output: Some(0.2), ..*lookup("gpt-4.1-nano").unwrap() };
        assert_eq!(p.cost_decimal(3, 3), Some(Decimal::from_str("0.0000009").unwrap()));
        // Batch on Claude Haiku 4.5: cache reads at half of $0.10.
        let h = lookup("claude-haiku-4-5").unwrap();
        let u = Usage { input: 10_000, cache_read: 90_000, output: 1_000, batch: true, ..Usage::default() };
        let c = h.cost_usage_decimal(&u).unwrap();
        assert_eq!(c.input, Decimal::from_str("0.005").unwrap());
        assert_eq!(c.cache_read, Decimal::from_str("0.0045").unwrap());
        assert_eq!(c.output, Decimal::from_str("0.0025").unwrap());
        assert_eq!(c.total, Decimal::from_str("0.012").unwrap());
        // xAI batch is 20% off, cached tokens included: 100,000 x $0.50 x 0.8 per 1M.
        let g = lookup("grok-4.7").unwrap();
        let c = g.cost_usage_decimal(&Usage { cache_read: 100_000, batch: true, ..Usage::default() }).unwrap();
        assert_eq!(c.cache_read, Decimal::from_str("0.04").unwrap());
        // A 1M-token prompt is over xAI's 200k threshold: $1.00 long-context cache reads, 20% off.
        let c = g.cost_usage_decimal(&Usage { cache_read: 1_000_000, batch: true, ..Usage::default() }).unwrap();
        assert!(c.long_context);
        assert_eq!(c.cache_read, Decimal::from_str("0.8").unwrap());
    }

    #[test]
    fn long_context_and_missing_prices() {
        let h = lookup("claude-haiku-5-5").unwrap();
        let c = h.cost_usage_decimal(&Usage { input: 100_001, ..Usage::default() }).unwrap();
        assert!(c.long_context);
        assert_eq!(c.input, Decimal::from_str("0.0500005").unwrap());
        // OpenAI rows list no 5m cache write.
        let w = Usage { input: 1, cache_write_5m: 1, ..Usage::default() };
        assert!(lookup("gpt-6-sol").unwrap().cost_usage_decimal(&w).is_none());
        // Embedding rows have no output price.
        assert!(lookup("text-embedding-3-small").unwrap().cost_decimal(1, 0).is_none());
        // Overflow is None, not a panic.
        let big = Usage { input: u64::MAX, output: u64::MAX, cache_read: u64::MAX, ..Usage::default() };
        let _ = lookup("o1-pro").unwrap().cost_usage_decimal(&big);
    }
}
