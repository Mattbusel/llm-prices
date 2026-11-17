"""The ``tokencost`` API on the llm-prices table.

Same function names, signatures, return types and errors as ``tokencost`` 0.1.26, so moving
over is one import line::

    # from tokencost import calculate_prompt_cost, calculate_completion_cost
    from llm_prices.compat.tokencost import calculate_prompt_cost, calculate_completion_cost

What changes is where the prices come from: the llm-prices table, read from each provider's
official pricing page and dated per row, instead of a copy of LiteLLM's map. Model ids resolve
the way :func:`llm_prices.lookup` resolves them (dated ids, ``azure/``, ``openai/`` and other
prefixes, Bedrock and Vertex ids), so ``"claude-sonnet-4-6"`` and
``"anthropic.claude-sonnet-4-6"`` both work.

Token counting needs ``tiktoken`` (``pip install "llm-prices[tokencost]"``); pricing a token
count does not. Counting Claude messages calls Anthropic's token counting API, as tokencost
does, and needs the ``anthropic`` package and an API key.

>>> from llm_prices.compat.tokencost import calculate_cost_by_tokens
>>> calculate_cost_by_tokens(10, "gpt-4o", "input")
Decimal('0.0000250')
>>> calculate_cost_by_tokens(100, "gpt-4o", "cached")
Decimal('0.00012500')
>>> calculate_cost_by_tokens(1_000, "claude-sonnet-4-6", "output") == Decimal("0.015")
True

Differences from tokencost, all on purpose:

- Costs are exact: the per-token price as a decimal times the token count, as in tokencost,
  but the per-token price comes from the table's decimal text.
- ``update_token_costs`` and ``refresh_prices`` fetch nothing. Prices ship with the package and
  change when you upgrade it; the table is re-read from the provider pages every day.
- Message counts for models outside OpenAI and Anthropic (Gemini, DeepSeek, Mistral, ...) are
  tiktoken estimates with a warning, where tokencost raises ``KeyError``. Unknown models still
  raise ``KeyError``.
"""

from __future__ import annotations

import logging
import re
from decimal import Decimal
from typing import Any, Dict, List, Literal, Optional, Pattern, Tuple, Union

from .._core import Price, lookup
from ..litellm import model_cost

__all__ = [
    "LEGACY_MODELS",
    "LEGACY_REDIRECTS",
    "TOKEN_COSTS",
    "TOKEN_COSTS_STATIC",
    "TokenType",
    "calculate_all_costs_and_tokens",
    "calculate_completion_cost",
    "calculate_cost_by_tokens",
    "calculate_prompt_cost",
    "configure_model",
    "count_message_tokens",
    "count_string_tokens",
    "count_tokens",
    "refresh_prices",
    "register_model_pattern",
    "update_token_costs",
]

logger = logging.getLogger(__name__)

TokenType = Literal["input", "output", "cached"]

_FIELDS = {
    "input": "input_cost_per_token",
    "output": "output_cost_per_token",
    "cached": "cache_read_input_token_cost",
}

#: The table as tokencost's ``TOKEN_COSTS``: LiteLLM-schema entries in USD per token, keyed by
#: model id. You can add or change entries, as with tokencost.
TOKEN_COSTS_STATIC: Dict[str, Dict[str, Any]] = model_cost()
TOKEN_COSTS: Dict[str, Dict[str, Any]] = {k: dict(v) for k, v in TOKEN_COSTS_STATIC.items()}

MODEL_PRICE_PATTERNS: List[Tuple[Pattern[str], Dict[str, Any]]] = []

# -- retired and legacy models ------------------------------------------------------------------

