# Service tiers and data residency

Besides the standard and Batch API prices, providers sell the same model at other prices:
cheaper slower tiers (flex, off-peak), faster or prioritized ones (priority, fast), and a
premium for keeping inference in one region. Since 1.4.0 the table holds these as data and the
Rust and Python packages price them.

## In prices.json

- `other_tiers`: tier name to prices. Each tier has `input`, `output`, `cache_read` and, where
  the page lists them, its own `long_context` prices.
- `inference_geo`: data residency multipliers on every price of the row (tiers and long
  context included), keyed by the provider's parameter value, or `null`.

| Provider | Tiers in the table | Source |
|---|---|---|
| Google Gemini | `flex`, `priority`, each with prices above 200k tokens on Gemini 2.5 Pro and 3.1 Pro Preview | https://ai.google.dev/gemini-api/docs/pricing (Standard, Batch, Flex and Priority tabs) |
| OpenAI | `flex`, `fast` (named Priority processing until 2026-07-30), `ultrafast` | https://developers.openai.com/api/docs/pricing |
| Anthropic | `fast` (fast mode) | https://platform.claude.com/docs/en/about-claude/pricing#fast-mode-pricing |
| DeepSeek | `offpeak` | https://api-docs.deepseek.com/quick_start/pricing |
| Fireworks AI | `priority`, `fast` and others per model | https://docs.fireworks.ai/serverless/pricing |
| Mistral | `priority` (1.75x standard; Batch is 0.5x and in `batch_input`/`batch_output`) | https://docs.mistral.ai/inference/pricing (Standard, Batch and Priority tabs) |
| xAI | `priority` (Priority Processing, 2x standard on every token type, long context included) | https://docs.x.ai/developers/pricing#priority-processing-pricing |

| Provider | `inference_geo` | Source |
|---|---|---|
| Anthropic | `{"us": 1.1}` on Claude 4.6 and later (Opus 4.6+, Sonnet 4.6+, Haiku 5.5, Fable); `null` on earlier models, which reject the parameter | https://platform.claude.com/docs/en/about-claude/pricing#data-residency-pricing |
| xAI | `{"us": 1.1}` on grok-4.7 and grok-4.6 (the US regional endpoint https://us.api.x.ai/v1) | https://docs.x.ai/developers/pricing#us-regional-endpoint-pricing |
| Together AI | `{"us": 1.25}` on the four models with a US-only row (Kimi K3, GLM-5.3, GLM-5.3 Flash, DeepSeek V4.1 Flash; us-api.together.ai) | https://docs.together.ai/docs/serverless/models#us-only-inference |
| Fireworks AI | `{"us": 1.5}` on the models with a "(US)" row (Kimi K3, DeepSeek V4.1 Flash and 1017, GLM 5.3 and GLM 5.3 Flash) | https://docs.fireworks.ai/serverless/pricing |

Both are read by the daily refresh from the pages above, like every other price. The
Anthropic parser reads the first model version and the multiplier from the sentence on the
page ("For Claude 4.6 and later models, ... incurs a 1.1x multiplier on all token pricing
categories"), so a change there shows up in the refresh PR.

### What is not in the table, and why

- **Anthropic Priority Tier.** It is a capacity commitment, not a per-token price, and the
  service tiers page says commitments "are no longer available for purchase"
  (https://platform.claude.com/docs/en/api/service-tiers, checked 2026-11-16). There is no
  public price to record.
- **OpenAI regional processing.** The pricing page charges a 10% uplift on regional (data
  residency) endpoints for models released on or after 2026-03-05. The page does not list
  release dates, so the row note carries this and `inference_geo` stays `null`.
- **Mistral regional inference.** The page's "Regional inference" toggle makes standard and
  batch prices 1.1x but priority 1.85x of standard (not 1.1 x 1.75), so one multiplier on
  every price would be wrong for priority. The row note carries it and `inference_geo` stays
  `null`.
- **Vertex AI regional endpoints** for Claude have their own prices on Google Cloud's page,
  which the table does not read. Claude on Bedrock has its own rows since 1.8.0, one for
  regional endpoints and one for the global profile ([hosts.md](hosts.md)).

## Python

```python
import llm_prices
from llm_prices import Usage

pro = llm_prices.lookup("gemini-2.5-pro")
sorted(pro.tiers)                         # ['flex', 'priority']
pro.tier("priority").long_context.input   # 4.5
u = Usage(input=300_000, output=1_000)
pro.cost_usage(u, tier="priority", exact=True).total     # Decimal('1.377')

sonnet = llm_prices.lookup("claude-sonnet-4-6")
sonnet.inference_geo                      # {'us': 1.1}
sonnet.with_inference_geo("us").input     # 3.3 (exact decimal product)
sonnet.cost_usage(u, inference_geo="us", exact=True)
```

`tier=` and `inference_geo=` combine (`tier="fast", inference_geo="us"` on Claude Opus 5.5:
fast mode prices times 1.1, which the page says stack). Either returns `None` when the table
has no such tier or multiplier for the model.

## Rust

```rust
use llm_prices::{lookup, Usage};

let pro = lookup("gemini-2.5-pro").unwrap();
let priority = pro.tier("priority").unwrap();          // &'static Price
let names: Vec<&str> = pro.tiers().collect();           // ["flex", "priority"]
let u = Usage { input: 300_000, output: 1_000, ..Default::default() };
let c = priority.cost_usage(&u).unwrap();               // 1.377

let sonnet = lookup("claude-sonnet-4-6").unwrap();
assert_eq!(sonnet.inference_geo("us"), Some(1.1));
let us = sonnet.cost_usage_geo(&u, "us");               // f64
// with the rust_decimal feature: sonnet.cost_usage_decimal_geo(&u, "us")
```

## LiteLLM

[litellm.json](litellm.md) carries `flex` and `priority` (and OpenAI `fast` as `priority`) with
their long-context prices under LiteLLM's key names.
