"""Ports of the Rust crate's lookup tests (crate/tests/lookup.rs and the unit tests in
crate/src/lib.rs and crate/src/provider.rs): every id form the Rust ``lookup`` resolves must
resolve to the same row here."""

from __future__ import annotations

import re

import pytest

from llm_prices import Price, Provider, all_prices, lookup, lookup_provider
from llm_prices._core import _strip_date, _strip_version


def ids(p: Price):
    yield p.model
    yield from p.aliases


def undated(id: str) -> str:
    m = re.fullmatch(r"(.*)-(20\d{6})", id)
    return m.group(1) if m else id


def shared_id(p: Price, id: str) -> bool:
    """Another provider sells a model under this exact id (``openai/gpt-oss-120b``)."""
    return any(q.provider != p.provider and any(m.lower() == id.lower() for m in ids(q)) for q in all_prices())


def assert_resolves(form: str, p: Price) -> None:
    found = lookup(form)
    assert found is not None, f"{form!r} did not resolve (expected {p.model})"
    assert found.model == p.model, f"{form!r} resolved to the wrong row"


def assert_resolves_under_provider(form: str, p: Price) -> None:
    found = lookup_provider(p.provider, form)
    assert found is not None, f"{form!r} did not resolve under {p.provider} (expected {p.model})"
    assert found is p, f"{form!r} under {p.provider} resolved to {found.provider}/{found.model}"


def test_every_row_has_a_source_and_date():
    assert len(all_prices()) > 50
    for p in all_prices():
        assert p.source_url.startswith("https://"), p.model
        assert re.fullmatch(r"\d{4}-\d{2}-\d{2}", p.checked), p.model


def test_lookups():
    assert lookup("claude-opus-5-5").input == 4.0
    assert lookup("us.anthropic.claude-sonnet-4-20250514-v1:0").model == "claude-sonnet-4-20250514"
    assert lookup("claude-opus-4-1@20250805").model == "claude-opus-4-1-20250805"
    assert lookup("claude-haiku-4-5-20251001").model == "claude-haiku-4-5"
    assert lookup("models/gemini-2.5-pro").output == 10.0
    assert lookup("deepseek-v4-flash").model == "deepseek-flash"
    assert lookup("gpt-4o-2024-08-06").model == "gpt-4o"
    assert lookup("gpt-4o-2024-05-13").model == "gpt-4o-2024-05-13"
    assert lookup("not-a-model") is None


def test_crate_doc_examples():
    p = lookup("claude-sonnet-5-5")
    assert (p.input, p.cache_read) == (2.0, 0.10)
    assert lookup("anthropic/claude-sonnet-5-5") is not None
    assert lookup("claude-sonnet-4-5") is not None
    assert lookup("mistral-large-latest").model == "mistral-large-4"
    assert lookup("xai/grok-4.7-latest").model == "grok-4.7"
    assert lookup("vertex_ai/claude-opus-4-1@20250805").model == "claude-opus-4-1-20250805"
    assert lookup("gpt-4o-2024-05-13").input == 5.0
    p = lookup("fireworks_ai/accounts/fireworks/models/gpt-oss-120b")
    assert p.provider == "fireworks"


def test_snapshot_and_latest_suffixes():
    assert _strip_date("gpt-4o-2024-08-06") == "gpt-4o"
    assert _strip_date("claude-opus-4-1-20250805") == "claude-opus-4-1"
    assert _strip_date("gpt-4-0613") is None
    assert _strip_date("grok-4.20-0309-reasoning") is None
    assert _strip_date("-2024-08-06") is None
    assert _strip_version("grok-4.7-latest") == "grok-4.7"
    assert _strip_version("chat-latest") == "chat"
    assert _strip_version("-latest") == "-latest"
    assert _strip_version("claude-3-5-haiku-latest") == "claude-3-5-haiku"
    assert lookup("chat-latest").model == "chat-latest"
    assert lookup("claude-3-5-haiku-latest").model == "claude-3-5-haiku-20241022"
    assert lookup("gpt-4-turbo").model == "gpt-4-turbo-2024-04-09"


def test_ids_and_aliases_resolve_exactly():
    for p in all_prices():
        for id in ids(p):
            for form in (id, id.upper(), f"  {id}\n"):
                assert_resolves(form, p)
                assert_resolves_under_provider(form, p)
            if not shared_id(p, id):
                assert lookup(id) is p, id


def test_ids_are_unique_per_provider():
    seen = set()
    for p in all_prices():
        for id in ids(p):
            key = (p.provider, id.lower())
            assert key not in seen, f"{id} appears on two {p.provider} rows"
            seen.add(key)


