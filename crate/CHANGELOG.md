# Changelog

All notable changes to the `llm-prices` crate. Versions follow semver: price-data-only
updates are patch releases, new API is a minor release.

## [1.7.2] - 2026-11-18

### Changed: prices (table `sha256:16750e481602ea77ca061b5f6d84e951017347c6d2d8e28f8bf4c6b3538ce048`)

- OpenAI: new rows `gpt-6.2-astra` ($8 input / $40 output / $0.40 cached input per 1M tokens),
  `gpt-6.2-sol` ($2 / $10 / $0.10) and `gpt-6.2-astra-law` ($8 / $40 / $0.40), each with its
  long-context prices; Astra and Sol also have batch, fast and ultrafast. `gpt-6-rosalind` dropped to $10 / $50 / $1
  (long context $20 / $75 / $2). `gpt-6-astra-law` gained its long-context row ($20 / $75 / $2).
- OpenRouter: rows for `openai/gpt-6.2-astra`, `-astra-pro`, `-sol` and `-sol-pro`, and the
  price changes OpenRouter's model API now reports (for example `~openai/gpt-astra-latest`
  $10 / $50 to $8 / $40).

### Fixed

- OpenAI batch and flex prices are kept. The OpenAI page renamed its "Batch" and "Flex" table
  headings to "Batch pricing data" and "Flex pricing data" on 2026-11-17; the parser missed
  them, and the refresh pull request of that day would have set every OpenAI `batch_input`,
  `batch_output` and flex tier to null. Both spellings are read now.
- OpenAI Ultrafast: for models with a promotional Ultrafast rate (GPT-6.2 Astra and Sol), the
  promotion table is the `ultrafast` tier in force, and the note gives the list price the page
  announces from 2027-03-01.

### Docs

