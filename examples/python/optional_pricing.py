"""llm-prices as an optional dependency: the pattern from docs/integrate.md.

The host project lists llm-prices under an extra (pip install yourpkg[pricing]) and keeps
working without it. Run: python examples/python/optional_pricing.py
"""

from typing import Optional

try:
    import llm_prices
except ImportError:  # the extra is not installed
    llm_prices = None  # type: ignore[assignment]


def cost_usd(provider: str, model: str, input_tokens: int, output_tokens: int) -> Optional[float]:
    """USD for one request, or None when pricing is not installed or the model is unknown."""
    if llm_prices is None:
        return None
    price = llm_prices.lookup_provider(provider, model)
    return price.cost(input_tokens, output_tokens) if price else None


if __name__ == "__main__":
    for provider, model in [
        ("anthropic", "claude-sonnet-4-6"),
        ("bedrock_converse", "us.anthropic.claude-sonnet-4-20250514-v1:0"),
        ("google_genai", "gemini-2.5-flash"),
        ("openai", "gpt-6.2-sol"),
        ("ollama", "llama3"),  # not a priced provider: None, never 0
    ]:
        print(f"{provider:18} {model:45} {cost_usd(provider, model, 12_000, 800)}")
    if llm_prices is not None:
        print("table", llm_prices.GENERATED, llm_prices.PRICES_VERSION)
