# GenAI cost in OpenTelemetry

Two components put the cost of every LLM call on its span, with no change to the application:

| Component | Where it runs | Install |
|---|---|---|
| `genaicost` processor | OpenTelemetry Collector (traces pipeline) | `github.com/Mattbusel/llm-prices/otel/genaicostprocessor` in an [ocb](https://opentelemetry.io/docs/collector/custom-collector/) manifest |
| `llm-prices-otel` | Python process, as a `SpanProcessor` | `pip install llm-prices-otel` |

Both read the token counts that OpenLLMetry, OpenInference, Logfire, rig and the
[GenAI semantic conventions](https://github.com/open-telemetry/semantic-conventions-genai) put on
a span, price them with the [llm-prices](../prices.json) table (or a LiteLLM or genai-prices
table), and write:

| Attribute | Value |
|---|---|
| `gen_ai.usage.cost.amount` | The cost in USD (double) |
| `gen_ai.usage.cost.currency` | `USD` |
| `gen_ai.usage.cost.source` | `local` (priced here) or `provider` (a cost the response carried, passed through) |
| `gen_ai.usage.cost.pricing.id` | `llm-prices`, `litellm.model_cost_map` or `genai-prices` |
| `gen_ai.usage.cost.pricing.version` | The version of that table, for llm-prices and LiteLLM `sha256:` and the SHA-256 of the table file |

The first three are the attributes proposed in
[semantic-conventions-genai#443](https://github.com/open-telemetry/semantic-conventions-genai/pull/443);
the last two are the ones proposed in
[#624](https://github.com/open-telemetry/semantic-conventions-genai/issues/624). Neither is merged
yet: the names follow the proposals as of 2026-11-17 and may change with them. The Python
package also records the `gen_ai.client.operation.cost` histogram from #443; in the Collector,
the `signal_to_metrics` connector derives it from the priced spans (configuration below).

The same 30 conformance cases ([otel/conformance/cases.json](../otel/conformance/cases.json)) and
the same golden corpus run against both components in CI, so a span gets the same attributes
from either.

## Python

```sh
pip install llm-prices-otel
# or straight from the repository:
pip install "llm-prices-otel @ git+https://github.com/Mattbusel/llm-prices#subdirectory=otel/python"
```

```python
from opentelemetry.sdk.trace import TracerProvider
from opentelemetry.sdk.trace.export import BatchSpanProcessor
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter
from llm_prices_otel import CostSpanProcessor

provider = TracerProvider()
provider.add_span_processor(CostSpanProcessor(BatchSpanProcessor(OTLPSpanExporter())))
```

`CostSpanProcessor` wraps the processors it should feed: when a span ends it prices it and hands
those processors a copy with the cost attributes (an ended span cannot be changed in place).
`CostSpanExporter(exporter)` does the same at the exporter, for setups where that is easier.
With Logfire, pass it in `logfire.configure(additional_span_processors=[CostSpanProcessor(...)])`.

Keyword arguments (all optional):

| Argument | Default | Meaning |
|---|---|---|
| `source` | `LLMPricesSource()` | `LiteLLMSource.load(url_or_path)`, `GenAIPricesSource()` (needs `llm-prices-otel[genai-prices]`) or your own `PriceSource` |
| `on_unpriced` | `"flag"` | `"error_attribute"` also sets `error.type` (see below) |
| `input_tokens_include_cache` | `"auto"` | see Token counts |
| `output_tokens_include_reasoning` | `"auto"` | see Token counts |
| `batch_attribute` | `"llm_prices.batch"` | boolean span attribute that marks a Batch API request; `None` turns it off |
| `reported_cost_attributes` | `()` | attributes holding a USD cost from the response, written as `source=provider` |
| `breakdown` | `False` | also write `llm_prices.cost.{input,output,cache_read,cache_write}` and `llm_prices.price.row` |
| `openinference` | `False` | also write OpenInference's `llm.cost.*` attributes |
| `record_metric` / `meter_provider` | `True` / global | the `gen_ai.client.operation.cost` histogram |

`CostPricer(**kwargs).attributes(span_attributes)` returns the attributes to add, for use
anywhere else (a LangChain callback, a log processor).

## Collector

```yaml
processors:
  genaicost:
    prices_url: release        # the latest signed llm-prices release; omit to use the built-in table only
    reload_interval: 6h
service:
  pipelines:
    traces:
      processors: [genaicost, batch]
```

[otel/genaicostprocessor/example](../otel/genaicostprocessor/example) has an ocb manifest
(`builder-config.yaml`) and a complete collector configuration (`collector-config.yaml`) with the
cost histogram. CI builds that manifest with ocb 0.165.0 on every push.

| Setting | Default | Meaning |
|---|---|---|
| `source` | `llm-prices` | or `litellm` |
| `prices_url` | (built-in table) | `release`, an `https://` URL, a `file://` URL or a path. For `litellm` it defaults to LiteLLM's map on GitHub |
| `reload_interval` | `0` (load once) | how often `prices_url` is fetched again (1m or more) |
| `public_key` | the llm-prices release key | base64 Ed25519 key the table must be signed with (`prices_url` + `.sig`) |
| `skip_signature` | `false` | load an unsigned llm-prices table (your own fork, a local file) |
| `download_timeout` | `30s` | per fetch |
| `on_unpriced` | `flag` | or `error_attribute` |
| `input_tokens_include_cache` | `auto` | `auto`, `true`, `false` |
| `output_tokens_include_reasoning` | `auto` | `auto`, `true`, `false` |
| `batch_attribute` | `llm_prices.batch` | empty turns batch pricing off |
| `reported_cost_attributes` | `[]` | as in Python |
| `breakdown`, `openinference` | `false` | as in Python |
| `max_span_age` | `15m` | spans that ended longer ago are flagged `stale_span`, not priced against today's table; `0` turns the check off |

Hot reload: the processor fetches `prices_url`, checks the Ed25519 signature in `prices_url.sig`,
parses the table and swaps it in atomically when its version differs. A failed fetch, a bad
signature or a table that does not parse keeps the table in use and counts
`processor_genaicost_price_reload_failures`. For `llm-prices` the built-in table serves until a
load succeeds; for `litellm`, which has no built-in table, the collector does not start without
one. Internal metrics: `processor_genaicost_spans_priced`,
`processor_genaicost_spans_unpriced{reason}`, `processor_genaicost_price_reloads`,
`processor_genaicost_price_reload_failures`.

### Recording time and the #443 text

#443 says the amount "reflects the pricing data in force at recording time and MUST NOT be
recomputed against later pricing data", and that "backend or collector-side enrichment that
reconstructs cost after the fact is out of scope". The Python processor prices at recording
time, in the process that recorded the span. The Collector processor prices a span as it passes
through the pipeline, normally seconds after the call, and writes `pricing.version` so the table
is named; `max_span_age` (15 minutes by default) refuses spans old enough that the table may
have changed since. Whether a collector stage close to the application counts as recording time
is a question for the #443 review; until it is settled, run it in the agent or gateway collector
next to the applications, not in a backend that replays old data.

## Pricing rules

**Which spans.** A span is priced when it carries a token count (input or output). Spans without
one (agent, tool and workflow spans) are left alone. A span that already has
`gen_ai.usage.cost.amount` is never changed: a cost is frozen when it is recorded.

**Model.** `gen_ai.response.model`, then `gen_ai.request.model`, then OpenInference's
`llm.model_name`. The first id the table prices wins, so a dated response model
(`gpt-4o-2024-08-06`) is tried before the request alias.

**Provider.** A known API host in `server.address` wins (an OpenAI SDK pointed at
`openrouter.ai` or `api.groq.com` is priced at OpenRouter or Groq, although the instrumentation
says "openai"). Otherwise `gen_ai.provider.name`, `gen_ai.system`, `llm.provider` or `llm.system`,
including the conventions' dotted names (`gcp.vertex_ai`, `aws.bedrock`, `azure.ai.openai`,
`x_ai`, ...). A provider the table does not price (`ollama`) is flagged `unknown_provider`
rather than priced at someone else's list price. With no provider at all the model id alone is
looked up.

**Token counts.** Read from the conventions' names first, then OpenLLMetry's older names, then
OpenInference's:

| Kind | Attributes |
|---|---|
| input | `gen_ai.usage.input_tokens`, `gen_ai.usage.prompt_tokens`, `llm.token_count.prompt` |
| output | `gen_ai.usage.output_tokens`, `gen_ai.usage.completion_tokens`, `llm.token_count.completion` |
| cache reads | `gen_ai.usage.cache_read.input_tokens`, `gen_ai.usage.cache_read_input_tokens`, `llm.token_count.prompt_details.cache_read` |
| cache writes | `gen_ai.usage.cache_write.input_tokens`, `gen_ai.usage.cache_creation.input_tokens`, `gen_ai.usage.cache_creation_input_tokens`, `llm.token_count.prompt_details.cache_write` |
| reasoning | `gen_ai.usage.reasoning.output_tokens`, `gen_ai.usage.reasoning_tokens`, `llm.token_count.completion_details.reasoning` |

Integers, whole doubles and digit strings are accepted (rig's spans carry counts as strings);
anything else, or a negative count, flags `invalid_usage`. The conventions say the input count
includes cache reads and writes and the output count includes reasoning tokens; with `auto`
that is assumed unless the cache counts are larger than the input count (an instrumentation that
reports Anthropic's uncached `input_tokens`), or the reasoning count is larger than the output
count, in which case the counts are taken as separate.

**Prices.** Uncached input, cache reads (at the cache-read price, or as input when none is
listed), cache writes (at the 5-minute write price: the conventions carry no cache lifetime and
5 minutes is Anthropic's default; as input when the provider does not charge for writes),
output with reasoning, long-context prices above a model's threshold, Batch API prices when
`llm_prices.batch` is true, and service tiers from `openai.response.service_tier` (or the request
tier): `flex`, `priority`, `fast` where the table lists them.

**Never a silent zero.** A span the table cannot price gets `llm_prices.unpriced` with the
reason and no cost: `no_model`, `unknown_provider`, `unknown_model`, `missing_rate` (the row
lacks a price it needs, such as batch prices), `unpriced_tier`, `invalid_usage`,
`unsupported_price` (LiteLLM entries priced with tables or off-peak windows) or, in the
Collector, `stale_span`. With `on_unpriced: error_attribute` the span also gets
`error.type = "llm_prices.unpriced"` when it has no `error.type` yet.

## Price sources

| Source | `pricing.id` | `pricing.version` | Notes |
|---|---|---|---|
| llm-prices (default) | `llm-prices` | `PRICES_VERSION` of the table: SHA-256 of prices.json | Every release states it in its notes and publishes `prices.json`, `prices.json.sha256` and `prices.json.sig` |
| LiteLLM | `litellm.model_cost_map` | SHA-256 of the file loaded | Go and Python ports of LiteLLM's token arithmetic, checked against LiteLLM itself (below) |
| genai-prices | `genai-prices` | `genai-prices==<version>` | Python only, through the package itself |

The LiteLLM source was checked against LiteLLM 1.109.1's own `generic_cost_per_token` on 2,084
cases over 304 entries of its price map (uncached, cached, long-context and service-tier usage):
all agree to 1e-9 ([otel/conformance/litellm](../otel/conformance/litellm), regenerated with
`make_reference.py`). Entries priced with `tiered_pricing` tables or off-peak windows are
flagged `unsupported_price`.

## Golden corpus

[otel/conformance/golden](../otel/conformance/golden) holds real spans exported by four
instrumentation suites for the same three calls to a mock with fixed usage, as OTLP JSON, and
the cost each must get ([results](otel-golden.md)).

## Verifying a release

```sh
curl -sLO https://github.com/Mattbusel/llm-prices/releases/latest/download/prices.json
sha256sum prices.json      # equals the "Prices version" in the release notes
```

The signature (`prices.json.sig`, base64) verifies against the Ed25519 public key
`JMWqWwM3DNtY9/KgjA/DLph5IJSsEf5Mf6QB2E0q+LM=`, which the Collector processor carries as its
default `public_key`.
