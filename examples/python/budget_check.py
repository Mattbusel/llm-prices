"""Refuse a call before sending it when its worst-case cost would break a budget.

    python examples/python/budget_check.py
"""

from decimal import Decimal

import llm_prices


class BudgetExceeded(Exception):
    pass


class Budget:
    def __init__(self, limit_usd: str) -> None:
        self.limit = Decimal(limit_usd)
        self.spent = Decimal(0)

    def check(self, model: str, prompt_tokens: int, max_output_tokens: int) -> Decimal:
        price = llm_prices.lookup(model)
        if price is None:
            raise KeyError(f"no price for {model}")
        worst = price.cost(prompt_tokens, max_output_tokens, exact=True)
        if worst is None:
            raise KeyError(f"{model} lists no input or output price")
        if self.spent + worst > self.limit:
            raise BudgetExceeded(f"{model}: worst case ${worst} would exceed ${self.limit} (spent ${self.spent})")
        return worst

    def record(self, model: str, input_tokens: int, output_tokens: int) -> None:
        price = llm_prices.lookup(model)
        assert price is not None
        cost = price.cost(input_tokens, output_tokens, exact=True)
        assert cost is not None
        self.spent += cost


budget = Budget("0.05")
for i in range(10):
    try:
        budget.check("gemini-2.5-pro", prompt_tokens=8_000, max_output_tokens=2_000)
    except BudgetExceeded as e:
        print(f"call {i}: refused, {e}")
        break
    budget.record("gemini-2.5-pro", 8_000, 1_500)  # what the response reported
    print(f"call {i}: sent, spent ${budget.spent}")
