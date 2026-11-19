# LLM cost tracking in the 30 most-starred agent frameworks and LLM observability tools

**Date:** 2026-11-19. **Tool:** [llm-costcheck](../costcheck) 0.1.0. **Price table:** [llm-prices](../prices.json), generated 2026-11-18 from the official Anthropic, OpenAI, Google and DeepSeek pages. Every claim below links to the exact line at the commit we scanned.

## What we found

- **6 of 30 projects** ship a cost path that books a current model at the wrong price. **One** of them books every model it does not know at **$0**, which means its budget cap never trips for any GPT-5, GPT-6, Claude 4, Claude 5 or Gemini 2.5+ model.
- **4 of those 6** are the same rows: LiteLLM's price map and three products that ship a copy of it. One wrong entry upstream is wrong in four places.
- **4 projects** keep their own price tables and every current price in them matches the provider pages: AutoGPT, Langfuse, promptfoo and Mastra's main registry.
- **3 projects** do not own a price table at all: they price through a maintained source (genai-prices or LiteLLM). Two of them return no cost, instead of a guess, for a model they cannot price.
- **17 projects** have no hard-coded LLM prices in their source.

The hard truth this report is about: a price table pasted into code is a bug on a timer. Nobody in these projects did anything unusual. The tables were right on the day they were written.

## How we measured

- The 30 projects are the most-starred non-archived repositories from a list of 55 open-source agent frameworks, agent platforms, LLM gateways and LLM observability or evaluation tools, ranked by GitHub stars on 2026-11-19.
- Each repository was cloned shallow at the commit linked below, scanned with `llm-costcheck --prices prices.json`, and deleted. Tests, fixtures, docs, examples, notebooks and recorded API responses were excluded: they are not the code that books money.
- **Standard call:** a 100,000-token prompt with a 5,000-token output, at standard (non-batch) prices. Cache figures use the same call with 60,000 of the prompt tokens read from or written to the cache. The dollar error is what the project books minus what the provider charges for that call.
- Every finding the tool reported was then read by hand at the linked line. Findings we could not confirm were dropped (see "What we did not count").

## Projects that get it wrong

### MetaGPT (FoundationAgents/MetaGPT, 71,360 stars)

