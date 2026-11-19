# USD per 1M tokens
PRICES = {
    "gpt-5.4": {"input": 2.5, "output": 15.0},
    "claude-sonnet-5-5": {"input": 2.0, "output": 10.0, "cache_read": 0.1},
    "gpt-4o": {"input": 2.5, "output": 10.0},  # cached input handled in usage parser
}
