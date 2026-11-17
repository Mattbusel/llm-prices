"""Price rows, cost rules and model-id lookup. A port of the Rust crate's ``lib.rs`` and
``provider.rs``; the two must give the same answers (the tests check the cases the Rust tests
check)."""

from __future__ import annotations

import decimal
import math
from dataclasses import dataclass, field
from decimal import Decimal
from enum import Enum
from types import MappingProxyType
from typing import Any, Callable, Dict, Iterator, Literal, Mapping, Optional, Tuple, Union, overload

from . import _data

__all__ = [
    "GENERATED",
    "PRICES_VERSION",
    "CostBreakdown",
    "DecimalCostBreakdown",
    "LongContext",
    "Price",
    "Provider",
    "RateName",
    "Usage",
    "all_prices",
    "lookup",
    "lookup_provider",
]

#: Date the table was generated (ISO 8601).
GENERATED: str = _data.GENERATED

#: The table's version: ``"sha256:"`` and the SHA-256 of the prices.json it was generated
#: from (UTF-8, LF line endings). Every release states it in its notes; record it as
#: ``gen_ai.usage.cost.pricing.version`` next to costs computed from this table.
PRICES_VERSION: str = _data.PRICES_VERSION

#: A price column, for :meth:`Price.rate`, :meth:`Price.rate_str` and
#: :meth:`LongContext.rate`. ``"cache_write"`` also takes a cache lifetime (``"5m"``, ``"1h"``,
#: ``"30m"``, ``"12h"``).
RateName = Literal["input", "output", "cache_read", "cache_write", "batch_input", "batch_output"]

_RATES = ("input", "output", "cache_read", "cache_write", "batch_input", "batch_output")
_DIGITS = frozenset("0123456789")
_ASCII_LOWER = {c: c + 32 for c in range(ord("A"), ord("Z") + 1)}
_M = 1_000_000.0


def _ascii_lower(s: str) -> str:
    return s.translate(_ASCII_LOWER)


def _eq_ci(a: str, b: str) -> bool:
    return len(a) == len(b) and _ascii_lower(a) == _ascii_lower(b)


def _pick(tier: Any, rate: str, ttl: Optional[str]) -> Any:
    if rate not in _RATES:
        raise ValueError(f"unknown price column {rate!r}; expected one of {', '.join(_RATES)}")
    if rate == "cache_write":
        if ttl is None:
            raise ValueError('rate("cache_write") needs a cache lifetime such as "5m" or "1h"')
        return tier.cache_write.get(ttl)
    if ttl is not None:
        raise ValueError(f"only cache_write takes a cache lifetime, not {rate!r}")
    return getattr(tier, rate)


def _hashable(m: Mapping[str, float]) -> Tuple[Tuple[str, float], ...]:
    return tuple(m.items())


@dataclass(frozen=True)
class LongContext:
    """Prices that apply when a request is over a provider's long-context threshold."""

    #: Requests whose prompt is over this many tokens pay these prices. The prompt counts
    #: uncached input, cache reads and cache writes together.
    threshold_tokens: int
    #: Uncached input price above the threshold, USD per 1M tokens.
    input: Optional[float] = None
    #: Output price above the threshold (reasoning tokens included), USD per 1M tokens.
    output: Optional[float] = None
    #: Price of input tokens served from the prompt cache above the threshold.
    cache_read: Optional[float] = None
    #: Cache write price per cache lifetime, for example ``{"5m": 0.625}``.
    cache_write: Mapping[str, float] = field(default_factory=dict, hash=False)
    #: Batch API input price above the threshold, USD per 1M tokens.
    batch_input: Optional[float] = None
    #: Batch API output price above the threshold, USD per 1M tokens.
    batch_output: Optional[float] = None

    def __post_init__(self) -> None:
        object.__setattr__(self, "cache_write", MappingProxyType(dict(self.cache_write)))

    def __hash__(self) -> int:
        return hash((self.threshold_tokens, self.input, self.output, self.cache_read, _hashable(self.cache_write), self.batch_input, self.batch_output))

    def rate(self, rate: RateName, ttl: Optional[str] = None) -> Optional[float]:
        """The long-context price for one column, USD per 1M tokens. ``None`` when it is not
        listed. ``rate("cache_write", "5m")`` takes the cache lifetime as ``ttl``."""
        value: Optional[float] = _pick(self, rate, ttl)
        return value


@dataclass(frozen=True)
class Usage:
    """Token counts from one API response, the input to :meth:`Price.cost_usage`.

    Fields you do not set stay zero::

        Usage(input=1_000, output=200)

    Mapping from provider responses:

    =================  ====================================  ==============================================  ===============================================
    Field              Anthropic ``usage``                   OpenAI ``usage``                                Gemini ``usageMetadata``
    =================  ====================================  ==============================================  ===============================================
    ``input``          ``input_tokens``                      ``prompt_tokens - cached_tokens``               ``promptTokenCount - cachedContentTokenCount``
    ``cache_read``     ``cache_read_input_tokens``           ``prompt_tokens_details.cached_tokens``         ``cachedContentTokenCount``
    ``cache_write_5m`` ``cache_creation.ephemeral_5m_...``   0                                               0
    ``cache_write_1h`` ``cache_creation.ephemeral_1h_...``   0                                               0
    ``output``         ``output_tokens``                     ``completion_tokens``                           ``candidatesTokenCount + thoughtsTokenCount``
    =================  ====================================  ==============================================  ===============================================

    When an Anthropic response only reports the total ``cache_creation_input_tokens``, those
    tokens were written with the default 5-minute lifetime. Cache writes on other lifetimes
    (the OpenAI rows' ``"30m"`` and ``"12h"``) are not priced by ``cost_usage``, exactly as
    in the Rust crate: add ``tokens * price.cache_write["30m"] / 1e6`` yourself.

    Token counts must be non-negative integers.
    """

    #: Uncached input tokens. Do not include tokens counted in ``cache_read`` or the cache
    #: write fields.
    input: int = 0
    #: Output tokens, reasoning or thinking tokens included.
    output: int = 0
    #: Input tokens served from the prompt cache.
    cache_read: int = 0
    #: Input tokens written to the cache with a 5-minute lifetime.
    cache_write_5m: int = 0
    #: Input tokens written to the cache with a 1-hour lifetime.
    cache_write_1h: int = 0
    #: The request went through the provider's Batch API.
    batch: bool = False

    def __post_init__(self) -> None:
        for name in ("input", "output", "cache_read", "cache_write_5m", "cache_write_1h"):
            v = getattr(self, name)
            if isinstance(v, bool) or not isinstance(v, int):
                raise TypeError(f"Usage.{name} must be an int, not {type(v).__name__}")
            if v < 0:
                raise ValueError(f"Usage.{name} must not be negative ({v})")
        object.__setattr__(self, "batch", bool(self.batch))

    def prompt_tokens(self) -> int:
        """Total prompt tokens: uncached input plus cache reads plus cache writes. This is the
        number compared against a model's long-context threshold."""
        return self.input + self.cache_read + self.cache_write_5m + self.cache_write_1h


