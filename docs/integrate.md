# Using llm-prices inside another project

This page is for maintainers deciding whether to depend on llm-prices: which call to make, how
to keep the dependency optional, what it pulls in, and how releases work. The per-language
references are [python.md](python.md), [javascript.md](javascript.md), [go.md](go.md) and the
crate docs on https://docs.rs/llm-prices.

## The calls to use

Each package exposes the same small surface. Most projects need two calls: find the price row
for the model a request used, then price the token counts the provider returned.

| | Python | Rust | JavaScript / TypeScript | Go |
|---|---|---|---|---|
| Install | `pip install llm-prices` | `cargo add llm-prices` | `npm i github:Mattbusel/llm-prices#v1` | `go get github.com/Mattbusel/llm-prices/go` |
| Row for a model, provider known | `lookup_provider(provider, model)` | `lookup_provider(provider, model)` | `lookupProvider(provider, model)` | `LookupProvider(provider, model)` |
| Row for a model id alone | `lookup(model)` | `lookup(model)` | `lookup(model)` | `Lookup(model)` |
| Input + output cost in USD | `price.cost(inp, out)` | `price.cost(inp, out)` | `price.cost(inp, out)` | `Cost(model, inp, out)` |
| Full response (cache, batch, long context) | `price.cost_usage(Usage(...))` | `price.cost_usage(&Usage {..})` | `price.costUsage({...})` | see [go.md](go.md) |
| Which table you shipped | `PRICES_VERSION`, `GENERATED` | `PRICES_VERSION`, `GENERATED` | `PRICES_VERSION`, `GENERATED` | `PricesVersion`, `Generated` |

A lookup that finds nothing returns `None` / `null` / `ok == false`. Treat that as "price
unknown", never as zero.

### Python, 5 lines

```python
import llm_prices

price = llm_prices.lookup_provider("anthropic", "claude-sonnet-4-6")
if price is not None:
    usd = price.cost(12_000, 800)   # 0.048 USD: 12,000 input + 800 output tokens
```

### Rust, 5 lines

```rust
fn cost_usd(provider: &str, model: &str, input: u64, output: u64) -> Option<f64> {
    llm_prices::lookup_provider(provider, model)?.cost(input, output)
}
// cost_usd("openai", "gpt-4o", 12_000, 800) == Some(0.038)
```

