# llm-costcheck

**Your LLM cost tracking is wrong, and nothing tells you.**

Every agent framework, gateway and observability tool that turns tokens into dollars carries a price table. Those tables go stale the week a provider changes a price, they quietly bill models they have never heard of at a made-up default, and most of them have no idea prompt caching exists. The dashboard still shows a number. The number is wrong.

`llm-costcheck` reads your code and fails CI when that happens:

Given this `src/pricing.py`:

```python
# USD per 1M tokens
PRICES = {
    "claude-opus-5-5": {"input": 15.0, "output": 75.0, "cache_read": 0.2},
    "gpt-5.4": {"input": 2.5, "output": 15.0},
}

# Anything we have not listed yet.
DEFAULT_PRICING = {"input": 1.0, "output": 3.0}


def cost(model, prompt_tokens, completion_tokens):
    p = PRICES.get(model, DEFAULT_PRICING)
    return (prompt_tokens * p["input"] + completion_tokens * p["output"]) / 1e6
```

```
$ llm-costcheck
llm-costcheck: scanned 1 files, 5 hard-coded prices for 2 known models, 1 default price(s) (prices table 2026-11-18).

1 price(s) differ from the provider's page:
  src/pricing.py:3: your cost tracking is wrong by +275.0% for claude-opus-5-5
      input 15.0 per 1M here, 4/1M on the provider page; output 75.0 per 1M here, 20/1M on the provider page. On a 100K-token prompt, 5K-token output: booked $1.88, real $0.500 (+$1.38).
      source: https://platform.claude.com/docs/en/about-claude/pricing (checked 2026-11-18)

1 default price(s) for unknown models:
  src/pricing.py:8: your cost tracking is wrong by -54.0% for claude-sonnet-5-5
      models this code does not price are booked at $1/$3 per 1M (input/output) (named-default). 53 listed anthropic/openai model(s) are not priced here. claude-sonnet-5-5 on a 100K-token prompt, 5K-token output: booked $0.115, real $0.250. Worst: gpt-5.4-pro booked $0.115, real $3.90 (-97.1%).
      source: https://platform.claude.com/docs/en/about-claude/pricing (checked 2026-11-18)

1 model(s) priced without a cache-read price:
  src/pricing.py:4: your cost tracking is wrong by +71.1% for gpt-5.4
      no cache-read price here; the provider bills cache reads at 0.25/1M against 2.5/1M input. On a 100K-token prompt with 60K served from cache, 5K-token output: booked $0.325 if cached tokens bill at the input price, real $0.190.
      source: https://developers.openai.com/api/docs/pricing (checked 2026-11-18)
```

Exit code 1. The build is red until the numbers are true.

## What it refuses to let through

| Check | What it finds | Why it matters |
| --- | --- | --- |
| `drift` | A hard-coded price that differs from the provider's official page, in any unit (per token, per 1K, per 1M, divided literals), in Python, TypeScript, JavaScript, Rust, Go, JSON, YAML and TOML tables | A price table is a bug on a timer. Every week a provider ships a model or a price, a pasted table gets less true. |
| `fallback` | A default price used for any model the table does not list: `_ => (1.0, 3.0)`, `DEFAULT_PRICING = {...}`, `"default": {...}`, `PRICES.get(model, {...})`, `?? {...}`, and the worst one, a default of **$0**, including `if model not in COSTS: return` | A silent default is how a $1 budget cap lets through $3.42. New models land in the default bucket on release day, so the newest (and usually most expensive) models are the ones billed wrong. |
| `cache` | A model priced with input and output but no cache-read price, when the provider bills cache reads at a fraction of input | Agents resend the same long prompt every turn. With 60% of a 100K prompt served from cache, billing those tokens at the input price overstates the call by 33% (gpt-4o) to 96% (DeepSeek); dropping them understates it. |
| `missing` | A model used in quotes that has no price anywhere in the scanned files (off by default) | The next model you switch to is the one your tracker cannot see. |

