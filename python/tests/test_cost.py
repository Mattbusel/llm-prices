"""Ports of the Rust crate's cost tests (crate/src/lib.rs, crate/src/decimal.rs and the crate
docs), plus checks that the generated table matches prices.json."""

from __future__ import annotations

import dataclasses
import decimal
import hashlib
import json
import math
from decimal import Decimal
from pathlib import Path

import pytest

import llm_prices
from llm_prices import CostBreakdown, DecimalCostBreakdown, Price, Usage, all_prices, lookup

D = Decimal
COLUMNS = ("input", "output", "cache_read", "batch_input", "batch_output")
PRICES_JSON = Path(__file__).resolve().parents[2] / "prices.json"


def close(a: float, b: float, eps: float = 1e-12) -> bool:
    return abs(a - b) < eps


# -- the generated table ------------------------------------------------------------------------


@pytest.mark.skipif(not PRICES_JSON.exists(), reason="prices.json is only in the repository")
def test_prices_version_is_the_sha256_of_prices_json():
    text = PRICES_JSON.read_bytes().replace(b"\r\n", b"\n")
    assert llm_prices.PRICES_VERSION == "sha256:" + hashlib.sha256(text).hexdigest()


def test_prices_version_shape():
    v = llm_prices.PRICES_VERSION
    assert v.startswith("sha256:") and len(v) == 71
    assert all(c in "0123456789abcdef" for c in v[7:])


@pytest.mark.skipif(not PRICES_JSON.exists(), reason="prices.json is only in the repository")
def test_table_matches_prices_json():
    doc = json.loads(PRICES_JSON.read_text(encoding="utf-8"))
    assert llm_prices.GENERATED == doc["generated"]
    assert len(all_prices()) == len(doc["models"])

    def same_tier(p, m):
        for f in COLUMNS:
            assert getattr(p, f) == m[f], f"{m['model']}.{f}"
        assert dict(p.cache_write) == {k: v for k, v in (m["cache_write"] or {}).items() if v is not None}

    for p, m in zip(all_prices(), doc["models"]):
        assert (p.provider, p.model, list(p.aliases), p.status) == (m["provider"], m["model"], m["aliases"], m["status"])
        assert (p.source_url, p.checked, p.name, p.note) == (m["source_url"], m["checked"], m["name"], m["note"])
        same_tier(p, m)
        if m["long_context"] is None:
            assert p.long_context is None
        else:
            assert p.long_context.threshold_tokens == m["long_context"]["threshold_tokens"]
            same_tier(p.long_context, m["long_context"])


def test_exact_table_matches_prices():
    for p in all_prices():
        for f in COLUMNS:
            value, text = p.rate(f), p.rate_str(f)
            assert (value is None) == (text is None), f"{p.model} {f}"
            if text is not None:
                assert float(text) == value
                assert "e" not in text and not text.startswith(".")
                assert p.rate_decimal(f) == D(text)
        for ttl in p.cache_write:
            text = p.rate_str("cache_write", ttl)
            assert text is not None and float(text) == p.cache_write[ttl]
            assert p.rate_decimal("cache_write", ttl) == D(text)
        lc = p.long_context
        if lc is None:
            assert p.long_context_rate_str("input") is None
        else:
            for f in COLUMNS:
                text = p.long_context_rate_str(f)
                assert (lc.rate(f) is None) == (text is None)
                if text is not None:
                    assert float(text) == lc.rate(f)
            for ttl in lc.cache_write:
                assert p.long_context_rate_str("cache_write", ttl) is not None


def test_rate_str_needs_an_unchanged_table_row():
    p = lookup("gpt-4o")
    assert p.rate_str("input") == "2.5"
    assert p.rate_str("cache_write", "5m") is None
    renamed = dataclasses.replace(p, model="my-model")
    assert renamed.rate_str("input") is None
    repriced = dataclasses.replace(p, output=11.0)
    assert repriced.rate_str("output") is None
    assert repriced.rate_str("input") == "2.5"
    h = lookup("claude-haiku-5-5")
    assert h.rate_str("input") == "0.1"
    assert h.rate_str("cache_write", "5m") == "0.125"
    assert h.long_context_rate_str("input") == "0.5"
    assert dataclasses.replace(h, input=0.2).rate_str("input") is None


