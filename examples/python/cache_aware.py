"""What prompt caching saves on a long system prompt, per call and over a day.

    python examples/python/cache_aware.py
"""

import llm_prices
from llm_prices import Usage

price = llm_prices.lookup("claude-sonnet-4-6")
assert price is not None
system_prompt, question, answer = 40_000, 500, 800
calls_per_day = 2_000

# First call writes the system prompt to the 5-minute cache; later calls read it.
first = price.cost_usage(Usage(input=question, cache_write_5m=system_prompt, output=answer), exact=True)
cached = price.cost_usage(Usage(input=question, cache_read=system_prompt, output=answer), exact=True)
uncached = price.cost_usage(Usage(input=system_prompt + question, output=answer), exact=True)
assert first and cached and uncached

print(f"uncached call:     ${uncached.total}")
print(f"cache-write call:  ${first.total}")
print(f"cache-read call:   ${cached.total}")
day_without = uncached.total * calls_per_day
# Assumes one cache write per 5 minutes (288 a day) and reads for the rest.
writes = 288
day_with = first.total * writes + cached.total * (calls_per_day - writes)
print(f"{calls_per_day} calls a day: ${day_without} without caching, ${day_with} with it")

# US-only inference (inference_geo "us") on Claude 4.6 and later costs 1.1x on every price.
us = price.cost_usage(Usage(input=question, cache_read=system_prompt, output=answer), exact=True, inference_geo="us")
assert us is not None
print(f"cache-read call with inference_geo=us: ${us.total}")
