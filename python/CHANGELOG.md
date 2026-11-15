# Changelog

All notable changes to the `llm-prices` Python package. Versions follow the Rust crate's:
price-data-only updates are patch releases, new API is a minor release.

## [1.2.0] - 2026-11-15

First release on PyPI. Same table as the Rust crate 1.2.0 (generated 2026-11-14, 136 rows:
Anthropic, OpenAI, Google, DeepSeek, Mistral, xAI, Cohere, Groq, Together AI, Fireworks AI).

### Added

- `lookup(model)`: the Rust crate's id handling (aliases, any ASCII case, provider prefixes,
  Bedrock and Vertex ids, dated snapshots, `-latest`), with the same answers.
- `lookup_provider(provider, model)` and `Provider` (`Provider.parse`, `rows`, `lookup`,
  `fallback_providers`), so a host's price is never taken for another host's.
- `Price` (frozen dataclass) with `rate`, `rate_str` (the exact text in prices.json),
  `rate_decimal`, `long_context_rate_str`, `provider_kind`, plus `name` and `note`.
- `Price.cost(input, output)` and `Price.cost_usage(usage)` with cache reads, 5m and 1h cache
  writes, batch discounts and the long-context switch. Float results equal the Rust crate's
  bit for bit; `exact=True` (or `cost_decimal` / `cost_usage_decimal`) returns
  `decimal.Decimal`.
- `Usage`, `CostBreakdown`, `DecimalCostBreakdown`, `LongContext`, `all_prices()`,
  `GENERATED`. Type hints throughout (`py.typed`).
