"""Price sources: the llm-prices table (default), LiteLLM's model_prices_and_context_window.json
and genai-prices. A source turns a :class:`Request` into a :class:`Priced` cost or the reason it
cannot price it."""

from __future__ import annotations

import hashlib
import json
import math
import os
import urllib.request
from dataclasses import dataclass
from typing import Any, Dict, Mapping, Optional, Tuple, Union

import llm_prices

from . import _rules as r
from ._rules import Tokens


@dataclass(frozen=True)
class Request:
    """What a source needs to price one span."""

    #: llm-prices name of the provider that served the request, ``None`` when the span does not say.
    provider: Optional[str]
    #: Model ids to try, in order (response model first).
    models: Tuple[str, ...]
    tokens: Tokens
    batch: bool = False
    #: Service tier ("flex", "priority", ...), ``None`` for standard.
    tier: Optional[str] = None


@dataclass(frozen=True)
class Priced:
    """A span's cost in USD, split by token kind."""

    total: float
    input: float = 0.0
    output: float = 0.0
    cache_read: float = 0.0
    cache_write: float = 0.0
    #: The matched price row, ``"provider/model"``.
    row: str = ""


Result = Union[Priced, str]


class PriceSource:
    """Base class of price sources. ``pricing_id`` and ``pricing_version`` are written as
    ``gen_ai.usage.cost.pricing.id`` and ``.pricing.version``."""

    pricing_id: str = ""
    pricing_version: str = ""

    def price(self, request: Request) -> Result:  # pragma: no cover - interface
        """A :class:`Priced` cost, or the reason (a string) the request cannot be priced."""
        raise NotImplementedError


def _cost_llm_prices(p: llm_prices.Price, t: Tokens, batch: bool) -> Result:
    """Cost of token counts at one llm-prices row. Cache writes bill at the 5-minute write price
    (the conventions carry no cache lifetime; it is Anthropic's default), or as uncached input
    when the row lists no cache-write price at all."""
    usage: Dict[str, Any] = {"input": t.input, "output": t.output, "cache_read": t.cache_read, "batch": batch}
    if t.cache_write:
        if "5m" in p.cache_write:
            usage["cache_write_5m"] = t.cache_write
        elif not p.cache_write:
            usage["input"] = t.input + t.cache_write
        else:
            return r.MISSING_RATE
    c = p.cost_usage(llm_prices.Usage(**usage))
    if c is None:
        return r.MISSING_RATE
    return Priced(total=c.total, input=c.input, output=c.output, cache_read=c.cache_read, cache_write=c.cache_write, row=f"{p.provider}/{p.model}")


class LLMPricesSource(PriceSource):
    """The llm-prices table bundled with the installed ``llm-prices`` package. Its version is
    ``llm_prices.PRICES_VERSION``, the SHA-256 of the release's prices.json."""

    pricing_id = "llm-prices"

    def __init__(self) -> None:
        self.pricing_version = llm_prices.PRICES_VERSION

    def price(self, request: Request) -> Result:
        row = None
        for m in request.models:
            row = llm_prices.lookup(m) if request.provider is None else llm_prices.lookup_provider(request.provider, m)
            if row is not None:
                break
        if row is None:
            return r.UNKNOWN_MODEL
        p = row.tier(request.tier)
        if p is None:
            return r.UNPRICED_TIER
        return _cost_llm_prices(p, request.tokens, request.batch)


# -- LiteLLM ---------------------------------------------------------------------------------

#: llm-prices provider names and LiteLLM's custom_llm_provider names.
LITELLM_PROVIDERS: Mapping[str, Tuple[str, ...]] = {
    "openai": ("openai", "text-completion-openai"),
    "anthropic": ("anthropic",),
    "google": ("gemini",),
    "vertex": ("vertex_ai",),
    "bedrock": ("bedrock", "bedrock_converse"),
    "azure": ("azure", "azure_ai"),
    "deepseek": ("deepseek",),
    "mistral": ("mistral",),
    "xai": ("xai",),
    "groq": ("groq",),
    "together": ("together_ai",),
    "fireworks": ("fireworks_ai",),
    "openrouter": ("openrouter",),
    "perplexity": ("perplexity",),
    "cohere": ("cohere_chat", "cohere"),
}
_TIER_SUFFIX = {"flex": "flex", "balanced": "balanced", "priority": "priority", "fast": "priority", "ultrafast": "ultrafast"}
_RATE_PREFIXES = ("input_cost_per_token", "output_cost_per_token", "cache_read_input_token_cost", "cache_creation_input_token_cost")

DEFAULT_LITELLM_URL = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json"


def _rate(e: Mapping[str, Any], key: str) -> Optional[float]:
    v = e.get(key)
    if isinstance(v, bool) or not isinstance(v, (int, float)):
        return None
    f = float(v)
    if math.isnan(f) or math.isinf(f) or f < 0:
        return None
    return f


def _parse_threshold(label: str) -> float:
    if label.endswith("k"):
        return float(label[:-1]) * 1000
    return float(label)


