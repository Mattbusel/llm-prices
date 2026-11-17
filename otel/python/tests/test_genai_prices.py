"""GenAIPricesSource gives the costs genai-prices itself computes."""

from __future__ import annotations

import math

import pytest

genai_prices = pytest.importorskip("genai_prices")

from llm_prices_otel import CostPricer, GenAIPricesSource, Request, Tokens  # noqa: E402

CASES = [
    ("anthropic", "claude-sonnet-4-6", Tokens(input=2048, cache_read=45000, cache_write=3500, output=1024)),
    ("openai", "gpt-4o-mini", Tokens(input=200, cache_read=1000, output=300)),
    ("openai", "gpt-4o", Tokens(input=1000, output=500)),
    (None, "claude-sonnet-4-6", Tokens(input=1000, output=1000)),
]


@pytest.mark.parametrize("provider,model,t", CASES)
def test_matches_calc_price(provider, model, t):
    src = GenAIPricesSource()
    got = src.price(Request(provider=provider, models=(model,), tokens=t))
    assert not isinstance(got, str), got
    usage = genai_prices.Usage(input_tokens=t.input + t.cache_read + t.cache_write, cache_read_tokens=t.cache_read, cache_write_tokens=t.cache_write, output_tokens=t.output)
    want = genai_prices.calc_price(usage, model, provider_id={"openai": "openai", "anthropic": "anthropic", None: None}[provider])
    assert math.isclose(got.total, float(want.total_price), rel_tol=1e-12)


def test_through_the_pricer():
    added, reason = CostPricer(source=GenAIPricesSource()).attributes({
        "gen_ai.provider.name": "anthropic",
        "gen_ai.request.model": "claude-sonnet-4-6",
        "gen_ai.usage.input_tokens": 50548,
        "gen_ai.usage.cache_read.input_tokens": 45000,
        "gen_ai.usage.cache_write.input_tokens": 3500,
        "gen_ai.usage.output_tokens": 1024,
    })
    assert reason is None
    assert added["gen_ai.usage.cost.pricing.id"] == "genai-prices"
    assert added["gen_ai.usage.cost.pricing.version"] == f"genai-prices=={genai_prices.__version__}"
    assert added["gen_ai.usage.cost.amount"] > 0


def test_unpriced_requests():
    src = GenAIPricesSource()
    t = Tokens(input=10, output=10)
    assert src.price(Request(provider="openai", models=("no-such-model",), tokens=t)) == "unknown_model"
    assert src.price(Request(provider="openai", models=("gpt-4o",), tokens=t, batch=True)) == "missing_rate"
    assert src.price(Request(provider="openai", models=("gpt-4o",), tokens=t, tier="flex")) == "unpriced_tier"
