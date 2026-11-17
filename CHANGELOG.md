# Changelog

Changes to the GitHub Action (`uses: Mattbusel/llm-prices@v1`), the price table and the
repository as a whole. The packages have their own changelogs:
[crate](crate/CHANGELOG.md), [Python](python/CHANGELOG.md), [JavaScript](js/CHANGELOG.md).
The Go module is tagged `go/vX.Y.Z` and listed here.
The `v1` tag moves to every 1.x release.

## [1.7.1] - 2026-11-17

Fixes to the OpenTelemetry pricing stage ([docs/otel.md](docs/otel.md)). No price changed and
no package other than the two below changed; the table is the 1.7.0 table
(`sha256:e8eed72ddf88c237f1ed8b7ed791495a375620c8166f36b8c554db4e5b616f87`).

`otel/genaicostprocessor` v0.2.0 (Go module, tag `otel/genaicostprocessor/v0.2.0`):

- **Changed: the cost is written as `llm_prices.cost.*` by default.** The current text of
  semantic-conventions-genai#443 reserves `gen_ai.usage.cost.*` for a cost computed at recording
  time and puts collector-side enrichment out of scope, so the Collector processor no longer
  writes those names unless it is set to `attributes: semconv` (meant for an agent collector next
  to the application). The default names are `llm_prices.cost.amount`, `.currency`, `.source`,
  `.pricing.id` and `.pricing.version`, with the same values. The example
  `collector-config.yaml` derives the cost histogram from `llm_prices.cost.amount`. A span that
  already carries either cost attribute is left alone.
- Fixed: embedding spans are priced (see llm-prices-otel 0.1.1 below; the same rule).
- Fixed: token counts above 10^12 flag `invalid_usage`. Before, counts near the int64 limit
  could wrap when cache or reasoning counts were added and produce an absurd cost, and Go and
  Python disagreed on them.
- Added: `llm_prices.price.fallback = true` and `llm_prices.price.row` on a span priced from
  another provider's row (Claude on Bedrock or Vertex AI at Anthropic's list price).
- Added: hot reload rejects a signed table whose `generated` date is older than the table in use
  (the built-in one included), so an older signed release served at the URL cannot roll prices
  back. It counts as a reload failure.

`llm-prices-otel` 0.1.1 (`otel/python`): embedding spans priced, the 10^12 token-count ceiling,
digit strings longer than 19 characters flagged `invalid_usage` instead of raising, and the
fallback flag ([changelog](otel/python/CHANGELOG.md)). Until it is on PyPI it installs from this
tag: `pip install "llm-prices-otel @ git+https://github.com/Mattbusel/llm-prices@v1.7.1#subdirectory=otel/python"`.

Docs: the conformance cases are now 38 (37 shared, `stale_span` Collector-only; the Collector
runs each a second time with its default names), and the Logfire note says that
`CostSpanProcessor` prices only the spans it passes on, not Logfire's own export.

## [1.7.0] - 2026-11-17

### Added

**GenAI cost in OpenTelemetry** ([docs/otel.md](docs/otel.md)). Two components put the cost of
every LLM call on its span, with no change to the application, as the attributes proposed in
open-telemetry/semantic-conventions-genai#443 (`gen_ai.usage.cost.amount`, `.currency`,
`.source`) and #624 (`gen_ai.usage.cost.pricing.id`, `.pricing.version`):

- `otel/genaicostprocessor` (Go module, tag `otel/genaicostprocessor/v0.1.0`): an OpenTelemetry
  Collector processor, `genaicost`, buildable with ocb (example manifest and configuration in
  `otel/genaicostprocessor/example`). Hot reload of prices.json from the latest release with an
  Ed25519 signature check, a LiteLLM price source, internal metrics, and the
  `gen_ai.client.operation.cost` histogram through the `signal_to_metrics` connector.
- `llm-prices-otel` (PyPI, `otel/python`): a `SpanProcessor` and a span exporter wrapper with the
  same rules, plus the cost histogram, and LiteLLM and genai-prices sources.
- Both read OpenLLMetry, OpenInference, Logfire, rig and GenAI semantic-convention token counts
  (cache reads and writes, reasoning, batch, service tiers, long context) and flag a span they
  cannot price with `llm_prices.unpriced` instead of writing a zero. 30 shared conformance cases,
  a golden corpus of real spans from the four suites ([results](docs/otel-golden.md)), a
  reference check of the LiteLLM source against LiteLLM 1.109.1 (2,084 cases agree), fuzz and
  property tests, a collector built with ocb in CI, and a Weaver live check against the #443
  registry.

