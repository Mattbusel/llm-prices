# Changelog

All notable changes to the `llm-prices-otel` Python package.

## [0.1.0] - 2026-11-17

First release. `CostSpanProcessor`, `CostSpanExporter` and `CostPricer` write
`gen_ai.usage.cost.amount`, `.currency`, `.source`, `.pricing.id` and `.pricing.version` on GenAI
spans from OpenLLMetry, OpenInference, Logfire and the GenAI semantic conventions, and record the
`gen_ai.client.operation.cost` histogram. Price sources: llm-prices (default), LiteLLM's
model_prices_and_context_window.json, genai-prices. Same rules as the `genaicost` Collector
processor; see https://github.com/Mattbusel/llm-prices/blob/main/docs/otel.md.
