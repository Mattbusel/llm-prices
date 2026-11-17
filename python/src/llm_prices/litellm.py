"""The table in LiteLLM's ``model_prices_and_context_window.json`` schema.

``litellm.json`` ships inside the package and is also served from the repository
(https://cdn.jsdelivr.net/gh/Mattbusel/llm-prices@main/litellm.json). It is generated from
prices.json on every refresh: USD per token, LiteLLM's key names (``input_cost_per_token``,
``cache_read_input_token_cost``, ``*_batches``, ``*_flex``, ``*_priority``,
``*_above_200k_tokens``), and two extra keys per entry, ``source`` (the provider's official
pricing page) and ``llm_prices_checked`` (the date the price was read from it).

Load it into LiteLLM so ``completion_cost`` and the proxy's spend tracking use these prices::

    import litellm
    from llm_prices.litellm import model_cost
    litellm.register_model(model_cost())

>>> from llm_prices.litellm import model_cost
>>> table = model_cost()
>>> table["claude-sonnet-4-6"]["input_cost_per_token"]
3e-06
>>> table["gemini/gemini-2.5-pro"]["input_cost_per_token_above_200k_tokens_priority"]
4.5e-06
>>> table["claude-sonnet-4-6"]["source"]
'https://platform.claude.com/docs/en/about-claude/pricing'
"""

from __future__ import annotations

import json
from importlib import resources
from typing import Any, Dict

__all__ = ["model_cost", "model_cost_json"]


def model_cost_json() -> str:
    """The text of ``litellm.json``."""
    return resources.files(__package__).joinpath("litellm.json").read_text(encoding="utf-8")


def model_cost() -> Dict[str, Dict[str, Any]]:
    """``litellm.json`` as a dict, without the ``sample_spec`` entry. A new dict on every
    call, so changing it changes nothing else."""
    table: Dict[str, Dict[str, Any]] = json.loads(model_cost_json())
    table.pop("sample_spec", None)
    return table