`PRICES_VERSION` in the crate, the Python and npm packages and the root module, and
`PricesVersion` in the Go module. Every release now names the exact table it ships. `PRICES_VERSION` is `"sha256:"` and the
SHA-256 of `prices.json` (UTF-8, LF line endings), the same value in every package; it is
generated with the data and CI checks it against the file. Record it as
`gen_ai.usage.cost.pricing.version` next to a cost computed from the table (the attribute
proposed in open-telemetry/semantic-conventions-genai#624). This release's table:
`sha256:e8eed72ddf88c237f1ed8b7ed791495a375620c8166f36b8c554db4e5b616f87`. No price changed.

Releases publish `prices.json`, `prices.json.sha256` and `prices.json.sig` (Ed25519, public key
`JMWqWwM3DNtY9/KgjA/DLph5IJSsEf5Mf6QB2E0q+LM=`) and state the prices version in their notes
(workflow `release-assets.yml`).

Go module `go/v0.3.0`: `Table` and `ParseTable` (read another prices.json with the same lookups),
`Default`, `Version`, `PricesVersion`, `Price.Tier` and `Price.Tiers` (service tiers, as in the
Python package), and a fuzz test for `ParseTable`.

## [1.6.2] - 2026-11-17

### Fixed (the GitHub Action only; the packages stay at 1.6.1)

The drift scan no longer reads an id that another provider sells at its own price as the model
maker's: a file listing OpenRouter's `deepseek/deepseek-v4-flash` at OpenRouter's price is not
quoting DeepSeek's price. Ids of rows from providers not being checked are skipped unless a
checked provider uses the same id. The repository's own self-check skips the generated table
files, which now hold Bedrock's and OpenRouter's prices next to the model makers'.

## [1.6.1] - 2026-11-17

### Added

`llm_prices.compat.tokencost` prices `perplexity/sonar-reasoning` (and `sonar-reasoning`) at its
last listed $1 / $5 per 1M tokens (archived pricing page of 2025-12-05; Perplexity removed the
model on 2025-12-15), as tokencost 0.1.26 did. No change to the table, the crate, the npm package
or the Go module; they move to 1.6.1 only to keep the versions in step.

## [1.6.0] - 2026-11-17

### Added

Crate, Python, npm, root module and Go module `go/v0.2.0` (new `Perplexity` provider).

Third-party host prices: Amazon Bedrock, OpenRouter and Perplexity now have rows of their own,
so `bedrock/us.deepseek.r1-v1:0`, `openrouter/meta-llama/llama-3.3-70b-instruct` and
`perplexity/sonar` cost what those hosts charge instead of finding nothing.

- Amazon Bedrock (64 rows): read daily from AWS's own price list (the AWS Price List bulk API
  offer file for AmazonBedrock, us-east-1), the data https://aws.amazon.com/bedrock/pricing/
  renders. On-demand input, output and cache-read prices, Batch inference, and the flex and
  priority service tiers. Model ids come from each model's card in the Bedrock user guide:
  the in-Region id (`deepseek.r1-v1:0`), the US geo inference profile (`us.deepseek.r1-v1:0`)
  and the bedrock-mantle id (`openai.gpt-oss-120b`). Global inference profiles bill at their
  own price and are not aliases. Claude on Bedrock still takes Anthropic's list price.
- OpenRouter (356 rows): read daily from OpenRouter's model list API
  (https://openrouter.ai/api/v1/models), keyed by OpenRouter's id. Prompt-length overrides
  become `long_context`, `:batch` variants the batch price; `:free`, zero-priced and router
  entries are left out.
- Perplexity (6 rows, checked by hand): the Agent API rates for the models Perplexity serves
  itself (`perplexity/sonar`, `perplexity/kimi-k3`, `perplexity/glm-5.3`, ...) from
  https://docs.perplexity.ai/docs/agent-api/models. New provider `perplexity` (also parses
  `pplx`), a host like Groq or Fireworks.

Lookup rules for the new rows:

