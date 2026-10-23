# llm-prices

Your agent's cost math is wrong the day a provider changes prices. This fails your CI before your users notice.

`llm-prices` is two things:

1. **A price table** ([prices.json](prices.json), [prices.toml](prices.toml)) for Anthropic, OpenAI, Google Gemini and DeepSeek. Every row names the official pricing page it was read from and the date it was checked. A parser in this repo rebuilds the table from those pages, and a daily job opens a pull request when a number changes.
2. **A GitHub Action** that scans your code for model ids sitting next to hard-coded prices, compares them with the table, and fails the build when they differ.

## Usage

`.github/workflows/llm-prices.yml`:

```yaml
on: [push, pull_request]
jobs:
  llm-prices:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v5
      - uses: Mattbusel/llm-prices@v1
```

Sample output on a repo with three stale lines:

```text
llm-prices: scanned 2 files, found 28 hard-coded prices for 11 known models (table generated 2026-10-23).

4 price(s) differ from the provider's official page:

file:line          model              field       yours                   real (USD/1M)  source
src/costs.py:6     gpt-5              output      0.012 per 1K (= 12/1M)  10             https://developers.openai.com/api/docs/pricing
src/pricing.rs:14  claude-sonnet-5-5  cache_read  0.20 per 1M (= 0.2/1M)  0.1            https://platform.claude.com/docs/en/about-claude/pricing
src/pricing.rs:38  claude-sonnet-5    input       3.0 per 1M (= 3/1M)     2              https://platform.claude.com/docs/en/about-claude/pricing
src/pricing.rs:38  claude-sonnet-5    output      15.0 per 1M (= 15/1M)   10             https://platform.claude.com/docs/en/about-claude/pricing
```

Each drifted price is also posted as an error annotation on the line in the pull request, and the table goes into the job summary.

### Inputs

| Input | Default | Meaning |
| --- | --- | --- |
| `paths` | `**` | Globs to scan, comma or newline separated. |
| `exclude` | (none) | Globs to skip. `node_modules`, `target`, `dist`, `build`, `vendor` and `.git` are always skipped. |
| `providers` | `all` | `anthropic`, `openai`, `google`, `deepseek`, comma separated. |
| `fail-on` | `drift` | `drift`: fail when a hard-coded price differs from the official page. `missing`: also fail when a model id used in quotes has no price anywhere in the scanned files (only for providers your repo already prices). `never`: report only. |
| `tolerance` | `0.01` | Relative difference allowed before a price counts as drift (0.01 is 1%). |
| `include-tests` | `false` | Also scan test files (`tests/`, `test/`, `__tests__/`, `benches/`, `fixtures/`, `*_test.go`, `test_*.py`, `*.test.ts`, `*.spec.js` and similar) and Rust `#[cfg(test)]` modules. Off by default: tests often use made-up prices. |
| `show-matches` | `false` | Also list every price that matched. |
| `annotations` | `true` | Post each drifted price as an error annotation on its line. |
| `working-directory` | workspace | Directory to scan. |

Outputs: `drift-count`, `missing-count`, `prices-found`.

The Action is a composite action that runs one Node script with the standard library only: no `npm install`, no build step, no network access. It checks against the `prices.json` of the tag you pin, so `@v1` moves forward when prices change and `@v1.0.0` stays fixed.

Run it locally without GitHub:

```sh
node path/to/llm-prices/src/cli.mjs --paths "src/**" --fail-on drift --all
```

## How matching works

The check reads `.json`, `.toml`, `.yaml`/`.yml`, `.py`, `.ts`/`.tsx`/`.js`/`.jsx`/`.mjs`/`.cjs`, `.rs` and `.go` files. It only looks at a number when all of these hold:

- A known model id is on the line, as a key (`"gpt-4o": {...}`, `gpt-4o:`, `"claude-opus-5-5" => (...)`, `case "o3":`, `[pricing."gpt-4o"]`) or as the value of a `model`/`id`/`name` field in the same record.
- The number is in that model's block: the same line, the braces that follow the key, the indented YAML/Python block, the TOML table, or the record around the `model` field. A line that names two different models is skipped, except one match arm or case label for several models (`"a" | "b" => ...`, `case "a", "b":`), which is checked against each of them.
- The number's key says what it is: `input`, `prompt`, `output`, `completion`, `cache_read`, `cached_input`, `cache_creation`, `cache_write_1h`, `batch_input` and similar spellings. Keys that are token counts or limits (`max_tokens`, `input_tokens`, `context_window`) are ignored. A bare `(input, output)` pair right after the model id on the same line is read as input then output.

Units: a key or divisor that names the unit wins (`per_token`, `per_1k`, `per_mtok`, `/ 1_000_000.0`). Otherwise the number is read as per 1M, per 1K and per token, and it passes if any reading equals a listed price. When none does, it is reported as drift only if the closest reading is within 20x of the real price; numbers further off are treated as "not a price".

A number passes when it matches any price the page lists for that model and field: standard, batch, long-context, flex, fast or priority, off-peak (DeepSeek), a published future price (Gemini promotions), or 1.1x the standard price (Anthropic and OpenAI regional / US-only endpoints). That keeps false alarms down at the cost of missing a price that was copied from the wrong tier.

