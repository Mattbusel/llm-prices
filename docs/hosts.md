# Third-party hosts: Bedrock, OpenRouter, Perplexity

The same open-weight or partner model costs different amounts depending on who serves it.
Since 1.6.0 the table carries what three resellers charge, each read from the host's own data:

| Provider | Rows | Source | Refreshed |
| - | -: | - | - |
| `bedrock` | 97 | AWS Price List bulk API for us-east-1 (what https://aws.amazon.com/bedrock/pricing/ renders): the AmazonBedrock offer file for the models Bedrock prices itself (64 rows), and since 1.8.0 the AmazonBedrockFoundationModels offer file for Claude (33 rows) | daily, `scripts/parse/bedrock.mjs` |
| `openrouter` | 360 | https://openrouter.ai/api/v1/models | daily, `scripts/parse/openrouter.mjs` |
| `perplexity` | 6 | https://docs.perplexity.ai/docs/agent-api/models | by hand |

## Ids

- Bedrock rows use the in-Region model id from the model's card in the Bedrock user guide
  (`deepseek.r1-v1:0`, `amazon.nova-pro-v1:0`, `meta.llama4-maverick-17b-instruct-v1:0`), with
  the US geo inference profile (`us.deepseek.r1-v1:0`) and the bedrock-mantle id
  (`openai.gpt-oss-120b`) as aliases. Prices are us-east-1 on-demand; other Regions and the
  `global.` inference profiles bill at their own prices and are not aliases.
- Claude on Bedrock (since 1.8.0) has two rows per model, because AWS bills two endpoint kinds
  differently:
  - **regional**: the foundation-model id (`anthropic.claude-sonnet-4-5-20250929-v1:0`) with
    the bedrock-mantle id and every geographic inference profile the model card lists
    (`us.`, `eu.`, `apac.`, `jp.`, `au.`, `ca.`) as aliases;
  - **global**: the global cross-Region profile (`global.anthropic.claude-sonnet-4-5-20250929-v1:0`).
    Claude Opus 4.1 has no global profile and only the regional row.

  For Claude Haiku 4.5, Sonnet 4.5, Opus 4.5 and later the regional prices are 10% above the
  global ones (Sonnet 4.5:
  $3.30 / $16.50 regional, $3 / $15 global, per 1M tokens), and the global prices equal
  Anthropic's list prices (a test checks input, output, cache-read and batch rates). Anthropic's
  pricing page says Bedrock sets these prices itself. The price list carries the same Claude
  prices in every Region checked on 2026-11-18 (us-east-1, us-west-2, eu-central-1,
  ap-northeast-1, ap-southeast-2, ca-central-1). Claude Mythos, Claude Opus 4 and models with
  no Bedrock model card (Claude 3.x and older) have no Bedrock row and keep Anthropic's list
  price. Batch, cache-read and 5-minute and 1-hour cache-write prices come from the same price
  list; the long-context switch of Claude Haiku 5.5 takes its threshold from the Anthropic row.
- OpenRouter rows use OpenRouter's id (`deepseek/deepseek-r1-0528`, `x-ai/grok-4.1-fast`).
- Perplexity rows use the Agent API id, which starts with `perplexity/` (`perplexity/sonar`,
  `perplexity/kimi-k3`). `sonar` is an alias.

## Lookup

```python
from llm_prices import lookup, lookup_provider

lookup("bedrock/us.deepseek.r1-v1:0").input                 # 1.35, Bedrock's price
lookup_provider("bedrock_converse", "us.deepseek.r1-v1:0")  # same row
lookup("us.anthropic.claude-sonnet-4-6").input              # 3.3, Bedrock regional
lookup("global.anthropic.claude-sonnet-4-6").input          # 3.0, Bedrock global
lookup_provider("bedrock", "claude-sonnet-4-6").provider    # "anthropic": not a Bedrock id
lookup("openrouter/meta-llama/llama-3.3-70b-instruct")      # OpenRouter's row
lookup("perplexity/sonar").output                           # 2.5
```

- Naming the host (`bedrock/`, `openrouter/`, `perplexity/`, or `lookup_provider`) finds the
  host's own row, then the model maker's list price where the host charges it (a Claude id
  Bedrock does not list, any maker on OpenRouter), and otherwise nothing.
- Without a host path, an exact Bedrock id comes first (so a Bedrock Claude id gets Bedrock's
  price), then model makers' rows; Bedrock and OpenRouter rows are otherwise only found by
  their exact id, so `gpt-4` never takes OpenRouter's `openai/gpt-4` row.
- Bedrock's Claude rows match only their exact ids, in any ASCII case and with or without a
  `bedrock/` path. Any other spelling of a Claude id (`claude-sonnet-4-6`,
  `apac.anthropic.claude-sonnet-4-5-20250929-v1:0`, which no model card lists) keeps
  Anthropic's list price, as before 1.8.0. `lookup_provider("anthropic", ...)` always returns
  Anthropic's own row.

## What is left out

- Per-request and tool fees (OpenRouter's `web_search` and `request` prices, Perplexity's
  `web_search` and `fetch_url` tool calls, Sonar's old request fees). The row notes say which
  ones the source lists.
- Bedrock cache-write prices for the models Bedrock prices itself, latency-optimized,
  custom-model and provisioned-throughput meters, and speech models priced per speech token.
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
