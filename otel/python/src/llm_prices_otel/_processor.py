"""The span processor and exporter wrapper."""

from __future__ import annotations

import logging
import math
from typing import Any, Dict, Iterable, Mapping, Optional, Sequence, Tuple

from opentelemetry.sdk.trace import ReadableSpan, SpanProcessor
from opentelemetry.sdk.trace.export import SpanExporter, SpanExportResult
from opentelemetry.sdk.util.instrumentation import InstrumentationScope
from opentelemetry.attributes import BoundedAttributes

from . import _rules as r
from ._sources import LLMPricesSource, PriceSource, Priced, Request

_log = logging.getLogger("llm_prices_otel")

#: Bucket boundaries the cost proposal (semantic-conventions-genai#443) advises for
#: ``gen_ai.client.operation.cost``, on a roughly US-dollar scale.
COST_BUCKETS = (0.001, 0.004, 0.016, 0.064, 0.256, 1.024, 4.096, 16.384, 65.536)
#: Span attributes copied onto the cost histogram when present (the metric attributes of the
#: GenAI conventions).
METRIC_ATTRIBUTES = (
    "gen_ai.operation.name",
    "gen_ai.provider.name",
    "gen_ai.request.model",
    "gen_ai.response.model",
    "server.address",
    "server.port",
)

_MODES = ("auto", "true", "false")


