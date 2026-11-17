package genaicostprocessor // import "github.com/Mattbusel/llm-prices/otel/genaicostprocessor"

import (
	"context"
	"math"
	"strings"
	"sync/atomic"
	"time"

	llmprices "github.com/Mattbusel/llm-prices/go"
	"go.opentelemetry.io/collector/component"
	"go.opentelemetry.io/collector/pdata/pcommon"
	"go.opentelemetry.io/collector/pdata/ptrace"
	"go.opentelemetry.io/otel/attribute"
	"go.opentelemetry.io/otel/metric"
	"go.uber.org/zap"

	"github.com/Mattbusel/llm-prices/otel/genaicostprocessor/internal/metadata"
)

type costProcessor struct {
	cfg       *Config
	logger    *zap.Logger
	telemetry *metadata.TelemetryBuilder
	source    atomic.Pointer[sourceBox]
	reloader  *reloader
	now       func() time.Time
}

// sourceBox lets an interface value live in an atomic.Pointer.
type sourceBox struct{ s priceSource }

func newCostProcessor(cfg *Config, set component.TelemetrySettings) (*costProcessor, error) {
	tb, err := metadata.NewTelemetryBuilder(set)
	if err != nil {
		return nil, err
	}
	p := &costProcessor{cfg: cfg, logger: set.Logger, telemetry: tb, now: time.Now}
	if cfg.Source == SourceLLMPrices {
		p.source.Store(&sourceBox{llmPricesSource{table: llmprices.Default()}})
	}
	if url := cfg.tableURL(); url != "" {
		p.reloader = newReloader(cfg, url, set.Logger, p.install, tb)
	}
	return p, nil
}

// install replaces the price source in use.
func (p *costProcessor) install(s priceSource) {
	p.source.Store(&sourceBox{s})
}

func (p *costProcessor) start(ctx context.Context, _ component.Host) error {
	if p.reloader == nil {
		return nil
	}
	// The embedded llm-prices table keeps serving when the first load fails; other sources
	// have nothing to fall back to, so the collector does not start without a table.
	return p.reloader.start(ctx, p.cfg.Source != SourceLLMPrices)
}

func (p *costProcessor) shutdown(context.Context) error {
	if p.reloader != nil {
		p.reloader.stop()
	}
	p.telemetry.Shutdown()
	return nil
}

func (p *costProcessor) processTraces(ctx context.Context, td ptrace.Traces) (ptrace.Traces, error) {
	box := p.source.Load()
	if box == nil {
		return td, nil
	}
	now := p.now()
	var pricedN, unpricedN int64
	unpriced := map[string]int64{}
	rss := td.ResourceSpans()
	for i := 0; i < rss.Len(); i++ {
		sss := rss.At(i).ScopeSpans()
		for j := 0; j < sss.Len(); j++ {
			spans := sss.At(j).Spans()
			for k := 0; k < spans.Len(); k++ {
				switch outcome, reason := p.priceSpan(box.s, spans.At(k), now); outcome {
				case outcomePriced:
					pricedN++
				case outcomeUnpriced:
					unpricedN++
					unpriced[reason]++
				}
			}
		}
	}
	if pricedN > 0 {
		p.telemetry.ProcessorGenaicostSpansPriced.Add(ctx, pricedN)
	}
	for reason, n := range unpriced {
		p.telemetry.ProcessorGenaicostSpansUnpriced.Add(ctx, n, metric.WithAttributes(attribute.String("reason", reason)))
	}
	return td, nil
}

type outcome int

const (
	outcomeSkipped outcome = iota
	outcomePriced
	outcomeUnpriced
)

func (p *costProcessor) priceSpan(src priceSource, span ptrace.Span, now time.Time) (outcome, string) {
	attrs := span.Attributes()
	if _, ok := attrs.Get(AttrCostAmount); ok {
		// A cost is already on the span (from the instrumentation or an earlier stage). It is
		// frozen at recording time, so it is never replaced.
		return outcomeSkipped, ""
	}
	if p.passThrough(attrs) {
		return outcomePriced, ""
	}
	t, isGenAI, valid := readTokens(attrs, p.cfg.InputTokensIncludeCache, p.cfg.OutputTokensIncludeReasoning)
	if !isGenAI {
		return outcomeSkipped, ""
	}
	if !valid {
		return p.flag(attrs, ReasonInvalidUsage)
	}
	if p.cfg.MaxSpanAge > 0 {
		if end := span.EndTimestamp(); end != 0 && now.Sub(end.AsTime()) > p.cfg.MaxSpanAge {
			return p.flag(attrs, ReasonStaleSpan)
		}
	}
	r := request{tokens: t}
	for _, k := range modelKeys {
		if v, ok := attrs.Get(k); ok && v.Type() == pcommon.ValueTypeStr {
			if m := strings.TrimSpace(v.Str()); m != "" && !contains(r.models, m) {
				r.models = append(r.models, m)
			}
		}
	}
	if len(r.models) == 0 {
		return p.flag(attrs, ReasonNoModel)
	}
	provider, known := spanProvider(attrs)
	if !known {
		return p.flag(attrs, ReasonUnknownProvider)
	}
	r.provider = provider
	if tier, ok := str(attrs, tierKeys); ok {
		r.tier = normalizeTier(tier)
	}
	if k := p.cfg.BatchAttribute; k != "" {
		if v, ok := attrs.Get(k); ok && v.Type() == pcommon.ValueTypeBool {
			r.batch = v.Bool()
		}
	}
	c, reason := src.price(r)
	if reason != "" {
		return p.flag(attrs, reason)
	}
	p.write(attrs, c, src, r.tier, r.tokens)
	return outcomePriced, ""
}

