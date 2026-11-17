package genaicostprocessor // import "github.com/Mattbusel/llm-prices/otel/genaicostprocessor"

import (
	llmprices "github.com/Mattbusel/llm-prices/go"
)

// request is what a price source needs to price one span.
type request struct {
	// provider is the llm-prices name of the provider that served the request ("openai",
	// "vertex", ...), "" when the span does not say.
	provider string
	// models are the model ids to try, in order (response model first).
	models []string
	tokens tokens
	batch  bool
	// tier is the service tier ("flex", "priority", ...), "" for standard.
	tier string
}

// priced is a span's cost in USD.
type priced struct {
	input, output, cacheRead, cacheWrite, total float64
	// row is the matched price row, "provider/model".
	row string
}

// priceSource prices requests from one price table. Implementations are immutable; a reload
// builds a new one.
type priceSource interface {
	// pricingID is written as gen_ai.usage.cost.pricing.id.
	pricingID() string
	// pricingVersion is written as gen_ai.usage.cost.pricing.version.
	pricingVersion() string
	// price returns the cost, or "" and the reason it cannot price the request.
	price(r request) (priced, string)
}

// llmPricesSource prices from an llm-prices table.
type llmPricesSource struct {
	table *llmprices.Table
}

func (s llmPricesSource) pricingID() string      { return "llm-prices" }
func (s llmPricesSource) pricingVersion() string { return s.table.Version() }

func (s llmPricesSource) price(r request) (priced, string) {
	var row llmprices.Price
	found := false
	for _, m := range r.models {
		if r.provider == "" {
			row, found = s.table.Lookup(m)
		} else {
			row, found = s.table.LookupIn(llmprices.Provider(r.provider), m)
		}
		if found {
			break
		}
	}
	if !found {
		return priced{}, ReasonUnknownModel
	}
	p, ok := row.Tier(r.tier)
	if !ok {
		return priced{}, ReasonUnpricedTier
	}
	return costLLMPrices(p, r.tokens, r.batch)
}

// costLLMPrices prices token counts with one llm-prices row. Cache writes are billed at the
// 5-minute write price (the GenAI conventions carry no cache lifetime; it is Anthropic's
// default), or as uncached input when the row lists no cache-write price at all (providers that
// do not charge for writes).
func costLLMPrices(p llmprices.Price, t tokens, batch bool) (priced, string) {
	u := llmprices.Usage{Input: t.input, Output: t.output, CacheRead: t.cacheRead, Batch: batch}
	if t.cacheWrite > 0 {
		if _, ok := p.CacheWrite["5m"]; ok {
			u.CacheWrite5m = t.cacheWrite
		} else if len(p.CacheWrite) == 0 {
			u.Input += t.cacheWrite
		} else {
			return priced{}, ReasonMissingRate
		}
	}
	c, ok := p.CostUsage(u)
	if !ok {
		return priced{}, ReasonMissingRate
	}
	return priced{
		input: c.Input, output: c.Output, cacheRead: c.CacheRead, cacheWrite: c.CacheWrite,
		total: c.Total, row: p.Provider + "/" + p.Model,
	}, ""
}
