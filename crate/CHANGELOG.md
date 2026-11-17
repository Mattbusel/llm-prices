# Changelog

All notable changes to the `llm-prices` crate. Versions follow semver: price-data-only
updates are patch releases, new API is a minor release.

## [1.4.0] - 2026-11-16

### Added

- Service tiers: `Price::tier(name)` returns the prices of `"flex"`, `"priority"`, `"fast"`,
  `"ultrafast"`, `"offpeak"` and the other tiers a provider's page lists, as a
  `&'static Price` (Gemini's Flex and Priority tiers with their own long-context prices).
  `Price::tiers()` lists the names. `tier("standard")` is the row itself.
- Data residency: `Price::inference_geo(geo)` and `inference_geo_str(geo)` return the
  multiplier the table lists (`Some(1.1)` for `"us"` on Claude 4.6 and later, `None` for
  earlier Claude models, `Some(1.0)` for `"global"`). `Price::cost_usage_geo(&usage, geo)`
  and, with `rust_decimal`, `Price::cost_usage_decimal_geo(&usage, geo)` apply it.
- Examples: `budget_check` (refuse a call whose worst case breaks a budget) and `tiers`.

### Table (generated 2026-11-16, 146 rows)

Anthropic, OpenAI, Google and DeepSeek rows re-read from their pricing pages on 2026-11-16. No
price changed. New data, all read from the official pages:

- `inference_geo` on every row: `{"us": 1.1}` on Claude 4.6 and later (Opus 4.6 to 5.5,
  Sonnet 4.6 to 5.5, Haiku 5.5, Fable 5 to 5.2), from "Data residency pricing" on
  https://platform.claude.com/docs/en/about-claude/pricing; `null` elsewhere. Earlier Claude
  rows now say the page rejects the parameter for them, instead of a note on every row.
- Gemini `other_tiers.flex` and `other_tiers.priority` carry their own `long_context` prices
  (Gemini 2.5 Pro: flex 1.25 / 7.5, priority 4.5 / 27 above 200k tokens; Gemini 3.1 Pro
  Preview: flex 2 / 9, priority 7.2 / 32.4), from the Flex and Priority tabs of
  https://ai.google.dev/gemini-api/docs/pricing.
- Anthropic Priority Tier has no row: it is a capacity commitment with no per-token price, and
  https://platform.claude.com/docs/en/api/service-tiers says commitments are no longer sold.
  See [docs/tiers.md](https://github.com/Mattbusel/llm-prices/blob/main/docs/tiers.md).

## [1.3.0] - 2026-11-15

### Added

- Provider names: `Provider` parsing accepts the LangChain `init_chat_model` spellings
  `google_genai`, `google_vertexai` and `bedrock_converse`, plus `google-generativeai`,
  `genai`, `google_ai`, `bedrock_runtime`, `amazon`, `azure_ai_foundry`, `groqcloud` and
  `cohere_ai`. `ollama`, `huggingface` and other unpriced hosts still return no provider.

### Table (generated 2026-11-15, 146 rows)

The daily refresh reads all four parsed pages again (it had kept the 2026-10-23 rows since
early November because three pages changed shape). No existing price changed.

- New rows: `claude-fable-5-2` (8 / 40, cache read 0.2), `gpt-6.1-terra` (1 / 5),
  `gpt-6-cyber` (20 / 100, long context 40 / 150), `gpt-5.5-cyber-preview`, `gpt-6-rosalind`,
  `gpt-rosalind-research`, `gpt-rosalind-discovery`, `gpt-6-astra-law`, `gemini-4-carbon`
  (2 / 10 until 2026-11-30, then 4 / 20) and `gemini-4-hydrogen` (1.5 / 7.5).
- `deepseek-chat` and `deepseek-reasoner` are aliases of `deepseek-flash`. DeepSeek's change
  log (2026-04-24) points both legacy names to `deepseek-v4-flash`, which the pricing page
  bills at the Flash price.
- Models that left their pricing page keep their last listed prices with status `retired`
  and a note saying since when: `claude-3-5-haiku-20241022`, `gpt-3.5-turbo`,
  `gpt-3.5-turbo-0125`, `gpt-4-0613`, `gpt-4-turbo-2024-04-09`, `gpt-4o-2024-05-13`,
  `gpt-4.1-nano`, `o1`, `o1-pro`, `o3-mini`, `o4-mini`, `gpt-5.5-cyber`. Lookups of those ids
  keep working.

## [1.2.0] - 2026-11-14

Table generated 2026-11-14: the 74 Anthropic, OpenAI, Gemini and DeepSeek rows are unchanged
(checked 2026-10-23); 62 rows for six more providers are new.

### Added

- **Provider-aware lookup.** `lookup_provider(provider, model)` and `Provider::lookup(model)`
  return only the row that provider sells, so a gateway does not price `openai/gpt-oss-120b`
  on Groq with Together AI's or Fireworks' price. `provider` is a `Provider` or a name
  (`"groq"`, `"together_ai"`, `"vertex_ai"`, ...).
  - `Provider` enum (`#[non_exhaustive]`): Anthropic, OpenAi, Google, Vertex, Bedrock, Azure,
    DeepSeek, Mistral, Groq, Together, Fireworks, Xai, Cohere, OpenRouter, with `as_str`,
    `parse` (accepts LiteLLM, OpenRouter and SDK spellings such as `gemini`, `vertex_ai`,
    `x-ai`), `FromStr`, `Display`, `AsRef<str>`, `ALL`, `rows()` and `fallback_providers()`.
  - Vertex AI, Bedrock and Azure have no rows of their own; they fall back to the maker's
    list price where the platform charges it on global endpoints (Vertex: Gemini and Claude;
    Bedrock: Claude; Azure: OpenAI). OpenRouter falls back to every model maker. The returned
    row's `provider` says whose price it is. Anything else returns `None`.
  - `Price::provider_kind()` parses `Price::provider`.
- **Dated snapshot and alias resolution in `lookup`.** OpenAI `-YYYY-MM-DD` snapshot ids
  resolve to their base row (`gpt-4o-2024-08-06` finds `gpt-4o`) unless the table prices
  the snapshot on its own row (`gpt-4o-2024-05-13`). `-latest` ids resolve to the model's
  row (`claude-3-5-haiku-latest`, `grok-4.7-latest`). Dated and undated forms now work for
  every provider, not only Anthropic. Ids are matched ignoring ASCII case.
- `lookup` honors a leading provider in the id (`groq/...`, `together_ai/...`,
  `fireworks_ai/...`, `accounts/fireworks/models/...`) and accepts the prefixes `mistral/`,
  `mistralai/`, `xai/`, `x-ai/`, `cohere/`, `groq/`, `together/`, `together_ai/`,
  `fireworks/`, `fireworks_ai/` and `accounts/fireworks/models/`.
- **Exact prices.** `Rate` enum and `Price::rate`, `Price::rate_str`,
  `Price::long_context_rate_str` and `LongContext::rate`. `rate_str` returns the price
  exactly as `prices.json` writes it (`"0.075"`), for billing code that never uses `f64`.
- **`rust_decimal` feature.** `Price::cost_usage_decimal(&Usage) -> Option<DecimalCostBreakdown>`,
  `Price::cost_decimal` and `Price::rate_decimal`, in `rust_decimal::Decimal`. Same rules and
  the same `None` cases as `cost_usage`, with no rounding (the batch discount on cache tokens
  is exact on every current row).
- **`serde` feature.** `Serialize` for `Price`, `LongContext` (cache writes as a map, the
  shape of `prices.json`), `CostBreakdown` and `DecimalCostBreakdown`; `Serialize` and
  `Deserialize` for `Usage` (missing fields default to zero) and `Provider` (as its name).
- `iter()`: an iterator over every row, next to the existing `all()`.
- **New rows**, each read on 2026-11-14 from the provider's official pricing page, with that
  page as `source_url`:
  - Mistral (11, https://docs.mistral.ai/inference/pricing, aliases such as
    `mistral-large-latest` from each model's docs page),
  - xAI (8, https://docs.x.ai/developers/pricing, with the 200k-token long-context tier and
    the 20% Batch API discount),
  - Cohere (1, https://cohere.com/pricing; the page prices only legacy models),
  - Groq (6, https://console.groq.com/docs/models),
  - Together AI (22, https://docs.together.ai/docs/serverless/models),
  - Fireworks AI (14, https://docs.fireworks.ai/serverless/pricing, with Priority and Fast
    tiers in `other_tiers`).

  These rows are checked by hand: the daily refresh keeps them as they are and does not
  re-read those pages. `scripts/build.mjs` regenerates the derived files after editing one.

### Changed

- `lookup("gpt-4o-2024-08-06")` and other dated OpenAI ids now return the base row instead
  of `None` (the 1.1 "Known behavior" note).
- Model ids are unique per provider, not across the table: `openai/gpt-oss-120b` is a Groq
  row and a Together AI row. Plain `lookup` returns the first in table order (Groq); use
  `lookup_provider` to pick.
- The GitHub Action's drift check still uses only the Anthropic, OpenAI, Gemini and DeepSeek
  rows unless `providers` names others, and skips ids two named providers both sell.

### Notes

- MSRV stays 1.70 with default features. The optional features need whatever `rust_decimal`
  and `serde` require (currently Rust 1.71 for `serde_derive`).
- Bedrock, Vertex AI and Azure rows were not added. The Bedrock and Azure pricing pages fill
  their prices in the browser from regional price feeds (the page itself holds placeholders),
  and the Vertex AI page lists display names, global and regional prices and promotions
  without API ids, so no row could be read from the page as published.

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
