# Changelog

All notable changes to the `@mattbusel/llm-prices` npm package. Versions follow the Rust
crate's: price-data-only updates are patch releases, new API is a minor release.

## [1.2.0] - 2026-11-15

First release on npm. Same table as the Rust crate 1.2.0 (generated 2026-11-14, 136 rows:
Anthropic, OpenAI, Google, DeepSeek, Mistral, xAI, Cohere, Groq, Together AI, Fireworks AI).
TypeScript, zero dependencies, ES module and CommonJS builds, Node 18 or later.

### Added

- `lookup(model)`: the Rust crate's id handling (aliases, any ASCII case, provider prefixes,
  Bedrock and Vertex ids, dated snapshots, `-latest`), with the same answers.
- `lookupProvider(provider, model)`, `Provider`, `PROVIDERS`, `parseProvider`,
  `providerRows`, `fallbackProviders`, so a host's price is never taken for another host's.
- `Price` (frozen) with the prices.json field names, plus `rate`, `rateStr` (the exact text
  in prices.json), `longContextRateStr`, `cacheWrite`, `providerKind`.
- `Price.cost(input, output)` and `Price.costUsage(usage)` with cache reads, 5m and 1h cache
  writes, batch discounts and the long-context switch. Results equal the Rust crate's bit for
  bit. `costExact` and `costUsageExact` return exact decimal strings (computed with `bigint`)
  for billing code. Token counts may be `number` or `bigint`.
- `all()`, `GENERATED`, `promptTokens`, `longContextRate`, and the `Usage`, `CostBreakdown`,
  `ExactCostBreakdown`, `LongContext`, `Tier`, `PriceInit`, `RateName` types.
