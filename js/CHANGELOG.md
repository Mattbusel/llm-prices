# Changelog

All notable changes to the `@mattbusel/llm-prices` npm package. Versions follow the Rust
crate's: price-data-only updates are patch releases, new API is a minor release.

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

## [1.3.0] - 2026-11-15

### Added

- The repository root is now an ES module package built from this one (`index.mjs`,
  `index.d.ts`, `data.mjs`), so it installs with `npm i github:Mattbusel/llm-prices#v1` and
  loads from jsDelivr and esm.sh without the npm registry. See
  [docs/javascript.md](https://github.com/Mattbusel/llm-prices/blob/main/docs/javascript.md).
- Provider names: `parseProvider` accepts the LangChain `init_chat_model` spellings
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
