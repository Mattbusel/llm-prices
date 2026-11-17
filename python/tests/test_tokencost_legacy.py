"""Retired and legacy ids in llm_prices.compat.tokencost (tokencost_legacy.json): priced from
archived official pricing pages, never shadowing a row the current table has."""
import json
from decimal import Decimal
from importlib import resources

import pytest

import llm_prices
from llm_prices.compat import tokencost as tc

DOC = json.loads(resources.files("llm_prices.compat").joinpath("tokencost_legacy.json").read_text(encoding="utf-8"))


@pytest.mark.parametrize(
    "model,inp,out",
    [
        ("gpt-4", "30", "60"),
        ("openai/gpt-4-32k", "60", "120"),
        ("chatgpt-4o-latest", "5", "15"),
        ("gpt-4o-search-preview", "2.5", "10"),
        ("o3-deep-research", "10", "40"),
        ("o4-mini-deep-research", "2", "8"),
        ("claude-3-opus-20240229", "15", "75"),
        ("claude-3-5-sonnet-latest", "3", "15"),
        ("claude-instant-1.2", "0.8", "2.4"),
        ("gemini/gemini-2.0-flash", "0.1", "0.4"),
        ("gemini-2.5-pro-preview-05-06", "1.25", "10"),
        ("mistral/mistral-large-2411", "2", "6"),
        ("xai/grok-2-1212", "2", "10"),
        ("command-r", "0.5", "1.5"),
        ("ft:gpt-3.5-turbo", "3", "6"),
    ],
)
def test_legacy_prices(model, inp, out):
    assert tc.calculate_cost_by_tokens(1_000_000, model, "input") == Decimal(inp)
    assert tc.calculate_cost_by_tokens(1_000_000, model, "output") == Decimal(out)


def test_redirects_bill_as_the_target():
    # xAI bills retired grok-3 at grok-4.3's price; DeepSeek serves deepseek-chat as flash.
    assert tc.calculate_cost_by_tokens(1_000_000, "xai/grok-3", "input") == Decimal("1.25")
    assert tc.TOKEN_COSTS["grok-3"]["llm_prices_redirect"] == "grok-4.3"
    flash = llm_prices.lookup("deepseek-flash")
    assert tc.calculate_cost_by_tokens(1_000_000, "deepseek/deepseek-v3", "input") == Decimal(str(flash.input))


def test_current_rows_win_over_legacy():
    for key in tc.LEGACY_MODELS:
        assert llm_prices.lookup(key) is None, key
    assert tc.calculate_cost_by_tokens(1_000_000, "claude-sonnet-4-20250514", "input") == Decimal("3")


def test_every_row_names_an_archived_official_source():
    for r in DOC["models"]:
        assert r["source_url"].startswith(("https://web.archive.org/web/", "https://cohere.com/", "https://docs.")), r["model"]
        assert len(r["source_date"]) == 10 and r["input"] is not None
    for r in DOC["redirects"].values():
        assert r["source_url"].startswith("https://")
    assert "—" not in json.dumps(DOC, ensure_ascii=False)


def test_legacy_entries_carry_source_and_retirement():
    e = tc.TOKEN_COSTS["gpt-4"]
    assert e["llm_prices_legacy"] is True and e["source"].startswith("https://web.archive.org/")
    assert tc.LEGACY_MODELS["gpt-4"]["model"] == "gpt-4-0613"
    assert tc.TOKEN_COSTS["gemini-1.5-pro-002"]["input_cost_per_token_above_128k_tokens"] == 2.5e-06
