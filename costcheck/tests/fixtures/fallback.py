# Prices in USD per 1M tokens.
PRICES = {
    "gpt-4o": {"input": 2.5, "output": 10.0},
    "gpt-4o-mini": {"input": 0.15, "output": 0.6},
    "claude-sonnet-4-6": {"input": 3.0, "output": 15.0},
}

# Used for any model not listed above.
DEFAULT_PRICING = {"input": 1.0, "output": 3.0}

TABLE = {
    "gpt-4.1": {"input": 2.0, "output": 8.0},
    "default": {"input": 2.5, "output": 10.0},
}


def cost(model, prompt_tokens, completion_tokens):
    p = TABLE.get(model, {"input": 0.5, "output": 1.5})
    return (prompt_tokens * p["input"] + completion_tokens * p["output"]) / 1e6

def cost_or_zero(model, tokens):
    rate = PRICES.get(model, 0)
    return tokens * rate
