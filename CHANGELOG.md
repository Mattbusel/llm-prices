# Changelog

Changes to the GitHub Action (`uses: Mattbusel/llm-prices@v1`), the price table and the
repository as a whole. The packages have their own changelogs:
[crate](crate/CHANGELOG.md), [Python](python/CHANGELOG.md), [JavaScript](js/CHANGELOG.md).
The Go module is tagged `go/vX.Y.Z` and listed here.
The `v1` tag moves to every 1.x release.

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