def _thresholds(e: Mapping[str, Any], suffix: str) -> Tuple[Tuple[str, float], ...]:
    seen: Dict[str, float] = {}
    for k in e:
        base = next((p for p in _RATE_PREFIXES if k.startswith(p + "_above_")), None)
        if base is None or "_tokens" not in k:
            continue
        label, _, tail = k[len(base) + len("_above_"):].partition("_tokens")
        if tail and tail != suffix:
            continue
        if _rate(e, k) is None or label in seen:
            continue
        try:
            seen[label] = _parse_threshold(label)
        except ValueError:
            continue
    return tuple(sorted(seen.items(), key=lambda kv: kv[1]))


def cost_litellm(e: Mapping[str, Any], provider: Optional[str], t: Tokens, batch: bool, tier: Optional[str]) -> Result:
    """The token arithmetic of LiteLLM's ``generic_cost_per_token`` (the Go source's
    ``costLiteLLM``): uncached input, cache reads and writes (default: the input rate), output
    with reasoning at ``output_cost_per_reasoning_token`` when listed, ``_above_<N>_tokens``
    thresholds (inclusive for xAI), service-tier keys and Batch API rates."""
    if "tiered_pricing" in e or any("off_peak" in k for k in e):
        return r.UNSUPPORTED_PRICE
    suffix = ""
    if tier is not None:
        sfx = _TIER_SUFFIX.get(tier)
        if sfx is None or _rate(e, "input_cost_per_token_" + sfx) is None:
            return r.UNPRICED_TIER
        suffix = "_" + sfx

    def key(base: str) -> str:
        return base + suffix

    def tiered(base: str) -> Optional[float]:
        # LiteLLM's get_cost_per_unit: a missing service-tier key falls back to the standard key.
        v = _rate(e, key(base))
        return _rate(e, base) if v is None and suffix else v

    inp = tiered("input_cost_per_token")
    out = tiered("output_cost_per_token")
    if inp is None or out is None:
        return r.MISSING_RATE
    creation = tiered("cache_creation_input_token_cost")
    read = tiered("cache_read_input_token_cost")
    if read is None and provider == "fireworks":
        # LiteLLM bills Fireworks cache reads at half the input rate when the entry lists none
        # (FIREWORKS_AI_DEFAULT_CACHE_READ_RATE_RATIO).
        read = inp * 0.5
    prompt = t.input + t.cache_read + t.cache_write
    for label, n in _thresholds(e, suffix):
        if not (prompt > n or (provider == "xai" and prompt == n)):
            continue

        def pick(base: str, cur: Optional[float]) -> Optional[float]:
            v = _rate(e, key(f"{base}_above_{label}_tokens"))
            if v is None and suffix:
                v = _rate(e, f"{base}_above_{label}_tokens")
            return cur if v is None else v

        inp = pick("input_cost_per_token", inp)
        out = pick("output_cost_per_token", out)
        creation = pick("cache_creation_input_token_cost", creation)
        read = pick("cache_read_input_token_cost", read)
    assert inp is not None and out is not None
    if read is None:
        read = inp
    if creation is None:
        creation = inp
    # LiteLLM's _resolve_reasoning_token_cost: the tier's reasoning price, else the tier's
    # output price, else the standard reasoning price, else the output price.
    reasoning_rate = _rate(e, key("output_cost_per_reasoning_token")) if suffix else None
    if reasoning_rate is None:
        if suffix and _rate(e, key("output_cost_per_token")) is not None:
            reasoning_rate = out
        else:
            reasoning_rate = _rate(e, "output_cost_per_reasoning_token")
            if reasoning_rate is None:
                reasoning_rate = out
    if batch:
        bi, bo = _rate(e, "input_cost_per_token_batches"), _rate(e, "output_cost_per_token_batches")
        if bi is None or bo is None:
            return r.MISSING_RATE
        if t.cache_read or t.cache_write:
            br, bw = _rate(e, "cache_read_input_token_cost_batches"), _rate(e, "cache_creation_input_token_cost_batches")
            if (t.cache_read and br is None) or (t.cache_write and bw is None):
                return r.MISSING_RATE
            read, creation = br or 0.0, bw or 0.0
        inp, out, reasoning_rate = bi, bo, bo
    text = t.output - t.reasoning
    c_in = float(t.input) * inp
    c_read = float(t.cache_read) * read
    c_write = float(t.cache_write) * creation
    c_out = float(text) * out + float(t.reasoning) * reasoning_rate
    return Priced(total=c_in + c_read + c_write + c_out, input=c_in, output=c_out, cache_read=c_read, cache_write=c_write)


def _read(location: str, timeout: float) -> bytes:
    if location.startswith(("http://", "https://")):
        req = urllib.request.Request(location, headers={"User-Agent": "llm-prices-otel (+https://github.com/Mattbusel/llm-prices)"})
        with urllib.request.urlopen(req, timeout=timeout) as resp:  # noqa: S310 - http(s) only
            return bytes(resp.read())
    if location.startswith("file://"):
        location = urllib.request.url2pathname(location[len("file://"):])
    with open(os.fspath(location), "rb") as f:
        return f.read()


def _version(data: bytes) -> str:
    return "sha256:" + hashlib.sha256(data.replace(b"\r\n", b"\n")).hexdigest()