#: Retired and legacy ids that tokencost priced and the current table does not (claude-3-x,
#: claude-2, gpt-4, gpt-4-32k, gpt-3.5-turbo snapshots, chatgpt-4o-latest, o1-mini,
#: o1-preview, gpt-4.5-preview, the search, audio and realtime previews, o3 and o4-mini deep
#: research, gemini-1.5 and 2.0, gemini-2.5 preview ids, older Mistral, Grok 2 and 3, Cohere
#: Command and Embed): id -> the row, with ``source_url`` (an archived snapshot of the
#: provider's own pricing page), ``source_date``, ``retired`` and ``note``. USD per 1M tokens.
#: Ids the table prices today are never taken from here.
LEGACY_MODELS: Dict[str, Dict[str, Any]] = {}

#: Retired ids the provider now serves and bills as another model (``grok-3`` -> ``grok-4.3``):
#: id -> ``{"to", "source_url", "note"}``. They are priced as the model they redirect to.
LEGACY_REDIRECTS: Dict[str, Dict[str, Any]] = {}

_PER_TOKEN = Decimal(1_000_000)


def _per_token(v: Any) -> Optional[float]:
    return None if v is None else float(Decimal(str(v)) / _PER_TOKEN)


def _legacy_entry(r: Dict[str, Any]) -> Dict[str, Any]:
    e: Dict[str, Any] = {
        "input_cost_per_token": _per_token(r["input"]),
        "output_cost_per_token": _per_token(r["output"]) if r["output"] is not None else 0.0,
        "litellm_provider": {"google": "gemini", "cohere": "cohere_chat"}.get(r["provider"], r["provider"]),
        "mode": "embedding" if r["output"] is None else "chat",
        "source": r["source_url"],
        "llm_prices_checked": r["source_date"],
        "llm_prices_legacy": True,
    }
    for key, field in (("cache_read_input_token_cost", "cache_read"), ("input_cost_per_token_batches", "batch_input"), ("output_cost_per_token_batches", "batch_output")):
        if r.get(field) is not None:
            e[key] = _per_token(r[field])
    lc = r.get("long_context")
    if lc:
        k = f"above_{int(lc['threshold_tokens']) // 1000}k_tokens"
        e[f"input_cost_per_token_{k}"] = _per_token(lc["input"])
        e[f"output_cost_per_token_{k}"] = _per_token(lc["output"])
        if lc.get("cache_read") is not None:
            e[f"cache_read_input_token_cost_{k}"] = _per_token(lc["cache_read"])
    if r.get("retired"):
        e["deprecation_date"] = r["retired"]
    return e


def _load_legacy() -> None:
    import json
    from importlib import resources

    doc = json.loads(resources.files(__package__).joinpath("tokencost_legacy.json").read_text(encoding="utf-8"))
    for r in doc["models"]:
        entry = _legacy_entry(r)
        for model_id in [r["model"], *r["aliases"]]:
            key = model_id.lower()
            if key in TOKEN_COSTS_STATIC or lookup(key) is not None:
                continue  # the current table prices it
            LEGACY_MODELS[key] = r
            TOKEN_COSTS_STATIC[key] = entry
            TOKEN_COSTS[key] = dict(entry)
    for model_id, red in doc["redirects"].items():
        key = model_id.lower()
        target = lookup(red["to"])
        if key in TOKEN_COSTS_STATIC or lookup(key) is not None or target is None or _key_of(target) not in TOKEN_COSTS_STATIC:
            continue
        LEGACY_REDIRECTS[key] = red
        entry = dict(TOKEN_COSTS_STATIC[_key_of(target)], llm_prices_redirect=red["to"], llm_prices_legacy=True)
        TOKEN_COSTS_STATIC[key] = entry
        TOKEN_COSTS[key] = dict(entry)

# Model ids by row, to map a resolved Price back to its TOKEN_COSTS key.
_PREFIX = {
    "google": "gemini/",
    "deepseek": "deepseek/",
    "mistral": "mistral/",
    "xai": "xai/",
    "groq": "groq/",
    "together": "together_ai/",
    "fireworks": "fireworks_ai/",
}


def _key_of(p: Price) -> str:
    return _PREFIX.get(p.provider, "") + p.model


