# Changelog

Changes to the GitHub Action (`uses: Mattbusel/llm-prices@v1`), the price table and the
repository as a whole. The packages have their own changelogs:
[crate](crate/CHANGELOG.md), [Python](python/CHANGELOG.md), [JavaScript](js/CHANGELOG.md).
The `v1` tag moves to every 1.x release.

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