Model ids match with or without provider prefixes (`anthropic/`, `models/`, `openai/`), Bedrock ids (`us.anthropic.claude-sonnet-4-20250514-v1:0`), Vertex `@date` suffixes, and the undated form of Anthropic's dated ids. OpenAI snapshot ids that the page does not list (for example `gpt-4o-2024-08-06`) are not matched to the undated row, because OpenAI prices some snapshots differently.

Known limits: prices computed at runtime, read from a database, or spread across files with no model id near them are not seen. Prefix matching in your code (`starts_with("claude-sonnet")`) is a separate bug class this does not detect.

## The data

[prices.json](prices.json) has one row per model (74 rows on 2026-10-23: 18 Anthropic, 44 OpenAI, 10 Gemini, 2 DeepSeek). All prices are USD per 1M tokens.

```json
{
  "provider": "anthropic",
  "model": "claude-haiku-5-5",
  "aliases": [],
  "status": "listed",
  "unit": "USD per 1M tokens",
  "input": 0.1,
  "output": 0.5,
  "cache_write": { "5m": 0.125, "1h": 0.2 },
  "cache_read": 0.01,
  "batch_input": 0.05,
  "batch_output": 0.25,
  "long_context": {
    "threshold_tokens": 100000,
    "input": 0.5, "output": 2.5,
    "cache_write": { "5m": 0.625, "1h": 1 }, "cache_read": 0.05,
    "batch_input": 0.25, "batch_output": 1.25
  },
  "other_tiers": null,
  "scheduled": null,
  "source_url": "https://platform.claude.com/docs/en/about-claude/pricing",
  "checked": "2026-10-23",
  "note": "..."
}
```

| Field | Meaning |
| --- | --- |
| `cache_write` | Price to write the prompt cache, keyed by cache lifetime: `5m`/`1h` (Anthropic), `30m`/`12h` (OpenAI). `null` when the page lists none. |
| `long_context` | Prices for requests over `threshold_tokens` (Claude Haiku 5.5 at 100,000; OpenAI at 272K; Gemini Pro at 200k). |
| `other_tiers` | `fast`, `flex`, `ultrafast`, `priority`, `offpeak` where the page lists them. |
| `scheduled` | Prices the page already announces for a future date (Gemini promotional pricing). |
| `status` | `retired` when Anthropic's page marks the model retired on its API; otherwise `listed`. |
| `note` | Anything a number alone would get wrong. A `null` price is always explained here. |

Sources, all read on 2026-10-23:

- Anthropic: https://platform.claude.com/docs/en/about-claude/pricing (model ids from the models overview, per-model and deprecation pages)
- OpenAI: https://developers.openai.com/api/docs/pricing
- Google Gemini API: https://ai.google.dev/gemini-api/docs/pricing (Paid Tier, text-token prices)
- DeepSeek: https://api-docs.deepseek.com/quick_start/pricing

Left out on purpose: Claude Mythos 5.1 and Mythos 5 (on the page, but with no public API id), image, audio, video, TTS and live models (not priced per text token), fine-tuning prices, and Bedrock or Vertex list prices (those pages differ by region).

### How the table stays current

[.github/workflows/refresh.yml](.github/workflows/refresh.yml) runs every day. It fetches the four pages, parses them with [scripts/update.mjs](scripts/update.mjs), and:

- opens (or updates) a pull request when any price changed, listing each old and new value with its source URL. It never merges anything; a person reviews the diff against the page.
- opens an issue when a page cannot be fetched or parsed, or when Anthropic lists a model name the parser has no API id for. Rows for that provider are kept from the last good run.

The parser fails loudly instead of guessing: a renamed column, a missing model or an unreadable cell stops the update for that provider.

## Rust crate

```toml
[dependencies]
llm-prices = "1"
```

```rust
let p = llm_prices::lookup("anthropic/claude-sonnet-5-5").unwrap();
assert_eq!(p.cache_read, Some(0.10));
let usd = llm_prices::lookup("claude-haiku-5-5").unwrap().cost(150_000, 2_000); // long-context prices apply
```

Zero dependencies; the table is compiled in. Source in [crate/](crate/).

## Other price tables

[LiteLLM's model_prices_and_context_window.json](https://github.com/BerriAI/litellm/blob/main/model_prices_and_context_window.json) and [models.dev](https://models.dev) both cover far more models and providers than this repo; use them when coverage matters most. This repo is narrower on purpose. What it adds:

- Every row carries its official source URL and the date it was read, and the table is regenerated from those pages by a parser you can read and run (`node scripts/update.mjs`).
- Cache-write lifetimes, long-context thresholds, scheduled price changes and off-peak prices are separate fields instead of being folded into one number.
- The CI check runs against your own code, so a stale number in your repo fails your build. A correct shared table does not help when your code has its own copy.

## Try it without installing

- LLM API cost calculator: https://tensorust.com/tools/llm-api-cost/?utm_source=github&utm_medium=readme&utm_campaign=llm-prices
- Token counter: https://tensorust.com/tools/token-counter/?utm_source=github&utm_medium=readme&utm_campaign=llm-prices

## Development

```sh
node --test test/*.test.mjs                 # unit, parser and CLI tests
node scripts/update.mjs                     # re-fetch the pages and rewrite the table if a price changed
node scripts/update.mjs --pages test/pages  # same, from the saved page excerpts
```

Found a wrong number? Open an issue with the provider page link. Corrections only land when the official page shows them.

## License

MIT. See [LICENSE](LICENSE).