def _to_per_token(cost_per_1k_tokens: Union[int, float, Decimal]) -> float:
    return float(Decimal(str(cost_per_1k_tokens)) / Decimal(1000))


def configure_model(
    model_name: str,
    input_cost_per_1k_tokens: Union[int, float, Decimal],
    output_cost_per_1k_tokens: Union[int, float, Decimal],
    *,
    max_input_tokens: Optional[int] = None,
    max_output_tokens: Optional[int] = None,
    litellm_provider: Optional[str] = None,
    mode: str = "chat",
) -> None:
    """Register or override pricing for one model name (case-insensitive), in USD per 1K
    tokens, as tokencost's ``configure_model``."""
    key = model_name.lower()
    entry: Dict[str, Any] = {
        "input_cost_per_token": _to_per_token(input_cost_per_1k_tokens),
        "output_cost_per_token": _to_per_token(output_cost_per_1k_tokens),
        "mode": mode,
    }
    if max_input_tokens is not None:
        entry["max_input_tokens"] = int(max_input_tokens)
    if max_output_tokens is not None:
        entry["max_output_tokens"] = int(max_output_tokens)
    if litellm_provider is not None:
        entry["litellm_provider"] = litellm_provider
    TOKEN_COSTS[key] = entry


def register_model_pattern(
    pattern: str,
    input_cost_per_1k_tokens: Union[int, float, Decimal],
    output_cost_per_1k_tokens: Union[int, float, Decimal],
    *,
    max_input_tokens: Optional[int] = None,
    max_output_tokens: Optional[int] = None,
    litellm_provider: Optional[str] = None,
    mode: str = "chat",
) -> None:
    """Price every model matching a ``*`` wildcard pattern, as tokencost's
    ``register_model_pattern``. Table rows still win over patterns."""
    compiled = re.compile("^" + re.escape(pattern.lower()).replace(r"\*", ".*") + "$")
    entry: Dict[str, Any] = {
        "input_cost_per_token": _to_per_token(input_cost_per_1k_tokens),
        "output_cost_per_token": _to_per_token(output_cost_per_1k_tokens),
        "mode": mode,
    }
    if max_input_tokens is not None:
        entry["max_input_tokens"] = int(max_input_tokens)
    if max_output_tokens is not None:
        entry["max_output_tokens"] = int(max_output_tokens)
    if litellm_provider is not None:
        entry["litellm_provider"] = litellm_provider
    MODEL_PRICE_PATTERNS.append((compiled, entry))


_load_legacy()


def strip_ft_model_name(model: str) -> str:
    """``ft:gpt-4o-mini-2024-07-18:org:suffix:id`` -> ``gpt-4o-mini-2024-07-18``: a fine-tuned
    model is looked up by its base model."""
    if model.startswith("ft:"):
        parts = model.split(":")
        if len(parts) > 1 and parts[1]:
            return parts[1]
    return model


def _resolve(model: str) -> Optional[str]:
    """The TOKEN_COSTS key for ``model``, or ``None``."""
    if model.lower() in TOKEN_COSTS:  # fine-tune prices such as "ft:gpt-3.5-turbo"
        return model.lower()
    m = strip_ft_model_name(model.lower())
    if m in TOKEN_COSTS:
        return m
    if m.startswith("bedrock/") and m.split("/", 1)[1] in TOKEN_COSTS:
        return m.split("/", 1)[1]
    p = lookup(m)
    if p is not None and _key_of(p) in TOKEN_COSTS:
        return _key_of(p)
    if "/" in m and m.rsplit("/", 1)[1] in TOKEN_COSTS:
        return m.rsplit("/", 1)[1]
    for regex, entry in MODEL_PRICE_PATTERNS:
        if regex.match(m):
            TOKEN_COSTS[m] = dict(entry)
            return m
    return None


def _not_implemented(model: str) -> KeyError:
    return KeyError(
        f"Model {model} is not implemented. Double-check your spelling, or see "
        "https://github.com/Mattbusel/llm-prices for the models the table prices."
    )


