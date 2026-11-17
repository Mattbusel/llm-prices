package genaicostprocessor

import (
	"context"
	"crypto/ed25519"
	"crypto/rand"
	"encoding/base64"
	"net/http"
	"net/http/httptest"
	"os"
	"path/filepath"
	"strings"
	"sync/atomic"
	"testing"
	"time"

	llmprices "github.com/Mattbusel/llm-prices/go"
	"github.com/stretchr/testify/require"
	"go.opentelemetry.io/collector/component/componenttest"
	"go.opentelemetry.io/collector/consumer/consumertest"
	"go.opentelemetry.io/collector/pdata/pcommon"
	"go.opentelemetry.io/collector/pdata/ptrace"
	"go.opentelemetry.io/collector/processor/processortest"
	"go.opentelemetry.io/otel/sdk/metric/metricdata"
	"go.opentelemetry.io/otel/sdk/metric/metricdata/metricdatatest"

	"github.com/Mattbusel/llm-prices/otel/genaicostprocessor/internal/metadata"
	"github.com/Mattbusel/llm-prices/otel/genaicostprocessor/internal/metadatatest"
)

// A one-row table: "test-model" at $1 / $2 per 1M tokens.
const tinyTable = `{"generated": "2026-11-17", "models": [{"provider": "openai", "model": "test-model", "aliases": [], "status": "listed", "input": 1, "output": 2}]}`

type signer struct {
	pub  string
	priv ed25519.PrivateKey
}

func newSigner(t *testing.T) signer {
	pub, priv, err := ed25519.GenerateKey(rand.Reader)
	require.NoError(t, err)
	return signer{base64.StdEncoding.EncodeToString(pub), priv}
}

func (s signer) sign(b []byte) []byte {
	return []byte(base64.StdEncoding.EncodeToString(ed25519.Sign(s.priv, b)) + "\n")
}

// tableServer serves body at /prices.json and sig at /prices.json.sig; both can change.
type tableServer struct {
	body, sig atomic.Pointer[[]byte]
	hits      atomic.Int64
	*httptest.Server
}

func newTableServer(t *testing.T, body, sig []byte) *tableServer {
	s := &tableServer{}
	s.body.Store(&body)
	s.sig.Store(&sig)
	s.Server = httptest.NewServer(http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		s.hits.Add(1)
		switch r.URL.Path {
		case "/prices.json":
			_, _ = w.Write(*s.body.Load())
		case "/prices.json.sig":
			if b := *s.sig.Load(); b != nil {
				_, _ = w.Write(b)
				return
			}
			http.NotFound(w, r)
		default:
			http.NotFound(w, r)
		}
	}))
	t.Cleanup(s.Close)
	return s
}

func span(model string, in, out int64) ptrace.Traces {
	td := ptrace.NewTraces()
	sp := td.ResourceSpans().AppendEmpty().ScopeSpans().AppendEmpty().Spans().AppendEmpty()
	sp.SetEndTimestamp(pcommon.NewTimestampFromTime(time.Now()))
	sp.Attributes().PutStr("gen_ai.provider.name", "openai")
	sp.Attributes().PutStr("gen_ai.request.model", model)
	sp.Attributes().PutInt("gen_ai.usage.input_tokens", in)
	sp.Attributes().PutInt("gen_ai.usage.output_tokens", out)
	return td
}

func firstAttrs(td ptrace.Traces) pcommon.Map {
	return td.ResourceSpans().At(0).ScopeSpans().At(0).Spans().At(0).Attributes()
}

func TestSignedTableIsLoadedAndUsed(t *testing.T) {
	s := newSigner(t)
	srv := newTableServer(t, []byte(tinyTable), s.sign([]byte(tinyTable)))
	cfg := createDefaultConfig()
	cfg.PricesURL = srv.URL + "/prices.json"
	cfg.PublicKey = s.pub
	require.NoError(t, cfg.Validate())

	tel := componenttest.NewTelemetry()
	t.Cleanup(func() { require.NoError(t, tel.Shutdown(context.Background())) })
	p, err := newCostProcessor(cfg, metadatatest.NewSettings(tel).TelemetrySettings)
	require.NoError(t, err)
	require.NoError(t, p.start(context.Background(), componenttest.NewNopHost()))
	t.Cleanup(func() { _ = p.shutdown(context.Background()) })

	td := span("test-model", 1_000_000, 1_000_000)
	_, err = p.processTraces(context.Background(), td)
	require.NoError(t, err)
	a := firstAttrs(td)
	amount, _ := a.Get(AttrCostAmount)
	require.InEpsilon(t, 3.0, amount.Double(), 1e-12)
	version, _ := a.Get(AttrCostPricingVersion)
	require.Equal(t, llmprices.Version([]byte(tinyTable)), version.Str())

	metadatatest.AssertEqualProcessorGenaicostPriceReloads(t, tel, []metricdata.DataPoint[int64]{{Value: 1}}, metricdatatest.IgnoreTimestamp())
	metadatatest.AssertEqualProcessorGenaicostSpansPriced(t, tel, []metricdata.DataPoint[int64]{{Value: 1}}, metricdatatest.IgnoreTimestamp())
}