UsageLike = Union[Usage, Mapping[str, Any]]


def _as_usage(usage: UsageLike) -> Usage:
    if isinstance(usage, Usage):
        return usage
    if isinstance(usage, Mapping):
        return Usage(**usage)
    raise TypeError(f"expected a Usage or a mapping of its fields, not {type(usage).__name__}")


@dataclass(frozen=True)
class CostBreakdown:
    """Cost in USD of one response, split by token kind. Returned by :meth:`Price.cost_usage`."""

    #: Cost of the uncached input tokens.
    input: float
    #: Cost of the output tokens.
    output: float
    #: Cost of the input tokens served from the prompt cache.
    cache_read: float
    #: Cost of the cache writes, all lifetimes together.
    cache_write: float
    #: Sum of the other cost fields.
    total: float
    #: The prompt was over the model's long-context threshold, so long-context prices applied.
    long_context: bool


@dataclass(frozen=True)
class DecimalCostBreakdown:
    """:class:`CostBreakdown` in exact :class:`~decimal.Decimal`. Returned by
    :meth:`Price.cost_usage_decimal` and ``cost_usage(..., exact=True)``."""

    #: Cost of the uncached input tokens.
    input: Decimal
    #: Cost of the output tokens.
    output: Decimal
    #: Cost of the input tokens served from the prompt cache.
    cache_read: Decimal
    #: Cost of the cache writes, all lifetimes together.
    cache_write: Decimal
    #: Sum of the other cost fields.
    total: Decimal
    #: The prompt was over the model's long-context threshold, so long-context prices applied.
    long_context: bool


# Exact arithmetic: products and sums never round (60 digits is far beyond any real token
# count times any price); the batch discount on cache tokens is the one division, and rounds
# at 28 significant digits like the Rust crate's rust_decimal when it does not terminate.
_EXACT = decimal.Context(prec=60, rounding=decimal.ROUND_HALF_EVEN, Emax=decimal.MAX_EMAX, Emin=decimal.MIN_EMIN)
_DIV = decimal.Context(prec=28, rounding=decimal.ROUND_HALF_EVEN, Emax=decimal.MAX_EMAX, Emin=decimal.MIN_EMIN)
_PER_M = Decimal("0.000001")


def _dec(v: Optional[float]) -> Optional[Decimal]:
    """The decimal a price was written as: Python's repr is the shortest text that reads back
    to the same float, which for every price in the table is the text in prices.json."""
    if v is None or not math.isfinite(v):
        return None
    return Decimal(repr(float(v)))


def _normalize(d: Decimal) -> Decimal:
    """``d`` without trailing zeros, and never in exponent form for whole numbers."""
    d = d.normalize(_EXACT)
    if d.as_tuple().exponent > 0:  # type: ignore[operator]
        d = d.quantize(Decimal(1), context=_EXACT)
    return d