PREFIXES = {
    "anthropic": ["anthropic/", "openrouter/", "bedrock/", "us.anthropic.", "bedrock/us.anthropic.", "vertex_ai/"],
    "openai": ["openai/", "openrouter/", "azure/"],
    "google": ["google/", "openrouter/", "models/", "gemini/", "vertex_ai/"],
    "deepseek": ["deepseek/", "openrouter/"],
    "mistral": ["mistral/", "openrouter/", "mistralai/"],
    "xai": ["xai/", "openrouter/", "x-ai/"],
    "cohere": ["cohere/", "openrouter/"],
    "groq": ["groq/"],
    "together": ["together_ai/", "together/"],
    "fireworks": ["fireworks_ai/", "fireworks/"],
}


def test_provider_prefixes_resolve():
    for p in all_prices():
        assert p.provider in PREFIXES, f"new provider {p.provider}: add its prefixes here"
        for id in ids(p):
            for prefix in PREFIXES[p.provider]:
                form = prefix + id
                assert_resolves(form, p)
                # The prefix names the provider, so plain lookup finds this exact row even
                # when another host sells the same id.
                if prefix not in ("openrouter/", "us.anthropic.", "models/"):
                    assert lookup(form) is p, form


def test_bedrock_ids_resolve_for_anthropic_rows():
    for p in (p for p in all_prices() if p.provider == "anthropic"):
        for id in ids(p):
            for region in ("", "us.", "eu.", "apac.", "global."):
                assert_resolves(f"{region}anthropic.{id}-v1:0", p)
                assert_resolves(f"{region}anthropic.{id}-v2:0", p)
                assert lookup_provider(Provider.BEDROCK, f"{region}anthropic.{id}-v1:0") is p


def test_vertex_at_date_resolves_for_anthropic_rows():
    for p in (p for p in all_prices() if p.provider == "anthropic"):
        for id in ids(p):
            base = undated(id)
            date = "20990101" if base == id else id[len(base) + 1 :]
            assert_resolves(f"{base}@{date}", p)
            assert_resolves(f"vertex_ai/{base}@{date}", p)
            assert lookup_provider("vertex_ai", f"{base}@{date}") is p


def test_dated_and_undated_anthropic_ids_resolve():
    for p in (p for p in all_prices() if p.provider == "anthropic"):
        for id in ids(p):
            base = undated(id)
            assert_resolves(base, p)
            assert_resolves(f"{base}-20990101", p)
            assert_resolves(f"anthropic/{base}-20990101", p)
            assert_resolves(f"{base}-latest", p)


@pytest.mark.parametrize(
    "id,base",
    [
        ("gpt-4o-2024-08-06", "gpt-4o"),
        ("gpt-4o-2024-11-20", "gpt-4o"),
        ("openai/gpt-4o-2024-11-20", "gpt-4o"),
        ("azure/gpt-4.1-2025-04-14", "gpt-4.1"),
        ("o3-2025-04-16", "o3"),
        ("gpt-4o-mini-2024-07-18", "gpt-4o-mini"),
        ("gpt-5-2025-08-07", "gpt-5"),
    ],
)
def test_dated_openai_ids_resolve_to_the_base_row(id, base):
    assert lookup(id).model == base
    assert lookup_provider("openai", id).model == base


def test_every_dated_row_is_still_its_own_answer():
    assert lookup("gpt-4o-2024-05-13").input == 5.0
    assert lookup("gpt-4o").input == 2.5
    for p in all_prices():
        for id in ids(p):
            assert_resolves(f"{id}-2099-01-01", p)


@pytest.mark.parametrize(
    "id,model",
    [
        ("mistral-large-latest", "mistral-large-4"),
        ("mistral-small-latest", "mistral-small-2603"),
        ("codestral-latest", "codestral-2508"),
        ("grok-4.7-latest", "grok-4.7"),
        ("xai/grok-4.3-latest", "grok-4.3"),
        ("claude-3-5-haiku-latest", "claude-3-5-haiku-20241022"),
        ("claude-opus-5-5-latest", "claude-opus-5-5"),
    ],
)
def test_latest_aliases_resolve(id, model):
    assert lookup(id).model == model


