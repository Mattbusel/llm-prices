package genaicostprocessor // import "github.com/Mattbusel/llm-prices/otel/genaicostprocessor"

import (
	"encoding/json"
	"errors"
	"fmt"
	"math"
	"sort"
	"strconv"
	"strings"

	llmprices "github.com/Mattbusel/llm-prices/go"
)

// ReasonUnsupportedPrice is written when a LiteLLM entry prices the model in a way this source
// does not implement (tiered_pricing tables, off-peak windows), so a cost would be wrong.
const ReasonUnsupportedPrice = "unsupported_price"

// liteLLMProviders maps llm-prices provider names to LiteLLM's custom_llm_provider names. The
// first is the key prefix LiteLLM uses ("vertex_ai/..."); a bare key matches when its
// litellm_provider starts with any of them.
var liteLLMProviders = map[string][]string{
	"openai":     {"openai", "text-completion-openai"},
	"anthropic":  {"anthropic"},
	"google":     {"gemini"},
	"vertex":     {"vertex_ai"},
	"bedrock":    {"bedrock", "bedrock_converse"},
	"azure":      {"azure", "azure_ai"},
	"deepseek":   {"deepseek"},
	"mistral":    {"mistral"},
	"xai":        {"xai"},
	"groq":       {"groq"},
	"together":   {"together_ai"},
	"fireworks":  {"fireworks_ai"},
	"openrouter": {"openrouter"},
	"perplexity": {"perplexity"},
	"cohere":     {"cohere_chat", "cohere"},
}

// liteLLMSource prices from LiteLLM's model_prices_and_context_window.json.
type liteLLMSource struct {
	entries map[string]map[string]json.RawMessage
	lower   map[string]string // lowercased key -> key
	version string
}

func parseLiteLLM(b []byte) (priceSource, error) {
	var raw map[string]json.RawMessage
	if err := json.Unmarshal(b, &raw); err != nil {
		return nil, fmt.Errorf("litellm prices: %w", err)
	}
	s := liteLLMSource{
		entries: make(map[string]map[string]json.RawMessage, len(raw)),
		lower:   make(map[string]string, len(raw)),
		version: llmprices.Version(b),
	}
	for k, v := range raw {
		var e map[string]json.RawMessage
		if json.Unmarshal(v, &e) != nil {
			continue // sample_spec and other non-entries
		}
		s.entries[k] = e
		if _, dup := s.lower[strings.ToLower(k)]; !dup {
			s.lower[strings.ToLower(k)] = k
		}
	}
	if len(s.entries) == 0 {
		return nil, errors.New("litellm prices: no entries")
	}
	return s, nil
}

func (s liteLLMSource) pricingID() string      { return "litellm.model_cost_map" }
func (s liteLLMSource) pricingVersion() string { return s.version }

func (s liteLLMSource) get(key string) (map[string]json.RawMessage, bool) {
	if e, ok := s.entries[key]; ok {
		return e, true
	}
	if k, ok := s.lower[strings.ToLower(key)]; ok {
		return s.entries[k], true
	}
	return nil, false
}

// find returns the entry for a model as sold by provider, the way LiteLLM's get_model_info
// does: "<provider>/<model>" first, then the bare model id when its litellm_provider is that
// provider. Without a provider only the bare id is tried.
func (s liteLLMSource) find(provider, model string) (map[string]json.RawMessage, string, bool) {
	if provider == "" {
		e, ok := s.get(model)
		return e, model, ok
	}
	names := liteLLMProviders[provider]
	for _, n := range names {
		if e, ok := s.get(n + "/" + model); ok {
			return e, n + "/" + model, true
		}
	}
	for _, n := range names {
		if strings.HasPrefix(model, n+"/") {
			if e, ok := s.get(model); ok {
				return e, model, true
			}
			break
		}
	}
	bare := model
	for _, n := range names {
		bare = strings.TrimPrefix(bare, n+"/")
	}
	if e, ok := s.get(bare); ok {
		lp := rawString(e["litellm_provider"])
		for _, n := range names {
			if lp == n || strings.HasPrefix(lp, n+"-") || strings.HasPrefix(lp, n+"_") {
				return e, bare, true
			}
		}
	}
	return nil, "", false
}

func (s liteLLMSource) price(r request) (priced, string) {
	if r.provider != "" && liteLLMProviders[r.provider] == nil {
		return priced{}, ReasonUnknownProvider
	}
	for _, m := range r.models {
		e, key, ok := s.find(r.provider, m)
		if !ok {
			continue
		}
		c, reason := costLiteLLM(e, r.provider, r.tokens, r.batch, r.tier)
		c.row = "litellm/" + key
		return c, reason
	}
	return priced{}, ReasonUnknownModel
}

func rawString(m json.RawMessage) string {
	var s string
	_ = json.Unmarshal(m, &s)
	return s
}

// rate reads a per-token price. ok is false when the key is missing or null.
func rate(e map[string]json.RawMessage, key string) (float64, bool) {
	m, ok := e[key]
	if !ok {
		return 0, false
	}
	var v *float64
	if json.Unmarshal(m, &v) != nil || v == nil || *v < 0 || math.IsInf(*v, 0) || math.IsNaN(*v) {
		return 0, false
	}
	return *v, true
}

var liteLLMTierSuffix = map[string]string{"flex": "flex", "balanced": "balanced", "priority": "priority", "fast": "priority", "ultrafast": "ultrafast"}

