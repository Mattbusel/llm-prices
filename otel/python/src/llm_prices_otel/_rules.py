"""Span attribute rules shared by every price source: which attributes hold the model, the
provider and the token counts, and how they are read. The same lists and rules as the Go
Collector processor (otel/genaicostprocessor/attrs.go); the conformance cases in
otel/conformance/cases.json check both."""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Any, Mapping, Optional, Sequence, Tuple

import llm_prices

#: Attributes this package writes. The ``gen_ai.usage.cost.*`` names are the ones proposed in
#: open-telemetry/semantic-conventions-genai#443 (amount, currency, source) and #624
#: (pricing.id, pricing.version).
COST_AMOUNT = "gen_ai.usage.cost.amount"
COST_CURRENCY = "gen_ai.usage.cost.currency"
COST_SOURCE = "gen_ai.usage.cost.source"
COST_PRICING_ID = "gen_ai.usage.cost.pricing.id"
COST_PRICING_VERSION = "gen_ai.usage.cost.pricing.version"
#: The reason a GenAI span with token counts got no cost.
UNPRICED = "llm_prices.unpriced"
#: The matched price row, ``"provider/model"`` (with ``breakdown=True``).
PRICE_ROW = "llm_prices.price.row"
#: The service tier a span was priced at, when not standard.
PRICE_TIER = "llm_prices.price.tier"
ERROR_TYPE = "error.type"

# Reasons written to llm_prices.unpriced.
NO_MODEL = "no_model"
UNKNOWN_PROVIDER = "unknown_provider"
UNKNOWN_MODEL = "unknown_model"
MISSING_RATE = "missing_rate"
UNPRICED_TIER = "unpriced_tier"
INVALID_USAGE = "invalid_usage"
UNSUPPORTED_PRICE = "unsupported_price"

INPUT_KEYS = ("gen_ai.usage.input_tokens", "gen_ai.usage.prompt_tokens", "llm.token_count.prompt", "llm.usage.prompt_tokens")
OUTPUT_KEYS = ("gen_ai.usage.output_tokens", "gen_ai.usage.completion_tokens", "llm.token_count.completion", "llm.usage.completion_tokens")
CACHE_READ_KEYS = ("gen_ai.usage.cache_read.input_tokens", "gen_ai.usage.cache_read_input_tokens", "llm.token_count.prompt_details.cache_read")
CACHE_WRITE_KEYS = (
    "gen_ai.usage.cache_write.input_tokens",
    "gen_ai.usage.cache_creation.input_tokens",
    "gen_ai.usage.cache_creation_input_tokens",
    "llm.token_count.prompt_details.cache_write",
)
REASONING_KEYS = (
    "gen_ai.usage.reasoning.output_tokens",
    "gen_ai.usage.reasoning_tokens",
    "llm.token_count.completion_details.reasoning",
    "llm.usage.reasoning_tokens",
)
MODEL_KEYS = ("gen_ai.response.model", "gen_ai.request.model", "llm.model_name", "llm.response.model", "llm.request.model")
PROVIDER_KEYS = ("gen_ai.provider.name", "gen_ai.system", "llm.provider", "llm.system")
TIER_KEYS = (
    "openai.response.service_tier",
    "gen_ai.openai.response.service_tier",
    "openai.request.service_tier",
    "gen_ai.openai.request.service_tier",
)
SERVER_KEYS = ("server.address", "net.peer.name")

#: Provider names of the GenAI semantic conventions and instrumentation libraries that
#: :meth:`llm_prices.Provider.parse` does not read itself, as llm-prices provider names.
PROVIDER_ALIASES = {
    "gcp.gen_ai": "vertex",
    "gcp.vertex_ai": "vertex",
    "gcp.gemini": "google",
    "vertex_ai": "vertex",
    "vertexai": "vertex",
    "az.ai.inference": "azure",
    "az.ai.openai": "azure",
    "azure.ai.inference": "azure",
    "azure.ai.openai": "azure",
    "aws.bedrock": "bedrock",
    "x_ai": "xai",
    "mistral_ai": "mistral",
}

#: API hosts (``server.address``) and the provider that bills requests sent there. A known host
#: wins over the provider attribute: instrumentations of an OpenAI-compatible client report
#: "openai" for every endpoint the client is pointed at.
HOST_PROVIDERS = (
    ("api.openai.com", "openai"),
    ("api.anthropic.com", "anthropic"),
    ("generativelanguage.googleapis.com", "google"),
    ("aiplatform.googleapis.com", "vertex"),
    ("openai.azure.com", "azure"),
    ("services.ai.azure.com", "azure"),
    ("api.deepseek.com", "deepseek"),
    ("api.mistral.ai", "mistral"),
    ("api.x.ai", "xai"),
    ("api.groq.com", "groq"),
    ("api.together.xyz", "together"),
    ("api.together.ai", "together"),
    ("api.fireworks.ai", "fireworks"),
    ("openrouter.ai", "openrouter"),
    ("api.perplexity.ai", "perplexity"),
    ("api.cohere.com", "cohere"),
    ("api.cohere.ai", "cohere"),
)

