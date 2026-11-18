# Changelog

All notable changes to the `llm-prices` Python package. Versions follow the Rust crate's:
price-data-only updates are patch releases, new API is a minor release.

## [1.8.0] - 2026-11-18

### Changed: Claude on Amazon Bedrock is priced at Bedrock's own rates

AWS bills Claude on Bedrock at two prices: regional endpoints (the in-Region model id and the
geographic inference profiles `us.`, `eu.`, `apac.`, `jp.`, `au.`, `ca.`) and the global
cross-Region profile (`global.`). For Claude Haiku 4.5, Sonnet 4.5, Opus 4.5 and every later
model the regional price is 10% above the global one; Anthropic's pricing page says Bedrock sets these prices itself. Until
1.7.2 every Bedrock Claude id took Anthropic's list price, so a `us.` or in-Region call to
those models came out about 9% low (Sonnet 4.5: $3 / $15 instead of $3.30 / $16.50 per
1M tokens). A reviewer of a downstream pull request pointed out the regional prices in the AWS
price list.

- 33 new `bedrock` rows for 17 Claude models, read daily from the AWS Price List API
  (AmazonBedrockFoundationModels offer file, us-east-1): one regional row (foundation-model id,
  bedrock-mantle id and every geographic profile the model card lists) and one global row
  (`global.anthropic....`) per model, with cache-read, 5-minute and 1-hour cache-write and batch
  prices where AWS lists them. Ids come from the Bedrock model cards. The global rows equal
  Anthropic's list prices (a test checks it); the price list has the same Claude prices in all
  six Regions checked.
- An exact Bedrock id now finds Bedrock's row: `lookup("us.anthropic.claude-sonnet-4-6")` is
  $3.30 input, `lookup("global.anthropic.claude-sonnet-4-6")` $3.00, and the returned row's
  `provider` is `bedrock`. These rows match only their exact ids (any ASCII case, with or
  without `bedrock/`); every other spelling of a Claude id (`claude-sonnet-4-6`,
  `bedrock/claude-sonnet-4-6`, a profile a model card does not list) keeps Anthropic's list
  price, and `lookup_provider("anthropic", ...)` always returns Anthropic's row. Code that
  checked `price.provider == "anthropic"` for a Bedrock id gets `"bedrock"` now.
- `jp.`, `au.` and `ca.` inference profile prefixes are recognized (they returned nothing
  before).
- The GitHub Action no longer compares a hard-coded price next to a Bedrock Claude id with
  Anthropic's row: by default such ids are skipped, and with `providers: bedrock` they are
  compared with Bedrock's own row.
- `litellm.json`: the new rows use LiteLLM's keys (`anthropic.claude-...`, `us.anthropic.claude-...`,
  `global.anthropic.claude-...`) and `litellm_provider: bedrock_converse`.

Same change in every package: Rust crate, Python package, npm package, root module and Go
module `go/v0.4.0`. The OpenTelemetry Collector processor still pins `go/v0.3.0`; its conformance
case for a Bedrock fallback now uses Claude Opus 4, which has no Bedrock row, so it passes on
both.

### Changed: prices (table `sha256:959dc0ad506ff66fc7b112ea10e77455d67dbd9d0f6592f3f11dd788c7d2da64`)

- OpenRouter: the price changes its model API reports on 2026-11-18 (for example
  `deepseek/deepseek-r1-0528` $0.50 / $2.15 to $0.571 / $2.286 with no cache-read price,
  `minimax/minimax-m3.1-flash` $0.15 / $0.60 to $0.075 / $0.30, `xiaomi/mimo-v2.5-pro`
  $0.435 / $0.87 to $0.48 / $1.80).

### Fixed

- The daily refresh no longer fails on DeepSeek. DeepSeek renamed the heading after its pricing
  table from "Search & Fetch Pricing" to "Search & Fetch API Pricing" on 2026-11-18; the
  excerpt step looked for the old text and would have reported the page as unreadable. It now
  ends the excerpt at the first heading after the table. DeepSeek's prices did not change.

### Docs

- [docs/hosts.md](docs/hosts.md): the Claude-on-Bedrock rows, how they are matched, and what
  is left out. [docs/integrate.md](docs/integrate.md): what a reviewer can check (release
  record, data provenance, refresh job, CI) and how to remove the integration.

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

