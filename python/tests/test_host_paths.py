"""Host and platform paths in model ids (1.5.1).

Fireworks, Together AI, Groq, Bedrock, Vertex AI, Azure and OpenRouter sell other companies'
models at their own prices. An id that names one of them is priced at that host's own row,
or raises ``KeyError`` (``None`` from :func:`llm_prices.lookup`) when the table does not price
the model there. It is never priced with the model maker's first-party row.
"""

from __future__ import annotations

from decimal import Decimal

import pytest

from llm_prices import lookup, lookup_provider
from llm_prices.compat import tokencost as tc

THIRD_PARTY_UNPRICED = [
    # Fireworks
    "accounts/fireworks/models/deepseek-r1",
    "accounts/fireworks/models/deepseek-chat",
    "fireworks_ai/accounts/fireworks/models/deepseek-r1",
    "fireworks_ai/deepseek-chat",
    "fireworks/gpt-4o",
    # Together AI
    "together_ai/deepseek-chat",
    "together_ai/deepseek-ai/DeepSeek-R1",
    "together/claude-sonnet-4-6",
    "together_ai/openai/gpt-4o",
    # Groq
    "groq/gpt-4o",
    "groq/deepseek-chat",
    # Bedrock, Vertex, Azure, OpenRouter: only the makers whose list price they charge
    "bedrock/deepseek-r1",
    "bedrock/deepseek-chat",
    "bedrock/gpt-4o",
    "vertex_ai/deepseek-chat",
    "vertex_ai/gpt-4o",
    "azure/deepseek-r1",
    "azure/claude-sonnet-4-6",
    "openrouter/acme/not-a-model",
]


@pytest.mark.parametrize("model", THIRD_PARTY_UNPRICED)
def test_third_party_paths_do_not_take_the_makers_price(model):
    assert lookup(model) is None
    with pytest.raises(KeyError, match=r"^'Model .* is not implemented"):
        tc.calculate_cost_by_tokens(1_000, model, "input")
    with pytest.raises(KeyError, match="is not implemented"):
        tc.calculate_completion_cost("hi", model)


def test_fireworks_deepseek_r1_is_not_deepseeks_price():
    # The reported bug: this used to cost DeepSeek's first-party $0.30 / $1.20.
    with pytest.raises(KeyError):
        tc.calculate_cost_by_tokens(1_000_000, "accounts/fireworks/models/deepseek-r1", "input")
    with pytest.raises(KeyError):
        tc.calculate_cost_by_tokens(1_000_000, "accounts/fireworks/models/deepseek-r1", "output")


@pytest.mark.parametrize(
    "model,provider,row",
    [
        ("accounts/fireworks/models/gpt-oss-120b", "fireworks", "accounts/fireworks/models/gpt-oss-120b"),
        ("fireworks_ai/accounts/fireworks/models/kimi-k3", "fireworks", "accounts/fireworks/models/kimi-k3"),
        ("fireworks_ai/deepseek-v4p1-flash", "fireworks", "accounts/fireworks/models/deepseek-v4p1-flash"),
        ("together_ai/openai/gpt-oss-120b", "together", "openai/gpt-oss-120b"),
        ("together_ai/moonshotai/Kimi-K3", "together", "moonshotai/Kimi-K3"),
        ("together/deepseek-ai/DeepSeek-V4.1-Flash", "together", "deepseek-ai/DeepSeek-V4.1-Flash"),
        ("groq/openai/gpt-oss-120b", "groq", "openai/gpt-oss-120b"),
        ("groq/qwen/qwen3.8-27b", "groq", "qwen/qwen3.8-27b"),
    ],
)
def test_host_paths_resolve_to_the_hosts_own_row(model, provider, row):
    p = lookup(model)
    assert (p.provider, p.model) == (provider, row)
    assert tc.calculate_cost_by_tokens(1_000_000, model, "input") == Decimal(p.rate_str("input"))


def test_the_same_model_at_two_hosts_keeps_two_prices():
    together = tc.calculate_cost_by_tokens(1_000_000, "together_ai/moonshotai/Kimi-K3", "input")
    fireworks = tc.calculate_cost_by_tokens(1_000_000, "accounts/fireworks/models/kimi-k3", "input")
    assert together == Decimal(lookup_provider("together", "moonshotai/Kimi-K3").rate_str("input"))
    assert fireworks == Decimal(lookup_provider("fireworks", "accounts/fireworks/models/kimi-k3").rate_str("input"))


@pytest.mark.parametrize(
    "model,provider",
    [
        ("anthropic/claude-sonnet-4-6", "anthropic"),
        ("openai/gpt-4o", "openai"),
        ("openai/gpt-4o-2024-11-20", "openai"),
        ("gemini/gemini-2.5-flash", "google"),
        ("google/gemini-2.5-flash", "google"),
        ("models/gemini-2.5-flash", "google"),
        ("deepseek/deepseek-chat", "deepseek"),
        ("mistral/mistral-large-latest", "mistral"),
        ("xai/grok-4.7", "xai"),
        # Platforms that charge the maker's list price keep resolving to the maker's row.
        ("bedrock/us.anthropic.claude-sonnet-4-20250514-v1:0", "anthropic"),
        ("vertex_ai/gemini-2.5-pro", "google"),
        ("vertex_ai/claude-opus-4-1@20250805", "anthropic"),
        ("azure/gpt-4o", "openai"),
        ("openrouter/anthropic/claude-sonnet-4-6", "anthropic"),
    ],
)
def test_first_party_prefixes_still_resolve(model, provider):
    p = lookup(model)
    assert p is not None and p.provider == provider, model
    assert tc.calculate_cost_by_tokens(1_000, model, "input") > 0


