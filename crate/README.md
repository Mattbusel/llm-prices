# llm-prices

Dated, source-cited LLM API prices for Anthropic, OpenAI, Google Gemini and DeepSeek, in USD per 1M tokens, with a model-id lookup. No dependencies, no network access.

```rust
let p = llm_prices::lookup("claude-sonnet-5-5").unwrap();
assert_eq!(p.input, Some(2.0));
assert_eq!(p.cache_read, Some(0.10));
println!("{} (checked {})", p.source_url, p.checked);
```

Every row carries the provider's official pricing page and the date it was read. The data is generated from [prices.json](https://github.com/Mattbusel/llm-prices/blob/main/prices.json); a daily job in that repo re-reads the provider pages and opens a pull request when a price changes, and a new crate version follows.

The same repo ships a GitHub Action that fails CI when prices hard-coded in your code drift from the official pages: https://github.com/Mattbusel/llm-prices

License: MIT
