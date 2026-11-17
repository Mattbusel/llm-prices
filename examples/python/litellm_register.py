"""Use the llm-prices table as LiteLLM's price map, so completion_cost and the proxy's spend
tracking use prices read from the provider pages.

    pip install llm-prices litellm
    python examples/python/litellm_register.py
"""

import litellm

from llm_prices.litellm import model_cost

table = model_cost()
litellm.register_model(table)
prompt_cost, completion_cost = litellm.cost_per_token(model="claude-sonnet-4-6", prompt_tokens=10_000, completion_tokens=1_000)
print(f"claude-sonnet-4-6, 10k in / 1k out: ${prompt_cost + completion_cost:.6f}")
print(f"source: {table['claude-sonnet-4-6']['source']} (checked {table['claude-sonnet-4-6']['llm_prices_checked']})")