@dataclass(frozen=True)
class Price:
    """One model's prices in USD per 1M tokens. ``None`` means the page lists no price.

    Rows of the table come from :func:`lookup`, :func:`lookup_provider` and
    :func:`all_prices`. A ``Price`` you build yourself works with every method; only
    :meth:`rate_str` needs an unchanged row of the table.
    """

    #: The provider that sells the model at these prices: ``anthropic``, ``openai``,
    #: ``google``, ``deepseek``, ``mistral``, ``xai``, ``cohere``, ``groq``, ``together`` or
    #: ``fireworks`` (the :class:`Provider` values).
    provider: str
    #: The API model id.
    model: str
    #: Other ids the provider documents for the same model.
    aliases: Tuple[str, ...] = ()
    #: ``listed``, ``retired`` (still sold on some cloud platforms) or ``limited``.
    status: str = "listed"
    #: Uncached input price, USD per 1M tokens.
    input: Optional[float] = None
    #: Output price, USD per 1M tokens. Reasoning tokens bill at this price. ``None`` for
    #: embedding models.
    output: Optional[float] = None
    #: Price of input tokens served from the prompt cache, USD per 1M tokens.
    cache_read: Optional[float] = None
    #: Cache write price per cache lifetime, for example ``{"5m": 2.5, "1h": 4.0}``. Empty
    #: when the provider does not charge separately for cache writes.
    cache_write: Mapping[str, float] = field(default_factory=dict, hash=False)
    #: Batch API input price, USD per 1M tokens.
    batch_input: Optional[float] = None
    #: Batch API output price, USD per 1M tokens.
    batch_output: Optional[float] = None
    #: Prices for requests over a long-context threshold, when the model has one.
    long_context: Optional[LongContext] = None
    #: The provider's official pricing page this row was read from.
    source_url: str = ""
    #: ISO date the row was checked against ``source_url``.
    checked: str = ""
    #: The model's display name on the pricing page.
    name: Optional[str] = None
    #: Notes from the table (tiers, conditions, why a price is missing).
    note: Optional[str] = None
    #: Other service tiers the page prices, by name (``"flex"``, ``"priority"``, ``"fast"``,
    #: ``"ultrafast"``, ``"offpeak"``, ...), each a :class:`Price` with its own long-context
    #: prices where the page lists them. Use :meth:`tier` to read one.
    tiers: Mapping[str, "Price"] = field(default_factory=dict, hash=False, repr=False)
    #: Data residency multipliers on every price of the row, keyed by the provider's
    #: parameter value: ``{"us": 1.1}`` on Claude 4.6 and later (``inference_geo: "us"``).
    #: Empty when the page lists none for the model.
    inference_geo: Mapping[str, float] = field(default_factory=dict, hash=False)

    def __post_init__(self) -> None:
        object.__setattr__(self, "aliases", tuple(self.aliases))
        object.__setattr__(self, "cache_write", MappingProxyType(dict(self.cache_write)))
        object.__setattr__(self, "tiers", MappingProxyType(dict(self.tiers)))
        object.__setattr__(self, "inference_geo", MappingProxyType(dict(self.inference_geo)))

    def __hash__(self) -> int:
        return hash((self.provider, self.model, self.input, self.output, self.cache_read, _hashable(self.cache_write)))

    # -- prices -----------------------------------------------------------------------------

    def rate(self, rate: RateName, ttl: Optional[str] = None) -> Optional[float]:
        """The standard price for one column, USD per 1M tokens. ``None`` when it is not
        listed.

        >>> p = lookup("claude-sonnet-4-6")
        >>> p.rate("output"), p.rate("cache_write", "1h")
        (15.0, 6.0)
        """
        value: Optional[float] = _pick(self, rate, ttl)
        return value

    def rate_str(self, rate: RateName, ttl: Optional[str] = None) -> Optional[str]:
        """The standard price for one column exactly as prices.json writes it, for example
        ``"0.075"`` or ``"15"``. ``None`` when the price is not listed, or when this ``Price``
        is not a row of the table with that price unchanged.

        >>> lookup("gpt-4o-mini").rate_str("cache_read")
        '0.075'
        """
        row = _EXACT_ROWS.get((self.provider, self.model))
        return None if row is None else _checked_text(_pick(row[0], rate, ttl), self.rate(rate, ttl))

    def long_context_rate_str(self, rate: RateName, ttl: Optional[str] = None) -> Optional[str]:
        """The long-context price for one column exactly as prices.json writes it. ``None``
        when the model has no long-context tier, the price is not listed, or this ``Price`` is
        not an unchanged row of the table."""
        row = _EXACT_ROWS.get((self.provider, self.model))
        if row is None or row[1] is None or self.long_context is None:
            return None
        return _checked_text(_pick(row[1], rate, ttl), self.long_context.rate(rate, ttl))

    def rate_decimal(self, rate: RateName, ttl: Optional[str] = None) -> Optional[Decimal]:
        """The standard price for one column as an exact :class:`~decimal.Decimal`. Same
        number as :meth:`rate_str`, also for a ``Price`` built by hand.

        >>> lookup("gpt-4o-mini").rate_decimal("cache_read")
        Decimal('0.075')
        """
        return _dec(self.rate(rate, ttl))

    @property
    def input_per_token(self) -> Optional[float]:
        """:attr:`input` in USD per token (the price per 1M tokens divided by 1,000,000), for
        code that keeps per-token prices. ``None`` when the page lists no price.

        >>> lookup_provider("deepseek", "deepseek-chat").input_per_token
        3e-07
        """
        return None if self.input is None else self.input / _M

    @property
    def output_per_token(self) -> Optional[float]:
        """:attr:`output` in USD per token. ``None`` when the page lists no price (embedding
        models)."""
        return None if self.output is None else self.output / _M

    def per_token(self, rate: RateName = "input", ttl: Optional[str] = None) -> Optional[float]:
        """Any price column in USD per token: :meth:`rate` divided by 1,000,000.

        >>> lookup("claude-sonnet-4-6").per_token("cache_read")
        3e-07
        """
        value = self.rate(rate, ttl)
        return None if value is None else value / _M

    # -- tiers and data residency -------------------------------------------------------------

    def tier(self, name: Optional[str]) -> Optional["Price"]:
        """The prices of one service tier as a :class:`Price` (same provider and model), or
        ``self`` for ``None`` and ``"standard"``. ``None`` when the page lists no such tier
        for the model.

        Tier prices follow the same cost rules as the row: a tier that lists no cache-read
        price bills cache reads as its input, and tiers have no batch prices.

        >>> p = lookup("gemini-2.5-pro")
        >>> sorted(p.tiers)
        ['flex', 'priority']
        >>> pri = p.tier("priority")
        >>> pri.input, pri.output, pri.long_context.input
        (2.25, 18.0, 4.5)
        >>> lookup("claude-opus-5-5").tier("fast").output
        40.0
        """
        if name is None or name == "standard":
            return self
        return self.tiers.get(name)

    def with_inference_geo(self, geo: Optional[str]) -> Optional["Price"]:
        """This row with every price multiplied by the data residency multiplier for
        ``geo`` (tiers and long-context prices included). ``self`` for ``None`` and
        ``"global"``. ``None`` when the table lists no multiplier for ``geo`` on this model
        (Anthropic: models before Claude 4.6 reject the parameter).

        The products are exact decimals, so ``3 x 1.1`` is ``3.3``, not ``3.3000000000000003``.

        >>> us = lookup("claude-sonnet-4-6").with_inference_geo("us")
        >>> us.input, us.output, us.cache_read
        (3.3, 16.5, 0.33)
        >>> lookup("claude-haiku-4-5").with_inference_geo("us") is None
        True
        """
        if geo is None or geo == "global":
            return self
        m = self.inference_geo.get(geo)
        if m is None:
            return None
        return _scaled(self, _dec(m), f'inference_geo "{geo}" ({m}x)')

    def provider_kind(self) -> Optional["Provider"]:
        """:attr:`provider` as a :class:`Provider`. ``None`` only for a hand-built ``Price``
        with a provider name the package does not know."""
        return Provider.parse(self.provider)

    # -- costs ------------------------------------------------------------------------------

    @overload
    def cost(self, input_tokens: int, output_tokens: int, *, exact: Literal[False] = ...) -> Optional[float]: ...
    @overload
    def cost(self, input_tokens: int, output_tokens: int, *, exact: Literal[True]) -> Optional[Decimal]: ...

    def cost(self, input_tokens: int, output_tokens: int, *, exact: bool = False) -> Union[float, Decimal, None]:
        """Standard (non-batch, uncached) cost in USD of one request. Uses the long-context
        prices when ``input_tokens`` is over the model's threshold. ``None`` when a needed
        price is not listed. ``exact=True`` returns a :class:`~decimal.Decimal`.

        Same as :meth:`cost_usage` with only ``input`` and ``output`` set. When the response
        reports cached tokens, use ``cost_usage``: cached tokens count toward the long-context
        threshold and bill at their own price.

        >>> lookup("claude-haiku-5-5").cost(10_000, 2_000, exact=True)
        Decimal('0.002')
        """
        usage = Usage(input=input_tokens, output=output_tokens)
        c = self.cost_usage_decimal(usage) if exact else self.cost_usage(usage)
        return None if c is None else c.total

    def cost_decimal(self, input_tokens: int, output_tokens: int) -> Optional[Decimal]:
        """:meth:`cost` in exact decimals."""
        return self.cost(input_tokens, output_tokens, exact=True)

    @overload
    def cost_usage(self, usage: UsageLike, *, exact: Literal[False] = ..., tier: Optional[str] = ..., inference_geo: Optional[str] = ...) -> Optional[CostBreakdown]: ...
    @overload
    def cost_usage(self, usage: UsageLike, *, exact: Literal[True], tier: Optional[str] = ..., inference_geo: Optional[str] = ...) -> Optional[DecimalCostBreakdown]: ...

    def cost_usage(
        self,
        usage: UsageLike,
        *,
        exact: bool = False,
        tier: Optional[str] = None,
        inference_geo: Optional[str] = None,
    ) -> Union[CostBreakdown, DecimalCostBreakdown, None]:
        """Cost in USD of one response from its token counts, split by token kind.

        ``usage`` is a :class:`Usage` or a mapping of its fields (``{"input": 10, ...}``).
        ``exact=True`` returns a :class:`DecimalCostBreakdown` (see
        :meth:`cost_usage_decimal`). ``tier`` prices the request at a service tier
        (:meth:`tier`, for example ``"flex"`` or ``"priority"``) and ``inference_geo`` applies
        a data residency multiplier (:meth:`with_inference_geo`); either returns ``None`` when
        the table has no such tier or multiplier for the model.

        >>> u = Usage(input=300_000, output=1_000)
        >>> lookup("gemini-2.5-pro").cost_usage(u, tier="priority", exact=True).total
        Decimal('1.377')
        >>> fast_us = lookup("claude-opus-5-5").cost_usage(
        ...     Usage(input=1_000, output=1_000), tier="fast", inference_geo="us", exact=True)
        >>> fast_us.total
        Decimal('0.0528')

        Rules (the Rust crate's ``Price::cost_usage``):

        - **Long context.** When the model has a :class:`LongContext` tier and
          :meth:`Usage.prompt_tokens` is strictly greater than its threshold, every token
          bills at the long-context prices.
        - **Batch.** When ``usage.batch`` is set, input and output bill at the batch prices;
          cache reads and writes get the same discount as input (batch input price divided by
          standard input price).
        - **Cache reads** bill at the cache-read price, or as ordinary input when the model
          lists none.
        - **Cache writes** bill at ``cache_write["5m"]`` and ``cache_write["1h"]``.

        Returns ``None`` when a price it needs is not listed: the input or output price (batch
        prices when ``usage.batch`` is set, long-context prices above the threshold), or the
        cache-write price for a lifetime with a non-zero token count.

        >>> c = lookup("claude-sonnet-4-6").cost_usage(
        ...     Usage(input=2_048, cache_read=45_000, cache_write_5m=3_500, output=1_024))
        >>> round(c.total, 9)
        0.048129
        """
        if tier is not None or inference_geo is not None:
            base = self.tier(tier)
            priced = None if base is None else base.with_inference_geo(inference_geo)
            if priced is None:
                return None
            if exact:
                return priced.cost_usage_decimal(usage)
            return priced.cost_usage(usage)
        if exact:
            return self.cost_usage_decimal(usage)
        u = _as_usage(usage)
        r, lc = self._rates_for(u)
        standard_input = r.input
        if standard_input is None:
            return None
        if u.batch:
            if r.batch_input is None or r.batch_output is None:
                return None
            in_rate, out_rate = r.batch_input, r.batch_output
            discount = in_rate / standard_input if standard_input > 0.0 else 1.0
        else:
            if r.output is None:
                return None
            in_rate, out_rate, discount = standard_input, r.output, 1.0
        read_rate = in_rate if r.cache_read is None else r.cache_read * discount

        def write(ttl: str, tokens: int) -> Optional[float]:
            if tokens == 0:
                return 0.0
            rate = r.cache_write.get(ttl)
            return None if rate is None else float(tokens) * rate * discount

        w5, w1 = write("5m", u.cache_write_5m), write("1h", u.cache_write_1h)
        if w5 is None or w1 is None:
            return None
        # Each product is tokens x USD-per-1M; the total adds the products before dividing
        # once, in the same order as the Rust crate, so the floats are bit-for-bit equal.
        p_input = float(u.input) * in_rate
        p_read = float(u.cache_read) * read_rate
        p_write = w5 + w1
        p_output = float(u.output) * out_rate
        return CostBreakdown(
            input=p_input / _M,
            output=p_output / _M,
            cache_read=p_read / _M,
            cache_write=p_write / _M,
            total=(p_input + p_read + p_write + p_output) / _M,
            long_context=lc,
        )

    def cost_usage_decimal(self, usage: UsageLike) -> Optional[DecimalCostBreakdown]:
        """:meth:`cost_usage` in exact :class:`~decimal.Decimal`, for billing code that never
        touches floats.

        The prices are the decimals prices.json lists (:meth:`rate_decimal`) and the rules
        are those of ``cost_usage``, so it returns ``None`` in exactly the same cases. Each
        component is ``tokens x price / 1,000,000`` with no rounding. The only division is the
        batch discount on cache tokens; it is exact whenever the ratio terminates (50% or 80%
        on every current row) and otherwise rounds at 28 significant digits. Results carry no
        trailing zeros. Independent of the active :mod:`decimal` context.

        >>> c = lookup("claude-sonnet-4-6").cost_usage_decimal(
        ...     Usage(input=2_048, cache_read=45_000, cache_write_5m=3_500, output=1_024))
        >>> c.total
        Decimal('0.048129')
        """
        u = _as_usage(usage)
        r, lc = self._rates_for(u)
        standard_input = _dec(r.input)
        if standard_input is None:
            return None
        ctx = _EXACT
        if u.batch:
            batch_input, batch_output = _dec(r.batch_input), _dec(r.batch_output)
            if batch_input is None or batch_output is None:
                return None
            in_rate, out_rate = batch_input, batch_output
            num, den = (Decimal(1), Decimal(1)) if standard_input.is_zero() else (batch_input, standard_input)
        else:
            output = _dec(r.output)
            if output is None:
                return None
            in_rate, out_rate, num, den = standard_input, output, Decimal(1), Decimal(1)

        def times(tokens: int, rate: Decimal) -> Decimal:
            return ctx.multiply(Decimal(tokens), rate)

        def discounted(tokens: int, rate: Decimal) -> Decimal:
            p = times(tokens, rate)
            return p if num == den else _DIV.divide(ctx.multiply(p, num), den)

        cache_read = _dec(r.cache_read)
        p_read = times(u.cache_read, in_rate) if cache_read is None else discounted(u.cache_read, cache_read)

        def write(ttl: str, tokens: int) -> Optional[Decimal]:
            if tokens == 0:
                return Decimal(0)
            rate = _dec(r.cache_write.get(ttl))
            return None if rate is None else discounted(tokens, rate)

        w5, w1 = write("5m", u.cache_write_5m), write("1h", u.cache_write_1h)
        if w5 is None or w1 is None:
            return None
        p_input = times(u.input, in_rate)
        p_write = ctx.add(w5, w1)
        p_output = times(u.output, out_rate)
        total = ctx.add(ctx.add(ctx.add(p_input, p_read), p_write), p_output)

        def per_m(v: Decimal) -> Decimal:
            return _normalize(ctx.multiply(v, _PER_M))

        return DecimalCostBreakdown(
            input=per_m(p_input),
            output=per_m(p_output),
            cache_read=per_m(p_read),
            cache_write=per_m(p_write),
            total=per_m(total),
            long_context=lc,
        )

    def _rates_for(self, usage: Usage) -> Tuple[Any, bool]:
        lc = self.long_context
        if lc is not None and usage.prompt_tokens() > lc.threshold_tokens:
            return lc, True
        return self, False


