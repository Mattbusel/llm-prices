package genaicostprocessor

import (
	"context"
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	llmprices "github.com/Mattbusel/llm-prices/go"
	"github.com/stretchr/testify/require"
	"go.opentelemetry.io/collector/pdata/ptrace"
)

// The golden corpus: real spans exported by OpenLLMetry, OpenInference, Logfire and rig for calls
// to a mock with fixed usage (otel/conformance/golden/generate.py), shared with the Python tests.
const goldenDir = "../conformance/golden"

type goldenSuite struct {
	File  string `json:"file"`
	Spans []struct {
		Row    string  `json:"row"`
		Amount float64 `json:"amount"`
	} `json:"spans"`
}

func loadGolden(t *testing.T) map[string]goldenSuite {
	t.Helper()
	b, err := os.ReadFile(filepath.Join(goldenDir, "expected.json"))
	if err != nil {
		t.Skipf("the golden corpus is only in the repository: %v", err)
	}
	var doc struct {
		Suites map[string]goldenSuite `json:"suites"`
	}
	require.NoError(t, json.Unmarshal(b, &doc))
	require.NotEmpty(t, doc.Suites)
	return doc.Suites
}

func readOTLPJSON(t *testing.T, path string) ptrace.Traces {
	t.Helper()
	b, err := os.ReadFile(path)
	require.NoError(t, err)
	td, err := (&ptrace.JSONUnmarshaler{}).UnmarshalTraces(b)
	require.NoError(t, err, path)
	return td
}

func TestGoldenCorpus(t *testing.T) {
	for name, suite := range loadGolden(t) {
		t.Run(name, func(t *testing.T) {
			td := readOTLPJSON(t, filepath.Join(goldenDir, suite.File))
			cfg := createDefaultConfig()
			cfg.Breakdown = true
			cfg.MaxSpanAge = 0 // the corpus was recorded on another day
			p := newTestProcessor(t, cfg)
			_, err := p.processTraces(context.Background(), td)
			require.NoError(t, err)

			var i int
			rss := td.ResourceSpans()
			for r := 0; r < rss.Len(); r++ {
				sss := rss.At(r).ScopeSpans()
				for s := 0; s < sss.Len(); s++ {
					spans := sss.At(s).Spans()
					for k := 0; k < spans.Len(); k++ {
						require.Less(t, i, len(suite.Spans), "more spans than expected")
						want := suite.Spans[i]
						attrs := spans.At(k).Attributes()
						if v, ok := attrs.Get(AttrUnpriced); ok {
							t.Fatalf("span %d (%s) unpriced: %s", i, spans.At(k).Name(), v.Str())
						}
						amount, ok := attrs.Get(AttrCostAmount)
						require.True(t, ok, "span %d (%s) has no cost", i, spans.At(k).Name())
						require.InEpsilon(t, want.Amount, amount.Double(), 1e-12, "span %d", i)
						row, _ := attrs.Get(AttrPriceRow)
						require.Equal(t, want.Row, row.Str())
						version, _ := attrs.Get(AttrCostPricingVersion)
						require.Equal(t, llmprices.PricesVersion, version.Str())
						src, _ := attrs.Get(AttrCostSource)
						require.Equal(t, "local", src.Str())
						i++
					}
				}
			}
			require.Equal(t, len(suite.Spans), i)
		})
	}
}