[docs/integrate.md](https://github.com/Mattbusel/llm-prices/blob/main/docs/integrate.md): the calls to use from another project, replacing a hand-written price dict, and llm-prices as an optional extra; runnable copy in examples/python/optional_pricing.py.

## [1.7.0] - 2026-11-17

### Added

`llm_prices.PRICES_VERSION`. Every release now names the exact table it ships. `PRICES_VERSION` is `"sha256:"` and the
SHA-256 of `prices.json` (UTF-8, LF line endings), the same value in every package; it is
generated with the data and CI checks it against the file. Record it as
`gen_ai.usage.cost.pricing.version` next to a cost computed from the table (the attribute
proposed in open-telemetry/semantic-conventions-genai#624). This release's table:
`sha256:e8eed72ddf88c237f1ed8b7ed791495a375620c8166f36b8c554db4e5b616f87`. No price changed.

[llm-prices-otel](https://github.com/Mattbusel/llm-prices/blob/main/docs/otel.md), an
OpenTelemetry span processor that writes `gen_ai.usage.cost.*` on GenAI spans from this table,
lives in otel/python of the same repository.

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
- `llm_prices.compat.tokencost` prices `bedrock/...`, `openrouter/...` and `perplexity/...`
  ids, and Sonar Pro, Sonar Reasoning Pro and Sonar Deep Research (`perplexity/sonar-pro`,
  ...) at the last prices Perplexity's pricing page listed before Sonar Chat Completions
  support ended on 2026-09-27 (archived snapshot of 2026-09-03), so code written for
  tokencost's `perplexity/` and `bedrock/` ids keeps working.

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
- `llm_prices.compat.tokencost` raises `KeyError` ("Model ... is not implemented"), as
  tokencost does for a model it does not price, for these ids, so a caller's
  `except KeyError` fallback runs instead of a silently wrong cost. Its fallback to the id
  after the last slash only applies when every provider in the path sells that model at that
  price: `anthropic/claude-3-opus-20240229`, `openai/gpt-4`, `gemini/gemini-1.5-pro`,
  `bedrock/claude-3-haiku-20240307` and `openrouter/anthropic/...` keep working;
  `accounts/fireworks/models/deepseek-r1`, `bedrock/deepseek-r1` and
  `huggingface/deepseek-r1` raise.

No change to the table.

## [1.5.0] - 2026-11-17

### Added

- `llm_prices.compat.tokencost` prices retired and legacy models tokencost 0.1.26 priced and
  the table does not: about 160 ids across OpenAI (`gpt-4`, `gpt-4-32k`, `gpt-3.5-turbo`
  snapshots, `chatgpt-4o-latest`, `gpt-4o-search-preview`, `o1-mini`, `o1-preview`,
  `gpt-4.5-preview`, `o3-deep-research`, `o4-mini-deep-research`, audio and realtime
  previews, `davinci-002`, `babbage-002`, fine-tunes), Anthropic (`claude-3-*`, `claude-2.x`,
  `claude-instant-1.2`, `-latest` aliases), Gemini (`gemini-1.0-pro`, `gemini-1.5-*`,
  `gemini-2.0-flash`, `-lite`, `-live-001`, `gemini-2.5-*-preview-*`), Mistral, xAI (Grok 2
  and 3), Cohere (Command, Command R, Embed v2 to v4) and `deepseek-coder`. Each row is the last
  price on the provider's own pricing page, read from an archived snapshot (URL and date on
  the row), with the shutdown date from the deprecation page. `LEGACY_MODELS` and
  `LEGACY_REDIRECTS` expose them; `TOKEN_COSTS` entries carry `source`, `llm_prices_checked`,
  `deprecation_date` and `llm_prices_legacy`. Rows the current table prices always win.
- Retired ids that the provider now bills as another model are priced as that model:
  `grok-3`, `grok-4-0709` (and aliases) as `grok-4.3`; `deepseek-v3`, `deepseek-r1` as
  `deepseek-flash`.
- `ft:` keys in `TOKEN_COSTS` (for example `ft:gpt-3.5-turbo`) are looked up before the
  fine-tune prefix is stripped.

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

- `llm_prices.compat.tokencost`: tokencost's API on the llm-prices table, same names,
  signatures, `Decimal` results and errors: `calculate_prompt_cost`,
  `calculate_completion_cost`, `calculate_cost_by_tokens`, `calculate_all_costs_and_tokens`,
  `count_message_tokens`, `count_string_tokens`, `configure_model`, `register_model_pattern`,
  `TOKEN_COSTS`, plus `count_tokens`. Token counting needs tiktoken:
  `pip install "llm-prices[tokencost]"`. tokencost's own test suite is ported in
  `tests/test_tokencost_compat.py`. Guide:
  [docs/migrate-from-tokencost.md](https://github.com/Mattbusel/llm-prices/blob/main/docs/migrate-from-tokencost.md).
- `llm_prices.litellm.model_cost()`: the table in LiteLLM's model_prices_and_context_window.json
  schema, for `litellm.register_model`. The JSON ships in the package.
- Service tiers: `Price.tiers` (name to `Price`), `Price.tier(name)`, and
  `cost_usage(..., tier="flex")`. Gemini Flex and Priority tiers carry their own long-context
  prices.
- Data residency: `Price.inference_geo` (`{"us": 1.1}` on Claude 4.6 and later),
  `Price.with_inference_geo(geo)` (exact decimal products) and
  `cost_usage(..., inference_geo="us")`.
- Classifiers for Python 3.9 to 3.13, a `Documentation` project URL.

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

- `Price.input_per_token`, `Price.output_per_token` and `Price.per_token(rate, ttl=None)`:
  prices in USD per token, for code that keeps per-token prices (cost callbacks, LangChain,
  LiteLLM-style tables). `None` when the page lists no price.
- Provider names: `Provider` parsing accepts the LangChain `init_chat_model` spellings
  `google_genai`, `google_vertexai` and `bedrock_converse`, plus `google-generativeai`,
  `genai`, `google_ai`, `bedrock_runtime`, `amazon`, `azure_ai_foundry`, `groqcloud` and
  `cohere_ai`. `ollama`, `huggingface` and other unpriced hosts still return no provider.
- `lookup_provider` is documented as the call for cost callbacks, with its exact semantics:
  provider spellings, model id handling, which rows are searched, the Vertex / Bedrock / Azure
  fallbacks and when it returns `None`. See
  [docs/python.md](https://github.com/Mattbusel/llm-prices/blob/main/docs/python.md).

### Changed

- `Provider.parse` and `lookup_provider` return `None` for a provider that is not a string
  (for example `None`) instead of raising `AttributeError`.

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