Every issue carries the error in percent and in dollars on a fixed reference call (a 100K-token prompt with a 5K-token output; 60K of the prompt cached for cache prices), with the file, the line, the provider page and the date it was checked.

Prices come from [llm-prices](https://github.com/Mattbusel/llm-prices): dated, source-cited, refreshed from the official Anthropic, OpenAI, Google and DeepSeek pages, with batch, long-context, regional, flex, priority and fast-mode prices, so a correct non-standard price is never called drift.

## Receipts

We ran it on the 30 most-starred open-source agent frameworks and LLM observability tools. The results, with file:line for every claim and credit for the projects that get it right: [reports/2026-11-cost-tracking.md](https://github.com/Mattbusel/llm-prices/blob/main/reports/2026-11-cost-tracking.md).

Fixes that came out of this kind of scan and were merged upstream:

- [PraisonAI #6433](https://github.com/MervinPraison/PraisonAI/pull/6433): every model newer than gpt-4o / Claude 3.x was billed at a $1/$3 default, so `max_budget=1.0` tripped on the 9th call instead of the 3rd.
- [microsoft/conductor #645](https://github.com/microsoft/conductor/pull/645): 12 rows wrong, mostly a 2/8 placeholder copied from model to model.
- [claurst #526](https://github.com/Kuberwastaken/claurst/pull/526): Opus 5.5 priced 3.75x too high.
- [eval-view #277](https://github.com/hidai25/eval-view/pull/277): Opus 3x high, GPT-5.4-mini 10x low.

## Install

```sh
cargo install llm-costcheck
```

## Use

```sh
llm-costcheck                          # scan the current directory
llm-costcheck path/to/repo --format json
llm-costcheck --fail-on drift          # only stale prices fail; defaults and cache gaps are reported
llm-costcheck --providers all          # also Mistral, xAI, Groq, Together, Fireworks, Cohere, Bedrock, OpenRouter, Perplexity
llm-costcheck --prices prices.json     # compare against a newer llm-prices table than the one built in
```

Prices a host or reseller charges (`azure/...`, `vertex_ai/...`, Bedrock ids, OpenRouter rows, the non-maker sections of a provider-keyed map such as models.dev, files named for a host such as `vertex_ai.json`) are their own prices, not the model maker's, so they are not compared. Prices per image, per audio token, per query, in credits or in another currency are not per-token USD prices and are skipped too.

Test files and Rust `#[cfg(test)]` modules are skipped unless you pass `--include-tests`, because tests use made-up prices on purpose.

Exit codes: 0 clean, 1 an issue of a failing kind, 2 usage or I/O error.

## GitHub Action

```yaml
- uses: actions/checkout@v5
- uses: Mattbusel/llm-prices/costcheck@main
  with:
    fail-on: drift,fallback,cache   # the default
```

The action installs the crate, downloads the newest `prices.json` from this repo on every run (so a price change upstream turns your build red the same day), posts each issue as an annotation on its line, and writes a table to the job summary. Outputs: `drift-count`, `fallback-count`, `cache-count`, `missing-count`, `prices-found`, `worst-error`.

## Library

```rust
use llm_costcheck::{check, Index, Opts, Table};

let table = Table::embedded();
let index = Index::new(&table, &[]);
let files = vec![("pricing.py".to_string(), std::fs::read_to_string("pricing.py")?)];
for issue in check(files, &index, Opts::default()).issues {
    println!("{}:{}: {}", issue.file, issue.line, issue.headline());
}
# Ok::<(), std::io::Error>(())
```

## The fix is not a better table

Pasting fresher numbers buys you a month. The fix is to stop owning the table: read prices from a maintained source ([llm-prices](https://github.com/Mattbusel/llm-prices) for Rust, Python, JavaScript and Go, or LiteLLM's map), return an error for a model you cannot price instead of guessing, and price cache reads and writes as their own token kinds. Then run this check so the next drift is a red build instead of a wrong invoice.

## License

MIT
