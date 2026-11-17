"""Cost of one API call from the usage the response reports.

    pip install llm-prices
    python examples/python/one_call.py
"""

import llm_prices
from llm_prices import Usage

# An OpenAI chat completion reported prompt_tokens=1_200 (300 of them cached) and
# completion_tokens=350. Usage.input is the uncached part.
price = llm_prices.lookup("gpt-4o-2024-08-06")  # dated ids resolve to the gpt-4o row
assert price is not None
usage = Usage(input=1_200 - 300, cache_read=300, output=350)
cost = price.cost_usage(usage, exact=True)
assert cost is not None
print(f"{price.model}: ${cost.total} (input ${cost.input}, cached ${cost.cache_read}, output ${cost.output})")
print(f"prices read from {price.source_url} on {price.checked}")
