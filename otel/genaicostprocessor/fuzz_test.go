package genaicostprocessor

import (
	"context"
	"math"
	"testing"
	"time"

	"go.opentelemetry.io/collector/component"
	"go.opentelemetry.io/collector/component/componenttest"
	"go.opentelemetry.io/collector/pdata/pcommon"
	"go.opentelemetry.io/collector/pdata/ptrace"
)

// FuzzPriceSpan feeds arbitrary attribute values (span attributes are untrusted: any
// instrumentation, any version) through the processor. It must never panic, never write a
// negative, NaN or infinite cost, and never write both a cost and an unpriced flag.
func FuzzPriceSpan(f *testing.F) {
	f.Add("anthropic", "claude-sonnet-4-6", "", int64(50548), int64(1024), int64(45000), int64(3500), int64(0), 0.0, "1000", false, "")
	f.Add("openai", "gpt-4o-2024-08-06", "openrouter.ai", int64(-1), int64(10), int64(0), int64(0), int64(5), 10.5, "x", true, "flex")
	f.Add("gcp.gemini", "gemini-2.5-pro", "", int64(300000), int64(1000), int64(0), int64(0), int64(0), math.Inf(1), "", false, "priority")
	f.Fuzz(func(t *testing.T, provider, model, host string, in, out, read, write, reasoning int64, dbl float64, str string, batch bool, tier string) {
		p, err := newCostProcessor(createDefaultConfig(), noopTelemetry())
		if err != nil {
			t.Fatal(err)
		}
		p.cfg.Breakdown, p.cfg.OpenInference = true, true
		td := ptrace.NewTraces()
		sp := td.ResourceSpans().AppendEmpty().ScopeSpans().AppendEmpty().Spans().AppendEmpty()
		sp.SetEndTimestamp(pcommon.NewTimestampFromTime(time.Now()))
		a := sp.Attributes()
		a.PutStr("gen_ai.provider.name", provider)
		a.PutStr("gen_ai.request.model", model)
		if host != "" {
			a.PutStr("server.address", host)
		}
		a.PutInt("gen_ai.usage.input_tokens", in)
		a.PutInt("gen_ai.usage.output_tokens", out)
		a.PutInt("gen_ai.usage.cache_read.input_tokens", read)
		a.PutInt("gen_ai.usage.cache_write.input_tokens", write)
		a.PutInt("gen_ai.usage.reasoning.output_tokens", reasoning)
		a.PutDouble("llm.token_count.prompt", dbl)
		a.PutStr("gen_ai.usage.prompt_tokens", str)
		a.PutBool("llm_prices.batch", batch)
		a.PutStr("openai.response.service_tier", tier)
		if _, err := p.processTraces(context.Background(), td); err != nil {
			t.Fatal(err)
		}
		amount, priced := a.Get(AttrCostAmount)
		_, flagged := a.Get(AttrUnpriced)
		if priced && flagged {
			t.Fatal("both a cost and an unpriced flag")
		}
		if priced {
			v := amount.Double()
			if math.IsNaN(v) || math.IsInf(v, 0) || v < 0 {
				t.Fatalf("bad cost %v", v)
			}
		}
		for k, v := range a.All() {
			if v.Type() == pcommon.ValueTypeDouble && (k == AttrCostAmount || len(k) > 8 && (k[:8] == "llm.cost" || k[:9] == "llm_price")) {
				if f := v.Double(); math.IsNaN(f) || math.IsInf(f, 0) || f < -1e-12 {
					t.Fatalf("%s = %v", k, f)
				}
			}
		}
	})
}

func FuzzParseLiteLLM(f *testing.F) {
	f.Add([]byte(`{"gpt-x": {"litellm_provider": "openai", "input_cost_per_token": 1e-6, "output_cost_per_token": 2e-6, "input_cost_per_token_above_1k_tokens": 3e-6}}`))
	f.Add([]byte(`{"a": {"input_cost_per_token": -1, "output_cost_per_token": "x", "input_cost_per_token_above_xk_tokens": 1}}`))
	f.Fuzz(func(t *testing.T, b []byte) {
		src, err := parseLiteLLM(b)
		if err != nil {
			return
		}
		s := src.(liteLLMSource)
		for k := range s.entries {
			for _, prov := range []string{"", "openai", "xai", "fireworks"} {
				c, reason := s.price(request{provider: prov, models: []string{k}, tokens: tokens{input: 1000, cacheRead: 10, cacheWrite: 10, output: 100, reasoning: 50}, tier: "flex"})
				if reason == "" && (math.IsNaN(c.total) || math.IsInf(c.total, 0) || c.total < 0) {
					t.Fatalf("%s: bad cost %v", k, c.total)
				}
			}
		}
	})
}

func noopTelemetry() component.TelemetrySettings { return componenttest.NewNopTelemetrySettings() }