The crate README lists every provider the table covers, the features, the MSRV and the version policy, and links [docs/integrate.md](https://github.com/Mattbusel/llm-prices/blob/main/docs/integrate.md): the calls to use from another project and how to put llm-prices behind an optional feature.

## [1.7.0] - 2026-11-17

### Added

`llm_prices::PRICES_VERSION`. Every release now names the exact table it ships. `PRICES_VERSION` is `"sha256:"` and the
SHA-256 of `prices.json` (UTF-8, LF line endings), the same value in every package; it is
generated with the data and CI checks it against the file. Record it as
`gen_ai.usage.cost.pricing.version` next to a cost computed from the table (the attribute
proposed in open-telemetry/semantic-conventions-genai#624). This release's table:
`sha256:e8eed72ddf88c237f1ed8b7ed791495a375620c8166f36b8c554db4e5b616f87`. No price changed.

## [1.6.1] - 2026-11-17

### Added

`llm_prices.compat.tokencost` prices `perplexity/sonar-reasoning` (and `sonar-reasoning`) at its
last listed $1 / $5 per 1M tokens (archived pricing page of 2025-12-05; Perplexity removed the
model on 2025-12-15), as tokencost 0.1.26 did. No change to the table, the crate, the npm package
or the Go module; they move to 1.6.1 only to keep the versions in step.

## [1.6.0] - 2026-11-17

### Added

Third-party host prices: Amazon Bedrock, OpenRouter and Perplexity now have rows of their own,
so `bedrock/us.deepseek.r1-v1:0`, `openrouter/meta-llama/llama-3.3-70b-instruct` and
`perplexity/sonar` cost what those hosts charge instead of finding nothing.

- Amazon Bedrock (64 rows): read daily from AWS's own price list (the AWS Price List bulk API
  offer file for AmazonBedrock, us-east-1), the data https://aws.amazon.com/bedrock/pricing/
  renders. On-demand input, output and cache-read prices, Batch inference, and the flex and
  priority service tiers. Model ids come from each model's card in the Bedrock user guide:
  the in-Region id (`deepseek.r1-v1:0`), the US geo inference profile (`us.deepseek.r1-v1:0`)
  and the bedrock-mantle id (`openai.gpt-oss-120b`). Global inference profiles bill at their
  own price and are not aliases. Claude on Bedrock still takes Anthropic's list price.
- OpenRouter (356 rows): read daily from OpenRouter's model list API
  (https://openrouter.ai/api/v1/models), keyed by OpenRouter's id. Prompt-length overrides
  become `long_context`, `:batch` variants the batch price; `:free`, zero-priced and router
  entries are left out.
- Perplexity (6 rows, checked by hand): the Agent API rates for the models Perplexity serves
  itself (`perplexity/sonar`, `perplexity/kimi-k3`, `perplexity/glm-5.3`, ...) from
  https://docs.perplexity.ai/docs/agent-api/models. New provider `perplexity` (also parses
  `pplx`), a host like Groq or Fireworks.

Lookup rules for the new rows:

- A provider's own row now wins over its fallback makers: `lookup_provider("openrouter",
  "x-ai/grok-4.7")` returns OpenRouter's row when OpenRouter lists the model.
- Without a provider path, Bedrock and OpenRouter rows are only found by their exact id
  (`deepseek.r1-v1:0`, `meta-llama/llama-3.3-70b-instruct`), so a bare `gpt-4` never takes
  OpenRouter's `openai/gpt-4` row.
- A maker's prefix on a model the maker does not sell (`x-ai/grok-4.1-fast`) tries the hosts'
  rows first and then a platform row with that exact id.
- A host lists models under another host's name (OpenRouter's `perplexity/sonar-pro`,
  `amazon/nova-2-lite-v1`); looking those up under the host that lists them finds them.

## [1.5.1] - 2026-11-17

### Fixed

Model ids with a host or platform path are no longer priced with the model maker's
first-party row. Fireworks, Together AI, Groq, Bedrock, Vertex AI, Azure and OpenRouter charge
their own prices, yet `fireworks_ai/deepseek-chat` or `together_ai/deepseek-chat` came back
at DeepSeek's own $0.30 / $1.20 (and, in `llm_prices.compat.tokencost`, so did
`accounts/fireworks/models/deepseek-r1`). They now find nothing. The rules:

- A host path (`groq/`, `together_ai/`, `together/`, `fireworks_ai/`, `fireworks/`,
  `accounts/fireworks/models/`) resolves to that host's own row (`groq/openai/gpt-oss-120b`,
  `accounts/fireworks/models/kimi-k3`) or to nothing.
- A platform path (`bedrock/`, `vertex_ai/`, `azure/`, `openrouter/`) resolves to the
  platform's row or the list price of the makers it charges at (Bedrock: Anthropic; Vertex:
  Google and Anthropic; Azure: OpenAI; OpenRouter: the model makers), as before, and to
  nothing for anything else (`bedrock/deepseek-r1`, `azure/claude-sonnet-4-6`).
- A model maker's prefix (`anthropic/`, `openai/`, `gemini/`, `deepseek/`, ...) resolves to
  that maker's rows, or to a row whose own id carries the same prefix (Groq's
  `openai/gpt-oss-120b`). `anthropic/gpt-4o` no longer finds OpenAI's row.
- Looking a model up under a named provider never crosses into another host's path:
  `lookup_provider("deepseek", "together_ai/deepseek-chat")` finds nothing.

No change to the table.

## [1.5.0] - 2026-11-17

No change to this package. Version kept in step with the Python package, whose 1.5.0 adds
retired and legacy model prices to `llm_prices.compat.tokencost`.

## [1.4.1] - 2026-11-17

### Table (generated 2026-11-17, 146 rows)

Price data only, no API change. Every row re-read from its official page: no standard input
or output price changed; Mistral rows gain cached-input, Batch and Priority prices, Groq
gpt-oss rows gain cached-input and Batch prices, xAI rows gain Priority Processing and the US
endpoint multiplier, Together AI and Fireworks AI rows carry their US-only multipliers as
`inference_geo`, and notes explain the Claude Sonnet 5, DeepSeek and Gemini 3.6 Flash prices
other tables disagree on. Details in the
[repository changelog](https://github.com/Mattbusel/llm-prices/blob/main/CHANGELOG.md#141---2026-11-17).

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
