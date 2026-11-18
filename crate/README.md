# llm-prices

Dated, source-cited LLM API prices in USD per 1M tokens, with a provider-aware model-id lookup
and cost arithmetic for whole responses (prompt-cache reads and writes, batch, long-context
tiers, service tiers). No required dependencies, no build script, no network access; the table
is compiled in.

```toml
[dependencies]
llm-prices = "1.7"
```

```rust
let p = llm_prices::lookup("claude-sonnet-5-5").unwrap();
assert_eq!(p.input, Some(2.0));
assert_eq!(p.cache_read, Some(0.10));
println!("{} (checked {})", p.source_url, p.checked);

// Price a request under the provider that served it; None means "unknown", never zero.
let usd = llm_prices::lookup_provider("openai", "gpt-4o").and_then(|p| p.cost(12_000, 800));
```

Providers: Anthropic, OpenAI, Google Gemini, DeepSeek, Amazon Bedrock and OpenRouter (re-read
from their official pages or price APIs every day), plus Mistral, xAI, Groq, Together AI,
Fireworks AI, Cohere and Perplexity. Every row carries the provider's pricing page and the
date the price was read. A daily job in the repository opens a pull request when a page
changes, and a patch release follows.

- Features: `rust_decimal` (exact `Decimal` costs), `serde` (`Serialize` / `Deserialize`).
  Both off by default.
- MSRV: Rust 1.70 (checked in CI).
- Versions: price-only changes are patch releases; new API is a minor release.
  `llm_prices::PRICES_VERSION` names the exact table a build ships.
- Using it from another project, including behind an optional feature:
  https://github.com/Mattbusel/llm-prices/blob/main/docs/integrate.md

The same table ships as a Python package (`pip install llm-prices`), a JavaScript module, a Go
module and a GitHub Action that fails CI when prices hard-coded in your code drift from the
official pages: https://github.com/Mattbusel/llm-prices

License: MIT
