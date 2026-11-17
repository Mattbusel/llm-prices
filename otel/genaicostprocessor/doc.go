//go:generate mdatagen metadata.yaml

// Package genaicostprocessor is an OpenTelemetry Collector processor that prices GenAI
// inference spans from their token counts and writes the cost with the pricing provenance
// proposed in open-telemetry/semantic-conventions-genai#624 (pricing.id and pricing.version), so
// a cost always names the exact price table that produced it.
//
// By default the cost is written as llm_prices.cost.amount, .currency, .source, .pricing.id and
// .pricing.version. With attributes: semconv it is written under the names proposed in #443
// (gen_ai.usage.cost.amount, .currency and .source) and #624 (gen_ai.usage.cost.pricing.id and
// .pricing.version). The #443 text reserves those names for a cost computed at recording time,
// so semconv output is meant for an agent collector next to the application.
//
// It reads the token counts that OpenLLMetry, OpenInference, Logfire, rig and the GenAI
// semantic conventions write, finds the model's price in the llm-prices table (or a LiteLLM
// table), prices uncached input, cache reads, cache writes, output (reasoning included), batch
// requests, service tiers and long-context tiers, and never writes a zero for a span it cannot
// price: such spans are flagged with llm_prices.unpriced. A cost that is already on the span is
// left alone. See docs/otel.md in the repository for the full rules.
package genaicostprocessor // import "github.com/Mattbusel/llm-prices/otel/genaicostprocessor"
