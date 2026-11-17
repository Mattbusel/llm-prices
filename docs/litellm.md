# litellm.json: the table in LiteLLM's format

[`litellm.json`](../litellm.json) is the llm-prices table in the schema of LiteLLM's
`model_prices_and_context_window.json`, the price map that LiteLLM, tokencost and many
gateways read. It is generated from `prices.json` by `scripts/build.mjs` on every refresh, so
it never drifts from the other packages.

Get it from:

- the repository: https://raw.githubusercontent.com/Mattbusel/llm-prices/main/litellm.json
- jsDelivr: https://cdn.jsdelivr.net/gh/Mattbusel/llm-prices@main/litellm.json
- the Python package: `llm_prices.litellm.model_cost()` (a dict) or `model_cost_json()` (text)

## Load it into LiteLLM

```python
import litellm
from llm_prices.litellm import model_cost

litellm.register_model(model_cost())
```

`register_model` merges the entries over LiteLLM's own map, so models the table does not
price keep LiteLLM's prices. Do not point `LITELLM_MODEL_COST_MAP_URL` at this file: LiteLLM
rejects a fetched map that is much smaller than its bundled one and falls back to the bundled
map.

`test/litellm_check.py` registers the file with LiteLLM and checks `litellm.cost_per_token`
against the table, including long-context and `service_tier="priority"` prices; CI runs it on
every push.

## Shape

USD per token, LiteLLM's key names:

| prices.json | litellm.json |
|---|---|
| `input`, `output` | `input_cost_per_token`, `output_cost_per_token` |
| `cache_read` | `cache_read_input_token_cost` |
| `cache_write["5m"]`, `cache_write["1h"]` | `cache_creation_input_token_cost`, `cache_creation_input_token_cost_above_1hr` |
| `batch_input`, `batch_output` | `input_cost_per_token_batches`, `output_cost_per_token_batches` |
| `long_context` (threshold 200,000) | `*_above_200k_tokens` (also `100k`, `272k`) |
| `other_tiers.flex` | `*_flex` |
| `other_tiers.priority` (Gemini, Fireworks) and OpenAI's `fast` | `*_priority` |

OpenAI renamed Priority processing to Fast mode on 2026-07-30 (stated on its pricing page);
LiteLLM still calls the tier `priority`, so OpenAI's fast prices go to the `_priority` keys.

Keys: OpenAI and Anthropic ids are bare (`gpt-4o`, `claude-sonnet-4-6`); other providers carry
LiteLLM's prefix (`gemini/`, `deepseek/`, `mistral/`, `xai/`, `groq/`, `together_ai/`,
`fireworks_ai/`). Aliases get their own entries.

Each entry also has two keys LiteLLM does not define and keeps as they are: `source`, the
official pricing page the price was read from, and `llm_prices_checked`, the date.

## What is left out

LiteLLM has no keys for these, so they are only in `prices.json` and the packages:

- Anthropic fast mode, OpenAI ultrafast, DeepSeek off-peak prices;
- data residency multipliers (`inference_geo`, see [tiers.md](tiers.md));
- OpenAI's 30-minute and 12-hour cache write prices;
- xAI's long-context threshold is "200k tokens or more" (`threshold_tokens` 199999); it is
  written as `_above_200k_tokens`, so a prompt of exactly 200,000 tokens prices at the
  standard rate in LiteLLM.