Commit [`11cdf46`](https://github.com/FoundationAgents/MetaGPT/tree/11cdf466d042aece04fc6cfd13b28e1a70341b1f).

**Unknown models are booked at $0, so the budget never trips.** [`cost_manager.py#L48-L50`](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/cost_manager.py#L48-L50):

```python
if model not in self.token_costs:
    logger.warning(f"Model {model} not found in TOKEN_COSTS.")
    return
```

`total_cost` is what [`team.py#L98-L100`](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/team.py#L98-L100) compares with `max_budget` before it raises `NoMoneyException`. [`TOKEN_COSTS`](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py#L18) has no gpt-4.1, GPT-5, GPT-6, o3, Claude 4, Claude 5 or Gemini 2.5+ entry, so a run on any of them adds nothing to `total_cost` and `invest(3.0)` never stops it.

| Model | Standard call, real | Booked | Error |
| --- | --- | --- | --- |
| gpt-5.5 | $0.650 | $0 | -100% |
| claude-opus-5-5 | $0.500 | $0 | -100% |
| gemini-3.1-pro-preview | $0.260 | $0 | -100% |
| gpt-6.2-sol | $0.250 | $0 | -100% |

**Stale rows in the table it does have** ([`token_counter.py`](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py), prices per 1K tokens):

| Line | Model | Written (per 1M) | Provider page (per 1M) | Standard call booked / real | Error |
| --- | --- | --- | --- | --- | --- |
| [L41](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py#L41) | gpt-4o | 5 / 15 | 2.50 / 10 | $0.575 / $0.300 | +91.7% |
| [L20](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py#L20) | gpt-3.5-turbo | 1.50 / 2 | 0.50 / 1.50 | $0.160 / $0.058 | +178.3% |
| [L28](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py#L28) | gpt-3.5-turbo-0125 | 1 / 2 | 0.50 / 1.50 | $0.110 / $0.058 | +91.3% |
| [L33](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py#L33) | gpt-4-0613 | 60 / 120 | 30 / 60 | $6.60 / $3.30 | +100.0% |
| [L50](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py#L50) | text-embedding-ada-002 | 0.40 | 0.10 | $0.040 / $0.010 (100K tokens) | +300.0% |

No row has a cache-read price, so cached prompt tokens are billed at the full input price: +33.3% on the cached call for gpt-4o ([L41](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py#L41)) and gpt-4o-mini ([L42](https://github.com/FoundationAgents/MetaGPT/blob/11cdf466d042aece04fc6cfd13b28e1a70341b1f/metagpt/utils/token_counter.py#L42)).

### LiteLLM (BerriAI/litellm, 63,549 stars)

Commit [`f1a1f6f`](https://github.com/BerriAI/litellm/tree/f1a1f6f0a1234a2a4f1031d83264af2e01883d9c). 693 prices for current Anthropic, OpenAI, Google and DeepSeek models were checked in `model_prices_and_context_window.json`. Apart from the discontinued DeepSeek names (see "What we did not count"), every model but one matches.

**gpt-6-rosalind is priced at 1.5x the page on cache writes and on every long-context price.** [`model_prices_and_context_window.json#L87076-L87090`](https://github.com/BerriAI/litellm/blob/f1a1f6f0a1234a2a4f1031d83264af2e01883d9c/model_prices_and_context_window.json#L87076-L87090) (the same rows are in `litellm/model_prices_and_context_window_backup.json`). OpenAI's pricing page lists gpt-6-rosalind at the same prices as gpt-6-astra (promotional through 2026-12-31):

| Price (per 1M) | LiteLLM | OpenAI page |
| --- | --- | --- |
| Cache write, 30 min | 18.75 | 12.50 |
| Cache write, 12 h | 37.50 | 25.00 |
| Input above 272K | 30.00 | 20.00 |
| Cached input above 272K | 3.00 | 2.00 |
| Output above 272K | 112.50 | 75.00 |

On the cached call (60K tokens written to the cache) LiteLLM books $1.78 against a real $1.40 (+26.8%). On a 300K-token prompt with a 5K-token output it books $9.56 against a real $6.38 (+50.0%). Input, cached input and output below 272K tokens are correct.

### crewAI (crewAIInc/crewAI, 61,104 stars), Opik (comet-ml/opik, 23,455 stars), MLflow (mlflow/mlflow, 28,900 stars)

All three ship a copy of LiteLLM's map, so all three carry the gpt-6-rosalind rows above:

- crewAI, commit `5605f9d`: [`lib/crewai/src/crewai/llms/litellm_catalog/model_prices_and_context_window.json#L87077`](https://github.com/crewAIInc/crewAI/blob/5605f9d89c2422ba1fae6e52fa30a8e666a731c0/lib/crewai/src/crewai/llms/litellm_catalog/model_prices_and_context_window.json#L87077)
- Opik, commit `cbfc766`, two copies: [`apps/opik-backend/src/main/resources/model_prices_and_context_window.json#L86637`](https://github.com/comet-ml/opik/blob/cbfc766f963adc2ec7579bb573873d938f0cc5ab/apps/opik-backend/src/main/resources/model_prices_and_context_window.json#L86637) and [`apps/opik-frontend/src/data/model_prices_and_context_window.json#L86637`](https://github.com/comet-ml/opik/blob/cbfc766f963adc2ec7579bb573873d938f0cc5ab/apps/opik-frontend/src/data/model_prices_and_context_window.json#L86637)
- MLflow, commit `938c99c`: [`mlflow/utils/model_catalog/openai.json#L3318`](https://github.com/mlflow/mlflow/blob/938c99cded0ef8802688a2192d108bf74d045784/mlflow/utils/model_catalog/openai.json#L3318)

Same error: +26.8% on the cached call, +50.0% above 272K tokens. Every other current-model price in the crewAI and Opik copies matches, and so does every other price in MLflow's OpenAI, Anthropic, Gemini and DeepSeek catalog files (596 checked).

The lesson is not about any of these teams. A copied table is a fork of a price list, and a fork does not receive the upstream fix.

### Mastra (mastra-ai/mastra, 29,652 stars): one template

Mastra's own pricing registry is correct (see below). One template is not: the KYC onboarding template prices **whatever model it is given** at gpt-5.6-luna's price. [`openai-multimodal.ts#L37-L41`](https://github.com/mastra-ai/mastra/blob/436f317e5522e523a53476707584912d25720f93/templates/template-kyc-customer-onboarding/src/providers/extraction/openai-multimodal.ts#L37-L41) sets `defaultPricing` to $0.20 / $1.20 per 1M, and the constructor at [L122-L128](https://github.com/mastra-ai/mastra/blob/436f317e5522e523a53476707584912d25720f93/templates/template-kyc-customer-onboarding/src/providers/extraction/openai-multimodal.ts#L122-L128) takes any `modelId` with that pricing as the default. Run it on gpt-5.6-sol and the standard call books $0.026 against a real $0.500 (-94.8%).

## Projects that get it right

| Project | Stars | What we checked | Result |
| --- | --- | --- | --- |
| [AutoGPT](https://github.com/Significant-Gravitas/AutoGPT/tree/eda70873cde1d9cb0b720bdcac8983d62f0f8d25) | 188,104 | 175 prices in `copilot/anthropic_rates.json`, `data/llm_registry/catalog.py`, `scripted_model/invoice.py` | All match, including cache and batch prices |
| [Langfuse](https://github.com/langfuse/langfuse/tree/c7a94d1a639cdb0cfdde9c4ffb5a1c2794621bef) | 37,635 | 225 prices in `worker/src/constants/default-model-prices.json` | All match |
| [Mastra](https://github.com/mastra-ai/mastra/tree/436f317e5522e523a53476707584912d25720f93) | 29,652 | 332 prices in the Anthropic, OpenAI, Google and DeepSeek sections of `observability/mastra/src/metrics/pricing-data.json` | All match |
| [promptfoo](https://github.com/promptfoo/promptfoo/tree/0d4978bace531463ebf200800f396fab9db4c21a) | 27,459 | 110 prices in its provider modules | All current-model prices match (the DeepSeek and Azure exceptions are explained under "What we did not count") |
| [browser-use](https://github.com/browser-use/browser-use/tree/ccbdb8095fbaf903c748880a600054a34c7b6191) | 121,493 | No price table: prices through genai-prices ([`agent/service.py#L18`](https://github.com/browser-use/browser-use/blob/ccbdb8095fbaf903c748880a600054a34c7b6191/browser_use/agent/service.py#L18)) | Cost is `None` when a request has no known price ([`agent/loop.py#L1212`](https://github.com/browser-use/browser-use/blob/ccbdb8095fbaf903c748880a600054a34c7b6191/browser_use/agent/loop.py#L1212)): it refuses to guess |
| [Pydantic AI](https://github.com/pydantic/pydantic-ai/tree/3fdd0fabaa2b1245e7980804d65f09dfde468a64) | 21,291 | No price table: `response.cost()` prices through genai-prices | Does not own a table |
| [DSPy](https://github.com/stanfordnlp/dspy/tree/7fbc8ff1665ee87122f06633270782dbf1f69c6b) | 39,138 | No price table: reads LiteLLM's `response_cost` ([`clients/costs.py#L38`](https://github.com/stanfordnlp/dspy/blob/7fbc8ff1665ee87122f06633270782dbf1f69c6b/dspy/clients/costs.py#L38)) | Does not own a table; inherits LiteLLM's prices |

These are the patterns that hold up: read prices from a maintained source, and when a model cannot be priced, say so instead of inventing a number.

## No hard-coded LLM prices in source

Dify, Langflow, LangChain, RAGFlow, OpenHands, AutoGen, LlamaIndex, LangGraph, Agno, ChatDev, OpenAI Agents SDK (Python), smolagents, Semantic Kernel, Vercel AI SDK, Haystack, Google ADK (Python), DB-GPT. Langflow's 8 prices are in two starter-project JSON files and all match. Letta's repository at the scanned commit holds no source code (2 files scanned), so it has no result.

## Already fixed

These cost-tracking bugs, found the same way, were fixed upstream by merged pull requests:

- [MervinPraison/PraisonAI #6433](https://github.com/MervinPraison/PraisonAI/pull/6433): every model newer than gpt-4o and Claude 3.x was billed at a $1/$3 default, so `max_budget=1.0` tripped on the 9th call instead of the 3rd.
- [microsoft/conductor #645](https://github.com/microsoft/conductor/pull/645): 12 wrong rows, mostly a 2/8 placeholder copied from model to model.
- [Kuberwastaken/claurst #526](https://github.com/Kuberwastaken/claurst/pull/526): Opus 5.5 priced at $15/$75 against a list price of $4/$20.
- [hidai25/eval-view #277](https://github.com/hidai25/eval-view/pull/277): Opus 3x high, GPT-5.4-mini 10x low.
- [Abraxas-365/langchain-rust #405](https://github.com/Abraxas-365/langchain-rust/pull/405), [liquidos-ai/AutoAgents #320](https://github.com/liquidos-ai/AutoAgents/pull/320), [AdamStrojek/rust-agentai #87](https://github.com/AdamStrojek/rust-agentai/pull/87): hard dollar caps and a usage-limit hook where there were only iteration caps.

Credit to every maintainer who merged a fix. Each one is a project whose users now see the real number.

## What we did not count

- **Host and reseller prices.** Azure, Vertex AI, Bedrock, OpenRouter and the 100+ reseller sections of models.dev-style maps charge their own prices for a model. They are not the maker's price, so they are not drift. (Example: promptfoo's `providers/azure/defaults.ts` and MLflow's catalog files for resellers.)
- **deepseek-chat and deepseek-reasoner.** Several tables price these at older rates (LiteLLM, MLflow, MetaGPT, promptfoo). DeepSeek's change log announced on 2026-04-24 that both names would be discontinued on 2026-07-24, and its pricing page no longer lists them, so no current call is billed under those names. Not counted.
- **Demo data, fixtures and recorded responses**, such as Langfuse's seeder scenarios and Mastra's eval recordings, and form placeholders such as Langfuse's "create model" defaults. They do not book real spend.

## Run it on your own code

```sh
cargo install llm-costcheck
llm-costcheck            # exit 1 when your cost tracking is wrong
```

Or in CI:

```yaml
- uses: Mattbusel/llm-prices/costcheck@main
```

Corrections: open an issue at https://github.com/Mattbusel/llm-prices/issues with the line and the provider page. We update this report and credit the fix.