def _times(v: Optional[float], m: Optional[Decimal]) -> Optional[float]:
    d = _dec(v)
    return None if d is None or m is None else float(_normalize(_EXACT.multiply(d, m)))


def _scaled(p: Price, m: Optional[Decimal], what: str) -> Price:
    """``p`` with every price times ``m``, tiers and long context included."""
    lc = p.long_context
    new_lc = None
    if lc is not None:
        new_lc = LongContext(
            threshold_tokens=lc.threshold_tokens,
            input=_times(lc.input, m),
            output=_times(lc.output, m),
            cache_read=_times(lc.cache_read, m),
            cache_write={k: float(_times(v, m) or 0.0) for k, v in lc.cache_write.items()},
            batch_input=_times(lc.batch_input, m),
            batch_output=_times(lc.batch_output, m),
        )
    return Price(
        provider=p.provider,
        model=p.model,
        aliases=p.aliases,
        status=p.status,
        input=_times(p.input, m),
        output=_times(p.output, m),
        cache_read=_times(p.cache_read, m),
        cache_write={k: float(_times(v, m) or 0.0) for k, v in p.cache_write.items()},
        batch_input=_times(p.batch_input, m),
        batch_output=_times(p.batch_output, m),
        long_context=new_lc,
        source_url=p.source_url,
        checked=p.checked,
        name=p.name,
        note=what if p.note is None else f"{what}; {p.note}",
        tiers={k: _scaled(t, m, what) for k, t in p.tiers.items()},
        # The multiplier is applied; applying it again would bill it twice.
        inference_geo={},
    )