# -- token counting -----------------------------------------------------------------------------


def _tiktoken() -> Any:
    try:
        import tiktoken  # type: ignore[import-not-found, unused-ignore]
    except ImportError as e:  # pragma: no cover - exercised without tiktoken installed
        raise ImportError('token counting needs tiktoken: pip install "llm-prices[tokencost]"') from e
    return tiktoken


_O200K = re.compile(r"^(gpt-4o|gpt-4\.1|gpt-4\.5|gpt-5|gpt-6|chatgpt|o[0-9])")


def _encoding(model: str) -> Any:
    tiktoken = _tiktoken()
    try:
        return tiktoken.encoding_for_model(model)
    except KeyError:
        name = "o200k_base" if _O200K.match(model) else "cl100k_base"
        logger.warning("Model %s not known to tiktoken. Using %s encoding.", model, name)
        return tiktoken.get_encoding(name)


def _is_openai_name(model: str) -> bool:
    return bool(re.match(r"^(gpt-|chatgpt|o[0-9]|text-|davinci|babbage|ft:gpt)", model))


def _anthropic_message_tokens(messages: List[Dict[str, str]], model: str) -> int:
    try:
        import anthropic  # type: ignore[import-not-found, unused-ignore]
    except ImportError as e:  # pragma: no cover
        raise ImportError(
            "counting Claude message tokens calls Anthropic's token counting API, as tokencost "
            "does: pip install anthropic and set ANTHROPIC_API_KEY. Or price the usage the "
            "response reports with llm_prices.lookup(model).cost_usage(...)."
        ) from e
    return int(anthropic.Anthropic().messages.count_tokens(model=model, messages=messages).input_tokens)


def count_message_tokens(messages: List[Dict[str, str]], model: str) -> int:
    """Number of tokens in a list of chat messages, with OpenAI's per-message overhead, as
    tokencost's ``count_message_tokens``. Raises ``KeyError`` for a model the table does not
    know."""
    model = strip_ft_model_name(model.lower())
    if "claude-" in model and not model.startswith("anthropic."):
        logger.warning("Anthropic token counting uses Anthropic's API; expect small differences in costs.")
        return _anthropic_message_tokens(messages, model)
    bare = model.rsplit("/", 1)[-1] if "/" in model else model
    if not _is_openai_name(bare):
        if not model or _resolve(model) is None:
            raise KeyError(f"count_message_tokens() is not implemented for model {model}.")
        logger.warning("%s is not an OpenAI model; its token count is a tiktoken estimate.", model)
    encoding = _encoding(bare)
    if bare == "gpt-3.5-turbo-0301":
        tokens_per_message, tokens_per_name = 4, -1
    else:
        tokens_per_message, tokens_per_name = 3, 1
    num_tokens = 0
    for message in messages:
        num_tokens += tokens_per_message
        for key, value in message.items():
            num_tokens += len(encoding.encode(value))
            if key == "name":
                num_tokens += tokens_per_name
    num_tokens += 3  # every reply is primed with <|start|>assistant<|message|>
    return num_tokens


def count_string_tokens(prompt: str, model: str) -> int:
    """Number of tokens in a text string, as tokencost's ``count_string_tokens``. Unknown models
    use tiktoken's ``cl100k_base`` encoding; Claude models raise ``ValueError`` (use
    :func:`count_message_tokens`)."""
    model = model.lower()
    if "/" in model:
        model = model.split("/")[-1]
    if "claude-" in model:
        raise ValueError(
            "Anthropic does not support this method. Use count_message_tokens for exact counts."
        )
    return len(_encoding(model).encode(prompt))


def count_tokens(text: Union[str, List[Dict[str, str]]], model: str) -> int:
    """:func:`count_string_tokens` for a string, :func:`count_message_tokens` for a list of
    messages."""
    if isinstance(text, str) and "claude-" not in model:
        return count_string_tokens(text, model)
    if isinstance(text, str):
        text = [{"role": "user", "content": text}]
    return count_message_tokens(text, model)


