"""Dated, source-cited LLM API prices, with no dependencies and no network access.

The same table as the Rust crate ``llm-prices``, generated from
`prices.json <https://github.com/Mattbusel/llm-prices/blob/main/prices.json>`_: Anthropic,
OpenAI, Google Gemini, DeepSeek, Amazon Bedrock and OpenRouter (re-read from their pricing
pages or price APIs every day) plus Mistral, xAI, Cohere, Groq, Together AI, Fireworks AI and
Perplexity. Every row carries the provider's official
pricing page and the date the price was read from it. Prices are USD per 1M tokens.

>>> import llm_prices
>>> p = llm_prices.lookup("claude-sonnet-5-5")
>>> p.input, p.cache_read
(2.0, 0.1)
>>> llm_prices.lookup("gpt-4o-2024-08-06").model
'gpt-4o'
>>> llm_prices.lookup_provider("groq", "openai/gpt-oss-120b").provider
'groq'

Price a whole response, prompt-cache reads and writes, batch discounts and the
long-context switch included, in floats or in exact decimals:

>>> from llm_prices import Usage
>>> usage = Usage(input=2_048, cache_read=45_000, cache_write_5m=3_500, output=1_024)
>>> llm_prices.lookup("claude-sonnet-4-6").cost_usage(usage, exact=True).total
Decimal('0.048129')
"""

from ._core import (
    GENERATED,
    PRICES_VERSION,
    CostBreakdown,
    DecimalCostBreakdown,
    LongContext,
    Price,
    Provider,
    RateName,
    Usage,
    all_prices,
    lookup,
    lookup_provider,
)

#: Same as :func:`all_prices`, named like the Rust crate's ``llm_prices::all``. Not exported
#: by ``from llm_prices import *``, where it would hide the builtin ``all``.
all = all_prices

__version__ = "1.8.0"

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