@dataclass(frozen=True)
class _Text:
    """The exact decimal text of one tier of a table row."""

    input: Optional[str]
    output: Optional[str]
    cache_read: Optional[str]
    cache_write: Mapping[str, str]
    batch_input: Optional[str]
    batch_output: Optional[str]


def _checked_text(text: Optional[str], value: Optional[float]) -> Optional[str]:
    """``text`` when it is the decimal form of ``value``, so an edited price never reports
    the table's text."""
    if text is None or value is None:
        return None
    return text if float(text) == value else None


def _num(s: Optional[str]) -> Optional[float]:
    return None if s is None else float(s)


def _text(t: Mapping[str, Any]) -> _Text:
    return _Text(t["input"], t["output"], t["cache_read"], MappingProxyType(dict(t["cache_write"])), t["batch_input"], t["batch_output"])


def _tier_numbers(t: Mapping[str, Any]) -> Dict[str, Any]:
    return {
        "input": _num(t["input"]),
        "output": _num(t["output"]),
        "cache_read": _num(t["cache_read"]),
        "cache_write": {k: float(v) for k, v in t["cache_write"].items()},
        "batch_input": _num(t["batch_input"]),
        "batch_output": _num(t["batch_output"]),
    }


def _build() -> Tuple[Tuple[Price, ...], Dict[Tuple[str, str], Tuple[_Text, Optional[_Text]]]]:
    rows = []
    exact: Dict[Tuple[str, str], Tuple[_Text, Optional[_Text]]] = {}
    for r in _data.ROWS:
        lc_raw = r["long_context"]
        lc = None if lc_raw is None else LongContext(threshold_tokens=lc_raw["threshold_tokens"], **_tier_numbers(lc_raw))
        tiers: Dict[str, Price] = {}
        for tname, t in r["tiers"].items():
            t_lc = t["long_context"]
            tiers[tname] = Price(
                provider=r["provider"],
                model=r["model"],
                aliases=r["aliases"],
                status=r["status"],
                long_context=None if t_lc is None else LongContext(threshold_tokens=t_lc["threshold_tokens"], **_tier_numbers(t_lc)),
                source_url=r["source_url"],
                checked=r["checked"],
                name=r["name"],
                note=f"{tname} tier",
                inference_geo={k: float(v) for k, v in r["inference_geo"].items()},
                **_tier_numbers(t),
            )
        rows.append(
            Price(
                provider=r["provider"],
                model=r["model"],
                aliases=r["aliases"],
                status=r["status"],
                long_context=lc,
                source_url=r["source_url"],
                checked=r["checked"],
                name=r["name"],
                note=r["note"],
                tiers=tiers,
                inference_geo={k: float(v) for k, v in r["inference_geo"].items()},
                **_tier_numbers(r),
            )
        )
        exact[(r["provider"], r["model"])] = (_text(r), None if lc_raw is None else _text(lc_raw))
    return tuple(rows), exact


