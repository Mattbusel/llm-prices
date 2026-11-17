package llmprices

import (
	"encoding/json"
	"math"
	"os"
	"regexp"
	"strings"
	"testing"
)

// Ports of the Python package's tests (python/tests), which port the Rust crate's: every id
// form those resolve must resolve to the same row here, and costs must match.

func mustLookup(t *testing.T, id string) Price {
	t.Helper()
	p, ok := Lookup(id)
	if !ok {
		t.Fatalf("Lookup(%q) found nothing", id)
	}
	return p
}

func f(v *float64) float64 {
	if v == nil {
		return math.NaN()
	}
	return *v
}

func near(a, b float64) bool { return math.Abs(a-b) <= 1e-12*math.Max(1, math.Abs(b)) }

func TestEmbeddedTableMatchesPricesJSON(t *testing.T) {
	root, err := os.ReadFile("../prices.json")
	if err != nil {
		t.Skip("not in the llm-prices repository:", err)
	}
	if string(root) != string(pricesJSON) {
		t.Fatal("go/prices.json differs from prices.json (run node scripts/build.mjs)")
	}
}

func TestEveryRowHasASourceAndDate(t *testing.T) {
	var doc struct{ Models []json.RawMessage }
	if err := json.Unmarshal(pricesJSON, &doc); err != nil {
		t.Fatal(err)
	}
	all := All()
	if len(all) != len(doc.Models) || len(all) < 50 {
		t.Fatalf("%d rows loaded, prices.json has %d", len(all), len(doc.Models))
	}
	date := regexp.MustCompile(`^\d{4}-\d{2}-\d{2}$`)
	for _, p := range all {
		if !strings.HasPrefix(p.SourceURL, "https://") || !date.MatchString(p.Checked) {
			t.Errorf("%s/%s: source %q checked %q", p.Provider, p.Model, p.SourceURL, p.Checked)
		}
	}
	if !date.MatchString(Generated) {
		t.Errorf("Generated = %q", Generated)
	}
}

func TestLookups(t *testing.T) {
	if got := f(mustLookup(t, "claude-opus-5-5").Input); got != 4.0 {
		t.Errorf("claude-opus-5-5 input = %v", got)
	}
	for id, want := range map[string]string{
		"us.anthropic.claude-sonnet-4-20250514-v1:0": "claude-sonnet-4-20250514",
		"claude-opus-4-1@20250805":                   "claude-opus-4-1-20250805",
		"claude-haiku-4-5-20251001":                  "claude-haiku-4-5",
		"deepseek-v4-flash":                          "deepseek-flash",
		"gpt-4o-2024-08-06":                          "gpt-4o",
		"gpt-4o-2024-05-13":                          "gpt-4o-2024-05-13",
		"gpt-4o-2024-11-20":                          "gpt-4o",
		"openai/gpt-4o-2024-11-20":                   "gpt-4o",
		"azure/gpt-4.1-2025-04-14":                   "gpt-4.1",
		"o3-2025-04-16":                              "o3",
		"gpt-4o-mini-2024-07-18":                     "gpt-4o-mini",
		"gpt-5-2025-08-07":                           "gpt-5",
		"mistral-large-latest":                       "mistral-large-4",
		"mistral-small-latest":                       "mistral-small-2603",
		"codestral-latest":                           "codestral-2508",
		"grok-4.7-latest":                            "grok-4.7",
		"xai/grok-4.3-latest":                        "grok-4.3",
		"claude-3-5-haiku-latest":                    "claude-3-5-haiku-20241022",
		"claude-opus-5-5-latest":                     "claude-opus-5-5",
		"  GPT-4o  ":                                 "gpt-4o",
	} {
		if got := mustLookup(t, id).Model; got != want {
			t.Errorf("Lookup(%q).Model = %q, want %q", id, got, want)
		}
	}
	if got := f(mustLookup(t, "models/gemini-2.5-pro").Output); got != 10.0 {
		t.Errorf("gemini-2.5-pro output = %v", got)
	}
	if f(mustLookup(t, "gpt-4o-2024-05-13").Input) != 5.0 || f(mustLookup(t, "gpt-4o").Input) != 2.5 {
		t.Error("dated gpt-4o row is not its own answer")
	}
}