def test_rate_names_are_checked():
    p = lookup("claude-sonnet-4-6")
    assert p.rate("output") == 15.0
    assert p.rate("cache_write", "1h") == 6.0
    with pytest.raises(ValueError):
        p.rate("cache_write")
    with pytest.raises(ValueError):
        p.rate("input", "5m")
    with pytest.raises(ValueError):
        p.rate("cached_input")  # type: ignore[arg-type]


def test_rows_are_immutable_and_hashable():
    p = lookup("gpt-4o")
    with pytest.raises(dataclasses.FrozenInstanceError):
        p.input = 1.0  # type: ignore[misc]
    with pytest.raises(TypeError):
        p.cache_write["5m"] = 1.0  # type: ignore[index]
    assert len({hash(q) for q in all_prices()}) > 100
    assert p in set(all_prices())
    assert llm_prices.all() is all_prices()
    assert "all" not in llm_prices.__all__


# -- f64-style costs (crate/src/lib.rs) ---------------------------------------------------------


def test_crate_doc_anthropic_example():
    usage = Usage(input=2_048, cache_read=45_000, cache_write_5m=3_500, output=1_024)
    c = lookup("claude-sonnet-4-6").cost_usage(usage)
    assert isinstance(c, CostBreakdown)
    assert close(c.input, 0.006_144)
    assert close(c.cache_read, 0.013_5)
    assert close(c.cache_write, 0.013_125)
    assert close(c.output, 0.015_36)
    assert close(c.total, 0.048_129)
    assert not c.long_context


def test_crate_doc_openai_long_context_example():
    prompt_tokens, cached_tokens, completion_tokens = 300_000, 200_000, 4_000
    usage = Usage(input=prompt_tokens - cached_tokens, cache_read=cached_tokens, output=completion_tokens)
    c = lookup("gpt-5.4").cost_usage(usage)
    assert c.long_context
    assert close(c.input, 0.5)
    assert close(c.cache_read, 0.1)
    assert close(c.output, 0.09)
    assert close(c.total, 0.69)
    assert close(lookup("gpt-5.4").cost(100_000, 4_000), 0.31)
    # A mapping works as well as a Usage.
    assert lookup("gpt-5.4").cost_usage({"input": 100_000, "cache_read": 200_000, "output": 4_000}) == c


def test_simple_cost():
    assert close(lookup("claude-haiku-5-5").cost(10_000, 2_000), 0.002)


def cost_1_0(p: Price, input_tokens: int, output_tokens: int):
    """The Rust crate's 1.0 ``cost``, which ``cost`` must still equal bit for bit."""
    lc = p.long_context
    if lc is not None and input_tokens > lc.threshold_tokens:
        i, o = lc.input, lc.output
    else:
        i, o = p.input, p.output
    if i is None or o is None:
        return None
    return (float(input_tokens) * i + float(output_tokens) * o) / 1_000_000.0


def test_cost_agrees_with_cost_usage_and_1_0():
    counts = [0, 1, 999, 100_000, 100_001, 200_000, 200_001, 272_000, 272_001, 1_000_000, 12_345_678]
    for p in all_prices():
        for i in counts:
            for o in counts:
                c = p.cost_usage(Usage(input=i, output=o))
                total = None if c is None else c.total
                assert p.cost(i, o) == total, f"{p.model} {i} {o}"
                assert p.cost(i, o) == cost_1_0(p, i, o), f"{p.model} {i} {o}"


def test_long_context_counts_cached_prompt_tokens():
    h = lookup("claude-haiku-5-5")
    at = Usage(input=60_000, cache_read=40_000)
    c = h.cost_usage(at)
    assert not c.long_context
    assert abs(c.total - (60_000 * 0.1 + 40_000 * 0.01) / 1e6) < 1e-15
    over = dataclasses.replace(at, cache_write_1h=1)
    c = h.cost_usage(over)
    assert c.long_context
    assert abs(c.input - 60_000 * 0.5 / 1e6) < 1e-15
    assert abs(c.cache_read - 40_000 * 0.05 / 1e6) < 1e-15
    assert abs(c.cache_write - 1.0 / 1e6) < 1e-15
    assert abs(c.total - (c.input + c.cache_read + c.cache_write + c.output)) < 1e-15


def test_cache_read_falls_back_to_input_price():
    p = lookup("gpt-5.5-pro")
    assert p.cache_read is None
    c = p.cost_usage(Usage(cache_read=100_000))
    assert c.cache_read == 3.0
    assert c.input == 0.0
    c = p.cost_usage(Usage(cache_read=1_000_000))
    assert c.long_context
    assert c.cache_read == 60.0