// costLiteLLM ports the token arithmetic of LiteLLM's generic_cost_per_token: uncached input,
// cache reads (default: the input rate) and cache writes (default: the input rate), output with
// reasoning tokens at output_cost_per_reasoning_token when listed, "_above_<N>_tokens"
// thresholds (inclusive for xAI), service-tier keys ("_flex", "_priority") and Batch API rates
// ("_batches").
func costLiteLLM(e map[string]json.RawMessage, provider string, t tokens, batch bool, tier string) (priced, string) {
	if _, ok := e["tiered_pricing"]; ok {
		return priced{}, ReasonUnsupportedPrice
	}
	for k := range e {
		if strings.Contains(k, "off_peak") {
			return priced{}, ReasonUnsupportedPrice
		}
	}
	suffix := ""
	if tier != "" {
		sfx, ok := liteLLMTierSuffix[tier]
		if !ok {
			return priced{}, ReasonUnpricedTier
		}
		suffix = "_" + sfx
		if _, ok := rate(e, "input_cost_per_token"+suffix); !ok {
			return priced{}, ReasonUnpricedTier
		}
	}
	key := func(base string) string { return base + suffix }

	// LiteLLM's get_cost_per_unit: a missing service-tier key falls back to the standard key.
	tiered := func(base string) (float64, bool) {
		if v, ok := rate(e, key(base)); ok || suffix == "" {
			return v, ok
		}
		return rate(e, base)
	}
	in, ok := tiered("input_cost_per_token")
	if !ok {
		return priced{}, ReasonMissingRate
	}
	out, ok := tiered("output_cost_per_token")
	if !ok {
		return priced{}, ReasonMissingRate
	}
	creation, hasCreation := tiered("cache_creation_input_token_cost")
	read, hasRead := tiered("cache_read_input_token_cost")
	if !hasRead && provider == "fireworks" {
		// LiteLLM bills Fireworks cache reads at half the input rate when the entry lists none
		// (FIREWORKS_AI_DEFAULT_CACHE_READ_RATE_RATIO).
		read, hasRead = in*0.5, true
	}

	prompt := t.input + t.cacheRead + t.cacheWrite
	for _, th := range thresholds(e, suffix) {
		over := float64(prompt) > th.n || (provider == "xai" && float64(prompt) == th.n)
		if !over {
			continue
		}
		pick := func(base string, cur float64, has bool) (float64, bool) {
			if v, ok := rate(e, key(base+"_above_"+th.label+"_tokens")); ok {
				return v, true
			}
			if suffix != "" {
				if v, ok := rate(e, base+"_above_"+th.label+"_tokens"); ok {
					return v, true
				}
			}
			return cur, has
		}
		in, _ = pick("input_cost_per_token", in, true)
		out, _ = pick("output_cost_per_token", out, true)
		creation, hasCreation = pick("cache_creation_input_token_cost", creation, hasCreation)
		read, hasRead = pick("cache_read_input_token_cost", read, hasRead)
	}
	if !hasRead {
		read = in
	}
	if !hasCreation {
		creation = in
	}
	// LiteLLM's _resolve_reasoning_token_cost: the tier's reasoning price, else the tier's
	// output price, else the standard reasoning price, else the output price.
	reasoningRate := out
	if v, ok := rate(e, key("output_cost_per_reasoning_token")); ok && suffix != "" {
		reasoningRate = v
	} else if _, ok := rate(e, key("output_cost_per_token")); ok && suffix != "" {
		reasoningRate = out
	} else if v, ok := rate(e, "output_cost_per_reasoning_token"); ok {
		reasoningRate = v
	}
	if batch {
		bi, ok1 := rate(e, "input_cost_per_token_batches")
		bo, ok2 := rate(e, "output_cost_per_token_batches")
		if !ok1 || !ok2 {
			return priced{}, ReasonMissingRate
		}
		if t.cacheRead > 0 || t.cacheWrite > 0 {
			br, ok3 := rate(e, "cache_read_input_token_cost_batches")
			bw, ok4 := rate(e, "cache_creation_input_token_cost_batches")
			if (t.cacheRead > 0 && !ok3) || (t.cacheWrite > 0 && !ok4) {
				return priced{}, ReasonMissingRate
			}
			read, creation = br, bw
		}
		in, out, reasoningRate = bi, bo, bo
	}
	text := t.output - t.reasoning
	c := priced{
		input:      float64(t.input) * in,
		cacheRead:  float64(t.cacheRead) * read,
		cacheWrite: float64(t.cacheWrite) * creation,
		output:     float64(text)*out + float64(t.reasoning)*reasoningRate,
	}
	c.total = c.input + c.cacheRead + c.cacheWrite + c.output
	return c, ""
}

type threshold struct {
	label string
	n     float64
}

// thresholds lists the "_above_<N>_tokens" thresholds an entry prices, ascending.
func thresholds(e map[string]json.RawMessage, suffix string) []threshold {
	seen := map[string]bool{}
	var out []threshold
	for k := range e {
		var base string
		for _, p := range []string{"input_cost_per_token", "output_cost_per_token", "cache_read_input_token_cost", "cache_creation_input_token_cost"} {
			if strings.HasPrefix(k, p+"_above_") {
				base = p
				break
			}
		}
		if base == "" || !strings.Contains(k, "_tokens") {
			continue
		}
		rest := strings.TrimPrefix(k, base+"_above_")
		label, tail, _ := strings.Cut(rest, "_tokens")
		if tail != "" && tail != suffix {
			continue // batch or another service tier's threshold
		}
		if _, ok := rate(e, k); !ok || seen[label] {
			continue
		}
		n, err := parseThreshold(label)
		if err != nil {
			continue
		}
		seen[label] = true
		out = append(out, threshold{label, n})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].n < out[j].n })
	return out
}

func parseThreshold(s string) (float64, error) {
	mult := 1.0
	if strings.HasSuffix(s, "k") {
		s, mult = strings.TrimSuffix(s, "k"), 1000
	}
	n, err := strconv.ParseFloat(s, 64)
	return n * mult, err
}
