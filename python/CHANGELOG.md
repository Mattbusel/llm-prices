# Changelog

All notable changes to the `llm-prices` Python package. Versions follow the Rust crate's:
price-data-only updates are patch releases, new API is a minor release.

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
