//! `serde` support (the `serde` feature).

use serde::de::{Deserializer, Error as _};
use serde::ser::Serializer;
use serde::{Deserialize, Serialize};

use crate::Provider;

/// Serializes a cache-write list as a map from cache lifetime to price, the shape
/// `prices.json` uses.
pub(crate) fn cache_write<S: Serializer>(v: &&'static [(&'static str, f64)], s: S) -> Result<S::Ok, S::Error> {
    s.collect_map(v.iter().map(|(ttl, price)| (*ttl, *price)))
}

impl Serialize for Provider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for Provider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let name = std::borrow::Cow::<'de, str>::deserialize(d)?;
        Provider::parse(&name).ok_or_else(|| D::Error::custom(format_args!("unknown LLM provider {name:?}")))
    }
}

#[cfg(test)]
mod tests {
    use crate::{lookup, Provider, Usage};

    #[test]
    fn price_serializes_like_prices_json() {
        let p = lookup("claude-haiku-5-5").unwrap();
        let v = serde_json::to_value(p).unwrap();
        assert_eq!(v["provider"], "anthropic");
        assert_eq!(v["model"], "claude-haiku-5-5");
        assert_eq!(v["input"], 0.1);
        assert_eq!(v["cache_write"]["5m"], 0.125);
        assert_eq!(v["long_context"]["threshold_tokens"], 100_000);
        assert_eq!(v["long_context"]["cache_write"]["1h"], 1.0);
        assert_eq!(serde_json::to_value(lookup("gpt-4o").unwrap()).unwrap()["cache_write"], serde_json::json!({}));
    }

    #[test]
    fn usage_round_trips_and_defaults() {
        let u = Usage { input: 5, cache_read: 7, batch: true, ..Usage::default() };
        let back: Usage = serde_json::from_str(&serde_json::to_string(&u).unwrap()).unwrap();
        assert_eq!(back, u);
        let partial: Usage = serde_json::from_str(r#"{"input": 10, "output": 2}"#).unwrap();
        assert_eq!(partial, Usage { input: 10, output: 2, ..Usage::default() });
    }

    #[test]
    fn cost_breakdown_serializes() {
        let c = lookup("claude-sonnet-4-6").unwrap().cost_usage(&Usage { input: 1_000_000, ..Usage::default() }).unwrap();
        let v = serde_json::to_value(c).unwrap();
        assert_eq!(v["input"], 3.0);
        assert_eq!(v["long_context"], false);
    }

    #[test]
    fn provider_as_string() {
        assert_eq!(serde_json::to_string(&Provider::Together).unwrap(), "\"together\"");
        let p: Provider = serde_json::from_str("\"together_ai\"").unwrap();
        assert_eq!(p, Provider::Together);
        assert!(serde_json::from_str::<Provider>("\"nope\"").is_err());
    }

    #[cfg(feature = "rust_decimal")]
    #[test]
    fn decimal_cost_serializes_as_strings() {
        let c = lookup("claude-haiku-5-5").unwrap().cost_usage_decimal(&Usage { input: 10_000, output: 2_000, ..Usage::default() }).unwrap();
        assert_eq!(serde_json::to_value(c).unwrap()["total"], "0.002");
    }
}