`provider` takes the names LangChain and LiteLLM use (`google_genai`, `vertex_ai`,
`bedrock_converse`, `together_ai`, `fireworks_ai`, ...). The full list is in
[python.md](python.md#pricing-a-request-when-you-know-the-provider-lookup_provider).

## Replacing a hand-written price table

The usual change in a host project is deleting a dict like this:

```python
# before: copied from a pricing page once, wrong the next time the provider changes a price
PRICES = {
    "gpt-4o": {"input": 2.50, "output": 10.00},
    "claude-3-5-sonnet": {"input": 3.00, "output": 15.00},
}

def cost(model, input_tokens, output_tokens):
    p = PRICES.get(model)
    return p and (input_tokens * p["input"] + output_tokens * p["output"]) / 1_000_000
```

```python
# after
import llm_prices

def cost(model, input_tokens, output_tokens):
    p = llm_prices.lookup(model)
    return p.cost(input_tokens, output_tokens) if p else None
```

What the host project gains, beyond the deleted lines:

- Model ids resolve the way providers send them back: dated snapshots
  (`claude-sonnet-4-5-20250929`), Bedrock ids (`us.anthropic.claude-sonnet-4-20250514-v1:0`),
  Vertex `@date` ids, `openai/`, `models/` and other prefixes, and aliases such as
  `deepseek-chat`. A hand-written dict usually misses these and reports a cost of nothing.
- `cost_usage` prices prompt-cache reads and writes, batch discounts, long-context tiers,
  service tiers (`tier="flex"`, `"priority"`, `"fast"`) and data-residency uplifts, which a flat
  input/output pair gets wrong.
- Each row has `source_url` and `checked`, so a reviewer can see where a number came from.
- A price change on a provider page reaches the host project as a patch release, not as an
  issue someone has to notice.

## Keeping it optional

When a project does not want a hard dependency, put llm-prices behind an extra or a feature
and fall back to whatever the project did before.

Python, `pyproject.toml`:

```toml
[project.optional-dependencies]
pricing = ["llm-prices>=1.7,<2"]
```

```python
try:
    import llm_prices
except ImportError:  # pip install yourpkg[pricing]
    llm_prices = None

def cost_usd(provider, model, input_tokens, output_tokens):
    if llm_prices is None:
        return None
    price = llm_prices.lookup_provider(provider, model)
    return price.cost(input_tokens, output_tokens) if price else None
```

A runnable copy is [examples/python/optional_pricing.py](../examples/python/optional_pricing.py).

Rust, `Cargo.toml`:

```toml
[dependencies]
llm-prices = { version = "1.7", optional = true }

[features]
pricing = ["dep:llm-prices"]
```

```rust
#[cfg(feature = "pricing")]
fn cost_usd(provider: &str, model: &str, input: u64, output: u64) -> Option<f64> {
    llm_prices::lookup_provider(provider, model)?.cost(input, output)
}

#[cfg(not(feature = "pricing"))]
fn cost_usd(_provider: &str, _model: &str, _input: u64, _output: u64) -> Option<f64> {
    None
}
```

JavaScript: load it with a dynamic `import('llm-prices')` inside `try`/`catch` and list it under
`optionalDependencies` or `peerDependenciesMeta` with `optional: true`.

Projects already built on LiteLLM's price map can keep it and layer this table over it with
`litellm.register_model(llm_prices.litellm.model_cost())` ([litellm.md](litellm.md)). Projects
that record OpenTelemetry GenAI spans can price them in a span processor or in the Collector
instead of in application code ([otel.md](otel.md)).

## What it adds to your dependency tree

| Package | Runtime dependencies | Minimum version | Size |
|---|---|---|---|
| Python `llm-prices` | none (`tiktoken` only with the `tokencost` extra) | Python 3.9 | wheel about 80 KB, typed (`py.typed`, checked with `mypy --strict` in CI) |
| Rust `llm-prices` | none; `rust_decimal` and `serde` are opt-in features | Rust 1.70 (`rust-version`, checked in CI) | table compiled in, no build script, no `unsafe` code |
| JavaScript | none | Node 18; also browsers, Deno and workers | ES module with `.d.ts` types |
| Go | standard library only | Go 1.21 | table embedded with `go:embed` |

No package makes network calls or reads files at runtime. The table is data compiled or bundled
into the package; updating prices means updating the package version.

## Versions and stability

- Semver. A release that only changes prices is a patch release (`1.7.1` to `1.7.2`). New API
  is a minor release. Removing or changing an existing call needs a major release.
- Pin `>=1.7,<2` (Python) or `"1.7"` (Cargo). Dependabot or Renovate then brings price updates as
  ordinary dependency bumps.
- `PRICES_VERSION` is `sha256:` plus the SHA-256 of the exact `prices.json` a release ships. Log
  it next to a computed cost (for example as `gen_ai.usage.cost.pricing.version`) so a cost can
  be traced to the table that produced it.
- How prices get in: a daily job re-reads the providers' official pages
  ([scripts/update.mjs](../scripts/update.mjs)) and opens a pull request when a number changes. A
  person reviews it against the page before it is merged and released. The parser stops for a
  provider when the page shape changes instead of guessing.

## License and maintenance

MIT ([LICENSE](../LICENSE)), for every package in this repository. llm-prices is maintained by
Matthew Busel, who keeps the table current with the providers' pages and answers issues at
https://github.com/Mattbusel/llm-prices/issues. A wrong price is fixed once the provider's
official page shows the correct one; open an issue with the page link.