class CostPricer:
    """Prices span attributes. :class:`CostSpanProcessor` and :class:`CostSpanExporter` use one;
    call :meth:`attributes` directly to price attributes from anywhere else.

    Arguments (all keyword-only) mirror the Collector processor's settings:

    - ``source``: a :class:`~llm_prices_otel.PriceSource`, :class:`LLMPricesSource` by default.
    - ``on_unpriced``: ``"flag"`` writes ``llm_prices.unpriced`` with the reason;
      ``"error_attribute"`` also sets ``error.type`` to ``"llm_prices.unpriced"`` when the span
      has none. A span is never given a zero cost because a price is missing.
    - ``input_tokens_include_cache`` / ``output_tokens_include_reasoning``: ``"auto"``
      (default), ``"true"`` or ``"false"``. See docs/otel.md.
    - ``batch_attribute``: boolean attribute that marks a Batch API request
      (``"llm_prices.batch"``); ``None`` turns batch pricing off.
    - ``reported_cost_attributes``: attribute keys holding a USD cost that was present in the
      response; written with ``gen_ai.usage.cost.source = "provider"``.
    - ``breakdown``: also write ``llm_prices.cost.*`` and ``llm_prices.price.row``.
    - ``openinference``: also write OpenInference's ``llm.cost.*`` attributes.
    """

    def __init__(
        self,
        *,
        source: Optional[PriceSource] = None,
        on_unpriced: str = "flag",
        input_tokens_include_cache: str = "auto",
        output_tokens_include_reasoning: str = "auto",
        batch_attribute: Optional[str] = "llm_prices.batch",
        reported_cost_attributes: Sequence[str] = (),
        breakdown: bool = False,
        openinference: bool = False,
    ) -> None:
        if on_unpriced not in ("flag", "error_attribute"):
            raise ValueError(f"on_unpriced {on_unpriced!r}: want 'flag' or 'error_attribute'")
        for name, v in (("input_tokens_include_cache", input_tokens_include_cache), ("output_tokens_include_reasoning", output_tokens_include_reasoning)):
            if v not in _MODES:
                raise ValueError(f"{name} {v!r}: want 'auto', 'true' or 'false'")
        if any(not k for k in reported_cost_attributes):
            raise ValueError("reported_cost_attributes: empty key")
        self.source = source if source is not None else LLMPricesSource()
        self.on_unpriced = on_unpriced
        self.input_tokens_include_cache = input_tokens_include_cache
        self.output_tokens_include_reasoning = output_tokens_include_reasoning
        self.batch_attribute = batch_attribute or None
        self.reported_cost_attributes = tuple(reported_cost_attributes)
        self.breakdown = breakdown
        self.openinference = openinference

    def attributes(self, attrs: Mapping[str, Any]) -> Tuple[Dict[str, Any], Optional[str]]:
        """The attributes to add to a span with attributes ``attrs``, and the unpriced reason
        (``None`` when priced or not a GenAI inference span). Empty when there is nothing to add."""
        if r.COST_AMOUNT in attrs:
            return {}, None
        passed = self._pass_through(attrs)
        if passed:
            return passed, None
        tokens, valid = r.read_tokens(attrs, self.input_tokens_include_cache, self.output_tokens_include_reasoning)
        if tokens is None and valid:
            return {}, None
        if tokens is None:
            return self._flag(attrs, r.INVALID_USAGE)
        models = r.span_models(attrs)
        if not models:
            return self._flag(attrs, r.NO_MODEL)
        provider, known = r.span_provider(attrs)
        if not known:
            return self._flag(attrs, r.UNKNOWN_PROVIDER)
        batch = False
        if self.batch_attribute is not None:
            b = attrs.get(self.batch_attribute)
            batch = b if isinstance(b, bool) else False
        tier = r.span_tier(attrs)
        result = self.source.price(Request(provider=provider, models=models, tokens=tokens, batch=batch, tier=tier))
        if isinstance(result, str):
            return self._flag(attrs, result)
        return self._write(result, tier, tokens), None

    def _pass_through(self, attrs: Mapping[str, Any]) -> Dict[str, Any]:
        for k in self.reported_cost_attributes:
            v = attrs.get(k)
            if isinstance(v, bool) or not isinstance(v, (int, float)):
                continue
            amount = float(v)
            if math.isnan(amount) or math.isinf(amount) or amount < 0:
                continue
            return {r.COST_AMOUNT: amount, r.COST_CURRENCY: "USD", r.COST_SOURCE: "provider"}
        return {}

    def _flag(self, attrs: Mapping[str, Any], reason: str) -> Tuple[Dict[str, Any], Optional[str]]:
        out: Dict[str, Any] = {r.UNPRICED: reason}
        if self.on_unpriced == "error_attribute" and r.ERROR_TYPE not in attrs:
            out[r.ERROR_TYPE] = "llm_prices.unpriced"
        return out, reason

    def _write(self, c: Priced, tier: Optional[str], t: r.Tokens) -> Dict[str, Any]:
        out: Dict[str, Any] = {
            r.COST_AMOUNT: c.total,
            r.COST_CURRENCY: "USD",
            r.COST_SOURCE: "local",
            r.COST_PRICING_ID: self.source.pricing_id,
            r.COST_PRICING_VERSION: self.source.pricing_version,
        }
        if tier is not None:
            out[r.PRICE_TIER] = tier
        if self.breakdown:
            out.update({
                "llm_prices.cost.input": c.input,
                "llm_prices.cost.output": c.output,
                "llm_prices.cost.cache_read": c.cache_read,
                "llm_prices.cost.cache_write": c.cache_write,
                r.PRICE_ROW: c.row,
            })
        if self.openinference:
            reasoning = 0.0
            out.update({
                "llm.cost.prompt": c.input + c.cache_read + c.cache_write,
                "llm.cost.completion": c.output,
                "llm.cost.total": c.total,
                "llm.cost.prompt_details.input": c.input,
                "llm.cost.prompt_details.cache_read": c.cache_read,
                "llm.cost.prompt_details.cache_write": c.cache_write,
            })
            if t.reasoning > 0 and t.output > 0:
                reasoning = c.output * float(t.reasoning) / float(t.output)
                out["llm.cost.completion_details.reasoning"] = reasoning
            out["llm.cost.completion_details.output"] = c.output - reasoning
        return out


def _with_attributes(span: ReadableSpan, extra: Mapping[str, Any]) -> ReadableSpan:
    """A copy of an ended span with ``extra`` added to its attributes."""
    merged = dict(span.attributes or {})
    merged.update(extra)
    attributes = BoundedAttributes(maxlen=None, attributes=merged, immutable=True)
    attributes.dropped = span.dropped_attributes
    scope: Optional[InstrumentationScope] = span.instrumentation_scope
    copy = ReadableSpan(
        name=span.name,
        context=span.context,
        parent=span.parent,
        resource=span.resource,
        attributes=attributes,
        # The private lists keep the dropped event and link counts the exporter reports.
        events=getattr(span, "_events", span.events),
        links=getattr(span, "_links", span.links),
        kind=span.kind,
        status=span.status,
        start_time=span.start_time,
        end_time=span.end_time,
        instrumentation_scope=scope,
    )
    return copy


