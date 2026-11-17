"""CostSpanProcessor and CostSpanExporter inside a real OpenTelemetry SDK pipeline."""

from __future__ import annotations

import pytest
from opentelemetry.sdk.metrics import MeterProvider
from opentelemetry.sdk.metrics.export import InMemoryMetricReader
from opentelemetry.sdk.trace import SpanLimits, TracerProvider
from opentelemetry.sdk.trace.export import SimpleSpanProcessor
from opentelemetry.sdk.trace.export.in_memory_span_exporter import InMemorySpanExporter

import llm_prices
from llm_prices_otel import COST_BUCKETS, CostPricer, CostSpanExporter, CostSpanProcessor

CHAT = {
    "gen_ai.operation.name": "chat",
    "gen_ai.provider.name": "anthropic",
    "gen_ai.request.model": "claude-sonnet-4-6",
    "gen_ai.usage.input_tokens": 50_548,
    "gen_ai.usage.cache_read.input_tokens": 45_000,
    "gen_ai.usage.cache_write.input_tokens": 3_500,
    "gen_ai.usage.output_tokens": 1_024,
}


def _pipeline(**kwargs):
    exporter = InMemorySpanExporter()
    reader = InMemoryMetricReader()
    meters = MeterProvider(metric_readers=[reader])
    provider = TracerProvider()
    provider.add_span_processor(CostSpanProcessor(SimpleSpanProcessor(exporter), meter_provider=meters, **kwargs))
    return provider.get_tracer("test"), exporter, reader


def test_processor_writes_cost_and_metric():
    tracer, exporter, reader = _pipeline()
    with tracer.start_as_current_span("chat claude-sonnet-4-6") as span:
        span.set_attributes(CHAT)
        span.add_event("gen_ai.choice")
    with tracer.start_as_current_span("not genai") as span:
        span.set_attribute("http.method", "GET")

    priced, plain = exporter.get_finished_spans()
    a = priced.attributes
    assert a["gen_ai.usage.cost.amount"] == pytest.approx(0.048129, rel=1e-12)
    assert a["gen_ai.usage.cost.currency"] == "USD"
    assert a["gen_ai.usage.cost.source"] == "local"
    assert a["gen_ai.usage.cost.pricing.id"] == "llm-prices"
    assert a["gen_ai.usage.cost.pricing.version"] == llm_prices.PRICES_VERSION
    assert a["gen_ai.request.model"] == "claude-sonnet-4-6"  # original attributes kept
    assert priced.name == "chat claude-sonnet-4-6" and len(priced.events) == 1
    assert priced.context.span_id and priced.end_time >= priced.start_time
    assert "gen_ai.usage.cost.amount" not in plain.attributes

    data = reader.get_metrics_data()
    metrics = [m for rm in data.resource_metrics for sm in rm.scope_metrics for m in sm.metrics]
    (cost,) = [m for m in metrics if m.name == "gen_ai.client.operation.cost"]
    assert cost.unit == "{cost}"
    (point,) = cost.data.data_points
    assert point.sum == pytest.approx(0.048129, rel=1e-12) and point.count == 1
    import inspect

    from opentelemetry.metrics import Meter

    if "explicit_bucket_boundaries_advisory" in inspect.signature(Meter.create_histogram).parameters:
        assert tuple(point.explicit_bounds) == COST_BUCKETS  # advisory buckets: opentelemetry-api 1.23+
    assert dict(point.attributes) == {
        "gen_ai.operation.name": "chat",
        "gen_ai.provider.name": "anthropic",
        "gen_ai.request.model": "claude-sonnet-4-6",
        "gen_ai.usage.cost.currency": "USD",
        "gen_ai.usage.cost.source": "local",
    }


def test_unpriced_span_is_flagged_not_zeroed():
    tracer, exporter, reader = _pipeline(on_unpriced="error_attribute")
    with tracer.start_as_current_span("chat") as span:
        span.set_attributes({**CHAT, "gen_ai.request.model": "my-finetune"})
    (s,) = exporter.get_finished_spans()
    assert s.attributes["llm_prices.unpriced"] == "unknown_model"
    assert s.attributes["error.type"] == "llm_prices.unpriced"
    assert "gen_ai.usage.cost.amount" not in s.attributes
    data = reader.get_metrics_data()
    assert data is None or not [m for rm in data.resource_metrics for sm in rm.scope_metrics for m in sm.metrics if m.name == "gen_ai.client.operation.cost"]


def test_dropped_attribute_count_survives():
    exporter = InMemorySpanExporter()
    provider = TracerProvider(span_limits=SpanLimits(max_span_attributes=8))
    provider.add_span_processor(CostSpanProcessor(SimpleSpanProcessor(exporter), record_metric=False))
    with provider.get_tracer("t").start_as_current_span("chat") as span:
        span.set_attributes(CHAT)
        span.set_attribute("extra.1", 1)
        span.set_attribute("extra.2", 2)
    (s,) = exporter.get_finished_spans()
    assert s.dropped_attributes == 1
    assert "gen_ai.usage.cost.amount" in s.attributes


def test_exporter_wrapper():
    inner = InMemorySpanExporter()
    provider = TracerProvider()
    provider.add_span_processor(SimpleSpanProcessor(CostSpanExporter(inner, breakdown=True)))
    with provider.get_tracer("t").start_as_current_span("chat") as span:
        span.set_attributes(CHAT)
    (s,) = inner.get_finished_spans()
    assert s.attributes["llm_prices.price.row"] == "anthropic/claude-sonnet-4-6"
    assert s.attributes["llm_prices.cost.output"] == pytest.approx(0.01536)


def test_processor_never_breaks_export(monkeypatch):
    tracer, exporter, _ = _pipeline(record_metric=False)

    def boom(self, attrs):
        raise RuntimeError("bad source")

    monkeypatch.setattr(CostPricer, "attributes", boom)
    with tracer.start_as_current_span("chat") as span:
        span.set_attributes(CHAT)
    (s,) = exporter.get_finished_spans()
    assert "gen_ai.usage.cost.amount" not in s.attributes


def test_bad_settings():
    with pytest.raises(ValueError):
        CostPricer(on_unpriced="zero")
    with pytest.raises(ValueError):
        CostPricer(input_tokens_include_cache="yes")
    with pytest.raises(ValueError):
        CostPricer(reported_cost_attributes=[""])
    with pytest.raises(TypeError):
        CostSpanProcessor()


def test_shutdown_and_flush_reach_the_wrapped_processor():
    calls = []

    class Inner(SimpleSpanProcessor):
        def shutdown(self):
            calls.append("shutdown")

        def force_flush(self, timeout_millis=30000):
            calls.append("flush")
            return True

    p = CostSpanProcessor(Inner(InMemorySpanExporter()), record_metric=False)
    assert p.force_flush()
    p.shutdown()
    assert calls == ["flush", "shutdown"]
