package llmprices

import (
	"bytes"
	"crypto/sha256"
	"encoding/hex"
	"math"
	"os"
	"strings"
	"testing"
)

func TestPricesVersionIsTheSHA256OfPricesJSON(t *testing.T) {
	b, err := os.ReadFile("../prices.json")
	if err != nil {
		t.Skip("prices.json is only in the repository")
	}
	sum := sha256.Sum256(bytes.ReplaceAll(b, []byte("\r\n"), []byte("\n")))
	if want := "sha256:" + hex.EncodeToString(sum[:]); PricesVersion != want {
		t.Fatalf("PricesVersion = %s, want %s", PricesVersion, want)
	}
	if Default().Version() != PricesVersion {
		t.Fatalf("Default().Version() = %s", Default().Version())
	}
	// CRLF checkouts hash the same.
	if Version(bytes.ReplaceAll(b, []byte("\n"), []byte("\r\n"))) != PricesVersion {
		t.Fatal("a CRLF copy has another version")
	}
}

func TestParseTableMatchesTheEmbeddedTable(t *testing.T) {
	tb, err := ParseTable(pricesJSON)
	if err != nil {
		t.Fatal(err)
	}
	if tb.Len() != len(All()) || tb.Generated() != Generated || tb.Version() != PricesVersion {
		t.Fatalf("Len %d Generated %s Version %s", tb.Len(), tb.Generated(), tb.Version())
	}
	for _, id := range []string{"claude-sonnet-4-6", "gpt-4o-2024-08-06", "us.anthropic.claude-sonnet-4-20250514-v1:0", "groq/openai/gpt-oss-120b"} {
		a, okA := Lookup(id)
		b, okB := tb.Lookup(id)
		if okA != okB || a.Provider != b.Provider || a.Model != b.Model {
			t.Errorf("%s: %v %s/%s vs %v %s/%s", id, okA, a.Provider, a.Model, okB, b.Provider, b.Model)
		}
	}
	if p, ok := tb.LookupProvider("vertex_ai", "claude-sonnet-4-6"); !ok || p.Provider != "anthropic" {
		t.Errorf("vertex fallback: %v %+v", ok, p.Provider)
	}
	if _, ok := tb.LookupProvider("ollama", "llama3"); ok {
		t.Error("unknown provider priced")
	}
	if n := len(tb.Rows(Groq)); n == 0 || n != len(Groq.Rows()) {
		t.Errorf("Rows(Groq) = %d", n)
	}
}

func TestParseTableRejectsBadDocuments(t *testing.T) {
	for name, doc := range map[string]string{
		"not json":       `{`,
		"no models":      `{"models": []}`,
		"no model id":    `{"models": [{"provider": "openai", "input": 1}]}`,
		"negative price": `{"models": [{"provider": "openai", "model": "m", "input": -1}]}`,
		"huge price":     `{"models": [{"provider": "openai", "model": "m", "input": 1e400}]}`,
	} {
		if _, err := ParseTable([]byte(doc)); err == nil {
			t.Errorf("%s: no error", name)
		}
	}
	tb, err := ParseTable([]byte(`{"generated": "2026-01-01", "models": [{"provider": "openai", "model": "m", "input": 1, "output": 2}]}`))
	if err != nil {
		t.Fatal(err)
	}
	if c, ok := tb.LookupProvider("openai", "m"); !ok || *c.Output != 2 {
		t.Fatalf("%v %+v", ok, c)
	}
	if _, ok := tb.Lookup("gpt-4o"); ok {
		t.Fatal("a parsed table found a row it does not hold")
	}
}

func TestTiers(t *testing.T) {
	// The Python package's doctests for Price.tier and cost_usage(tier=...).
	p, _ := Lookup("gemini-2.5-pro")
	pri, ok := p.Tier("priority")
	if !ok || *pri.Input != 2.25 || *pri.Output != 18 || *pri.LongContext.Input != 4.5 {
		t.Fatalf("priority: %v %+v", ok, pri)
	}
	if pri.BatchInput != nil || pri.LongContext.BatchInput != nil {
		t.Fatal("a tier has batch prices")
	}
	c, ok := pri.CostUsage(Usage{Input: 300_000, Output: 1_000})
	if !ok || math.Abs(c.Total-1.377) > 1e-12 {
		t.Fatalf("priority cost %v %v", ok, c.Total)
	}
	if _, ok := p.Tier("fast"); ok {
		t.Fatal("gemini has no fast tier")
	}
	if s, ok := p.Tier("standard"); !ok || s.Model != p.Model || *s.Input != *p.Input {
		t.Fatal("standard tier")
	}
	opus, _ := Lookup("claude-opus-5-5")
	if f, ok := opus.Tier("fast"); !ok || *f.Output != 40 || !strings.Contains(f.Note, "fast") {
		t.Fatalf("fast: %v %+v", ok, f)
	}
	// Tiers are copies.
	f, _ := opus.Tier("fast")
	*f.Output = 1
	if g, _ := opus.Tier("fast"); *g.Output != 40 {
		t.Fatal("Tier returned a shared pointer")
	}
}

func FuzzParseTable(f *testing.F) {
	f.Add([]byte(`{"models": [{"provider": "openai", "model": "m", "input": 1, "output": 2, "other_tiers": {"flex": {"input": 0.5}}}]}`))
	f.Add([]byte(`{"models": [{"provider": "x", "model": "y", "long_context": {"threshold_tokens": 5, "input": 1}}]}`))
	f.Add(pricesJSON[:2000])
	f.Fuzz(func(t *testing.T, b []byte) {
		tb, err := ParseTable(b)
		if err != nil {
			return
		}
		for _, p := range tb.All() {
			if _, ok := tb.LookupProvider(p.Provider, p.Model); !ok && canonical(p.Provider) {
				// A row's own id under its own provider always resolves, unless the id names
				// another host's path.
				if !strings.Contains(p.Model, "/") {
					t.Fatalf("%s/%s does not resolve", p.Provider, p.Model)
				}
			}
			p.CostUsage(Usage{Input: 1, Output: 1, CacheRead: 1, CacheWrite5m: 1, CacheWrite1h: 1})
		}
	})
}

// canonical reports whether name is a provider's canonical name, the form prices.json rows use.
func canonical(name string) bool {
	p, ok := ParseProvider(name)
	return ok && string(p) == name
}