_STANDARD_TIERS = frozenset({"", "auto", "default", "standard", "on_demand", "on-demand"})
_MAX_EXACT = 1 << 53
_MAX_INT64 = (1 << 63) - 1


def provider_from_host(host: str) -> Optional[str]:
    host = host.strip().rstrip(".").lower()
    if host.startswith("bedrock-runtime.") and host.endswith(".amazonaws.com"):
        return "bedrock"
    for suffix, provider in HOST_PROVIDERS:
        if host == suffix or host.endswith("." + suffix):
            return provider
    return None


def canonical_provider(raw: str) -> Optional[str]:
    """The llm-prices name of a provider attribute value, ``None`` when it is not one the table
    prices ("ollama")."""
    lower = raw.lower()
    if lower in PROVIDER_ALIASES:
        return PROVIDER_ALIASES[lower]
    p = llm_prices.Provider.parse(lower)
    return None if p is None else p.value


def _str(attrs: Mapping[str, Any], keys: Sequence[str]) -> Optional[str]:
    for k in keys:
        v = attrs.get(k)
        if isinstance(v, str) and v.strip():
            return v.strip()
    return None


def span_provider(attrs: Mapping[str, Any]) -> Tuple[Optional[str], bool]:
    """The llm-prices name of the provider that served a span (``None`` when the span does not
    say) and whether it is one the table knows."""
    host = _str(attrs, SERVER_KEYS)
    if host is not None:
        p = provider_from_host(host)
        if p is not None:
            return p, True
    raw = _str(attrs, PROVIDER_KEYS)
    if raw is None:
        return None, True
    p = canonical_provider(raw)
    return p, p is not None


def span_models(attrs: Mapping[str, Any]) -> Tuple[str, ...]:
    out = []
    for k in MODEL_KEYS:
        v = attrs.get(k)
        if isinstance(v, str) and v.strip() and v.strip() not in out:
            out.append(v.strip())
    return tuple(out)


def span_tier(attrs: Mapping[str, Any]) -> Optional[str]:
    v = _str(attrs, TIER_KEYS)
    if v is None:
        return None
    v = v.lower()
    return None if v in _STANDARD_TIERS else v


def _count(attrs: Mapping[str, Any], keys: Sequence[str]) -> Tuple[int, bool, bool]:
    """(value, present, valid) of the first of ``keys`` that is set."""
    for k in keys:
        if k not in attrs:
            continue
        v = attrs[k]
        if isinstance(v, bool):
            return 0, True, False
        if isinstance(v, int):
            return v, True, 0 <= v <= _MAX_INT64
        if isinstance(v, float):
            if not math.isfinite(v) or v < 0 or v > _MAX_EXACT or v != math.trunc(v):
                return 0, True, False
            return int(v), True, True
        if isinstance(v, str):
            s = v.strip()
            body = s[1:] if s[:1] in "+-" else s
            if not body.isdigit() or not body.isascii():
                return 0, True, False
            i = int(s)
            return i, True, 0 <= i <= _MAX_INT64
        return 0, True, False
    return 0, False, True


@dataclass(frozen=True)
class Tokens:
    """Token counts of one span after the include-cache and include-reasoning rules."""

    #: Uncached input tokens.
    input: int = 0
    cache_read: int = 0
    cache_write: int = 0
    #: Output tokens, reasoning included.
    output: int = 0
    #: The reasoning part of ``output``.
    reasoning: int = 0


def read_tokens(attrs: Mapping[str, Any], include_cache: str, include_reasoning: str) -> Tuple[Optional[Tokens], bool]:
    """(tokens, valid). ``tokens`` is ``None`` for a span with no token counts."""
    inp, in_ok, in_valid = _count(attrs, INPUT_KEYS)
    out, out_ok, out_valid = _count(attrs, OUTPUT_KEYS)
    read, _, read_valid = _count(attrs, CACHE_READ_KEYS)
    write, _, write_valid = _count(attrs, CACHE_WRITE_KEYS)
    reasoning, _, reasoning_valid = _count(attrs, REASONING_KEYS)
    if not in_ok and not out_ok:
        return None, True
    if not (in_valid and out_valid and read_valid and write_valid and reasoning_valid):
        return None, False
    uncached = inp
    cached = read + write
    if include_cache == "true":
        if cached > inp:
            return None, False
        uncached = inp - cached
    elif include_cache == "auto" and cached <= inp:
        uncached = inp - cached
    output = out
    if include_reasoning == "true":
        if reasoning > out:
            return None, False
    elif include_reasoning == "false":
        output = out + reasoning
    elif reasoning > out:
        output = out + reasoning
    return Tokens(input=uncached, cache_read=read, cache_write=write, output=output, reasoning=min(reasoning, output)), True
