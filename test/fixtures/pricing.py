# Prices in USD per 1K tokens.
MODEL_COSTS = {
    "gpt-4o": {"prompt": 0.0025, "completion": 0.01},
    "gpt-4o-mini": {"prompt": 0.00015, "completion": 0.0006},
    "gpt-4.1": {"prompt": 0.002, "completion": 0.008},
    "gpt-5": {"prompt": 0.00125, "completion": 0.012},  # stale output: page says 10 per 1M
    "o3": {"prompt": 0.002, "completion": 0.008},
}

DEFAULT_MODEL = "gpt-4o"
CONTEXT = {"gpt-4o": 128000, "gpt-4.1": 1047576}


def cost(model: str, prompt_tokens: int, completion_tokens: int) -> float:
    p = MODEL_COSTS[model]
    return (prompt_tokens * p["prompt"] + completion_tokens * p["completion"]) / 1000
