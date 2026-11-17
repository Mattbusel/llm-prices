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

func TestConformanceCases(t *testing.T) {
	now := time.Date(2026, 11, 17, 12, 0, 0, 0, time.UTC)
	for _, c := range loadCases(t) {
		t.Run(c.Name, func(t *testing.T) {
			p := newTestProcessor(t, configFor(t, c.Config))
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
				_, has := after.Get(AttrCostAmount)
				require.False(t, has, "an unpriced span got a cost")
			}
			for k, raw := range c.Expect {
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
				default:
					t.Fatalf("%s: unsupported expectation %s", k, raw)
				}
			}
			for _, k := range c.ExpectAbsent {
				_, ok := after.Get(k)
				require.False(t, ok, "unexpected %s", k)
			}
		})
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
