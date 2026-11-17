# Third-party hosts: Bedrock, OpenRouter, Perplexity

The same open-weight or partner model costs different amounts depending on who serves it.
Since 1.6.0 the table carries what three resellers charge, each read from the host's own data:

| Provider | Rows | Source | Refreshed |
| - | -: | - | - |
| `bedrock` | 64 | AWS Price List bulk API, AmazonBedrock offer file for us-east-1 (what https://aws.amazon.com/bedrock/pricing/ renders) | daily, `scripts/parse/bedrock.mjs` |
| `openrouter` | 356 | https://openrouter.ai/api/v1/models | daily, `scripts/parse/openrouter.mjs` |
| `perplexity` | 6 | https://docs.perplexity.ai/docs/agent-api/models | by hand |

## Ids

- Bedrock rows use the in-Region model id from the model's card in the Bedrock user guide
  (`deepseek.r1-v1:0`, `amazon.nova-pro-v1:0`, `meta.llama4-maverick-17b-instruct-v1:0`), with
  the US geo inference profile (`us.deepseek.r1-v1:0`) and the bedrock-mantle id
  (`openai.gpt-oss-120b`) as aliases. Prices are us-east-1 on-demand; other Regions and the
  `global.` inference profiles bill at their own prices and are not aliases. Claude on Bedrock
  has no row here and takes Anthropic's list price, as before.
- OpenRouter rows use OpenRouter's id (`deepseek/deepseek-r1-0528`, `x-ai/grok-4.1-fast`).
- Perplexity rows use the Agent API id, which starts with `perplexity/` (`perplexity/sonar`,
  `perplexity/kimi-k3`). `sonar` is an alias.

## Lookup

```python
from llm_prices import lookup, lookup_provider

lookup("bedrock/us.deepseek.r1-v1:0").input                 # 1.35, Bedrock's price
lookup_provider("bedrock_converse", "us.deepseek.r1-v1:0")  # same row
lookup("openrouter/meta-llama/llama-3.3-70b-instruct")      # OpenRouter's row
lookup("perplexity/sonar").output                           # 2.5
```

- Naming the host (`bedrock/`, `openrouter/`, `perplexity/`, or `lookup_provider`) finds the
  host's own row, then the model maker's list price where the host charges it (Claude on
  Bedrock, any maker on OpenRouter), and otherwise nothing.
- Without a host path, model makers' rows come first, and Bedrock and OpenRouter rows are only
  found by their exact id, so `gpt-4` never takes OpenRouter's `openai/gpt-4` row.

## What is left out

- Per-request and tool fees (OpenRouter's `web_search` and `request` prices, Perplexity's
  `web_search` and `fetch_url` tool calls, Sonar's old request fees). The row notes say which
  ones the source lists.
- Bedrock cache-write prices, latency-optimized, custom-model and provisioned-throughput
  meters, and speech models priced per speech token.
- OpenRouter's `:free`, `:thinking` and other variant ids, and router entries such as
  `openrouter/auto`.
- Vertex AI partner models and Azure AI Foundry models: lookups through `vertex_ai/` and
  `azure/` still fall back to Google, Anthropic and OpenAI list prices only.

## tokencost compatibility

`llm_prices.compat.tokencost` prices the `bedrock/...`, `openrouter/...` and `perplexity/...`
ids tokencost and LiteLLM use. Sonar Pro, Sonar Reasoning Pro and Sonar Deep Research left
the Sonar API on 2026-09-27; `perplexity/sonar-pro` and the others are priced at the last
rates Perplexity's pricing page listed (archived snapshot of 2026-09-03), without the
per-request fees.