func TestUnknownIDsDoNotResolve(t *testing.T) {
	for _, id := range []string{"", "   ", "not-a-model", "anthropic/", "claude", "gpt", "models/", "groq/", "-latest", "latest", "2024-08-06"} {
		if p, ok := Lookup(id); ok {
			t.Errorf("Lookup(%q) = %s/%s", id, p.Provider, p.Model)
		}
	}
}

var undatedRe = regexp.MustCompile(`^(.*)-(20\d{6})$`)

func undated(id string) string {
	if m := undatedRe.FindStringSubmatch(id); m != nil {
		return m[1]
	}
	return id
}

func same(a, b Price) bool { return a.Provider == b.Provider && a.Model == b.Model }

func TestAnthropicIDForms(t *testing.T) {
	for _, p := range Anthropic.Rows() {
		for _, id := range ids(&p) {
			base := undated(id)
			date := "20990101"
			if base != id {
				date = id[len(base)+1:]
			}
			forms := []string{
				base, base + "-20990101", "anthropic/" + base + "-20990101", base + "-latest",
				base + "@" + date, "vertex_ai/" + base + "@" + date,
			}
			for _, region := range []string{"", "us.", "eu.", "apac.", "global."} {
				forms = append(forms, region+"anthropic."+id+"-v1:0", region+"anthropic."+id+"-v2:0")
			}
			for _, form := range forms {
				if got, ok := Lookup(form); !ok || !same(got, p) {
					t.Errorf("Lookup(%q) = %s, want %s", form, got.Model, p.Model)
				}
			}
			if got, ok := Bedrock.Lookup("us.anthropic." + id + "-v1:0"); !ok || !same(got, p) {
				t.Errorf("Bedrock.Lookup(%q) = %s", id, got.Model)
			}
			if got, ok := LookupProvider("vertex_ai", base+"@"+date); !ok || !same(got, p) {
				t.Errorf("vertex_ai %q = %s", base+"@"+date, got.Model)
			}
		}
	}
}

func TestEveryIDResolvesUnderItsProvider(t *testing.T) {
	for _, p := range All() {
		for _, id := range ids(&p) {
			for _, form := range []string{id, strings.ToUpper(id), id + "-2099-01-01"} {
				got, ok := LookupProvider(p.Provider, form)
				if !ok || !same(got, p) {
					t.Errorf("LookupProvider(%q, %q) = %s/%s, want %s", p.Provider, form, got.Provider, got.Model, p.Model)
				}
			}
			if got, ok := Lookup(p.Provider + "/" + id); !ok || !same(got, p) {
				t.Errorf("Lookup(%q) = %s/%s", p.Provider+"/"+id, got.Provider, got.Model)
			}
		}
	}
}

func TestProviderLookupKeepsHostsApart(t *testing.T) {
	groq, _ := LookupProvider("groq", "openai/gpt-oss-120b")
	together, _ := LookupProvider("together_ai", "openai/gpt-oss-120b")
	fireworks, _ := LookupProvider("fireworks", "accounts/fireworks/models/gpt-oss-120b")
	if groq.Provider != "groq" || together.Provider != "together" || fireworks.Provider != "fireworks" {
		t.Fatalf("got %q %q %q", groq.Provider, together.Provider, fireworks.Provider)
	}
	if f(groq.CacheRead) != 0.075 || together.CacheRead != nil || f(fireworks.CacheRead) != 0.015 {
		t.Error("cache read prices of the gpt-oss-120b hosts")
	}
	for _, c := range [][2]string{
		{"openai", "openai/gpt-oss-120b"}, {"openai", "gpt-oss-120b"}, {"anthropic", "gpt-4o"},
		{"groq", "claude-sonnet-4-6"}, {"together", "gpt-4o"}, {"bedrock", "gpt-4o"},
		{"azure", "claude-sonnet-4-6"}, {"vertex", "mistral-large-4"}, {"bedrock", "deepseek-flash"},
		{"acme", "gpt-4o"}, {"groq", "llama-3.3-70b-versatile"}, {"bedrock", "mistral.mistral-large-2407-v1:0"},
		{"deepseek", "gpt-4o"}, {"ollama", "llama3"}, {"huggingface", "deepseek-ai/DeepSeek-V4-Pro"},
	} {
		if p, ok := LookupProvider(c[0], c[1]); ok {
			t.Errorf("LookupProvider(%q, %q) = %s/%s, want nothing", c[0], c[1], p.Provider, p.Model)
		}
	}
	for _, c := range [][3]string{
		{"fireworks_ai", "fireworks_ai/accounts/fireworks/models/gpt-oss-120b", "fireworks"},
		{"bedrock", "claude-sonnet-4-6", "anthropic"},
		{"vertex", "gemini-2.5-pro", "google"},
		{"azure", "gpt-4o", "openai"},
		{"openrouter", "mistralai/mistral-large-4", "mistral"},
		{"openrouter", "x-ai/grok-4.7", "xai"},
		{"together", "moonshotai/Kimi-K3", "together"},
		{"fireworks", "accounts/fireworks/models/kimi-k3", "fireworks"},
		{"bedrock", "global.anthropic.claude-opus-4-5-20251101-v1:0", "anthropic"},
	} {
		if p, ok := LookupProvider(c[0], c[1]); !ok || p.Provider != c[2] {
			t.Errorf("LookupProvider(%q, %q).Provider = %q, want %q", c[0], c[1], p.Provider, c[2])
		}
	}
}

