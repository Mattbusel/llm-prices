"""Moving from tokencost: change the import, keep the calls.

    pip install "llm-prices[tokencost]"
    python examples/python/tokencost_migration.py
"""

# from tokencost import calculate_prompt_cost, calculate_completion_cost, count_string_tokens
from llm_prices.compat.tokencost import calculate_completion_cost, calculate_prompt_cost, count_string_tokens

model = "gpt-4o"
prompt = [{"role": "user", "content": "Summarize the attached report in three bullet points."}]
completion = "- Revenue grew 12%\n- Costs were flat\n- Guidance raised"

print("prompt cost:    ", calculate_prompt_cost(prompt, model))
print("completion cost:", calculate_completion_cost(completion, model))
print("completion tokens:", count_string_tokens(completion, model))
