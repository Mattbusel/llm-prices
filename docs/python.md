# llm-prices for Python

`pip install llm-prices` (Python 3.9+, no dependencies, no network access). The package holds
the same table as the Rust crate and the GitHub Action, generated from
[`prices.json`](../prices.json). Prices are USD per 1M tokens; every row carries the
provider's pricing page (`source_url`) and the date it was read (`checked`).

## Pricing a request when you know the provider: `lookup_provider`

```python
import llm_prices

p = llm_prices.lookup_provider("google_genai", "gemini-2.5-flash")
p.input, p.output                    # (0.3, 2.5)   USD per 1M tokens
p.input_per_token, p.output_per_token  # (3e-07, 2.5e-06) USD per token
p.cost(12_000, 800)                  # 0.0056       USD for one request
p.source_url, p.checked              # where the price was read, and when
```

`lookup_provider(provider, model)` returns a `Price` or `None`. It is the call to make in a
cost callback or a gateway, where the framework already tells you which provider served the
request.

**Provider names.** `provider` is a `Provider` or a name in any ASCII case, with `_`, `-` and
spaces treated alike. These all work:

| Provider | Accepted names |
|---|---|
| Anthropic | `anthropic`, `claude` |
| OpenAI | `openai`, `open_ai` |
| Google AI Studio (Gemini API) | `google`, `gemini`, `google_genai`, `google-generativeai`, `genai`, `google_ai`, `ai_studio` |
| Vertex AI | `vertex`, `vertex_ai`, `vertexai`, `google_vertexai`, `google_vertex_ai` |
| Amazon Bedrock | `bedrock`, `bedrock_converse`, `bedrock_runtime`, `aws`, `aws_bedrock`, `amazon_bedrock`, `amazon` |
| Azure | `azure`, `azure_openai`, `azure_ai`, `azure_foundry`, `azure_ai_foundry`, `microsoft_foundry` |
| DeepSeek | `deepseek` |
| Mistral | `mistral`, `mistralai`, `mistral_ai` |
| Groq | `groq`, `groqcloud` |
| Together AI | `together`, `together_ai`, `togetherai` |
| Fireworks AI | `fireworks`, `fireworks_ai`, `fireworksai` |
| xAI | `xai`, `x-ai`, `grok` |
| Cohere | `cohere`, `cohere_chat`, `cohere_ai` |
| OpenRouter | `openrouter`, `open_router` |

They cover the provider strings of LangChain's `init_chat_model` (`google_genai`,
`google_vertexai`, `bedrock_converse`, `azure_openai`, `mistralai`, ...) and LiteLLM
(`vertex_ai`, `together_ai`, `fireworks_ai`, ...). Anything else (`ollama`, `huggingface`,
`vllm`) returns `None`.

**Model ids.** `model` gets the same handling as `lookup`: aliases (`deepseek-chat`,
`mistral-large-latest`), any ASCII case, dated snapshots (`gpt-4o-2024-08-06` finds `gpt-4o`
unless the snapshot has its own row), `-latest` ids, Bedrock ids
(`us.anthropic.claude-sonnet-4-20250514-v1:0`), Vertex `@date` ids and provider prefixes
(`models/`, `openai/`, `together_ai/`, ...).

**Which rows are searched.** Only the rows that provider sells, so `openai/gpt-oss-120b` on
Groq gets Groq's price, never Together AI's. Bedrock and OpenRouter have rows of their own
(Bedrock's Claude rows give its regional and global prices, see [hosts.md](hosts.md)); Vertex
AI and Azure have none. For a model without a row there, they fall back to the list price of
the model maker they resell (Vertex: Google and Anthropic, Bedrock: Anthropic, Azure: OpenAI),
and the returned row's `provider` says whose price it is. OpenRouter falls back to every model
maker.

**When it returns `None`.** The provider name is unknown, or the table has no price for that
model there (`lookup_provider("groq", "llama-3.3-70b-versatile")`,
`lookup_provider("bedrock", "mistral.mistral-large-2407-v1:0")`). Keep your own fallback for
that case; never treat `None` as zero.

### A cost callback in a few lines

```python
from typing import Optional

import llm_prices

def request_cost(provider: str, model: str, prompt_tokens: int, completion_tokens: int) -> Optional[float]:
    price = llm_prices.lookup_provider(provider, model)
    if price is None or price.input_per_token is None or price.output_per_token is None:
        return None  # unknown model: use your own default
    return prompt_tokens * price.input_per_token + completion_tokens * price.output_per_token
```

For prompt caching, batch discounts and long-context tiers use
`price.cost_usage(llm_prices.Usage(input=..., cache_read=..., cache_write_5m=..., output=..., batch=...))`,
and `exact=True` for `decimal.Decimal` results.

## Other lookups

- `lookup(model)`: any provider, first match in table order (model makers before the hosts
  that resell open-weight models). Use it only when you do not know the provider.
- `Provider.parse(name)`, `Provider.rows()`, `Provider.fallback_providers()`.
- `Price.per_token(rate, ttl=None)`: any column in USD per token, for example
  `price.per_token("cache_read")`.
- `all_prices()`: every row.

## Retired and delisted models

When a model leaves a provider's pricing page, the daily refresh keeps its last listed prices
with `status == "retired"` and a note that says since when the page no longer lists it
(`gpt-3.5-turbo`, `o1`, `gpt-4o-2024-05-13`, `claude-3-5-haiku-20241022`, ...). Code that still
calls those ids, or prices old logs, gets the price the provider last published.

## See also

- [Service tiers and data residency](tiers.md): `Price.tier("flex")`, `cost_usage(..., tier=..., inference_geo="us")`.
- [Moving from tokencost](migrate-from-tokencost.md): `llm_prices.compat.tokencost`, same functions and signatures.
- [litellm.json](litellm.md): the table in LiteLLM's format, `llm_prices.litellm.model_cost()`.
- Runnable examples: [examples/python](../examples/python).
