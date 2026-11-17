package genaicostprocessor // import "github.com/Mattbusel/llm-prices/otel/genaicostprocessor"

import (
	"math"
	"strconv"
	"strings"

	"go.opentelemetry.io/collector/pdata/pcommon"
)

// Attributes the processor writes. The gen_ai.usage.cost.* names are the ones proposed in
// open-telemetry/semantic-conventions-genai#443 (amount, currency, source) and #624
// (pricing.id, pricing.version).
const (
	AttrCostAmount         = "gen_ai.usage.cost.amount"
	AttrCostCurrency       = "gen_ai.usage.cost.currency"
	AttrCostSource         = "gen_ai.usage.cost.source"
	AttrCostPricingID      = "gen_ai.usage.cost.pricing.id"
	AttrCostPricingVersion = "gen_ai.usage.cost.pricing.version"

	// AttrUnpriced holds the reason a GenAI span with token counts got no cost.
	AttrUnpriced = "llm_prices.unpriced"
	// AttrPriceRow is the matched price row, "provider/model" (with breakdown: true).
	AttrPriceRow = "llm_prices.price.row"
	// AttrPriceTier is the service tier the span was priced at, when not standard.
	AttrPriceTier = "llm_prices.price.tier"

	attrErrorType = "error.type"
)

// Reasons written to llm_prices.unpriced.
const (
	ReasonNoModel         = "no_model"
	ReasonUnknownProvider = "unknown_provider"
	ReasonUnknownModel    = "unknown_model"
	ReasonMissingRate     = "missing_rate"
	ReasonUnpricedTier    = "unpriced_tier"
	ReasonInvalidUsage    = "invalid_usage"
	ReasonStaleSpan       = "stale_span"
)

// Token-count attribute keys, first match wins: the GenAI semantic conventions, then the older
// names OpenLLMetry wrote, then OpenInference.
var (
	inputKeys = []string{
		"gen_ai.usage.input_tokens", "gen_ai.usage.prompt_tokens",
		"llm.token_count.prompt", "llm.usage.prompt_tokens",
	}
	outputKeys = []string{
		"gen_ai.usage.output_tokens", "gen_ai.usage.completion_tokens",
		"llm.token_count.completion", "llm.usage.completion_tokens",
	}
	cacheReadKeys = []string{
		"gen_ai.usage.cache_read.input_tokens", "gen_ai.usage.cache_read_input_tokens",
		"llm.token_count.prompt_details.cache_read",
	}
	cacheWriteKeys = []string{
		"gen_ai.usage.cache_write.input_tokens", "gen_ai.usage.cache_creation.input_tokens",
		"gen_ai.usage.cache_creation_input_tokens", "llm.token_count.prompt_details.cache_write",
	}
	reasoningKeys = []string{
		"gen_ai.usage.reasoning.output_tokens", "gen_ai.usage.reasoning_tokens",
		"llm.token_count.completion_details.reasoning", "llm.usage.reasoning_tokens",
	}
	modelKeys = []string{
		"gen_ai.response.model", "gen_ai.request.model", "llm.model_name",
		"llm.response.model", "llm.request.model",
	}
	providerKeys = []string{"gen_ai.provider.name", "gen_ai.system", "llm.provider", "llm.system"}
	tierKeys     = []string{
		"openai.response.service_tier", "gen_ai.openai.response.service_tier",
		"openai.request.service_tier", "gen_ai.openai.request.service_tier",
	}
	serverKeys = []string{"server.address", "net.peer.name"}
)

// providerAliases maps provider names used by the GenAI semantic conventions (gen_ai.provider.name
// and the older gen_ai.system) and by instrumentation libraries to llm-prices provider names.
// Names not listed here go through llmprices.ParseProvider.
var providerAliases = map[string]string{
	"gcp.gen_ai":         "vertex",
	"gcp.vertex_ai":      "vertex",
	"gcp.gemini":         "google",
	"vertex_ai":          "vertex",
	"vertexai":           "vertex",
	"az.ai.inference":    "azure",
	"az.ai.openai":       "azure",
	"azure.ai.inference": "azure",
	"azure.ai.openai":    "azure",
	"aws.bedrock":        "bedrock",
	"x_ai":               "xai",
	"mistral_ai":         "mistral",
}