- A provider's own row now wins over its fallback makers: `lookup_provider("openrouter",
  "x-ai/grok-4.7")` returns OpenRouter's row when OpenRouter lists the model.
- Without a provider path, Bedrock and OpenRouter rows are only found by their exact id
  (`deepseek.r1-v1:0`, `meta-llama/llama-3.3-70b-instruct`), so a bare `gpt-4` never takes
  OpenRouter's `openai/gpt-4` row.
- A maker's prefix on a model the maker does not sell (`x-ai/grok-4.1-fast`) tries the hosts'
  rows first and then a platform row with that exact id.
- A host lists models under another host's name (OpenRouter's `perplexity/sonar-pro`,
  `amazon/nova-2-lite-v1`); looking those up under the host that lists them finds them.
- `llm_prices.compat.tokencost` prices `bedrock/...`, `openrouter/...` and `perplexity/...`
  ids, and Sonar Pro, Sonar Reasoning Pro and Sonar Deep Research (`perplexity/sonar-pro`,
  ...) at the last prices Perplexity's pricing page listed before Sonar Chat Completions
  support ended on 2026-09-27 (archived snapshot of 2026-09-03), so code written for
  tokencost's `perplexity/` and `bedrock/` ids keeps working.

The daily refresh (`scripts/update.mjs`) now also reads the AWS price list and the OpenRouter
model list (`scripts/parse/bedrock.mjs`, `scripts/parse/openrouter.mjs`).

## [1.5.1] - 2026-11-17

### Fixed (crate, Python, npm, root module, Go module `go/v0.1.3`)

Model ids with a host or platform path are no longer priced with the model maker's
first-party row. Fireworks, Together AI, Groq, Bedrock, Vertex AI, Azure and OpenRouter charge
their own prices, yet `fireworks_ai/deepseek-chat` or `together_ai/deepseek-chat` came back
at DeepSeek's own $0.30 / $1.20 (and, in `llm_prices.compat.tokencost`, so did
`accounts/fireworks/models/deepseek-r1`). They now find nothing. The rules:

- A host path (`groq/`, `together_ai/`, `together/`, `fireworks_ai/`, `fireworks/`,
  `accounts/fireworks/models/`) resolves to that host's own row (`groq/openai/gpt-oss-120b`,
  `accounts/fireworks/models/kimi-k3`) or to nothing.
- A platform path (`bedrock/`, `vertex_ai/`, `azure/`, `openrouter/`) resolves to the
  platform's row or the list price of the makers it charges at (Bedrock: Anthropic; Vertex:
  Google and Anthropic; Azure: OpenAI; OpenRouter: the model makers), as before, and to
  nothing for anything else (`bedrock/deepseek-r1`, `azure/claude-sonnet-4-6`).
- A model maker's prefix (`anthropic/`, `openai/`, `gemini/`, `deepseek/`, ...) resolves to
  that maker's rows, or to a row whose own id carries the same prefix (Groq's
  `openai/gpt-oss-120b`). `anthropic/gpt-4o` no longer finds OpenAI's row.
- Looking a model up under a named provider never crosses into another host's path:
  `lookup_provider("deepseek", "together_ai/deepseek-chat")` finds nothing.
- `llm_prices.compat.tokencost` raises `KeyError` ("Model ... is not implemented"), as
  tokencost does for a model it does not price, for these ids instead of returning the
  maker's price. Its fallback to the id after the last slash only applies when every
  provider in the path sells that model at that price (`anthropic/claude-3-opus-20240229`,
  `bedrock/claude-3-haiku-20240307`).

No change to the table.

## [1.5.0] - 2026-11-17

- `llm_prices.compat.tokencost` prices about 160 retired and legacy ids tokencost priced
  (gpt-4, gpt-4-32k, chatgpt-4o-latest, gpt-4o-search-preview, o3/o4-mini deep research,
  claude-3-x, claude-2, gemini-1.5, gemini-2.0-flash, gemini-2.5 preview ids, older Mistral,
  Grok 2 and 3, Cohere Command and Embed) from archived snapshots of the providers' own
  pricing pages. See [docs/migrate-from-tokencost.md](docs/migrate-from-tokencost.md#retired-and-legacy-models)
  and the [Python changelog](python/CHANGELOG.md). No change to the table, the crate, the Go
  module or the Action; the crate, npm and root packages move to 1.5.0 only to keep
  the versions in step.

## [1.4.1] - 2026-11-17

### Table (generated 2026-11-17, 146 rows)

Every row re-read from its official page on 2026-11-17. No standard input or output price
changed. Checked against the numbers other price tables carry for the same models:

- `deepseek-flash` (`deepseek-v4-flash`) 0.30 / 1.20 and `deepseek-v4-pro` 1.32 / 3.96 are
  the peak cache-miss prices on https://api-docs.deepseek.com/quick_start/pricing; off-peak is
  half (`other_tiers.offpeak`), the cache-hit price is `cache_read`. Tables showing 0.14 / 0.28
  and 0.435 / 0.87 carry the prices from before the 2026-08-16 adjustment in
  https://api-docs.deepseek.com/updates.
- `claude-sonnet-5` 2 / 10: the page says the introductory price is now the standard price and
  the 3 / 15 increase planned for 2026-09-01 will not happen. The parser now copies the page's
  footnotes into the row note.
- `gemini-3.6-flash` 0.75 / 3.75 through 2026-12-31 and 1.50 / 7.50 from 2027-01-01 (already
  in `scheduled`); there is no context-length tier for this model.

Fixed or added, all from the official pages:

- Mistral (11 rows): `cache_read` (the page's cached-input column, for example Mistral Small 4
  0.015, Codestral 0.03), `batch_input` / `batch_output` (Batch tab, 0.5x) and
  `other_tiers.priority` (Priority tab, 1.75x); the note says what the Regional inference
  toggle costs. Before, these were `null` and the note said the page listed no price.
- Groq: `cache_read` 0.075 / 0.0375 / 0.0375 on gpt-oss-120b / gpt-oss-20b /
  gpt-oss-safeguard-20b (https://console.groq.com/docs/prompt-caching, 50% off input) and
  Batch API prices on gpt-oss-120b (0.075 / 0.30) and gpt-oss-20b (0.0375 / 0.15)
  (https://console.groq.com/docs/batch, 50% off).
- xAI (8 rows): `other_tiers.priority` (Priority Processing, 2x, with its long-context prices)
  and `inference_geo` `{"us": 1.1}` on grok-4.7 and grok-4.6.
- Together AI: `inference_geo` `{"us": 1.25}` only on the four models the page prices for
  us-api.together.ai; the other 18 rows said "the US-only endpoint costs 25% more" and now say
  the page lists no US-only price for them. Notes say the Batch API runs these models at
  standard rates.
- Fireworks AI: `inference_geo` `{"us": 1.5}` on the five models with a "(US)" row.
- DeepSeek notes say `input` is the cache-miss price.

## [1.4.0] - 2026-11-16

### Table (generated 2026-11-16, 146 rows)

Anthropic, OpenAI, Google and DeepSeek rows re-read from their pricing pages on 2026-11-16. No
price changed. New data, all read from the official pages:

- `inference_geo` on every row: `{"us": 1.1}` on Claude 4.6 and later (Opus 4.6 to 5.5,
  Sonnet 4.6 to 5.5, Haiku 5.5, Fable 5 to 5.2), from "Data residency pricing" on
  https://platform.claude.com/docs/en/about-claude/pricing; `null` elsewhere. Earlier Claude
  rows now say the page rejects the parameter for them, instead of a note on every row.
- Gemini `other_tiers.flex` and `other_tiers.priority` carry their own `long_context` prices
  (Gemini 2.5 Pro: flex 1.25 / 7.5, priority 4.5 / 27 above 200k tokens; Gemini 3.1 Pro
  Preview: flex 2 / 9, priority 7.2 / 32.4), from the Flex and Priority tabs of
  https://ai.google.dev/gemini-api/docs/pricing.
- Anthropic Priority Tier has no row: it is a capacity commitment with no per-token price, and
  https://platform.claude.com/docs/en/api/service-tiers says commitments are no longer sold.
  See [docs/tiers.md](docs/tiers.md).

### litellm.json (new)

- `litellm.json`: the table in LiteLLM's `model_prices_and_context_window.json` schema (USD per
  token, `*_batches`, `*_flex`, `*_priority`, `*_above_200k_tokens` keys, plus `source` and
  `llm_prices_checked` on every entry). Generated with the other files, committed by the daily
  refresh, served by jsDelivr. CI registers it with LiteLLM and checks `cost_per_token`
  against the table. Docs: [docs/litellm.md](docs/litellm.md).

### Action (drift check)

- Prices of a flex or priority tier above a long-context threshold are accepted (for example
  LiteLLM's `input_cost_per_token_above_200k_tokens_priority`). Before, they were compared
  with the standard long-context price and reported as drift.

### Examples (new)

- `examples/python` (one call, cache-aware cost, budget check, service tiers, tokencost
  migration, LiteLLM), `examples/go` (one call, cache-aware cost, budget check; `go test`
  checks the output) and `crate/examples` (`budget_check`, `tiers`). CI runs all of them.

### Go module 0.1.1

- `go/prices.json` carries the 2026-11-16 table. No API change.

## [Go 0.1.0] - 2026-11-15

### Go module (new)

- `go get github.com/Mattbusel/llm-prices/go`: the same table and lookup rules as the crate
  and the Python package, standard library only, with `prices.json` embedded via `go:embed`.
  `Lookup`, `LookupProvider`, `Cost`, `CostProvider`, `Price.CostUsage` (cache reads, 5m and
  1h cache writes, Batch API, long-context tiers), `ParseProvider` and `All`. Costs are
  bit-for-bit equal to the crate's. Docs: [docs/go.md](docs/go.md).
- `go/prices.json` is generated by `scripts/build.mjs` with the other package data, committed
  by the daily refresh, and checked against `prices.json` in CI, which also runs `go vet`,
  `go test` and `gofmt` on Go 1.21 and stable.

## [1.3.0] - 2026-11-15

### Action (drift check)

- **Ratio keys are not prices.** Keys naming a multiplier, ratio, factor, discount, markup,
  uplift, surcharge, percent or fraction (`cacheWriteMultiplier: 1.25`, `batch_discount: 0.5`)
  are skipped. Before, `cacheWriteMultiplier: 1.25` next to `claude-sonnet-4-6` read as a
  cache write price of 1.25 against the real 3.75 and failed the check on a number that was
  never a price.
- **Threshold tiers.** Prices inside a record that has a threshold key
  (`{ threshold: 200000, prompt: 2.5, completion: 15 }`, `above_tokens`, `min_prompt_tokens`,
  or a YAML list item with one) are compared with the model's long-context prices only. A
  model the table prices without a long-context tier is not judged there. Before, a tier price
  could pass because it matched the standard price, or fail because it did not.

### Daily refresh

- The OpenAI parser reads the new page layout: Cyber and Life Sciences models moved into the
  Specialized table as "(short context)" / "(long context)" rows with 30m and 12h cache write
  columns, and "Finetuning" became "Fine-tuning".
- The Gemini parser reads the page's new Markdoc source (`{% tabs %}`, `{% table %}`).
- Claude Fable 5.2 is mapped to `claude-fable-5-2`; Claude Mythos 5.2 is skipped like the
  other limited-availability Mythos models.
- A model that leaves its pricing page no longer fails the run. Its row is kept with its last
  listed prices, status `retired` and a note saying since when (`scripts/merge.mjs`); a page
  where most known models are missing still fails loudly. This closes the daily
  "Pricing page could not be read" issue and replaces the stale auto-update PR, which would
  have deleted those rows.

### Table

146 rows, generated 2026-11-15. New: `claude-fable-5-2`, `gpt-6.1-terra`, `gpt-6-cyber`,
`gpt-5.5-cyber-preview`, `gpt-6-rosalind`, `gpt-rosalind-research`, `gpt-rosalind-discovery`,
`gpt-6-astra-law`, `gemini-4-carbon`, `gemini-4-hydrogen`. `deepseek-chat` and
`deepseek-reasoner` are aliases of `deepseek-flash`. Twelve delisted OpenAI and Anthropic
models are kept as `retired`. No existing price changed.

### JavaScript from GitHub and CDNs

- `package.json`, `index.mjs`, `index.d.ts` and `data.mjs` at the root:
  `npm i github:Mattbusel/llm-prices#v1`, or
  `import { lookup } from 'https://cdn.jsdelivr.net/gh/Mattbusel/llm-prices@v1/index.mjs'`
  (also `https://esm.sh/gh/Mattbusel/llm-prices@v1`). See [docs/javascript.md](docs/javascript.md).

### Packages

- crates.io and PyPI 1.3.0: LangChain provider spellings (`google_genai`, `google_vertexai`,
  `bedrock_converse`, ...) in `Provider` parsing; Python `Price.input_per_token`,
  `output_per_token` and `per_token`. See [docs/python.md](docs/python.md).
