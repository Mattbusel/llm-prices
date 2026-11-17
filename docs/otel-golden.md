# Golden corpus results

The corpus in [otel/conformance/golden](../otel/conformance/golden) is what four instrumentation
suites actually export for the same three calls: `gpt-4o-mini` and `o3` through the OpenAI SDK
and `claude-sonnet-4-6` through the Anthropic SDK, against a local mock that returns fixed usage
(`generate.py`; rig through `rig/`). Recorded 2026-11-17.

| Suite | Versions |
|---|---|
| OpenLLMetry | opentelemetry-instrumentation-openai 0.63.1, opentelemetry-instrumentation-anthropic 0.63.1 |
| OpenInference | openinference-instrumentation-openai 0.1.74, openinference-instrumentation-anthropic 3.3.6 |
| Logfire | logfire 5.6.0 |
| rig | rig-core 0.46.0 through tracing-opentelemetry 0.34.0 |

All with opentelemetry-sdk 1.45.2 (Python), openai 3.52.0 and anthropic 2.9.0.

## What each suite records

| | OpenLLMetry | OpenInference | Logfire | rig |
|---|---|---|---|---|
| provider | `gen_ai.provider.name` | `llm.system` (and `llm.provider` for Anthropic) | `gen_ai.provider.name`, `gen_ai.system` | `gen_ai.provider.name` |
| model | request and response model | `llm.model_name` (the response model) | request and response model | request and response model |
| input | `gen_ai.usage.input_tokens`, cache included | `llm.token_count.prompt`, cache included | `gen_ai.usage.input_tokens`, cache included | `gen_ai.usage.input_tokens`, cache included |
| cache reads | `gen_ai.usage.cache_read.input_tokens` | `llm.token_count.prompt_details.cache_read` | `gen_ai.usage.cache_read.input_tokens` | `gen_ai.usage.cache_read.input_tokens` |
| cache writes (Anthropic) | `gen_ai.usage.cache_creation.input_tokens` | `llm.token_count.prompt_details.cache_write` | `gen_ai.usage.cache_creation.input_tokens` | `gen_ai.usage.cache_creation.input_tokens` |
| reasoning (o3) | `gen_ai.usage.reasoning_tokens` | `llm.token_count.completion_details.reasoning` | not recorded | `gen_ai.usage.reasoning_tokens` |
| count type | int | int | int | string |

Two things differ from the GenAI registry: every suite but OpenInference writes Anthropic cache
writes as `gen_ai.usage.cache_creation.input_tokens`, where the registry name is
`gen_ai.usage.cache_write.input_tokens`, and rig's counts arrive as strings (Weaver flags them as
`type_mismatch`: the registry says int). The processors read all of these.

## Costs

The mock usage and the llm-prices rows (USD per 1M tokens) give:

| Call | Usage | Cost |
|---|---|---|
| gpt-4o-mini | 1,200 input of which 1,000 cached, 300 output | 200 x 0.15 + 1,000 x 0.075 + 300 x 0.60 = $0.000285 |
| o3 | 10,000 input, 2,000 output of which 1,500 reasoning | 10,000 x 2 + 2,000 x 8 = $0.036 |
| claude-sonnet-4-6 | 2,048 uncached, 45,000 cache reads, 3,500 cache writes, 1,024 output | 2,048 x 3 + 45,000 x 0.30 + 3,500 x 3.75 + 1,024 x 15 = $0.048129 |

All 12 spans (3 calls x 4 suites) get exactly these costs from the Python package and from the
Go processor (set to `attributes: semconv`, the names the Python processor writes), in unit
tests and in a collector built with ocb from the example manifest (CI job
`otel-collector-e2e`), where the `signal_to_metrics` connector also turns them into a
`gen_ai.client.operation.cost` histogram with count 12 and sum $0.337656.

## Weaver live check

The same CI job sends the priced spans to `weaver registry live-check` (Weaver 0.28.0) with the
GenAI registry from the #443 branch (`elang2/semantic-conventions-genai`,
`feat/operation-cost-conventions`). Findings for the attributes the processor writes, on all 12
spans:

| Attribute | Finding |
|---|---|
| `gen_ai.usage.cost.amount` | `not_stable` (the proposal marks it development) |
| `gen_ai.usage.cost.currency` | `not_stable` |
| `gen_ai.usage.cost.source` | `not_stable` |
| `gen_ai.usage.cost.pricing.id` | `missing_attribute`, `extends_namespace` (proposed in #624, not in that registry) |
| `gen_ai.usage.cost.pricing.version` | `missing_attribute`, `extends_namespace` |

No type or enum findings for the cost attributes.
