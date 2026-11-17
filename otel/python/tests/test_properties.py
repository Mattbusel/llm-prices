"""Property tests: span attributes are untrusted input (any instrumentation, any version), so the
pricer must never raise, never write a negative, NaN or zero-for-missing cost, and must be
monotonic in every token count."""

from __future__ import annotations

import math

import pytest

hypothesis = pytest.importorskip("hypothesis")
from hypothesis import given, settings  # noqa: E402
from hypothesis import strategies as st  # noqa: E402

from llm_prices_otel import CostPricer  # noqa: E402
from llm_prices_otel import _rules as r  # noqa: E402

KEYS = (
    r.INPUT_KEYS + r.OUTPUT_KEYS + r.CACHE_READ_KEYS + r.CACHE_WRITE_KEYS + r.REASONING_KEYS
    + r.MODEL_KEYS + r.PROVIDER_KEYS + r.TIER_KEYS + r.SERVER_KEYS + ("llm_prices.batch", "gen_ai.usage.cost.amount")
)
MODELS = ["gpt-4o", "claude-sonnet-4-6", "gemini-2.5-pro", "o3", "openai/gpt-oss-120b", "us.anthropic.claude-sonnet-4-20250514-v1:0", "x", ""]
PROVIDERS = ["openai", "anthropic", "gcp.gemini", "aws.bedrock", "groq", "ollama", "OpenAI", "x_ai", ""]

values = st.one_of(
    st.integers(min_value=-(2**70), max_value=2**70),
    st.floats(allow_nan=True, allow_infinity=True),
    st.text(max_size=12),
    st.booleans(),
    st.sampled_from(MODELS + PROVIDERS + ["flex", "priority", "scale", "openrouter.ai", "api.groq.com"]),
    st.lists(st.integers(), max_size=2),
)


@settings(max_examples=3000, deadline=None)
@given(st.dictionaries(st.sampled_from(KEYS), values, max_size=12), st.sampled_from(["flag", "error_attribute"]), st.booleans())
def test_never_raises_and_never_writes_a_bad_cost(attrs, on_unpriced, openinference):
    added, reason = CostPricer(on_unpriced=on_unpriced, openinference=openinference, breakdown=True).attributes(attrs)
    if "gen_ai.usage.cost.amount" in attrs:
        assert added == {}
        return
    amount = added.get("gen_ai.usage.cost.amount")
    if amount is not None:
        assert isinstance(amount, float) and math.isfinite(amount) and amount >= 0
        assert reason is None and "llm_prices.unpriced" not in added
        for k, v in added.items():
            if isinstance(v, float):
                assert math.isfinite(v) and v >= -1e-12, k
    if reason is not None:
        assert added["llm_prices.unpriced"] == reason
        assert "gen_ai.usage.cost.amount" not in added


counts = st.integers(min_value=0, max_value=3_000_000)


@settings(max_examples=1500, deadline=None)
@given(st.sampled_from(["gpt-4o", "claude-sonnet-4-6", "gemini-2.5-flash", "o3"]), counts, counts, counts, counts, st.sampled_from(["input", "output", "read"]), st.integers(min_value=1, max_value=100_000))
def test_more_tokens_never_cost_less(model, uncached, read, write, out, which, extra):
    pricer = CostPricer()

    def cost(u, rd, o):
        attrs = {
            "gen_ai.request.model": model,
            "gen_ai.usage.input_tokens": u + rd + write,
            "gen_ai.usage.cache_read.input_tokens": rd,
            "gen_ai.usage.cache_write.input_tokens": write,
            "gen_ai.usage.output_tokens": o,
        }
        added, reason = pricer.attributes(attrs)
        assert reason is None, reason
        return added["gen_ai.usage.cost.amount"]

    base = cost(uncached, read, out)
    more = {"input": (uncached + extra, read, out), "output": (uncached, read, out + extra), "read": (uncached, read + extra, out)}[which]
    if model == "gemini-2.5-pro":
        return  # the long-context switch reprices every token
    assert cost(*more) >= base


@settings(max_examples=500, deadline=None)
@given(st.sampled_from(["gpt-4o", "claude-sonnet-4-6", "o3"]), counts, counts)
def test_string_int_and_whole_double_counts_agree(model, i, o):
    p = CostPricer()
    base = {"gen_ai.request.model": model}
    a, _ = p.attributes({**base, "gen_ai.usage.input_tokens": i, "gen_ai.usage.output_tokens": o})
    b, _ = p.attributes({**base, "gen_ai.usage.input_tokens": str(i), "gen_ai.usage.output_tokens": float(o)})
    assert a == b