@pytest.mark.parametrize(
    "model",
    ["anthropic/claude-3-opus-20240229", "openai/gpt-4", "gemini/gemini-1.5-pro", "bedrock/claude-3-haiku-20240307", "openrouter/anthropic/claude-3-opus-20240229"],
)
def test_first_party_prefixes_find_legacy_rows(model):
    assert tc.calculate_cost_by_tokens(1_000, model, "input") > 0


@pytest.mark.parametrize(
    "model",
    ["anthropic/gpt-4o", "openai/claude-sonnet-4-6", "deepseek/gpt-4", "anthropic/gpt-4", "openai/deepseek-r1"],
)
def test_another_makers_prefix_does_not_resolve(model):
    assert lookup(model) is None
    with pytest.raises(KeyError, match="is not implemented"):
        tc.calculate_cost_by_tokens(1_000, model, "input")


@pytest.mark.parametrize(
    "model",
    ["not-a-model", "huggingface/deepseek-r1", "ollama/llama3", "deepseek-ai/DeepSeek-R1", "acme/gpt-4o", "accounts/fireworks/models/not-a-model"],
)
def test_unknown_ids_raise_key_error(model):
    assert lookup(model) is None
    with pytest.raises(KeyError, match="is not implemented"):
        tc.calculate_cost_by_tokens(1_000, model, "input")
    with pytest.raises(KeyError):
        tc.calculate_prompt_cost("hi", model)


def test_lookup_provider_does_not_cross_hosts():
    assert lookup_provider("deepseek", "accounts/fireworks/models/deepseek-chat") is None
    assert lookup_provider("deepseek", "together_ai/deepseek-chat") is None
    assert lookup_provider("groq", "fireworks_ai/accounts/fireworks/models/gpt-oss-120b") is None
    assert lookup_provider("openrouter", "groq/openai/gpt-oss-120b") is None
    assert lookup_provider("fireworks", "fireworks_ai/accounts/fireworks/models/gpt-oss-120b").provider == "fireworks"
    # A platform that charges the maker's list price: the maker's row.
    assert lookup_provider("openai", "azure/gpt-4.1").provider == "openai"
    assert lookup_provider("anthropic", "bedrock/anthropic.claude-sonnet-4-20250514-v1:0").provider == "anthropic"


@pytest.mark.parametrize(
    "model,provider,price",
    [
        # Bedrock's own price for DeepSeek-R1 (AWS price list, us-east-1), not DeepSeek's.
        ("bedrock/us.deepseek.r1-v1:0", "bedrock", (1.35, 5.4)),
        ("bedrock/deepseek.r1-v1:0", "bedrock", (1.35, 5.4)),
        ("bedrock/amazon.nova-pro-v1:0", "bedrock", (0.8, 3.2)),
        ("bedrock/meta.llama4-maverick-17b-instruct-v1:0", "bedrock", (0.24, 0.97)),
        # OpenRouter's own price from its model list.
        ("openrouter/deepseek/deepseek-r1-0528", "openrouter", (0.5, 2.15)),
        ("openrouter/perplexity/sonar-pro", "openrouter", (3.0, 15.0)),
        # Perplexity's Agent API price for the models it serves.
        ("perplexity/sonar", "perplexity", (0.25, 2.5)),
        ("perplexity/kimi-k3", "perplexity", (3.0, 15.0)),
    ],
)
def test_host_rows_price_their_own_paths(model, provider, price):
    p = lookup(model)
    assert p is not None and p.provider == provider
    assert (p.input, p.output) == price
    cost = tc.calculate_cost_by_tokens(1_000_000, model, "input")
    assert float(cost) == price[0]


def test_kiwiq_style_ids():
    # The ids rcortx/kiwiq passes to tokencost (llm_node.py map_model_to_tokencost_format).
    assert float(tc.calculate_cost_by_tokens(1_000_000, "bedrock/us.deepseek.r1-v1:0", "output")) == 5.4
    assert float(tc.calculate_cost_by_tokens(1_000_000, "perplexity/sonar", "input")) == 0.25
    # Sonar Pro, Sonar Reasoning Pro and Sonar Deep Research left the Sonar API on 2026-09-27;
    # their last listed prices come from the archived pricing page.
    assert float(tc.calculate_cost_by_tokens(1_000_000, "perplexity/sonar-pro", "input")) == 3.0
    assert float(tc.calculate_cost_by_tokens(1_000_000, "perplexity/sonar-deep-research", "output")) == 8.0
    assert float(tc.calculate_cost_by_tokens(1_000_000, "perplexity/sonar-reasoning-pro", "input")) == 2.0


def test_platform_rows_never_answer_a_bare_maker_id():
    # gpt-4 is a retired OpenAI id; OpenRouter still lists openai/gpt-4, but a bare id only
    # matches a platform row by its exact id.
    p = lookup("gpt-4")
    assert p is None or p.provider == "openai"
    assert lookup("meta-llama/llama-3.3-70b-instruct").provider == "openrouter"
    assert lookup("deepseek.r1-v1:0").provider == "bedrock"
    # A host's row still wins over a platform's for a maker-prefixed open-weight id.
    assert lookup("openai/gpt-oss-120b").provider == "groq"