def test_batch_and_cache_writes():
    s = lookup("claude-sonnet-4-6")
    u = Usage(cache_write_5m=1_000_000, cache_write_1h=1_000_000)
    assert s.cost_usage(u).cache_write == 3.75 + 6.0
    c = s.cost_usage(dataclasses.replace(u, batch=True))
    assert c.cache_write == (3.75 + 6.0) / 2.0
    pro = lookup("gpt-5.5-pro")
    assert pro.cost_usage(Usage(input=272_001, batch=True)) is None
    assert pro.cost_usage(Usage(input=272_000, batch=True)) is not None
    assert lookup("text-embedding-3-small").cost_usage(Usage()) is None


def test_batch_example():
    haiku = lookup("claude-haiku-4-5")
    u = Usage(input=10_000, cache_read=90_000, output=1_000, batch=True)
    c = haiku.cost_usage(u)
    assert close(c.input, 0.005)
    assert close(c.cache_read, 0.0045)
    assert close(c.output, 0.0025)
    assert lookup("gpt-3.5-turbo").cost_usage(u) is None
    assert lookup("gpt-6-sol").cost_usage(Usage(input=1, cache_write_5m=1)) is None


def test_other_cache_lifetimes_are_added_by_hand():
    p = lookup("gpt-6-sol")
    assert p.cache_write["30m"] == 2.5
    c = p.cost_usage(Usage(input=10_000, output=1_000))
    total = c.total + 50_000 * p.cache_write["30m"] / 1_000_000
    assert close(total, 0.155)


def test_long_context_cost():
    h = lookup("claude-haiku-5-5")
    assert close(h.cost(100_000, 0), 0.01)
    assert close(h.cost(100_001, 0), 100_001 * 0.5 / 1e6)
    assert h.cache_write["1h"] == 0.2


def test_xai_long_context_starts_at_200k():
    g = lookup("grok-4.7")
    assert g.long_context.threshold_tokens == 199_999
    assert g.cost_usage(Usage(input=200_000)).long_context
    assert not g.cost_usage(Usage(input=199_999)).long_context


def test_usage_validation():
    assert Usage(input=1_000, output=200).prompt_tokens() == 1_000
    assert Usage(input=1, cache_read=2, cache_write_5m=3, cache_write_1h=4).prompt_tokens() == 10
    with pytest.raises(ValueError):
        Usage(input=-1)
    with pytest.raises(TypeError):
        Usage(input=1.5)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        Usage(output=True)
    with pytest.raises(TypeError):
        lookup("gpt-4o").cost_usage(42)  # type: ignore[arg-type]
    with pytest.raises(TypeError):
        lookup("gpt-4o").cost_usage({"inputs": 1})


def test_hand_built_price():
    p = Price(provider="acme", model="m1", input=1.0, output=2.0, cache_read=0.1, cache_write={"5m": 1.25})
    assert p.provider_kind() is None
    assert p.rate_str("input") is None
    assert p.rate_decimal("input") == D("1")
    c = p.cost_usage(Usage(input=1_000_000, output=1_000_000, cache_read=1_000_000, cache_write_5m=1_000_000))
    assert c.total == 1.0 + 2.0 + 0.1 + 1.25
    assert p.cost(1_000_000, 0, exact=True) == D(1)


# -- exact decimal costs (crate/src/decimal.rs) ------------------------------------------------


def test_rate_decimal_is_the_source_text():
    assert lookup("gpt-4o-mini").rate_decimal("cache_read") == D("0.075")
    for p in all_prices():
        for f in COLUMNS:
            text = p.rate_str(f)
            assert p.rate_decimal(f) == (None if text is None else D(text))


def test_decimal_doc_examples():
    assert lookup("claude-haiku-5-5").cost_decimal(10_000, 2_000) == D("0.002")
    assert lookup("claude-haiku-5-5").cost(10_000, 2_000, exact=True) == D("0.002")
    usage = Usage(input=2_048, cache_read=45_000, cache_write_5m=3_500, output=1_024)
    c = lookup("claude-sonnet-4-6").cost_usage_decimal(usage)
    assert isinstance(c, DecimalCostBreakdown)
    assert c.total == D("0.048129")
    assert c.cache_write == D("0.013125")
    assert lookup("claude-sonnet-4-6").cost_usage(usage, exact=True) == c
    # No trailing zeros, no exponent on whole numbers.
    assert str(c.total) == "0.048129"
    assert str(lookup("claude-opus-5-5").cost_decimal(1_000_000, 1_000_000)) == "24"