# -- costs ------------------------------------------------------------------------------------


def calculate_cost_by_tokens(num_tokens: int, model: str, token_type: TokenType) -> Decimal:
    """Cost in USD of ``num_tokens`` tokens of one kind: ``"input"``, ``"output"`` or
    ``"cached"`` (prompt-cache reads). Raises ``KeyError`` for an unknown model or a price the
    table does not list, ``ValueError`` for another token type."""
    if token_type not in _FIELDS:
        raise ValueError(f"Invalid token type: {token_type}.")
    key = _resolve(model)
    if key is None:
        raise _not_implemented(model)
    value = TOKEN_COSTS[key].get(_FIELDS[token_type])
    if value is None:
        raise KeyError(f"Model {model} does not have cost data for `{token_type}` tokens.")
    return Decimal(str(value)) * Decimal(num_tokens)


def calculate_prompt_cost(prompt: Union[List[dict], str], model: str) -> Decimal:  # type: ignore[type-arg]
    """Cost in USD of a prompt: a string or a list of chat messages."""
    model = strip_ft_model_name(model.lower())
    key = _resolve(model)
    if key is None:
        raise _not_implemented(model)
    if not isinstance(prompt, (list, str)):
        raise TypeError(f"Prompt must be either a string or list of message objects but found {type(prompt)} instead.")
    if isinstance(prompt, str) and "claude-" not in model:
        prompt_tokens = count_string_tokens(prompt, model)
    elif isinstance(prompt, str):
        prompt_tokens = count_message_tokens([{"role": "user", "content": prompt}], model)
    else:
        prompt_tokens = count_message_tokens(prompt, model)
    return calculate_cost_by_tokens(prompt_tokens, key, "input")


def calculate_completion_cost(completion: str, model: str) -> Decimal:
    """Cost in USD of a completion string."""
    model = strip_ft_model_name(model)
    key = _resolve(model)
    if key is None:
        raise _not_implemented(model)
    if not isinstance(completion, str):
        raise TypeError(f"Prompt must be a string but found {type(completion)} instead.")
    if "claude-" in model.lower():
        # Anthropic adds about 13 tokens of framing to a one-message count.
        completion_tokens = count_message_tokens([{"role": "assistant", "content": completion}], model) - 13
    else:
        completion_tokens = count_string_tokens(completion, model)
    return calculate_cost_by_tokens(completion_tokens, key, "output")


def calculate_all_costs_and_tokens(prompt: Union[List[dict], str], completion: str, model: str) -> dict:  # type: ignore[type-arg]
    """``{"prompt_cost", "prompt_tokens", "completion_cost", "completion_tokens"}`` for one
    call."""
    prompt_cost = calculate_prompt_cost(prompt, model)
    completion_cost = calculate_completion_cost(completion, model)
    if isinstance(prompt, str):
        prompt_tokens = count_tokens(prompt, model)
    else:
        prompt_tokens = count_message_tokens(prompt, model)
    if "claude-" in model.lower() and not model.lower().startswith("anthropic."):
        completion_tokens = count_message_tokens([{"role": "assistant", "content": completion}], model) - 13
    else:
        completion_tokens = count_string_tokens(completion, model)
    return {
        "prompt_cost": prompt_cost,
        "prompt_tokens": prompt_tokens,
        "completion_cost": completion_cost,
        "completion_tokens": completion_tokens,
    }


async def update_token_costs() -> Dict[str, Dict[str, Any]]:
    """Kept for tokencost compatibility. Fetches nothing: the prices ship with the package.
    Returns :data:`TOKEN_COSTS`."""
    return TOKEN_COSTS


def refresh_prices(write_file: bool = True) -> Dict[str, Dict[str, Any]]:
    """Kept for tokencost compatibility. Fetches and writes nothing. Returns
    :data:`TOKEN_COSTS`."""
    return TOKEN_COSTS