_PRICES, _EXACT_ROWS = _build()


def all_prices() -> Tuple[Price, ...]:
    """Every row in the table, in prices.json order: Anthropic, OpenAI, Google and DeepSeek
    first, then Amazon Bedrock and OpenRouter, then Mistral, xAI, Cohere, Groq, Together AI,
    Fireworks AI and Perplexity."""
    return _PRICES


# -- providers --------------------------------------------------------------------------------


class Provider(str, Enum):
    """A company that sells model access: a model maker's own API, a cloud platform that
    resells it, or a host for open-weight models. The value is the canonical lowercase name.

    >>> Provider.parse("vertex_ai")
    <Provider.VERTEX: 'vertex'>
    >>> Provider.parse("Together_AI") is Provider.TOGETHER
    True
    """

    ANTHROPIC = "anthropic"
    OPENAI = "openai"
    GOOGLE = "google"
    VERTEX = "vertex"
    BEDROCK = "bedrock"
    AZURE = "azure"
    DEEPSEEK = "deepseek"
    MISTRAL = "mistral"
    GROQ = "groq"
    TOGETHER = "together"
    FIREWORKS = "fireworks"
    XAI = "xai"
    COHERE = "cohere"
    OPENROUTER = "openrouter"
    PERPLEXITY = "perplexity"

    def __str__(self) -> str:
        return self.value

    def as_str(self) -> str:
        """The canonical lowercase name, the :attr:`Price.provider` of this provider's rows."""
        return self.value

    @classmethod
    def parse(cls, name: Union[str, "Provider"]) -> Optional["Provider"]:
        """Parses a provider name, ignoring ASCII case and treating ``_``, ``-`` and spaces
        alike. Accepts the canonical names and the spellings LiteLLM, LangChain, OpenRouter
        and the provider SDKs use (``gemini``, ``google_genai``, ``vertex_ai``,
        ``google_vertexai``, ``bedrock_converse``, ``azure_openai``, ``together_ai``,
        ``x-ai``, ...). ``None`` for anything else, including a value that is not a string.

        >>> Provider.parse("google_genai") is Provider.GOOGLE
        True
        >>> Provider.parse("ollama") is None
        True
        """
        if isinstance(name, Provider):
            return name
        if not isinstance(name, str):
            return None
        name = name.strip()
        if len(name.encode("utf-8")) > 24:
            return None
        key = _ascii_lower(name).replace("_", "-").replace(" ", "-")
        return _PROVIDER_NAMES.get(key)

    def rows(self) -> Iterator[Price]:
        """The rows this provider sells, in table order. Bedrock's rows are the models it prices
        itself (Amazon Nova, DeepSeek, Llama, Mistral, Qwen, ...); OpenRouter's are its own
        model list. Empty for Vertex and Azure, which have no rows of their own yet."""
        return (p for p in _PRICES if p.provider == self.value)

    def fallback_providers(self) -> Tuple["Provider", ...]:
        """The model makers whose list prices :meth:`lookup` falls back to when this provider
        has no row for a model: Vertex AI to Google and Anthropic, Bedrock to Anthropic, Azure
        to OpenAI, OpenRouter to every model maker in the table. Empty for every other
        provider."""
        return _FALLBACKS.get(self, ())

    def lookup(self, model: str) -> Optional[Price]:
        """Finds ``model`` among the rows this provider sells, with the same id handling as
        :func:`lookup`. When the provider has no row for it, the rows of its
        :meth:`fallback_providers` are tried, so Claude on Bedrock gets Anthropic's row.
        ``None`` when neither has the model.

        >>> Provider.BEDROCK.lookup("global.anthropic.claude-opus-4-5-20251101-v1:0").provider
        'anthropic'
        >>> Provider.GROQ.lookup("claude-opus-4-5") is None
        True
        """
        name = self.value
        lower = _ascii_lower(model.strip())
        host = _host_hint(lower)
        if host is not None and host is not self and self not in host.fallback_providers():
            # The id names another host or platform, which sells at its own price. Only this
            # provider's own ids that start with the same path match (OpenRouter lists
            # ``amazon/nova-2-lite-v1`` and ``perplexity/sonar-pro`` under those names).
            prefix = lower.split("/", 1)[0] + "/"
            return _resolve(model, lambda p: p.provider == name and _carries(p, prefix))
        found = _resolve(model, lambda p: p.provider == name)
        if found is not None:
            return found
        makers = {m.value for m in self.fallback_providers()}
        if not makers:
            return None
        return _resolve(model, lambda p: p.provider in makers)


_PROVIDER_NAMES: Dict[str, Provider] = {}
for _p, _names in (
    (Provider.ANTHROPIC, ("anthropic", "claude")),
    (Provider.OPENAI, ("openai", "open-ai")),
    (Provider.GOOGLE, ("google", "gemini", "google-ai-studio", "google-gemini", "ai-studio", "google-genai", "google-generativeai", "genai", "google-ai", "googleai")),
    (Provider.VERTEX, ("vertex", "vertex-ai", "vertexai", "google-vertex", "google-vertex-ai", "vertex-ai-beta", "google-vertexai")),
    (Provider.BEDROCK, ("bedrock", "aws", "aws-bedrock", "amazon-bedrock", "bedrock-converse", "bedrock-runtime", "amazon")),
    (Provider.AZURE, ("azure", "azure-openai", "azure-ai", "azure-foundry", "microsoft-foundry", "azure-ai-foundry")),
    (Provider.DEEPSEEK, ("deepseek",)),
    (Provider.MISTRAL, ("mistral", "mistralai", "mistral-ai")),
    (Provider.GROQ, ("groq", "groqcloud", "groq-cloud")),
    (Provider.TOGETHER, ("together", "together-ai", "togetherai")),
    (Provider.FIREWORKS, ("fireworks", "fireworks-ai", "fireworksai")),
    (Provider.XAI, ("xai", "x-ai", "grok")),
    (Provider.COHERE, ("cohere", "cohere-chat", "cohere-ai")),
    (Provider.OPENROUTER, ("openrouter", "open-router")),
    (Provider.PERPLEXITY, ("perplexity", "perplexity-ai", "perplexityai", "pplx")),
):
    for _n in _names:
        _PROVIDER_NAMES[_n] = _p
