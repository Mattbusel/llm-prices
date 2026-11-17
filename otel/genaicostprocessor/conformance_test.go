package genaicostprocessor

import (
	"bytes"
	"context"
	"encoding/json"
	"math"
	"os"
	"path/filepath"
	"strings"
	"testing"
	"time"

	llmprices "github.com/Mattbusel/llm-prices/go"
	"github.com/stretchr/testify/require"
	"go.opentelemetry.io/collector/component/componenttest"
	"go.opentelemetry.io/collector/confmap"
	"go.opentelemetry.io/collector/pdata/pcommon"
	"go.opentelemetry.io/collector/pdata/ptrace"
)

// The conformance cases shared with the Python package (otel/conformance/cases.json).
const casesPath = "../conformance/cases.json"

type conformanceCase struct {
	Name           string                     `json:"name"`
	Only           string                     `json:"only"`
	Config         map[string]any             `json:"config"`
	Attributes     map[string]json.RawMessage `json:"attributes"`
	EndAgeSeconds  float64                    `json:"end_age_seconds"`
	Expect         map[string]json.RawMessage `json:"expect"`
	ExpectUnpriced string                     `json:"expect_unpriced"`
	ExpectAbsent   []string                   `json:"expect_absent"`
	ExpectSame     bool                       `json:"expect_unchanged"`
}

func loadCases(t *testing.T) []conformanceCase {
	t.Helper()
	b, err := os.ReadFile(casesPath)
	if err != nil {
		t.Skipf("%s is only in the repository: %v", casesPath, err)
	}
	var doc struct {
		Cases []conformanceCase `json:"cases"`
	}
	require.NoError(t, json.Unmarshal(b, &doc))
	require.NotEmpty(t, doc.Cases)
	return doc.Cases
}

// putJSON sets one attribute from its JSON form: integers are ints, numbers with a decimal
// point or exponent are doubles.
func putJSON(t *testing.T, m pcommon.Map, k string, raw json.RawMessage) {
	t.Helper()
	dec := json.NewDecoder(bytes.NewReader(raw))
	dec.UseNumber()
	var v any
	require.NoError(t, dec.Decode(&v))
	switch x := v.(type) {
	case string:
		m.PutStr(k, x)
	case bool:
		m.PutBool(k, x)
	case json.Number:
		if strings.ContainsAny(x.String(), ".eE") {
			f, err := x.Float64()
			require.NoError(t, err)
			m.PutDouble(k, f)
		} else {
			i, err := x.Int64()
			require.NoError(t, err)
			m.PutInt(k, i)
		}
	default:
		t.Fatalf("%s: unsupported attribute value %s", k, raw)
	}
}

func configFor(t *testing.T, overrides map[string]any) *Config {
	t.Helper()
	cfg := createDefaultConfig()
	if overrides != nil {
		require.NoError(t, confmap.NewFromStringMap(overrides).Unmarshal(cfg))
	}
	require.NoError(t, cfg.Validate())
	return cfg
}

func newTestProcessor(t *testing.T, cfg *Config) *costProcessor {
	t.Helper()
	p, err := newCostProcessor(cfg, componenttest.NewNopTelemetrySettings())
	require.NoError(t, err)
	t.Cleanup(func() { _ = p.shutdown(context.Background()) })
	return p
}

// llmPricesName maps a gen_ai.usage.cost.* name in a case to the name the processor writes
// with attributes: llm_prices (the default).
func llmPricesName(k string) string {
	if rest, ok := strings.CutPrefix(k, "gen_ai.usage.cost."); ok {
		return "llm_prices.cost." + rest
	}
	return k
}

// The cases are written with the semantic-convention names the Python processor writes. The
// Collector runs each case twice: with attributes: semconv, compared as written, and with the
// default attributes: llm_prices, where the same values must appear under llm_prices.cost.*
// and no gen_ai.usage.cost.* attribute may be added.
func TestConformanceCases(t *testing.T) {
	now := time.Date(2026, 11, 17, 12, 0, 0, 0, time.UTC)
	for _, c := range loadCases(t) {
		for _, mode := range []string{AttributesSemconv, AttributesLLMPrices} {
			t.Run(c.Name+"/"+mode, func(t *testing.T) {
				runCase(t, c, mode, now)
			})
		}
	}
}