func TestBadSignatureKeepsTheEmbeddedTable(t *testing.T) {
	s, other := newSigner(t), newSigner(t)
	srv := newTableServer(t, []byte(tinyTable), other.sign([]byte(tinyTable)))
	cfg := createDefaultConfig()
	cfg.PricesURL = srv.URL + "/prices.json"
	cfg.PublicKey = s.pub
	p := newTestProcessor(t, cfg)
	require.NoError(t, p.start(context.Background(), componenttest.NewNopHost()), "llm-prices keeps serving the embedded table")
	require.Equal(t, llmprices.PricesVersion, p.source.Load().s.pricingVersion())

	// A tampered body with the right key's signature of the original is rejected too.
	srv2 := newTableServer(t, []byte(strings.Replace(tinyTable, `"input": 1`, `"input": 0`, 1)), s.sign([]byte(tinyTable)))
	cfg.PricesURL = srv2.URL + "/prices.json"
	p2 := newTestProcessor(t, cfg)
	require.NoError(t, p2.start(context.Background(), componenttest.NewNopHost()))
	require.Equal(t, llmprices.PricesVersion, p2.source.Load().s.pricingVersion())

	// A missing signature is a failed load.
	srv3 := newTableServer(t, []byte(tinyTable), nil)
	cfg.PricesURL = srv3.URL + "/prices.json"
	p3 := newTestProcessor(t, cfg)
	require.NoError(t, p3.start(context.Background(), componenttest.NewNopHost()))
	require.Equal(t, llmprices.PricesVersion, p3.source.Load().s.pricingVersion())
}

func TestDefaultKeyRejectsUnsignedTables(t *testing.T) {
	srv := newTableServer(t, []byte(tinyTable), []byte("not a signature"))
	cfg := createDefaultConfig()
	cfg.PricesURL = srv.URL + "/prices.json" // no public_key: the llm-prices release key applies
	require.Equal(t, ReleasePublicKey, base64.StdEncoding.EncodeToString(cfg.publicKey()))
	p := newTestProcessor(t, cfg)
	require.NoError(t, p.start(context.Background(), componenttest.NewNopHost()))
	require.Equal(t, llmprices.PricesVersion, p.source.Load().s.pricingVersion())

	cfg.SkipSignature = true
	require.Nil(t, cfg.publicKey())
	p2 := newTestProcessor(t, cfg)
	require.NoError(t, p2.start(context.Background(), componenttest.NewNopHost()))
	require.Equal(t, llmprices.Version([]byte(tinyTable)), p2.source.Load().s.pricingVersion())
}

func TestHotReloadReplacesTheTable(t *testing.T) {
	s := newSigner(t)
	srv := newTableServer(t, []byte(tinyTable), s.sign([]byte(tinyTable)))
	cfg := createDefaultConfig()
	cfg.PricesURL = srv.URL + "/prices.json"
	cfg.PublicKey = s.pub
	p := newTestProcessor(t, cfg)
	require.NoError(t, p.start(context.Background(), componenttest.NewNopHost()))
	before := p.source.Load().s.pricingVersion()

	newer := strings.Replace(tinyTable, `"input": 1`, `"input": 4`, 1)
	b, sig := []byte(newer), s.sign([]byte(newer))
	srv.body.Store(&b)
	srv.sig.Store(&sig)
	require.NoError(t, p.reloader.reload(context.Background()))
	after := p.source.Load().s.pricingVersion()
	require.NotEqual(t, before, after)
	require.Equal(t, llmprices.Version([]byte(newer)), after)

	td := span("test-model", 1_000_000, 0)
	_, err := p.processTraces(context.Background(), td)
	require.NoError(t, err)
	amount, _ := firstAttrs(td).Get(AttrCostAmount)
	require.InEpsilon(t, 4.0, amount.Double(), 1e-12)

	// The same table again is not a reload.
	require.NoError(t, p.reloader.reload(context.Background()))
	require.Equal(t, after, p.source.Load().s.pricingVersion())
}

