"""OpenTelemetry cost attributes for GenAI spans, from the llm-prices table.

Wrap the span processor you export with, and every GenAI inference span from OpenLLMetry,
OpenInference, Logfire or any instrumentation that follows the GenAI semantic conventions
leaves the process with ``gen_ai.usage.cost.amount``, ``.currency`` and ``.source`` (proposed in
open-telemetry/semantic-conventions-genai#443) and ``gen_ai.usage.cost.pricing.id`` and
``.pricing.version`` (#624), so each cost names the exact price table that produced it::

    from opentelemetry.sdk.trace import TracerProvider
    from opentelemetry.sdk.trace.export import BatchSpanProcessor
    from llm_prices_otel import CostSpanProcessor

    provider = TracerProvider()
    provider.add_span_processor(CostSpanProcessor(BatchSpanProcessor(exporter)))

A span that cannot be priced (unknown model, missing rate) never gets a zero: it gets
``llm_prices.unpriced`` with the reason. The same rules as the OpenTelemetry Collector
processor in the same repository (otel/genaicostprocessor); see docs/otel.md.

>>> from llm_prices_otel import CostPricer
>>> added, reason = CostPricer().attributes({
...     "gen_ai.provider.name": "anthropic",
...     "gen_ai.request.model": "claude-sonnet-4-6",
...     "gen_ai.usage.input_tokens": 50_548,
...     "gen_ai.usage.cache_read.input_tokens": 45_000,
...     "gen_ai.usage.cache_write.input_tokens": 3_500,
...     "gen_ai.usage.output_tokens": 1_024,
... })
>>> round(added["gen_ai.usage.cost.amount"], 9), added["gen_ai.usage.cost.source"]
(0.048129, 'local')
>>> added["gen_ai.usage.cost.pricing.id"]
'llm-prices'
"""

from ._processor import COST_BUCKETS, CostPricer, CostSpanExporter, CostSpanProcessor
from ._rules import Tokens
from ._sources import GenAIPricesSource, LiteLLMSource, LLMPricesSource, PriceSource, Priced, Request

__version__ = "0.1.1"

__all__ = [
    "COST_BUCKETS",
    "CostPricer",
    "CostSpanExporter",
    "CostSpanProcessor",
    "GenAIPricesSource",
    "LLMPricesSource",
    "LiteLLMSource",
    "PriceSource",
    "Priced",
    "Request",
    "Tokens",
]