func TestCostCallbackCases(t *testing.T) {
	for _, c := range []struct {
		provider, model, wantProvider, wantModel string
		in, out                                  float64
	}{
		{"openai", "gpt-4o", "openai", "gpt-4o", 2.5, 10},
		{"openai", "gpt-4o-2024-05-13", "openai", "gpt-4o-2024-05-13", 5, 15},
		{"anthropic", "claude-sonnet-4-5", "anthropic", "claude-sonnet-4-5-20250929", 3, 15},
		{"google_genai", "gemini-2.5-flash", "google", "gemini-2.5-flash", 0.3, 2.5},
		{"google_vertexai", "gemini-2.5-pro", "google", "gemini-2.5-pro", 1.25, 10},
		{"deepseek", "deepseek-chat", "deepseek", "deepseek-flash", 0.3, 1.2},
		{"deepseek", "deepseek-reasoner", "deepseek", "deepseek-flash", 0.3, 1.2},
		{"mistralai", "mistral-large-latest", "mistral", "mistral-large-4", 0.7, 2.1},
		{"xai", "grok-4.7", "xai", "grok-4.7", 2, 6},
		{"groq", "openai/gpt-oss-120b", "groq", "openai/gpt-oss-120b", 0.15, 0.6},
		{"together", "openai/gpt-oss-120b", "together", "openai/gpt-oss-120b", 0.15, 0.6},
		{"fireworks", "accounts/fireworks/models/gpt-oss-120b", "fireworks", "accounts/fireworks/models/gpt-oss-120b", 0.15, 0.6},
		{"cohere", "command-r-plus-08-2024", "cohere", "command-r-plus-08-2024", 2.5, 10},
		{"bedrock", "anthropic.claude-haiku-4-5-20251001-v1:0", "anthropic", "claude-haiku-4-5", 1, 5},
		{"bedrock_converse", "us.anthropic.claude-sonnet-4-20250514-v1:0", "anthropic", "claude-sonnet-4-20250514", 3, 15},
		{"azure_openai", "gpt-4o-mini", "openai", "gpt-4o-mini", 0.15, 0.6},
	} {
		p, ok := LookupProvider(c.provider, c.model)
		if !ok || p.Provider != c.wantProvider || p.Model != c.wantModel || f(p.Input) != c.in || f(p.Output) != c.out {
			t.Errorf("LookupProvider(%q, %q) = %s/%s %v/%v", c.provider, c.model, p.Provider, p.Model, f(p.Input), f(p.Output))
			continue
		}
		if in, _ := p.InputPerToken(); !near(in, c.in/1e6) {
			t.Errorf("%s InputPerToken = %v", c.model, in)
		}
		if out, _ := p.OutputPerToken(); !near(out, c.out/1e6) {
			t.Errorf("%s OutputPerToken = %v", c.model, out)
		}
	}
	emb, _ := LookupProvider("openai", "text-embedding-3-small")
	if _, ok := emb.OutputPerToken(); ok {
		t.Error("embedding model has an output price")
	}
	if _, ok := Cost("text-embedding-3-small", 10, 0); ok {
		t.Error("Cost on an embedding model")
	}
}

