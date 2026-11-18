# llm-prices for Go

[![Go Reference](https://pkg.go.dev/badge/github.com/Mattbusel/llm-prices/go.svg)](https://pkg.go.dev/github.com/Mattbusel/llm-prices/go)

```sh
go get github.com/Mattbusel/llm-prices/go@latest
```

Go 1.21+, standard library only, no network access. The module embeds the same
[`prices.json`](../prices.json) as the Rust crate, the Python package and the JavaScript module
(the copy in `go/prices.json` is regenerated with the others and CI checks it byte for byte),
and gives the same answers: the lookup rules and the cost arithmetic are ports of the crate's,
and the tests port the Python package's. Prices are USD per 1M tokens; every row carries the
provider's pricing page (`SourceURL`) and the date it was read (`Checked`).

```go
import llmprices "github.com/Mattbusel/llm-prices/go"

p, ok := llmprices.LookupProvider("google_genai", "gemini-2.5-flash")
// ok == true, *p.Input == 0.3, *p.Output == 2.5 (USD per 1M tokens)

usd, ok := llmprices.Cost("claude-haiku-5-5", 10_000, 2_000) // 0.002, true
```

A false `ok` means the table has no price for that model (or that provider): keep your own
fallback, never treat it as zero.

## API

| Call | What it does |
|---|---|
| `Lookup(model) (Price, bool)` | Finds a model across every provider: aliases, any ASCII case, dated snapshots (`gpt-4o-2024-08-06` finds `gpt-4o` unless the snapshot has its own row), `-latest` ids, Bedrock ids (`us.anthropic.claude-sonnet-4-20250514-v1:0`), Vertex `@date` ids and provider prefixes (`models/`, `openai/`, `together_ai/`, `accounts/fireworks/models/`, ...). |
| `LookupProvider(provider, model) (Price, bool)` | Only the rows that provider sells, so `openai/gpt-oss-120b` on Groq gets Groq's price. Bedrock (including its regional and global Claude rows, see [hosts.md](hosts.md)) and OpenRouter have rows of their own; for a model without one, Vertex AI, Bedrock, Azure and OpenRouter fall back to the model maker's list price. The row's `Provider` says whose price it is. |
| `Cost(model, in, out)` / `CostProvider(provider, model, in, out)` | Standard cost in USD of one request (long-context prices above the threshold). |
| `Price.CostUsage(Usage) (CostBreakdown, bool)` | Cost of one response with cache reads, 5m and 1h cache writes and the Batch API, split by token kind. |
| `Price.InputPerToken()`, `Price.OutputPerToken()` | USD per token, for code that keeps per-token prices. |
| `ParseProvider(name) (Provider, bool)` | Provider names in any case with `_`, `-` and spaces alike: `google_genai`, `vertex_ai`, `bedrock_converse`, `azure_openai`, `together_ai`, `x-ai`, ... (the LangChain, LiteLLM and OpenRouter spellings). |
| `All() []Price`, `Provider.Rows()`, `Generated` | The whole table, one provider's rows, and the date it was generated. |
| `Price.Tier(name) (Price, bool)` | The prices of a service tier (`"flex"`, `"priority"`, `"fast"`, ...) as a `Price`, for `CostUsage`. |
| `PricesVersion`, `Version(b []byte)` | The table's version: `"sha256:"` and the SHA-256 of its prices.json (LF line endings). Every release states it; record it as `gen_ai.usage.cost.pricing.version`. |
| `ParseTable(b) (*Table, error)`, `Default()` | Read another prices.json (a newer release, your own fork) into a `Table` with the same `Lookup`, `LookupProvider`, `LookupIn`, `All`, `Rows` and `Version`; `Default()` is the embedded table. A `Table` never changes, so it is safe to share and to swap atomically. |

Returned rows are copies; changing them does not change the table.

### A cost callback for a gateway

```go
func requestCost(provider, model string, promptTokens, completionTokens int64) (float64, bool) {
	p, ok := llmprices.LookupProvider(provider, model)
	if !ok {
		return 0, false // unknown model: use your own default
	}
	// Set CacheRead, CacheWrite5m, CacheWrite1h and Batch too when the response reports them.
	c, ok := p.CostUsage(llmprices.Usage{Input: promptTokens, Output: completionTokens})
	return c.Total, ok
}
```

`Usage` field mapping from provider responses is in the
[package docs](https://pkg.go.dev/github.com/Mattbusel/llm-prices/go#Usage).

### Loading a newer table

```go
b, _ := os.ReadFile("prices.json") // or the prices.json of the latest release
t, err := llmprices.ParseTable(b)
if err != nil {
	t = llmprices.Default()
}
p, ok := t.LookupProvider("anthropic", "claude-sonnet-4-6")
fmt.Println(t.Version()) // sha256:...
```

Releases from 1.7.0 on publish `prices.json` with an Ed25519 signature (`prices.json.sig`); the
OpenTelemetry Collector processor in [otel/genaicostprocessor](otel.md) checks it before it
swaps a table in.

## Versions

The module lives in the `go/` directory and is tagged `go/vX.Y.Z`, so `go get` resolves it
independently of the crate and the Python and npm versions. A new tag follows every price
refresh that changes the table.