// hostProviders maps API host names (server.address) to the provider that bills requests sent
// there. A known host wins over the provider attribute, because instrumentations of an
// OpenAI-compatible client report "openai" for every endpoint the client is pointed at.
var hostProviders = []struct{ suffix, provider string }{
	{"api.openai.com", "openai"},
	{"api.anthropic.com", "anthropic"},
	{"generativelanguage.googleapis.com", "google"},
	{"aiplatform.googleapis.com", "vertex"},
	{"openai.azure.com", "azure"},
	{"services.ai.azure.com", "azure"},
	{"api.deepseek.com", "deepseek"},
	{"api.mistral.ai", "mistral"},
	{"api.x.ai", "xai"},
	{"api.groq.com", "groq"},
	{"api.together.xyz", "together"},
	{"api.together.ai", "together"},
	{"api.fireworks.ai", "fireworks"},
	{"openrouter.ai", "openrouter"},
	{"api.perplexity.ai", "perplexity"},
	{"api.cohere.com", "cohere"},
	{"api.cohere.ai", "cohere"},
}

func providerFromHost(host string) string {
	host = strings.ToLower(strings.TrimSuffix(strings.TrimSpace(host), "."))
	if strings.HasPrefix(host, "bedrock-runtime.") && strings.HasSuffix(host, ".amazonaws.com") {
		return "bedrock"
	}
	for _, h := range hostProviders {
		if host == h.suffix || strings.HasSuffix(host, "."+h.suffix) {
			return h.provider
		}
	}
	return ""
}

func str(attrs pcommon.Map, keys []string) (string, bool) {
	for _, k := range keys {
		if v, ok := attrs.Get(k); ok && v.Type() == pcommon.ValueTypeStr {
			if s := strings.TrimSpace(v.Str()); s != "" {
				return s, true
			}
		}
	}
	return "", false
}

// count reads the first of keys that is set. present is false when none is set; valid is false
// when the value is not a whole non-negative number.
func count(attrs pcommon.Map, keys []string) (n int64, present, valid bool) {
	for _, k := range keys {
		v, ok := attrs.Get(k)
		if !ok {
			continue
		}
		switch v.Type() {
		case pcommon.ValueTypeInt:
			return v.Int(), true, v.Int() >= 0
		case pcommon.ValueTypeDouble:
			f := v.Double()
			if f < 0 || f != math.Trunc(f) || f > 1<<53 {
				return 0, true, false
			}
			return int64(f), true, true
		case pcommon.ValueTypeStr:
			i, err := strconv.ParseInt(strings.TrimSpace(v.Str()), 10, 64)
			return i, true, err == nil && i >= 0
		default:
			return 0, true, false
		}
	}
	return 0, false, true
}

// tokens are the token counts of one span, after the include-cache and include-reasoning rules.
type tokens struct {
	input      int64 // uncached input
	cacheRead  int64
	cacheWrite int64
	output     int64 // reasoning included
	reasoning  int64 // the reasoning part of output
}

// readTokens reads a span's token counts. ok is false when the span has none (not an inference
// span); valid is false when a count is malformed or the counts contradict the configuration.
func readTokens(attrs pcommon.Map, includeCache, includeReasoning string) (t tokens, ok, valid bool) {
	in, inOK, inValid := count(attrs, inputKeys)
	out, outOK, outValid := count(attrs, outputKeys)
	read, _, readValid := count(attrs, cacheReadKeys)
	write, _, writeValid := count(attrs, cacheWriteKeys)
	reasoning, _, reasoningValid := count(attrs, reasoningKeys)
	if !inOK && !outOK {
		return tokens{}, false, true
	}
	if !inValid || !outValid || !readValid || !writeValid || !reasoningValid {
		return tokens{}, true, false
	}
	t = tokens{input: in, cacheRead: read, cacheWrite: write, output: out}
	cached := read + write
	switch includeCache {
	case modeTrue:
		if cached > in {
			return tokens{}, true, false
		}
		t.input = in - cached
	case modeAuto:
		if cached <= in {
			t.input = in - cached
		}
	}
	switch includeReasoning {
	case modeTrue:
		if reasoning > out {
			return tokens{}, true, false
		}
	case modeFalse:
		t.output = out + reasoning
	case modeAuto:
		if reasoning > out {
			t.output = out + reasoning
		}
	}
	t.reasoning = min(reasoning, t.output)
	return t, true, true
}

// normalizeTier maps a service tier attribute value to an llm-prices tier name ("" for the
// standard tier).
func normalizeTier(v string) string {
	switch v = strings.ToLower(strings.TrimSpace(v)); v {
	case "", "auto", "default", "standard", "on_demand", "on-demand":
		return ""
	}
	return v
}