del _p, _names, _n

_FALLBACKS: Dict[Provider, Tuple[Provider, ...]] = {
    Provider.VERTEX: (Provider.GOOGLE, Provider.ANTHROPIC),
    Provider.BEDROCK: (Provider.ANTHROPIC,),
    Provider.AZURE: (Provider.OPENAI,),
    Provider.OPENROUTER: (
        Provider.ANTHROPIC,
        Provider.OPENAI,
        Provider.GOOGLE,
        Provider.DEEPSEEK,
        Provider.MISTRAL,
        Provider.XAI,
        Provider.COHERE,
    ),
}

_PLATFORMS = frozenset({"vertex", "bedrock", "azure", "openrouter"})

#: Providers that sell other companies' models at their own prices: the cloud platforms and
#: the open-weight hosts. An id that starts with one of these is only priced at that provider.
_HOSTS = frozenset({Provider.VERTEX, Provider.BEDROCK, Provider.AZURE, Provider.OPENROUTER, Provider.GROQ, Provider.TOGETHER, Provider.FIREWORKS, Provider.PERPLEXITY})


def _id_hint(lower: str) -> Optional[Tuple[Provider, str]]:
    """The provider an id names at its start (``groq/...``, ``accounts/fireworks/...``) and
    the rest of the id, or ``None``."""
    if lower.startswith("accounts/fireworks/"):
        return (Provider.FIREWORKS, lower)
    if "/" in lower:
        head, rest = lower.split("/", 1)
        provider = Provider.parse(head)
        if provider is not None:
            return (provider, rest)
    return None


def _host_hint(lower: str) -> Optional[Provider]:
    """The host or platform an id names at its start, or ``None``."""
    hint = _id_hint(lower)
    return hint[0] if hint is not None and hint[0] in _HOSTS else None


# -- model ids --------------------------------------------------------------------------------

_PREFIXES = (
    "models/",
    "anthropic/",
    "openai/",
    "google/",
    "gemini/",
    "vertex_ai/",
    "bedrock/",
    "azure/",
    "openrouter/",
    "deepseek/",
    "mistral/",
    "mistralai/",
    "xai/",
    "x-ai/",
    "cohere/",
    "groq/",
    "together/",
    "together_ai/",
    "fireworks/",
    "fireworks_ai/",
    "accounts/fireworks/models/",
    "perplexity/",
    # Bedrock's vendor prefix with an optional cross-region inference profile.
    "us.anthropic.",
    "eu.anthropic.",
    "apac.anthropic.",
    "global.anthropic.",
    "anthropic.",
)


def _digits(s: str) -> bool:
    return all(c in _DIGITS for c in s)


def _strip_prefix(s: str) -> str:
    while True:
        before = s
        for p in _PREFIXES:
            if len(s) >= len(p) and _eq_ci(s[: len(p)], p):
                s = s[len(p) :]
        if s == before:
            return s


def _strip_date(id: str) -> Optional[str]:
    """``id`` without a trailing ``-YYYYMMDD`` (Anthropic) or ``-YYYY-MM-DD`` (OpenAI)."""
    if "-" not in id:
        return None
    head, tail = id.rsplit("-", 1)
    if len(tail) == 8 and tail.startswith("20") and _digits(tail):
        return head
    if "-" not in head:
        return None
    rest, month = head.rsplit("-", 1)
    if "-" not in rest:
        return None
    base, year = rest.rsplit("-", 1)
    shape = len(tail) == 2 and len(month) == 2 and len(year) == 4 and year.startswith("20")
    return base if shape and _digits(tail) and _digits(month) and _digits(year) and base else None


def _strip_version(id: str) -> str:
    """``id`` without a trailing ``-latest``, then without a trailing snapshot date."""
    s = id[:-7] if len(id) > 7 and _eq_ci(id[-7:], "-latest") else id
    d = _strip_date(s)
    return s if d is None else d


def _ids(p: Price) -> Iterator[str]:
    yield p.model
    yield from p.aliases


def _normalize_id(lower: str) -> str:
    """The id without provider prefixes, without Bedrock's ``-v1:0`` suffix, and with
    Vertex's ``@`` turned into ``-``."""
    s = _strip_prefix(lower)
    i = s.find("-v")
    if i >= 0:
        rest = s[i + 2 :]
        if rest and rest[0] in _DIGITS and "-" not in rest:
            s = s[:i]
    return s.replace("@", "-", 1)


def _carries(p: Price, prefix: str) -> bool:
    """The row's id or an alias starts with ``prefix`` (``openai/``), ignoring ASCII case."""
    return any(_ascii_lower(m).startswith(prefix) for m in _ids(p))


def _exact(lower: str, keep: Callable[[Price], bool]) -> Optional[Price]:
    """The first kept row whose id or an alias is ``lower``, ignoring ASCII case."""
    for p in _PRICES:
        if keep(p) and any(_eq_ci(m, lower) for m in _ids(p)):
            return p
    return None


def _resolve(id: str, keep: Callable[[Price], bool]) -> Optional[Price]:
    rows = [p for p in _PRICES if keep(p)]
    lower = _ascii_lower(id.strip())
    for p in rows:
        if any(_eq_ci(m, lower) for m in _ids(p)):
            return p
    s = _normalize_id(lower)
    if not s:
        return None

    def stripped_eq(want: str) -> Optional[Price]:
        for p in rows:
            if any(_eq_ci(_strip_prefix(m), want) for m in _ids(p)):
                return p
        return None

    found = stripped_eq(s)
    if found is not None:
        return found
    base = _strip_version(s)
    if base != s:
        found = stripped_eq(base)
        if found is not None:
            return found
    for p in rows:
        for m in _ids(p):
            u = _strip_date(_strip_prefix(m))
            if u is not None and _eq_ci(u, base):
                return p
    return None