func TestProviderNames(t *testing.T) {
	for name, want := range map[string]Provider{
		"fireworks_ai": Fireworks, "x-ai": XAI, "open_router": OpenRouter, "google_genai": Google,
		"google_vertexai": Vertex, "bedrock_converse": Bedrock, "google-generativeai": Google,
		"azure_ai_foundry": Azure, "GroqCloud": Groq, "cohere_ai": Cohere, " Together AI ": Together,
		"vertex_ai": Vertex, "gemini": Google, "claude": Anthropic, "grok": XAI,
	} {
		if got, ok := ParseProvider(name); !ok || got != want {
			t.Errorf("ParseProvider(%q) = %q, want %q", name, got, want)
		}
	}
	for _, bad := range []string{"", "deepseek-ai", "meta-llama", "qwen", "ollama", "huggingface", "a-very-long-provider-name-that-is-not-real"} {
		if got, ok := ParseProvider(bad); ok {
			t.Errorf("ParseProvider(%q) = %q", bad, got)
		}
	}
	n := 0
	for _, p := range Providers {
		if got, ok := ParseProvider(p.String()); !ok || got != p {
			t.Errorf("ParseProvider(%q) round trip", p)
		}
		n += len(p.Rows())
	}
	if n != len(All()) {
		t.Errorf("provider rows cover %d of %d rows", n, len(All()))
	}
}

func TestDelistedRowsStillPrice(t *testing.T) {
	for _, m := range []string{"gpt-3.5-turbo", "o1", "claude-3-5-haiku-20241022"} {
		p := mustLookup(t, m)
		if p.Status != "retired" || !strings.Contains(p.Note, "no longer on the pricing page") {
			t.Errorf("%s: status %q note %q", m, p.Status, p.Note)
		}
	}
}

func TestAnthropicCacheExample(t *testing.T) {
	c, ok := mustLookup(t, "claude-sonnet-4-6").CostUsage(Usage{Input: 2_048, CacheRead: 45_000, CacheWrite5m: 3_500, Output: 1_024})
	if !ok || c.LongContext || !near(c.Input, 0.006_144) || !near(c.CacheRead, 0.013_5) ||
		!near(c.CacheWrite, 0.013_125) || !near(c.Output, 0.015_36) || !near(c.Total, 0.048_129) {
		t.Errorf("got %+v", c)
	}
}

func TestOpenAILongContextExample(t *testing.T) {
	p := mustLookup(t, "gpt-5.4")
	c, ok := p.CostUsage(Usage{Input: 100_000, CacheRead: 200_000, Output: 4_000})
	if !ok || !c.LongContext || !near(c.Input, 0.5) || !near(c.CacheRead, 0.1) || !near(c.Output, 0.09) || !near(c.Total, 0.69) {
		t.Errorf("got %+v", c)
	}
	if usd, ok := p.Cost(100_000, 4_000); !ok || !near(usd, 0.31) {
		t.Errorf("Cost = %v", usd)
	}
}

func TestSimpleAndLongContextCost(t *testing.T) {
	if usd, ok := Cost("claude-haiku-5-5", 10_000, 2_000); !ok || !near(usd, 0.002) {
		t.Errorf("Cost = %v %v", usd, ok)
	}
	h := mustLookup(t, "claude-haiku-5-5")
	if usd, _ := h.Cost(100_000, 0); !near(usd, 0.01) {
		t.Errorf("at threshold %v", usd)
	}
	if usd, _ := h.Cost(100_001, 0); !near(usd, 100_001*0.5/1e6) {
		t.Errorf("over threshold %v", usd)
	}
	if h.CacheWrite["1h"] != 0.2 {
		t.Errorf("1h cache write %v", h.CacheWrite["1h"])
	}
	at := Usage{Input: 60_000, CacheRead: 40_000}
	if c, _ := h.CostUsage(at); c.LongContext || math.Abs(c.Total-(60_000*0.1+40_000*0.01)/1e6) > 1e-15 {
		t.Errorf("at %+v", c)
	}
	at.CacheWrite1h = 1
	if c, _ := h.CostUsage(at); !c.LongContext || math.Abs(c.Input-60_000*0.5/1e6) > 1e-15 || math.Abs(c.CacheWrite-1.0/1e6) > 1e-15 {
		t.Errorf("over %+v", c)
	}
	g := mustLookup(t, "grok-4.7")
	if g.LongContext == nil || g.LongContext.ThresholdTokens != 199_999 {
		t.Fatal("grok-4.7 long context threshold")
	}
	if c, _ := g.CostUsage(Usage{Input: 200_000}); !c.LongContext {
		t.Error("grok-4.7 at 200k is long context")
	}
}

