"""tokencost's own tests (tokencost 0.1.26, tests/test_costs.py), ported to
llm_prices.compat.tokencost.

Kept: the test names, the inputs, the token counts and the error types. Changed: the models are
ones the llm-prices table prices today (tokencost's tests use gpt-3.5 and gpt-4 rows that are
no longer on OpenAI's pricing page), and the expected costs are the current prices from
https://developers.openai.com/api/docs/pricing (gpt-4o: $2.50 input, $1.25 cached input, $10
output per 1M tokens; gpt-4o-mini: $0.15 / $0.075 / $0.60; text-embedding-3-small: $0.02).
The Claude cases need Anthropic's API, as in tokencost, and are left out.
"""

from decimal import Decimal

import pytest

from llm_prices.compat import tokencost as tc

try:
    import tiktoken  # noqa: F401

    HAVE_TIKTOKEN = True
except ImportError:  # pragma: no cover
    HAVE_TIKTOKEN = False

counting = pytest.mark.skipif(not HAVE_TIKTOKEN, reason="token counting needs tiktoken (pip install llm-prices[tokencost])")

# 15 tokens
MESSAGES = [
    {"role": "user", "content": "Hello"},
    {"role": "assistant", "content": "Hi there!"},
]

MESSAGES_WITH_NAME = [
    {"role": "user", "content": "Hello", "name": "John"},
    {"role": "assistant", "content": "Hi there!"},
]

# 4 tokens
STRING = "Hello, world!"


# 1
@counting
@pytest.mark.parametrize("model,expected_output", [("gpt-4o", 15), ("azure/gpt-4o", 15), ("gpt-4o-mini", 15), ("gpt-4.1", 15)])
def test_count_message_tokens(model, expected_output):
    assert tc.count_message_tokens(MESSAGES, model) == expected_output


# 2
@counting
@pytest.mark.parametrize("model,expected_output", [("gpt-4o", 17), ("azure/gpt-4o", 17), ("gpt-4o-mini", 17)])
def test_count_message_tokens_with_name(model, expected_output):
    """Notice: name 'John' appears"""
    assert tc.count_message_tokens(MESSAGES_WITH_NAME, model) == expected_output


# 3
def test_count_message_tokens_empty_input():
    """Empty input should raise a KeyError"""
    with pytest.raises(KeyError):
        tc.count_message_tokens("", "")


# 4
def test_count_message_tokens_invalid_model():
    """Invalid model should raise a KeyError"""
    with pytest.raises(KeyError):
        tc.count_message_tokens(MESSAGES, model="invalid_model")


# 5
@counting
@pytest.mark.parametrize("model,expected_output", [("gpt-4o", 4), ("gpt-4o-mini", 4), ("text-embedding-3-small", 4)])
def test_count_string_tokens(model, expected_output):
    assert tc.count_string_tokens(STRING, model=model) == expected_output
    assert tc.count_string_tokens("", model=model) == 0


# 6
@counting
def test_count_string_invalid_model():
    assert tc.count_string_tokens(STRING, model="invalid model") == 4


# 7
@counting
@pytest.mark.parametrize(
    "prompt,model,expected_output",
    [
        (MESSAGES, "gpt-4o", Decimal("0.0000375")),
        (MESSAGES, "azure/gpt-4o", Decimal("0.0000375")),
        (MESSAGES, "gpt-4o-mini", Decimal("0.00000225")),
        (STRING, "text-embedding-3-small", Decimal("0.00000008")),
    ],
)
def test_calculate_prompt_cost(prompt, model, expected_output):
    assert tc.calculate_prompt_cost(prompt, model) == expected_output


# 8
def test_invalid_prompt_format():
    with pytest.raises(TypeError):
        tc.calculate_prompt_cost({"role": "user", "content": "invalid message type"}, model="gpt-4o")