def lookup(model: str) -> Optional[Price]:
    """Finds a model by id across every provider in the table.

    Accepts the documented id or an alias in any ASCII case, provider prefixes
    (``anthropic/``, ``models/``, ``openrouter/``, LiteLLM's ``together_ai/``, ...),
    Fireworks' ``accounts/fireworks/models/`` path, Bedrock ids
    (``us.anthropic.claude-...-v1:0``), Vertex ``@date`` suffixes, dated snapshot ids
    (``gpt-4o-2024-08-06`` finds ``gpt-4o``), ``-latest`` ids, and the undated form of a
    dated id.

    When the id starts with a provider name and a slash (``groq/``, ``together_ai/``, ...),
    that provider is tried first. A host or platform path (``groq/``, ``together_ai/``,
    ``fireworks_ai/``, ``accounts/fireworks/models/``, ``perplexity/``, ``bedrock/``,
    ``vertex_ai/``, ``azure/``, ``openrouter/``) is priced at that host's own row
    (``bedrock/us.deepseek.r1-v1:0``, ``openrouter/meta-llama/llama-3.3-70b-instruct``), or at
    the model maker's list price where the platform charges it
    (:meth:`Provider.fallback_providers`); otherwise the result is ``None``, never another
    provider's row. A model maker's prefix (``anthropic/``, ``openai/``, ...) on a model that
    maker does not sell only matches rows whose own id carries the same prefix (Groq's
    ``openai/gpt-oss-120b``, then OpenRouter's ``x-ai/grok-4.1-fast`` by its exact id).
    Without a provider prefix the first matching row in table order wins: model makers
    before the hosts that resell open-weight models; Bedrock and OpenRouter rows only match
    their exact id there. Use :func:`lookup_provider` whenever you know which provider served
    the request.

    >>> lookup("accounts/fireworks/models/deepseek-r1") is None  # Fireworks' price, not DeepSeek's
    True
    >>> lookup("bedrock/us.deepseek.r1-v1:0").input  # Bedrock's own price for DeepSeek-R1
    1.35

    >>> lookup("gpt-4o-2024-08-06").model
    'gpt-4o'
    >>> lookup("mistral-large-latest").model
    'mistral-large-4'
    >>> lookup("not-a-model") is None
    True
    """
    lower = _ascii_lower(model.strip())
    hint = _id_hint(lower)
    if hint is not None:
        found = hint[0].lookup(hint[1])
        if found is not None:
            return found
        if hint[0] in _HOSTS:
            # A host or platform path: that host's price or nothing. Its price for a model is
            # not the model maker's, so falling back to the maker's row would be wrong.
            return None
        # A model maker's prefix on a model it does not sell (``openai/gpt-oss-120b``) only
        # matches rows whose own id starts with that prefix, such as a host's open-weight row.
        # A platform's own rows (OpenRouter's ``x-ai/grok-4.1-fast``) only match their exact id.
        prefix = lower.split("/", 1)[0] + "/"
        return _resolve(lower, lambda p: p.provider not in _PLATFORMS and _carries(p, prefix)) or _exact(lower, lambda p: p.provider in _PLATFORMS)
    # Without a provider path, the rows of Bedrock and OpenRouter, which resell other makers'
    # models, are only found by their exact id (``deepseek.r1-v1:0``,
    # ``meta-llama/llama-3.3-70b-instruct``), so ``gpt-4`` never takes OpenRouter's row.
    return _resolve(lower, lambda p: p.provider not in _PLATFORMS) or _exact(lower, lambda p: p.provider in _PLATFORMS)


def lookup_provider(provider: Union[str, Provider], model: str) -> Optional[Price]:
    """Finds ``model`` as sold by ``provider``, or ``None``.

    This is the call to make when you know which provider served a request (a gateway, a
    LangChain or LiteLLM provider name, a cost callback):

    - ``provider`` is a :class:`Provider` or any spelling :meth:`Provider.parse` accepts:
      ``"openai"``, ``"anthropic"``, ``"google_genai"``, ``"gemini"``, ``"vertex_ai"``,
      ``"google_vertexai"``, ``"bedrock"``, ``"bedrock_converse"``, ``"azure_openai"``,
      ``"deepseek"``, ``"mistralai"``, ``"groq"``, ``"xai"``, ``"together"``,
      ``"together_ai"``, ``"fireworks"``, ``"cohere"``, ``"openrouter"``, ``"perplexity"``,
      in any ASCII case.
    - ``model`` gets the same id handling as :func:`lookup`: aliases (``deepseek-chat``,
      ``mistral-large-latest``), dated snapshots, ``-latest`` ids, Bedrock ids
      (``us.anthropic.claude-...-v1:0``), Vertex ``@date`` ids and provider prefixes.
    - Only rows that provider sells are searched. Bedrock and OpenRouter have rows of their
      own (Bedrock's from the AWS price list, ``lookup_provider("bedrock",
      "us.deepseek.r1-v1:0")``; OpenRouter's from its model list,
      ``lookup_provider("openrouter", "deepseek/deepseek-r1-0528")``). For a model they have no
      row for, and for Vertex AI and Azure, the list price of the model maker they resell is
      used (see :meth:`Provider.fallback_providers`); the returned row's
      :attr:`Price.provider` says whose price it is. A host is never priced with another
      host's row.
    - ``None`` when the provider name is unknown (``"ollama"``, ``"huggingface"``) or the
      table has no price for the model there, so the caller can fall back to its own default.

    The returned :class:`Price` holds USD per 1M tokens (``price.input``, ``price.output``);
    ``price.input_per_token`` and ``price.output_per_token`` give USD per token, and
    ``price.cost(input_tokens, output_tokens)`` the cost of one request.

    >>> lookup_provider("groq", "openai/gpt-oss-120b").provider
    'groq'
    >>> lookup_provider("openai", "gpt-oss-120b") is None
    True
    >>> p = lookup_provider("google_genai", "gemini-2.5-flash")
    >>> p.input, p.output
    (0.3, 2.5)
    >>> lookup_provider("bedrock", "us.anthropic.claude-sonnet-4-20250514-v1:0").model
    'claude-sonnet-4-20250514'
    >>> lookup_provider("ollama", "llama3") is None
    True
    """
    p = Provider.parse(provider)
    return None if p is None else p.lookup(model)