func TestBackgroundReloadStopsOnShutdown(t *testing.T) {
	s := newSigner(t)
	srv := newTableServer(t, []byte(tinyTable), s.sign([]byte(tinyTable)))
	cfg := createDefaultConfig()
	cfg.PricesURL = srv.URL + "/prices.json"
	cfg.PublicKey = s.pub
	cfg.ReloadInterval = time.Minute
	require.NoError(t, cfg.Validate())
	cfg.ReloadInterval = 20 * time.Millisecond // below the validated minimum, for the test only
	p, err := newCostProcessor(cfg, componenttest.NewNopTelemetrySettings())
	require.NoError(t, err)
	require.NoError(t, p.start(context.Background(), componenttest.NewNopHost()))
	require.Eventually(t, func() bool { return srv.hits.Load() >= 6 }, 5*time.Second, 10*time.Millisecond)
	require.NoError(t, p.shutdown(context.Background()))
	n := srv.hits.Load()
	time.Sleep(100 * time.Millisecond)
	require.Equal(t, n, srv.hits.Load(), "reloads after shutdown")
}

func TestLiteLLMSourceFromFile(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "model_prices.json")
	require.NoError(t, os.WriteFile(path, []byte(`{"gpt-x": {"litellm_provider": "openai", "mode": "chat", "input_cost_per_token": 1e-6, "output_cost_per_token": 2e-6}}`), 0o600))
	cfg := createDefaultConfig()
	cfg.Source = SourceLiteLLM
	cfg.PricesURL = path
	p := newTestProcessor(t, cfg)
	require.NoError(t, p.start(context.Background(), componenttest.NewNopHost()))
	td := span("gpt-x", 1000, 1000)
	_, err := p.processTraces(context.Background(), td)
	require.NoError(t, err)
	a := firstAttrs(td)
	amount, _ := a.Get(AttrCostAmount)
	require.InEpsilon(t, 0.003, amount.Double(), 1e-12)
	id, _ := a.Get(AttrCostPricingID)
	require.Equal(t, "litellm.model_cost_map", id.Str())

	// litellm has no embedded table: a failed first load stops the collector from starting.
	cfg2 := createDefaultConfig()
	cfg2.Source = SourceLiteLLM
	cfg2.PricesURL = filepath.Join(dir, "missing.json")
	p2 := newTestProcessor(t, cfg2)
	require.Error(t, p2.start(context.Background(), componenttest.NewNopHost()))
}

func TestFileURL(t *testing.T) {
	dir := t.TempDir()
	path := filepath.Join(dir, "prices.json")
	require.NoError(t, os.WriteFile(path, []byte(tinyTable), 0o600))
	cfg := createDefaultConfig()
	cfg.PricesURL = "file://" + filepath.ToSlash(path)
	if !strings.HasPrefix(cfg.PricesURL, "file:///") {
		cfg.PricesURL = "file:///" + filepath.ToSlash(path)
	}
	cfg.SkipSignature = true
	p := newTestProcessor(t, cfg)
	require.NoError(t, p.start(context.Background(), componenttest.NewNopHost()))
	require.Equal(t, llmprices.Version([]byte(tinyTable)), p.source.Load().s.pricingVersion())
}

func TestFactoryPipeline(t *testing.T) {
	f := NewFactory()
	require.Equal(t, metadata.Type, f.Type())
	sink := new(consumertest.TracesSink)
	tp, err := f.CreateTraces(context.Background(), processortest.NewNopSettings(metadata.Type), f.CreateDefaultConfig(), sink)
	require.NoError(t, err)
	require.True(t, tp.Capabilities().MutatesData)
	require.NoError(t, tp.Start(context.Background(), componenttest.NewNopHost()))
	require.NoError(t, tp.ConsumeTraces(context.Background(), span("claude-sonnet-4-6", 1000, 1000)))
	require.NoError(t, tp.Shutdown(context.Background()))
	require.Len(t, sink.AllTraces(), 1)
	// "openai" + claude: OpenAI does not sell Claude, so the span is flagged, not priced.
	v, ok := firstAttrs(sink.AllTraces()[0]).Get(AttrUnpriced)
	require.True(t, ok)
	require.Equal(t, ReasonUnknownModel, v.Str())
}
