"""The conformance cases shared with the Go Collector processor (otel/conformance/cases.json)."""

from __future__ import annotations

import json
import math
from decimal import Decimal
from pathlib import Path
from typing import Any, Dict

import pytest

import llm_prices
from llm_prices_otel import CostPricer

CASES = Path(__file__).resolve().parents[2] / "conformance" / "cases.json"

pytestmark = pytest.mark.skipif(not CASES.exists(), reason="the conformance cases are only in the repository")


def _load() -> Any:
    # parse_float keeps 500.0 a float and parse_int keeps ints: the same typing the Go suite uses.
    return json.loads(CASES.read_text(encoding="utf-8"))["cases"] if CASES.exists() else []


def _cases():
    return [pytest.param(c, id=c["name"]) for c in _load() if c.get("only") != "collector"]


@pytest.mark.parametrize("case", _cases())
def test_case(case: Dict[str, Any]) -> None:
    pricer = CostPricer(**case.get("config", {}))
    attrs = dict(case["attributes"])
    added, reason = pricer.attributes(attrs)
    after = {**attrs, **added}

    if case.get("expect_unchanged"):
        assert added == {}
        return
    if "expect_unpriced" in case:
        assert reason == case["expect_unpriced"]
        assert after["llm_prices.unpriced"] == case["expect_unpriced"]
        assert "gen_ai.usage.cost.amount" not in added
    for k, want in case.get("expect", {}).items():
        assert k in after, f"missing {k}: {after}"
        got = after[k]
        if isinstance(want, float):
            assert isinstance(got, float), k
            assert math.isclose(got, want, rel_tol=1e-12), (k, got, want)
        else:
            if want == "PRICES_VERSION":
                want = llm_prices.PRICES_VERSION
            assert got == want, k
    for k in case.get("expect_absent", []):
        assert k not in after, k


def test_every_case_runs_somewhere() -> None:
    cases = _load()
    assert len(cases) >= 25
    assert {c.get("only", "both") for c in cases} <= {"both", "collector"}


def test_amounts_match_exact_decimals() -> None:
    # The float the pricer writes is the exact decimal cost rounded once, for every priced case.
    for c in _load():
        want = c.get("expect", {}).get("gen_ai.usage.cost.amount")
        if isinstance(want, float) and c.get("only") != "collector":
            added, _ = CostPricer(**c.get("config", {})).attributes(c["attributes"])
            assert abs(Decimal(repr(added["gen_ai.usage.cost.amount"])) - Decimal(repr(want))) <= Decimal("1e-15"), c["name"]
