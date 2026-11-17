"""The LiteLLM price source against LiteLLM itself: litellm_reference.json holds the costs
LiteLLM's generic_cost_per_token computed for a sample of its price map (make_reference.py)."""

from __future__ import annotations

import json
import math
from pathlib import Path

import pytest

from llm_prices_otel import LiteLLMSource, Request, Tokens
from llm_prices_otel._sources import cost_litellm

REF = Path(__file__).resolve().parents[2] / "conformance" / "litellm" / "litellm_reference.json"

pytestmark = pytest.mark.skipif(not REF.exists(), reason="the LiteLLM reference is only in the repository")


def _doc():
    return json.loads(REF.read_text(encoding="utf-8"))


def _tokens(c):
    return Tokens(input=c["input"], cache_read=c["cache_read"], cache_write=c["cache_write"], output=c["output"], reasoning=c["reasoning"])


def test_costs_match_litellm():
    doc = _doc()
    mismatches = []
    for c in doc["cases"]:
        got = cost_litellm(doc["entries"][c["key"]], c["provider"], _tokens(c), False, c["tier"])
        if isinstance(got, str) or not math.isclose(got.total, c["total"], rel_tol=1e-9, abs_tol=1e-15):
            mismatches.append((c["key"], c["tier"], c["input"], c["cache_read"], c["output"], got if isinstance(got, str) else got.total, c["total"]))
    assert not mismatches, f"{len(mismatches)} of {len(doc['cases'])} differ from LiteLLM, first: {mismatches[:5]}"
    assert len(doc["cases"]) >= 500


def test_lookup_through_the_source():
    doc = _doc()
    src = LiteLLMSource(json.dumps(doc["entries"]).encode())
    found = 0
    for c in doc["cases"]:
        if c["tier"] is not None or c["cache_read"] or c["input"] != 1000:
            continue
        key = c["key"]
        lp = c["litellm_provider"]
        model = key.split("/", 1)[1] if key.startswith(lp + "/") else key
        got = src.price(Request(provider=c["provider"], models=(model,), tokens=_tokens(c)))
        assert not isinstance(got, str), (key, got)
        # LiteLLM's lookup prefers "<provider>/<model>" when both keys exist, and so does the source.
        if got.row == "litellm/" + key:
            assert math.isclose(got.total, c["total"], rel_tol=1e-9, abs_tol=1e-15), key
            found += 1
    assert found >= 100
