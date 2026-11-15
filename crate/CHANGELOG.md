# Changelog

All notable changes to the `llm-prices` crate. Versions follow semver: price-data-only
updates are patch releases, new API is a minor release.

## [1.1.1] - 2026-11-14

### Fixed

- Review fixes on top of 1.1.0: cache-write mapping docs, per-provider lookup tests, pinned clippy toolchain in CI.

## 1.1.0 (unreleased)

Price data is unchanged from 1.0.0 (table generated 2026-10-23).

### Added

- `Usage` and `Price::cost_usage(&Usage) -> Option<CostBreakdown>`: the cost of a whole API
  response, not just uncached input plus output.
  - Cache reads bill at the cache-read price, or at the input price when the model lists none.
  - Cache writes bill by lifetime (`cache_write_5m`, `cache_write_1h`). `None` when the model
    lists no price for a lifetime that was used.
  - `batch: true` bills at the Batch API prices, with the same discount applied to cache
    tokens. `None` when the model lists no batch price.
  - The long-context switch uses the whole prompt (uncached input + cache reads + cache
    writes), strictly greater than the threshold, which is how providers count it.
- `CostBreakdown` with per-component dollars (`input`, `output`, `cache_read`,
  `cache_write`), `total` and a `long_context` flag. It is `#[non_exhaustive]` so fields can
  be added without a major release.
- `Usage::prompt_tokens()`.
- Rustdoc for every public item (`#![deny(missing_docs)]`), including a field mapping from
  Anthropic, OpenAI and Gemini usage objects to `Usage`, and worked doctests for an Anthropic
  cached-prompt call and an OpenAI long-context call.
- `examples/cost.rs`: `cargo run --example cost -- claude-sonnet-4-6 2048 1024 45000 3500`.
- `tests/lookup.rs`: every row's id and alias is checked through the provider prefixes,
  Bedrock ids (`us.anthropic.<id>-v1:0`), Vertex `@date` ids and dated/undated Anthropic ids.
- CI checks MSRV 1.70, `clippy -D warnings` and `cargo doc` with warnings denied.

### Changed

- `Price::cost` is now implemented on top of `cost_usage`. Its results are bit-for-bit the
  same as 1.0.0 for every row (tested).

### Known behavior

- `lookup` does not strip OpenAI's `-YYYY-MM-DD` snapshot suffix. `gpt-4o-2024-08-06` returns
  `None`, because OpenAI prices some snapshots differently from the alias (`gpt-4o-2024-05-13`
  is its own row). This is now covered by a test.
- `Usage` only has cache-write fields for the 5-minute and 1-hour lifetimes. Rows priced on
  other cache lifetimes (the OpenAI `gpt-6-*`, `gpt-6.1-sol` and `gpt-5.6-*` rows, which list
  `30m` and `12h`) cannot have their cache writes priced through `Usage` in 1.x, and those
  tokens do not count toward the long-context threshold. Add
  `tokens * price.cache_write("30m")` (or `"12h"`), divided by 1M, yourself. The `Usage` docs
  show how.

### Planned for 2.0

These need breaking changes, so they will ship together in one major release:

- `#[non_exhaustive]` on `Price`, `LongContext` and `Usage`.
- `Provider` and `Status` enums in place of the `provider` and `status` strings.
- More cache lifetimes in `Usage` (OpenAI `30m` and `12h`).

## 1.0.0 (2026-10-23)

- First release: 74 dated, source-cited rows for Anthropic, OpenAI, Google Gemini and
  DeepSeek, `lookup`, `all`, `Price::cost`, `Price::cache_write`, `GENERATED` and
  `PRICES_JSON`. No dependencies.
