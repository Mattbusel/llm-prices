package genaicostprocessor

import (
	"encoding/json"
	"math"
	"os"
	"strings"
	"testing"

	"github.com/stretchr/testify/require"
)

// litellm_reference.json holds the costs LiteLLM itself (generic_cost_per_token) computed for a
// sample of its price map; see otel/conformance/litellm/make_reference.py.
const liteLLMRef = "../conformance/litellm/litellm_reference.json"

type liteLLMCase struct {
	Key             string  `json:"key"`
	Provider        string  `json:"provider"`
	LiteLLMProvider string  `json:"litellm_provider"`
	Tier            *string `json:"tier"`
	Input           int64   `json:"input"`
	CacheRead       int64   `json:"cache_read"`
	CacheWrite      int64   `json:"cache_write"`
	Output          int64   `json:"output"`
	Reasoning       int64   `json:"reasoning"`
	Total           float64 `json:"total"`
}

func loadLiteLLMRef(t *testing.T) (json.RawMessage, []liteLLMCase) {
	t.Helper()
	b, err := os.ReadFile(liteLLMRef)
	if err != nil {
		t.Skipf("the LiteLLM reference is only in the repository: %v", err)
	}
	var doc struct {
		Entries json.RawMessage `json:"entries"`
		Cases   []liteLLMCase   `json:"cases"`
	}
	require.NoError(t, json.Unmarshal(b, &doc))
	require.GreaterOrEqual(t, len(doc.Cases), 500)
	return doc.Entries, doc.Cases
}

func refTokens(c liteLLMCase) tokens {
	return tokens{input: c.Input, cacheRead: c.CacheRead, cacheWrite: c.CacheWrite, output: c.Output, reasoning: c.Reasoning}
}

func close9(a, b float64) bool {
	return math.Abs(a-b) <= 1e-15 || math.Abs(a-b) <= 1e-9*math.Max(math.Abs(a), math.Abs(b))
}

func TestLiteLLMMatchesLiteLLM(t *testing.T) {
	entries, cases := loadLiteLLMRef(t)
	src, err := parseLiteLLM(entries)
	require.NoError(t, err)
	s := src.(liteLLMSource)
	var bad []string
	for _, c := range cases {
		tier := ""
		if c.Tier != nil {
			tier = *c.Tier
		}
		got, reason := costLiteLLM(s.entries[c.Key], c.Provider, refTokens(c), false, tier)
		if reason != "" || !close9(got.total, c.Total) {
			bad = append(bad, c.Key)
		}
	}
	require.Empty(t, bad, "%d of %d cases differ from LiteLLM", len(bad), len(cases))
}

func TestLiteLLMLookup(t *testing.T) {
	entries, cases := loadLiteLLMRef(t)
	src, err := parseLiteLLM(entries)
	require.NoError(t, err)
	found := 0
	for _, c := range cases {
		if c.Tier != nil || c.CacheRead != 0 || c.Input != 1000 {
			continue
		}
		model := c.Key
		if strings.HasPrefix(c.Key, c.LiteLLMProvider+"/") {
			model = strings.TrimPrefix(c.Key, c.LiteLLMProvider+"/")
		}
		got, reason := src.price(request{provider: c.Provider, models: []string{model}, tokens: refTokens(c)})
		require.Empty(t, reason, c.Key)
		// LiteLLM's lookup prefers "<provider>/<model>" when both keys exist, and so does the source.
		if got.row == "litellm/"+c.Key {
			require.True(t, close9(got.total, c.Total), c.Key)
			found++
		}
	}
	require.GreaterOrEqual(t, found, 100)
}

func TestLiteLLMSpecialRules(t *testing.T) {
	src, err := parseLiteLLM([]byte(`{
		"sample_spec": "not an entry",
		"gpt-x": {"litellm_provider": "openai", "mode": "chat", "input_cost_per_token": 1e-6, "output_cost_per_token": 2e-6,
			"input_cost_per_token_batches": 5e-7, "output_cost_per_token_batches": 1e-6},
		"tiered": {"litellm_provider": "openai", "input_cost_per_token": 1e-6, "output_cost_per_token": 1e-6, "tiered_pricing": []},
		"xai/grok-x": {"litellm_provider": "xai", "input_cost_per_token": 1e-6, "output_cost_per_token": 1e-6,
			"input_cost_per_token_above_128k_tokens": 2e-6, "output_cost_per_token_above_128k_tokens": 2e-6}
	}`))
	require.NoError(t, err)
	tk := tokens{input: 1000, output: 1000}

	c, reason := src.price(request{provider: "openai", models: []string{"gpt-x"}, tokens: tk, batch: true})
	require.Empty(t, reason)
	require.InEpsilon(t, 0.0015, c.total, 1e-12)

	_, reason = src.price(request{provider: "openai", models: []string{"tiered"}, tokens: tk})
	require.Equal(t, ReasonUnsupportedPrice, reason)

	_, reason = src.price(request{provider: "openai", models: []string{"gpt-x"}, tokens: tk, tier: "flex"})
	require.Equal(t, ReasonUnpricedTier, reason)

	_, reason = src.price(request{provider: "ollama", models: []string{"gpt-x"}, tokens: tk})
	require.Equal(t, ReasonUnknownProvider, reason)

	_, reason = src.price(request{provider: "anthropic", models: []string{"gpt-x"}, tokens: tk})
	require.Equal(t, ReasonUnknownModel, reason, "a bare id only matches its own provider")

	// xAI bills the higher tier once the prompt reaches the threshold (inclusive).
	c, reason = src.price(request{provider: "xai", models: []string{"grok-x"}, tokens: tokens{input: 128000, output: 0}})
	require.Empty(t, reason)
	require.InEpsilon(t, 0.256, c.total, 1e-12)

	_, err = parseLiteLLM([]byte(`[]`))
	require.Error(t, err)
	_, err = parseLiteLLM([]byte(`{"a": 1}`))
	require.Error(t, err)
}
