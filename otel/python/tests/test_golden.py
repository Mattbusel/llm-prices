"""Real spans from OpenLLMetry, OpenInference, Logfire and rig (otel/conformance/golden) get the
costs in expected.json: the same files and expectations the Go processor's tests use."""

from __future__ import annotations

import json
import math
from pathlib import Path

import pytest

import llm_prices
from llm_prices_otel import CostPricer

from otlp import all_spans

GOLDEN = Path(__file__).resolve().parents[2] / "conformance" / "golden"
EXPECTED = GOLDEN / "expected.json"

pytestmark = pytest.mark.skipif(not EXPECTED.exists(), reason="the golden corpus is only in the repository")


def _suites():
    if not EXPECTED.exists():
        return []
    return [pytest.param(name, s, id=name) for name, s in json.loads(EXPECTED.read_text(encoding="utf-8"))["suites"].items()]


@pytest.mark.parametrize("name,suite", _suites())
def test_golden_suite(name, suite):
    spans = all_spans(GOLDEN / suite["file"])
    assert len(spans) == len(suite["spans"]), name
    pricer = CostPricer(breakdown=True)
    for (scope, span_name, attrs), want in zip(spans, suite["spans"]):
        added, reason = pricer.attributes(attrs)
        assert reason is None, (name, span_name, reason)
        assert math.isclose(added["gen_ai.usage.cost.amount"], want["amount"], rel_tol=1e-12), (name, span_name, added)
        assert added["llm_prices.price.row"] == want["row"]
        assert added["gen_ai.usage.cost.source"] == "local"
        assert added["gen_ai.usage.cost.pricing.version"] == llm_prices.PRICES_VERSION