def test_provider_lookup_keeps_hosts_apart():
    groq = lookup_provider("groq", "openai/gpt-oss-120b")
    together = lookup_provider("together_ai", "openai/gpt-oss-120b")
    fireworks = lookup_provider("fireworks", "accounts/fireworks/models/gpt-oss-120b")
    assert (groq.provider, together.provider, fireworks.provider) == ("groq", "together", "fireworks")
    assert groq.cache_read == 0.075  # Groq prompt caching: 50% off input
    assert fireworks.cache_read == 0.015
    assert together.cache_read is None
    assert lookup_provider("openai", "openai/gpt-oss-120b") is None
    assert lookup_provider("openai", "gpt-oss-120b") is None
    assert lookup_provider("anthropic", "gpt-4o") is None
    assert lookup_provider("groq", "claude-sonnet-4-6") is None
    assert lookup_provider("together", "gpt-4o") is None
    assert lookup_provider("fireworks", "gpt-oss-120b") is fireworks
    assert lookup_provider("fireworks_ai", "fireworks_ai/accounts/fireworks/models/gpt-oss-120b") is fireworks
    assert lookup_provider("bedrock", "claude-sonnet-4-6").provider == "anthropic"
    assert lookup_provider("vertex", "gemini-2.5-pro").provider == "google"
    assert lookup_provider("azure", "gpt-4o").provider == "openai"
    assert lookup_provider("openrouter", "mistralai/mistral-large-4").provider == "mistral"
    assert lookup_provider("openrouter", "x-ai/grok-4.7").provider == "xai"
    assert lookup_provider("bedrock", "gpt-4o") is None
    assert lookup_provider("azure", "claude-sonnet-4-6") is None
    assert lookup_provider("vertex", "mistral-large-4") is None
    assert lookup_provider("bedrock", "deepseek-flash") is None
    assert lookup_provider("acme", "gpt-4o") is None
    mistral = lookup_provider("mistral", "mistral-large-latest")
    assert mistral.input == 0.7
    assert lookup_provider(Provider.MISTRAL, "mistral/mistral-large-4") is mistral


def test_provider_doc_examples():
    p = Provider.TOGETHER.lookup("moonshotai/Kimi-K3")
    assert (p.provider, p.input) == ("together", 3.0)
    assert Provider.FIREWORKS.lookup("accounts/fireworks/models/kimi-k3") is not None
    assert Provider.BEDROCK.lookup("global.anthropic.claude-opus-4-5-20251101-v1:0").provider == "anthropic"
    assert Provider.GROQ.lookup("claude-opus-4-5") is None
    assert all(p.provider == "groq" for p in Provider.GROQ.rows())
    assert "openai/gpt-oss-120b" in [p.model for p in all_prices() if p.provider == "groq"]


def test_provider_rows_cover_the_table():
    assert sum(len(list(p.rows())) for p in Provider) == len(all_prices())
    for name in ("mistral", "xai", "cohere", "groq", "together", "fireworks"):
        rows = list(Provider.parse(name).rows())
        assert rows, name
        for row in rows:
            assert row.source_url.startswith("https://"), row.model


def test_unknown_ids_do_not_resolve():
    for id in ("", "   ", "not-a-model", "anthropic/", "claude", "gpt", "models/", "groq/", "-latest", "latest", "2024-08-06"):
        assert lookup(id) is None, repr(id)


# -- providers (crate/src/provider.rs) --------------------------------------------------------


def test_provider_names_round_trip():
    for p in Provider:
        assert Provider.parse(p.as_str()) is p
        assert Provider.parse(p.as_str().upper()) is p
        assert str(p) == p.as_str() == p.value
        assert Provider.parse(p) is p


def test_every_table_provider_parses():
    for p in all_prices():
        kind = p.provider_kind()
        assert kind is not None, p.provider
        assert kind.as_str() == p.provider
        # Platform rows would change the order plain lookup tries rows in.
        assert kind.value not in ("vertex", "bedrock", "azure", "openrouter")


@pytest.mark.parametrize(
    "name,want",
    [
        ("gemini", Provider.GOOGLE),
        ("vertex_ai", Provider.VERTEX),
        ("Vertex AI", Provider.VERTEX),
        ("aws", Provider.BEDROCK),
        ("azure_openai", Provider.AZURE),
        ("mistralai", Provider.MISTRAL),
        ("together_ai", Provider.TOGETHER),
        ("Together_AI", Provider.TOGETHER),
        ("fireworks_ai", Provider.FIREWORKS),
        ("x-ai", Provider.XAI),
        ("open_router", Provider.OPENROUTER),
        # LangChain init_chat_model provider names.
        ("google_genai", Provider.GOOGLE),
        ("google_vertexai", Provider.VERTEX),
        ("bedrock_converse", Provider.BEDROCK),
        ("google-generativeai", Provider.GOOGLE),
        ("azure_ai_foundry", Provider.AZURE),
        ("GroqCloud", Provider.GROQ),
        ("cohere_ai", Provider.COHERE),
    ],
)
def test_provider_aliases(name, want):
    assert Provider.parse(name) is want