class LiteLLMSource(PriceSource):
    """LiteLLM's model_prices_and_context_window.json, loaded once. Its version is the SHA-256
    of the bytes it was loaded from. Entries priced with ``tiered_pricing`` tables or off-peak
    windows are reported as ``unsupported_price`` rather than priced."""

    pricing_id = "litellm.model_cost_map"

    def __init__(self, data: bytes) -> None:
        raw = json.loads(data)
        if not isinstance(raw, dict):
            raise ValueError("litellm prices: expected a JSON object")
        self._entries: Dict[str, Mapping[str, Any]] = {k: v for k, v in raw.items() if isinstance(v, dict)}
        if not self._entries:
            raise ValueError("litellm prices: no entries")
        self._lower: Dict[str, str] = {}
        for k in self._entries:
            self._lower.setdefault(k.lower(), k)
        self.pricing_version = _version(data)

    @classmethod
    def load(cls, location: str = DEFAULT_LITELLM_URL, timeout: float = 30.0) -> "LiteLLMSource":
        """Load from an http(s) URL, a file:// URL or a path."""
        return cls(_read(location, timeout))

    def _get(self, key: str) -> Optional[Mapping[str, Any]]:
        e = self._entries.get(key)
        if e is None:
            k = self._lower.get(key.lower())
            e = None if k is None else self._entries[k]
        return e

    def _find(self, provider: Optional[str], model: str) -> Optional[Tuple[Mapping[str, Any], str]]:
        if provider is None:
            e = self._get(model)
            return None if e is None else (e, model)
        names = LITELLM_PROVIDERS[provider]
        for n in names:
            e = self._get(f"{n}/{model}")
            if e is not None:
                return e, f"{n}/{model}"
        if any(model.startswith(n + "/") for n in names):
            e = self._get(model)
            if e is not None:
                return e, model
        bare = model
        for n in names:
            if bare.startswith(n + "/"):
                bare = bare[len(n) + 1:]
        e = self._get(bare)
        if e is not None:
            lp = e.get("litellm_provider")
            lp = lp if isinstance(lp, str) else ""
            if any(lp == n or lp.startswith(n + "-") or lp.startswith(n + "_") for n in names):
                return e, bare
        return None

    def price(self, request: Request) -> Result:
        if request.provider is not None and request.provider not in LITELLM_PROVIDERS:
            return r.UNKNOWN_PROVIDER
        for m in request.models:
            found = self._find(request.provider, m)
            if found is None:
                continue
            e, key = found
            c = cost_litellm(e, request.provider, request.tokens, request.batch, request.tier)
            if isinstance(c, str):
                return c
            return Priced(total=c.total, input=c.input, output=c.output, cache_read=c.cache_read, cache_write=c.cache_write, row="litellm/" + key)
        return r.UNKNOWN_MODEL


# -- genai-prices ----------------------------------------------------------------------------

#: llm-prices provider names and genai-prices provider ids.
GENAI_PRICES_PROVIDERS: Mapping[str, str] = {
    "openai": "openai",
    "anthropic": "anthropic",
    "google": "google",
    "vertex": "google",
    "bedrock": "aws",
    "azure": "azure",
    "deepseek": "deepseek",
    "mistral": "mistral",
    "xai": "x-ai",
    "groq": "groq",
    "together": "together",
    "fireworks": "fireworks",
    "openrouter": "openrouter",
    "perplexity": "perplexity",
    "cohere": "cohere",
}


class GenAIPricesSource(PriceSource):
    """Prices with pydantic's ``genai-prices`` package (``pip install llm-prices-otel[genai-prices]``),
    using its bundled data or whatever snapshot it has loaded. Its version is
    ``genai-prices==<package version>``: the package does not expose a digest of its data."""

    pricing_id = "genai-prices"

    def __init__(self) -> None:
        try:
            import genai_prices
        except ImportError as e:  # pragma: no cover - depends on the extra
            raise ImportError("GenAIPricesSource needs genai-prices: pip install 'llm-prices-otel[genai-prices]'") from e
        self._gp = genai_prices
        self.pricing_version = f"genai-prices=={genai_prices.__version__}"

    def price(self, request: Request) -> Result:
        if request.batch:
            return r.MISSING_RATE
        if request.tier is not None:
            return r.UNPRICED_TIER
        provider_id = None
        if request.provider is not None:
            provider_id = GENAI_PRICES_PROVIDERS.get(request.provider)
            if provider_id is None:
                return r.UNKNOWN_PROVIDER
        t = request.tokens
        usage = self._gp.Usage(
            input_tokens=t.input + t.cache_read + t.cache_write,
            cache_read_tokens=t.cache_read,
            cache_write_tokens=t.cache_write,
            output_tokens=t.output,
        )
        for m in request.models:
            try:
                calc = self._gp.calc_price(usage, m, provider_id=provider_id)
            except LookupError:
                continue
            total = float(calc.total_price)
            return Priced(total=total, input=float(calc.input_price), output=float(calc.output_price), row=f"{calc.provider.id}/{calc.model.id}")
        return r.UNKNOWN_MODEL