class _Histogram:
    def __init__(self, meter_provider: Any) -> None:
        from opentelemetry import metrics

        from . import __version__

        meter = (meter_provider or metrics.get_meter_provider()).get_meter("llm_prices_otel", __version__)
        kwargs: Dict[str, Any] = {"unit": "{cost}", "description": "Monetary cost of a single GenAI client operation."}
        try:
            self._h = meter.create_histogram("gen_ai.client.operation.cost", explicit_bucket_boundaries_advisory=list(COST_BUCKETS), **kwargs)
        except TypeError:  # opentelemetry-api before 1.23
            self._h = meter.create_histogram("gen_ai.client.operation.cost", **kwargs)

    def record(self, attrs: Mapping[str, Any], added: Mapping[str, Any]) -> None:
        amount = added.get(r.COST_AMOUNT)
        if not isinstance(amount, float):
            return
        labels = {k: attrs[k] for k in METRIC_ATTRIBUTES if k in attrs}
        labels[r.COST_CURRENCY] = added[r.COST_CURRENCY]
        labels[r.COST_SOURCE] = added[r.COST_SOURCE]
        if r.ERROR_TYPE in attrs:
            labels[r.ERROR_TYPE] = attrs[r.ERROR_TYPE]
        self._h.record(amount, labels)


class CostSpanProcessor(SpanProcessor):
    """A span processor that prices each ended GenAI span and passes a copy with the cost
    attributes to the processors it wraps (usually a ``BatchSpanProcessor``)::

        provider.add_span_processor(CostSpanProcessor(BatchSpanProcessor(OTLPSpanExporter())))

    The cost is computed when the span ends, in the process that recorded it, from the price
    table loaded in that process: ``gen_ai.usage.cost.source`` is ``"local"`` and
    ``gen_ai.usage.cost.pricing.id`` / ``.pricing.version`` name the table. Spans that are not
    GenAI inference spans pass through unchanged. With ``record_metric=True`` (default) the cost
    is also recorded on the ``gen_ai.client.operation.cost`` histogram through
    ``meter_provider`` (the global one when ``None``).

    Other keyword arguments are those of :class:`CostPricer`.
    """

    def __init__(
        self,
        *processors: SpanProcessor,
        record_metric: bool = True,
        meter_provider: Any = None,
        **pricer_kwargs: Any,
    ) -> None:
        if not processors:
            raise TypeError("CostSpanProcessor needs at least one span processor to pass spans to")
        self._processors: Tuple[SpanProcessor, ...] = processors
        self.pricer = CostPricer(**pricer_kwargs)
        self._histogram = _Histogram(meter_provider) if record_metric else None

    def on_start(self, span: Any, parent_context: Any = None) -> None:
        for p in self._processors:
            p.on_start(span, parent_context=parent_context)

    def _on_ending(self, span: Any) -> None:
        for p in self._processors:
            hook = getattr(p, "_on_ending", None)
            if hook is not None:
                hook(span)

    def on_end(self, span: ReadableSpan) -> None:
        out = span
        try:
            attrs = span.attributes or {}
            added, _ = self.pricer.attributes(attrs)
            if added:
                out = _with_attributes(span, added)
                if self._histogram is not None:
                    self._histogram.record(attrs, added)
        except Exception:  # never break the export path
            _log.exception("llm-prices-otel: could not price span %r", getattr(span, "name", None))
        for p in self._processors:
            p.on_end(out)

    def shutdown(self) -> None:
        for p in self._processors:
            p.shutdown()

    def force_flush(self, timeout_millis: int = 30000) -> bool:
        return all(p.force_flush(timeout_millis) for p in self._processors)


class CostSpanExporter(SpanExporter):
    """A span exporter that prices GenAI spans before handing them to the exporter it wraps::

        BatchSpanProcessor(CostSpanExporter(OTLPSpanExporter()))

    Same pricing as :class:`CostSpanProcessor` (keyword arguments of :class:`CostPricer`),
    for setups where wrapping the exporter is easier than wrapping the processor. No metric.
    """

    def __init__(self, exporter: SpanExporter, **pricer_kwargs: Any) -> None:
        self._exporter = exporter
        self.pricer = CostPricer(**pricer_kwargs)

    def export(self, spans: Sequence[ReadableSpan]) -> SpanExportResult:
        return self._exporter.export(list(self._price(spans)))

    def _price(self, spans: Iterable[ReadableSpan]) -> Iterable[ReadableSpan]:
        for span in spans:
            try:
                added, _ = self.pricer.attributes(span.attributes or {})
                yield _with_attributes(span, added) if added else span
            except Exception:
                _log.exception("llm-prices-otel: could not price span %r", getattr(span, "name", None))
                yield span

    def shutdown(self) -> None:
        self._exporter.shutdown()

    def force_flush(self, timeout_millis: int = 30000) -> bool:
        return self._exporter.force_flush(timeout_millis)
