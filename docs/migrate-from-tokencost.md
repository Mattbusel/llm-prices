# Moving from tokencost to llm-prices

[tokencost](https://pypi.org/project/tokencost/) (AgentOps) prices LLM calls from a copy of
LiteLLM's price map. Its last release is 0.1.26 (2025-08-13), so models released since then
are missing or carry old prices. `llm_prices.compat.tokencost` has the same functions, the
same signatures, the same `Decimal` return values and the same errors, on top of the
llm-prices table: every price read from the provider's official pricing page, with the page
and the date on each row.

## The change

```bash
pip uninstall tokencost
pip install "llm-prices[tokencost]"   # the extra installs tiktoken, for token counting
```

```python
# before
from tokencost import calculate_prompt_cost, calculate_completion_cost, count_string_tokens

# after
from llm_prices.compat.tokencost import calculate_prompt_cost, calculate_completion_cost, count_string_tokens
```

Nothing else in your code changes. If you only price token counts you already have
(`calculate_cost_by_tokens`), plain `pip install llm-prices` is enough: no tiktoken, no
dependencies at all.

## What maps to what

| tokencost | llm_prices.compat.tokencost | Notes |
|---|---|---|
| `calculate_prompt_cost(prompt, model)` | same | `prompt` is a string or a list of messages; returns `Decimal` |
| `calculate_completion_cost(completion, model)` | same | returns `Decimal` |
| `calculate_cost_by_tokens(num_tokens, model, token_type)` | same | `token_type` is `"input"`, `"output"` or `"cached"` |
| `calculate_all_costs_and_tokens(prompt, completion, model)` | same | same four keys |
| `count_message_tokens(messages, model)` | same | OpenAI message overhead (3 tokens per message, 3 per reply) |
| `count_string_tokens(prompt, model)` | same | unknown models use `cl100k_base`, as in tokencost |
| (none) | `count_tokens(text_or_messages, model)` | picks one of the two above |
| `configure_model(name, in_per_1k, out_per_1k, ...)` | same | |
| `register_model_pattern(pattern, in_per_1k, out_per_1k, ...)` | same | `*` wildcards |
| `TOKEN_COSTS`, `TOKEN_COSTS_STATIC` | same | LiteLLM-schema dicts, USD per token |
| `update_token_costs()`, `refresh_prices()` | same names | fetch nothing; see below |

Errors are the same types: `KeyError` for a model the table does not price, `TypeError` for a
prompt that is neither a string nor a list, `ValueError` for an unknown token type or for
`count_string_tokens` on a Claude model.

## Retired and legacy models

tokencost priced many models the providers have since retired: `gpt-4`, `gpt-4-32k`, the
`gpt-3.5-turbo` snapshots, `chatgpt-4o-latest`, `gpt-4o-search-preview`, `o1-mini`,
`o1-preview`, `gpt-4.5-preview`, `o3-deep-research`, `o4-mini-deep-research`, the audio and
realtime previews, `claude-3-*` and `claude-2.x`, `gemini-1.5-*`, `gemini-2.0-flash`, the
`gemini-2.5-*-preview-*` ids, older Mistral models, Grok 2 and 3, Cohere Command and Embed.
`llm_prices.compat.tokencost` prices them too, from a separate table
([tokencost_legacy.json](../python/src/llm_prices/compat/tokencost_legacy.json), about 160
ids). Each row is the last price the provider's own pricing page listed, read from an
archived snapshot of that page (the snapshot URL and date are on the row), with the shutdown
date from the provider's deprecation page:

```python
from llm_prices.compat.tokencost import calculate_cost_by_tokens, LEGACY_MODELS, TOKEN_COSTS

calculate_cost_by_tokens(1_000, "gpt-4", "input")   # Decimal('0.03000')
LEGACY_MODELS["gpt-4"]["model"], LEGACY_MODELS["gpt-4"]["retired"]   # ('gpt-4-0613', '2026-10-23')
TOKEN_COSTS["gpt-4"]["source"]   # https://web.archive.org/web/20261102160440/https://developers.openai.com/api/docs/pricing
```

- The current table always wins: an id it prices is never taken from the legacy table.
- Retired ids the provider still answers and bills as another model are priced as that
  model (`LEGACY_REDIRECTS`): `grok-3` and `grok-4-0709` bill as `grok-4.3` since 2026-05-15
  (https://docs.x.ai/developers/migration/may-15-retirement), and tokencost's `deepseek-v3` /
  `deepseek-r1` bill as `deepseek-flash`.
- Some tokencost prices were wrong and are corrected here, for example `claude-instant-1`
  (0.80 / 2.40, not 1.63 / 5.51), `command-r7b-12-2024` (0.0375 / 0.15) and `command-r` /
  `command-r-plus` (aliases of the 03-2024 and 04-2024 models: 0.50 / 1.50 and 3 / 15).
- Still not priced, because no official page gave them a per-token price: the free
  experimental Gemini ids (`gemini-exp-1206`, `gemini-2.0-flash-exp`, ...), Gemma,
  `gemini-pro-vision`, TTS and image-generation ids, `gpt-3.5-turbo-16k`,
  `gpt-3.5-turbo-instruct-0914`, `gpt-5-chat`, `claude-3-haiku-latest`, `command-nightly` and
  fine-tunes of dated snapshots. Hosts (Bedrock, Azure, Vertex, OpenRouter and the rest) are
  out of scope, as in the main table. `configure_model` adds any of them.

## What is different, on purpose

- **Where prices come from.** The llm-prices table, re-read from the Anthropic, OpenAI, Google
  and DeepSeek pricing pages every day (Mistral, xAI, Cohere, Groq, Together AI and Fireworks
  AI rows are checked by hand; each row's `checked` date says when). Look up the page and date
  for any model with `llm_prices.lookup(model).source_url` and `.checked`.
- **No network at run time.** `update_token_costs()` and `refresh_prices()` return
  `TOKEN_COSTS` and fetch nothing. New prices arrive with a new package version.
- **Model ids resolve like `llm_prices.lookup`.** Dated ids (`gpt-4o-2024-08-06`), prefixes
  (`azure/`, `openai/`, `gemini/`), Bedrock ids (`anthropic.claude-sonnet-4-6`,
  `us.anthropic.claude-...-v1:0`) and Vertex ids reach the right row. An id with a
  third-party host path (`accounts/fireworks/models/`, `together_ai/`, `groq/`, `bedrock/`,
  `vertex_ai/`, `azure/`, `openrouter/`) is priced at that host's own row, or at the maker's
  list price only where the platform charges it (Claude on Vertex, a Claude id Bedrock does
  not list, OpenAI models on Azure). Bedrock's own Claude ids get Bedrock's regional or global
  price (`anthropic.claude-sonnet-4-6` is $3.30 per 1M input tokens, `global.anthropic.claude-sonnet-4-6` $3). Otherwise it raises `KeyError` like any unknown model: Fireworks' price for
  `deepseek-r1` is not DeepSeek's, so the table does not guess it.
- **Non-OpenAI message counts.** For Gemini, DeepSeek, Mistral and the other non-OpenAI models
  in the table, `count_message_tokens` returns a tiktoken estimate and logs a warning, where
  tokencost raises `KeyError`. For exact numbers, price the usage the API response reports
  (below).
- **Claude counts** call Anthropic's token counting API, as tokencost does, without
  tokencost's fixed list of supported models. They need `pip install anthropic` and
  `ANTHROPIC_API_KEY`.

## Better: price what the response reports

Counting tokens client side is an estimate. Every provider returns exact usage, and llm-prices
prices it with prompt-cache reads and writes, batch discounts, long-context tiers, service
tiers and data residency:

```python
import llm_prices
from llm_prices import Usage

p = llm_prices.lookup("claude-sonnet-4-6")
u = Usage(input=resp.usage.input_tokens,
          cache_read=resp.usage.cache_read_input_tokens,
          cache_write_5m=resp.usage.cache_creation_input_tokens,
          output=resp.usage.output_tokens)
p.cost_usage(u, exact=True).total          # Decimal
p.cost_usage(u, exact=True, inference_geo="us").total   # US-only inference, 1.1x
```

## Tests

`python/tests/test_tokencost_compat.py` holds tokencost's own test suite (tests/test_costs.py
from 0.1.26), ported: same test names, inputs, token counts and error types, with models the
table prices today and their current prices. Run it with:

```bash
cd python
pip install pytest tiktoken
python -m pytest tests/test_tokencost_compat.py
```

A runnable before/after is in [examples/python/tokencost_migration.py](../examples/python/tokencost_migration.py).