// spanProvider returns the llm-prices name of the provider that served a span, "" when the span
// does not say. known is false when the span names a provider llm-prices does not price.
func spanProvider(attrs pcommon.Map) (name string, known bool) {
	if host, ok := str(attrs, serverKeys); ok {
		if p := providerFromHost(host); p != "" {
			return p, true
		}
	}
	raw, ok := str(attrs, providerKeys)
	if !ok {
		return "", true
	}
	return canonicalProvider(raw)
}

func canonicalProvider(raw string) (string, bool) {
	lower := strings.ToLower(raw)
	if p, ok := providerAliases[lower]; ok {
		return p, true
	}
	if p, ok := llmprices.ParseProvider(lower); ok {
		return string(p), true
	}
	return "", false
}

func (p *costProcessor) passThrough(attrs pcommon.Map) bool {
	for _, k := range p.cfg.ReportedCostAttributes {
		v, ok := attrs.Get(k)
		if !ok {
			continue
		}
		var amount float64
		switch v.Type() {
		case pcommon.ValueTypeDouble:
			amount = v.Double()
		case pcommon.ValueTypeInt:
			amount = float64(v.Int())
		default:
			continue
		}
		if amount < 0 || math.IsNaN(amount) || math.IsInf(amount, 0) {
			continue
		}
		attrs.PutDouble(AttrCostAmount, amount)
		attrs.PutStr(AttrCostCurrency, "USD")
		attrs.PutStr(AttrCostSource, "provider")
		return true
	}
	return false
}

func (p *costProcessor) flag(attrs pcommon.Map, reason string) (outcome, string) {
	attrs.PutStr(AttrUnpriced, reason)
	if p.cfg.OnUnpriced == UnpricedErrorAttribute {
		if _, ok := attrs.Get(attrErrorType); !ok {
			attrs.PutStr(attrErrorType, "llm_prices.unpriced")
		}
	}
	return outcomeUnpriced, reason
}

func (p *costProcessor) write(attrs pcommon.Map, c priced, src priceSource, tier string, t tokens) {
	attrs.PutDouble(AttrCostAmount, c.total)
	attrs.PutStr(AttrCostCurrency, "USD")
	attrs.PutStr(AttrCostSource, "local")
	attrs.PutStr(AttrCostPricingID, src.pricingID())
	attrs.PutStr(AttrCostPricingVersion, src.pricingVersion())
	if tier != "" {
		attrs.PutStr(AttrPriceTier, tier)
	}
	if p.cfg.Breakdown {
		attrs.PutDouble("llm_prices.cost.input", c.input)
		attrs.PutDouble("llm_prices.cost.output", c.output)
		attrs.PutDouble("llm_prices.cost.cache_read", c.cacheRead)
		attrs.PutDouble("llm_prices.cost.cache_write", c.cacheWrite)
		attrs.PutStr(AttrPriceRow, c.row)
	}
	if p.cfg.OpenInference {
		prompt := c.input + c.cacheRead + c.cacheWrite
		attrs.PutDouble("llm.cost.prompt", prompt)
		attrs.PutDouble("llm.cost.completion", c.output)
		attrs.PutDouble("llm.cost.total", c.total)
		attrs.PutDouble("llm.cost.prompt_details.input", c.input)
		attrs.PutDouble("llm.cost.prompt_details.cache_read", c.cacheRead)
		attrs.PutDouble("llm.cost.prompt_details.cache_write", c.cacheWrite)
		reasoning := 0.0
		if t.reasoning > 0 && t.output > 0 {
			reasoning = c.output * float64(t.reasoning) / float64(t.output)
			attrs.PutDouble("llm.cost.completion_details.reasoning", reasoning)
		}
		attrs.PutDouble("llm.cost.completion_details.output", c.output-reasoning)
	}
}

func contains(xs []string, x string) bool {
	for _, y := range xs {
		if y == x {
			return true
		}
	}
	return false
}
