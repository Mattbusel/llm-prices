# Changelog

All notable changes to the `llm-prices` Python package. Versions follow the Rust crate's:
price-data-only updates are patch releases, new API is a minor release.

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