// The Rust crate's 1.0 cost, which Cost must equal bit for bit.
func cost10(p Price, in, out int64) (float64, bool) {
	i, o := p.Input, p.Output
	if l := p.LongContext; l != nil && in > l.ThresholdTokens {
		i, o = l.Input, l.Output
	}
	if i == nil || o == nil {
		return 0, false
	}
	return float64(float64(float64(in)**i)+float64(float64(out)**o)) / 1_000_000.0, true
}

func TestCostAgreesWithCostUsageAnd10(t *testing.T) {
	counts := []int64{0, 1, 999, 100_000, 100_001, 200_000, 200_001, 272_000, 272_001, 1_000_000, 12_345_678}
	for _, p := range All() {
		for _, i := range counts {
			for _, o := range counts {
				got, ok := p.Cost(i, o)
				want, wok := cost10(p, i, o)
				c, cok := p.CostUsage(Usage{Input: i, Output: o})
				if ok != wok || got != want || cok != ok || c.Total != got {
					t.Fatalf("%s %d %d: %v %v, 1.0 %v %v", p.Model, i, o, got, ok, want, wok)
				}
			}
		}
	}
}

func TestCacheBatchAndMissingPrices(t *testing.T) {
	pro := mustLookup(t, "gpt-5.5-pro")
	if c, _ := pro.CostUsage(Usage{CacheRead: 100_000}); c.CacheRead != 3.0 || c.Input != 0 {
		t.Errorf("cache read falls back to input: %+v", c)
	}
	if c, _ := pro.CostUsage(Usage{CacheRead: 1_000_000}); !c.LongContext || c.CacheRead != 60.0 {
		t.Errorf("long context cache read: %+v", c)
	}
	if _, ok := pro.CostUsage(Usage{Input: 272_001, Batch: true}); ok {
		t.Error("gpt-5.5-pro has no long-context batch price")
	}
	if _, ok := pro.CostUsage(Usage{Input: 272_000, Batch: true}); !ok {
		t.Error("gpt-5.5-pro batch at the threshold")
	}
	s := mustLookup(t, "claude-sonnet-4-6")
	u := Usage{CacheWrite5m: 1_000_000, CacheWrite1h: 1_000_000}
	if c, _ := s.CostUsage(u); c.CacheWrite != 3.75+6.0 {
		t.Errorf("cache writes %v", c.CacheWrite)
	}
	u.Batch = true
	if c, _ := s.CostUsage(u); c.CacheWrite != (3.75+6.0)/2.0 {
		t.Errorf("batch cache writes %v", c.CacheWrite)
	}
	haiku := mustLookup(t, "claude-haiku-4-5")
	b := Usage{Input: 10_000, CacheRead: 90_000, Output: 1_000, Batch: true}
	if c, _ := haiku.CostUsage(b); !near(c.Input, 0.005) || !near(c.CacheRead, 0.0045) || !near(c.Output, 0.0025) {
		t.Errorf("batch %+v", c)
	}
	if _, ok := mustLookup(t, "gpt-3.5-turbo").CostUsage(b); ok {
		t.Error("gpt-3.5-turbo has no batch price")
	}
	if _, ok := mustLookup(t, "gpt-6-sol").CostUsage(Usage{Input: 1, CacheWrite5m: 1}); ok {
		t.Error("gpt-6-sol has no 5m cache write price")
	}
	if _, ok := mustLookup(t, "text-embedding-3-small").CostUsage(Usage{}); ok {
		t.Error("embedding model priced")
	}
	if _, ok := s.CostUsage(Usage{Input: -1}); ok {
		t.Error("negative token count priced")
	}
}

func TestRowsAreCopies(t *testing.T) {
	p := mustLookup(t, "claude-sonnet-4-6")
	*p.Input = 999
	p.CacheWrite["5m"] = 999
	p.Aliases = append(p.Aliases, "x")
	q := mustLookup(t, "claude-sonnet-4-6")
	if f(q.Input) == 999 || q.CacheWrite["5m"] == 999 {
		t.Error("changing a returned row changed the table")
	}
}