func runCase(t *testing.T, c conformanceCase, mode string, now time.Time) {
	t.Helper()
	overrides := map[string]any{"attributes": mode}
	for k, v := range c.Config {
		overrides[k] = v
	}
	name := func(k string) string { return k }
	if mode == AttributesLLMPrices {
		name = llmPricesName
	}
	p := newTestProcessor(t, configFor(t, overrides))
	p.now = func() time.Time { return now }

	td := ptrace.NewTraces()
	span := td.ResourceSpans().AppendEmpty().ScopeSpans().AppendEmpty().Spans().AppendEmpty()
	span.SetName("chat")
	end := now.Add(-time.Duration(c.EndAgeSeconds * float64(time.Second)))
	span.SetStartTimestamp(pcommon.NewTimestampFromTime(end.Add(-time.Second)))
	span.SetEndTimestamp(pcommon.NewTimestampFromTime(end))
	for k, raw := range c.Attributes {
		putJSON(t, span.Attributes(), k, raw)
	}
	before := map[string]any{}
	for k, v := range span.Attributes().All() {
		before[k] = v.AsRaw()
	}

	_, err := p.processTraces(context.Background(), td)
	require.NoError(t, err)
	after := span.Attributes()

	if c.ExpectSame {
		require.Equal(t, before, after.AsRaw())
		return
	}
	if c.ExpectUnpriced != "" {
		v, ok := after.Get(AttrUnpriced)
		require.True(t, ok, "no %s", AttrUnpriced)
		require.Equal(t, c.ExpectUnpriced, v.Str())
		for _, k := range []string{AttrCostAmount, AttrLLMPricesCostAmount} {
			_, has := after.Get(k)
			require.False(t, has, "an unpriced span got a cost")
		}
	}
	if mode == AttributesLLMPrices {
		for k := range after.All() {
			if _, had := before[k]; !had {
				require.False(t, strings.HasPrefix(k, "gen_ai."), "llm_prices mode added %s", k)
			}
		}
	}
	for k, raw := range c.Expect {
		k := name(k)
		got, ok := after.Get(k)
		require.True(t, ok, "missing %s; attributes: %v", k, after.AsRaw())
		var want any
		require.NoError(t, json.Unmarshal(raw, &want))
		switch w := want.(type) {
		case float64:
			require.Equal(t, pcommon.ValueTypeDouble, got.Type(), k)
			require.InEpsilon(t, w, got.Double(), 1e-12, k)
		case string:
			if w == "PRICES_VERSION" {
				w = llmprices.PricesVersion
			}
			require.Equal(t, w, got.Str(), k)
		case bool:
			require.Equal(t, pcommon.ValueTypeBool, got.Type(), k)
			require.Equal(t, w, got.Bool(), k)
		default:
			t.Fatalf("%s: unsupported expectation %s", k, raw)
		}
	}
	for _, k := range c.ExpectAbsent {
		_, ok := after.Get(name(k))
		require.False(t, ok, "unexpected %s", k)
	}
}

func TestConformanceCasesFileIsShared(t *testing.T) {
	// The Python package reads the same file; keep both suites pointed at it.
	_, err := os.Stat(filepath.Join("..", "python", "tests", "test_conformance.py"))
	if os.IsNotExist(err) {
		t.Skip("python package not present")
	}
	require.NoError(t, err)
}

func TestAmountsAreFinite(t *testing.T) {
	for _, c := range loadCases(t) {
		for k, raw := range c.Expect {
			var f float64
			if json.Unmarshal(raw, &f) == nil {
				require.False(t, math.IsNaN(f) || math.IsInf(f, 0) || f < 0, "%s %s", c.Name, k)
			}
		}
	}
}