def test_decimal_agrees_with_cost_usage():
    counts = [0, 1, 999, 60_000, 100_001, 199_999, 200_000, 272_001, 1_000_000, 12_345_678]
    for p in all_prices():
        for i in counts:
            for r in (0, 40_000, 250_000):
                for batch in (False, True):
                    u = Usage(input=i, cache_read=r, output=i // 3, batch=batch)
                    f, d = p.cost_usage(u), p.cost_usage_decimal(u)
                    assert (f is None) == (d is None), f"{p.model} {u}"
                    if f is not None and d is not None:
                        assert f.long_context == d.long_context
                        dt = float(d.total)
                        assert abs(f.total - dt) <= 1e-9 * max(abs(f.total), 1.0), f"{p.model} {u}"
                        # Summed at 60 digits: a batch row whose batch/standard ratio does not
                        # terminate (OpenRouter's meta/muse-glimmer-30b, 0.24/0.35) has 28-digit
                        # cache-read costs, and the default context would round their sum.
                        with decimal.localcontext(decimal.Context(prec=60)):
                            assert d.total == d.input + d.output + d.cache_read + d.cache_write


def test_exact_where_float_is_not():
    p = dataclasses.replace(lookup("gpt-4.1-nano"), input=0.1, output=0.2)
    assert p.cost(3, 3) != 9e-7  # the float total is 9.000000000000001e-07
    assert p.cost_decimal(3, 3) == D("0.0000009")
    h = lookup("claude-haiku-4-5")
    c = h.cost_usage_decimal(Usage(input=10_000, cache_read=90_000, output=1_000, batch=True))
    assert (c.input, c.cache_read, c.output, c.total) == (D("0.005"), D("0.0045"), D("0.0025"), D("0.012"))
    g = lookup("grok-4.7")
    assert g.cost_usage_decimal(Usage(cache_read=100_000, batch=True)).cache_read == D("0.04")
    c = g.cost_usage_decimal(Usage(cache_read=1_000_000, batch=True))
    assert c.long_context
    assert c.cache_read == D("0.8")


def test_decimal_long_context_and_missing_prices():
    h = lookup("claude-haiku-5-5")
    c = h.cost_usage_decimal(Usage(input=100_001))
    assert c.long_context
    assert c.input == D("0.0500005")
    assert lookup("gpt-6-sol").cost_usage_decimal(Usage(input=1, cache_write_5m=1)) is None
    assert lookup("text-embedding-3-small").cost_decimal(1, 0) is None
    # Huge counts stay exact instead of overflowing.
    big = 2**64 - 1
    c = lookup("o1-pro").cost_usage_decimal(Usage(input=big, output=big, cache_read=big))
    assert c is not None and c.total == c.input + c.output + c.cache_read


def test_decimal_ignores_the_active_context():
    import decimal

    with decimal.localcontext() as ctx:
        ctx.prec = 3
        assert lookup("claude-sonnet-4-6").cost_decimal(2_048, 1_024) == D("0.021504")
    assert math.isclose(float(lookup("claude-sonnet-4-6").cost_decimal(2_048, 1_024)), lookup("claude-sonnet-4-6").cost(2_048, 1_024))


def test_non_terminating_batch_discount_rounds_at_28_digits():
    third = Price(provider="acme", model="third", input=0.3, output=1.0, batch_input=0.1, batch_output=0.5, cache_read=0.3)
    assert third.cost_usage_decimal(Usage(cache_read=1_000_000, batch=True)).cache_read == D("0.1")
    seventh = Price(provider="acme", model="seventh", input=0.7, output=1.0, batch_input=0.1, batch_output=0.5, cache_read=1.0)
    # 1,000,000 x $1 x 0.1 / 0.7 per 1M = 1/7, rounded at 28 significant digits.
    c = seventh.cost_usage_decimal(Usage(cache_read=1_000_000, batch=True))
    assert c.cache_read == D("0.1428571428571428571428571429")


def test_version_matches_pyproject():
    import re

    pyproject = Path(__file__).resolve().parents[1] / "pyproject.toml"
    if not pyproject.exists():
        pytest.skip("pyproject.toml is not next to the tests")
    m = re.search(r'^version = "([^"]+)"$', pyproject.read_text(encoding="utf-8"), re.M)
    assert m is not None
    assert llm_prices.__version__ == m.group(1)