@pytest.mark.parametrize("bad", ["", "deepseek-ai", "meta-llama", "qwen", "ollama", "huggingface", "a-very-long-provider-name-that-is-not-real"])
def test_unknown_provider_names(bad):
    assert Provider.parse(bad) is None
    assert lookup_provider(bad, "gpt-4o") is None


def test_provider_names_that_are_not_strings():
    assert Provider.parse(None) is None  # type: ignore[arg-type]
    assert lookup_provider(None, "gpt-4o") is None  # type: ignore[arg-type]


# What a cost callback sees: a provider name from its framework and the model id it sent.
@pytest.mark.parametrize(
    "provider,model,want_provider,want_model,per_m",
    [
        ("openai", "gpt-4o", "openai", "gpt-4o", (2.5, 10.0)),
        ("openai", "gpt-4o-2024-05-13", "openai", "gpt-4o-2024-05-13", (5.0, 15.0)),
        ("anthropic", "claude-sonnet-4-5", "anthropic", "claude-sonnet-4-5-20250929", (3.0, 15.0)),
        ("google_genai", "gemini-2.5-flash", "google", "gemini-2.5-flash", (0.3, 2.5)),
        ("google_vertexai", "gemini-2.5-pro", "google", "gemini-2.5-pro", (1.25, 10.0)),
        ("deepseek", "deepseek-chat", "deepseek", "deepseek-flash", (0.3, 1.2)),
        ("deepseek", "deepseek-reasoner", "deepseek", "deepseek-flash", (0.3, 1.2)),
        ("mistralai", "mistral-large-latest", "mistral", "mistral-large-4", (0.7, 2.1)),
        ("xai", "grok-4.7", "xai", "grok-4.7", (2.0, 6.0)),
        ("groq", "openai/gpt-oss-120b", "groq", "openai/gpt-oss-120b", (0.15, 0.6)),
        ("together", "openai/gpt-oss-120b", "together", "openai/gpt-oss-120b", (0.15, 0.6)),
        ("fireworks", "accounts/fireworks/models/gpt-oss-120b", "fireworks", "accounts/fireworks/models/gpt-oss-120b", (0.15, 0.6)),
        ("cohere", "command-r-plus-08-2024", "cohere", "command-r-plus-08-2024", (2.5, 10.0)),
        ("bedrock", "anthropic.claude-haiku-4-5-20251001-v1:0", "anthropic", "claude-haiku-4-5", (1.0, 5.0)),
        ("bedrock_converse", "us.anthropic.claude-sonnet-4-20250514-v1:0", "anthropic", "claude-sonnet-4-20250514", (3.0, 15.0)),
        ("azure_openai", "gpt-4o-mini", "openai", "gpt-4o-mini", (0.15, 0.6)),
    ],
)
def test_lookup_provider_for_cost_callbacks(provider, model, want_provider, want_model, per_m):
    p = lookup_provider(provider, model)
    assert p is not None, (provider, model)
    assert (p.provider, p.model) == (want_provider, want_model)
    assert (p.input, p.output) == per_m
    assert p.input_per_token == pytest.approx(per_m[0] / 1e6, rel=1e-12)
    assert p.output_per_token == pytest.approx(per_m[1] / 1e6, rel=1e-12)
    assert p.per_token("output") == p.output_per_token


@pytest.mark.parametrize(
    "provider,model",
    [
        ("groq", "llama-3.3-70b-versatile"),  # not in the table
        ("bedrock", "mistral.mistral-large-2407-v1:0"),  # Bedrock only falls back to Claude
        ("deepseek", "gpt-4o"),  # another provider's model
        ("ollama", "llama3"),  # not a priced provider
        ("huggingface", "deepseek-ai/DeepSeek-V4-Pro"),
    ],
)
def test_lookup_provider_returns_none_when_it_cannot_price(provider, model):
    assert lookup_provider(provider, model) is None


def test_per_token_is_none_without_a_price():
    emb = lookup_provider("openai", "text-embedding-3-small")
    assert emb is not None
    assert emb.output is None and emb.output_per_token is None
    assert emb.input_per_token == pytest.approx(0.02 / 1e6)


def test_delisted_rows_still_price():
    for model in ("gpt-3.5-turbo", "o1", "claude-3-5-haiku-20241022"):
        p = lookup(model)
        assert p is not None and p.status == "retired", model
        assert "no longer on the pricing page" in (p.note or "")
