"""The same request on each service tier the provider prices: standard, batch, flex, priority.

    python examples/python/service_tiers.py
"""

import llm_prices
from llm_prices import Usage

for model in ["gemini-2.5-pro", "gpt-5.4", "claude-opus-5-5"]:
    price = llm_prices.lookup(model)
    assert price is not None
    usage = Usage(input=250_000, output=4_000)  # over Gemini's 200k long-context threshold
    print(model)
    std = price.cost_usage(usage, exact=True)
    print(f"  standard  ${std.total if std else 'n/a'}")
    batch = price.cost_usage(Usage(input=250_000, output=4_000, batch=True), exact=True)
    print(f"  batch     ${batch.total if batch else 'n/a'}")
    for tier in sorted(price.tiers):
        c = price.cost_usage(usage, exact=True, tier=tier)
        print(f"  {tier:<9} ${c.total if c else 'n/a'}")
