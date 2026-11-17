# Changelog

All notable changes to the `llm-prices-otel` Python package.

## [0.1.1] - 2026-11-17

### Fixed

- Embedding spans are priced. A span with no output tokens is priced on its input, cache reads
  and batch input on a row that lists no output price (`text-embedding-3-small` and the other
  embedding rows); 0.1.0 flagged every such span `missing_rate`. Output tokens on such a row
  still flag `missing_rate`.
- A token count above 10^12 is `invalid_usage`, as in the Collector processor, so a buggy or
  hostile span cannot write an absurd cost. A digit string longer than 19 characters is
  `invalid_usage` instead of raising `ValueError` (Python refuses to convert digit strings
  longer than 4300 characters), which logged a traceback for every such span.

### Added

- A span priced from a row of another provider than the one that served it (Claude on Bedrock
  or Vertex AI at Anthropic's list price) gets `llm_prices.price.fallback = true` and the matched
  row in `llm_prices.price.row`, also without `breakdown=True`.

## [0.1.0] - 2026-11-17

First release. `CostSpanProcessor`, `CostSpanExporter` and `CostPricer` write
`gen_ai.usage.cost.amount`, `.currency`, `.source`, `.pricing.id` and `.pricing.version` on GenAI
spans from OpenLLMetry, OpenInference, Logfire and the GenAI semantic conventions, and record the
`gen_ai.client.operation.cost` histogram. Price sources: llm-prices (default), LiteLLM's
model_prices_and_context_window.json, genai-prices. Same rules as the `genaicost` Collector
processor; see https://github.com/Mattbusel/llm-prices/blob/main/docs/otel.md.