# 9
@counting
@pytest.mark.parametrize(
    "prompt,model,expected_output",
    [
        (STRING, "gpt-4o", Decimal("0.00004")),
        (STRING, "azure/gpt-4o", Decimal("0.00004")),
        (STRING, "gpt-4o-mini", Decimal("0.0000024")),
        (STRING, "text-embedding-3-small", 0),
    ],
)
def test_calculate_completion_cost(prompt, model, expected_output):
    assert tc.calculate_completion_cost(prompt, model) == expected_output


# 10
def test_calculate_cost_invalid_model():
    with pytest.raises(KeyError):
        tc.calculate_prompt_cost(STRING, model="invalid_model")


# 11
def test_calculate_invalid_input_types():
    with pytest.raises(KeyError):
        tc.calculate_prompt_cost(STRING, model="invalid_model")
    with pytest.raises(KeyError):
        tc.calculate_completion_cost(STRING, model="invalid_model")
    with pytest.raises(KeyError):
        tc.calculate_prompt_cost(MESSAGES[0], model="invalid_model")


# 12
@pytest.mark.parametrize(
    "num_tokens,model,token_type,expected_output",
    [
        (10, "gpt-4o", "input", Decimal("0.000025")),
        (5, "gpt-4o", "output", Decimal("0.00005")),
        (10, "gpt-4o-mini", "input", Decimal("0.0000015")),
        (100, "gpt-4o", "cached", Decimal("0.000125")),
    ],
)
def test_calculate_cost_by_tokens(num_tokens, model, token_type, expected_output):
    assert tc.calculate_cost_by_tokens(num_tokens, model, token_type) == expected_output


# 13
def test_calculate_cached_tokens_cost():
    model = "gpt-4o"
    num_tokens = 1000
    cache_cost_per_token = tc.TOKEN_COSTS[model]["cache_read_input_token_cost"]
    expected_cost = Decimal(str(cache_cost_per_token)) * Decimal(num_tokens)
    actual_cost = tc.calculate_cost_by_tokens(num_tokens, model, "cached")
    assert actual_cost == expected_cost
    assert actual_cost > 0


# Beyond tokencost's suite: what moving over relies on.


def test_ids_resolve_like_llm_prices_lookup():
    # Dated, prefixed and Bedrock ids reach the same row as the bare id.
    base = tc.calculate_cost_by_tokens(1_000_000, "claude-sonnet-4-6", "input")
    assert base == Decimal("3")
    assert tc.calculate_cost_by_tokens(1_000_000, "anthropic.claude-sonnet-4-6", "input") == base
    assert tc.calculate_cost_by_tokens(1_000_000, "gpt-4o-2024-08-06", "output") == Decimal("10")
    assert tc.calculate_cost_by_tokens(1_000_000, "gemini/gemini-2.5-pro", "input") == Decimal("1.25")
    assert tc.calculate_cost_by_tokens(1_000_000, "gemini-2.5-pro", "input") == Decimal("1.25")


def test_configure_model_and_patterns():
    tc.configure_model("my-model", 0.003, 0.015)
    assert tc.calculate_cost_by_tokens(1000, "my-model", "input") == Decimal("0.003")
    tc.register_model_pattern("my-org/*", 0.001, 0.002)
    assert tc.calculate_cost_by_tokens(1000, "my-org/anything", "output") == Decimal("0.002")


def test_invalid_token_type():
    with pytest.raises(ValueError):
        tc.calculate_cost_by_tokens(1, "gpt-4o", "reasoning")  # type: ignore[arg-type]


def test_missing_price_is_a_key_error():
    # gpt-5.5-pro lists no cached input price.
    with pytest.raises(KeyError):
        tc.calculate_cost_by_tokens(1, "gpt-5.5-pro", "cached")


def test_refresh_fetches_nothing():
    assert tc.refresh_prices() is tc.TOKEN_COSTS


@counting
def test_all_costs_and_tokens():
    out = tc.calculate_all_costs_and_tokens(STRING, "How may I assist you today?", "gpt-4o")
    assert out["prompt_tokens"] == 4
    assert out["prompt_cost"] == Decimal("0.00001")
    assert out["completion_tokens"] == 7
    assert out["completion_cost"] == Decimal("0.00007")
