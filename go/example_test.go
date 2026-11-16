package llmprices_test

import (
	"fmt"

	llmprices "github.com/Mattbusel/llm-prices/go"
)

func ExampleLookup() {
	p, ok := llmprices.Lookup("gpt-4o-2024-08-06")
	fmt.Println(ok, p.Model, *p.Input, *p.Output)
	// Output: true gpt-4o 2.5 10
}

func ExampleLookupProvider() {
	p, _ := llmprices.LookupProvider("groq", "openai/gpt-oss-120b")
	fmt.Println(p.Provider, *p.Input, *p.Output)

	// Bedrock has no rows of its own: Claude on Bedrock gets Anthropic's list price.
	p, _ = llmprices.LookupProvider("bedrock_converse", "us.anthropic.claude-sonnet-4-20250514-v1:0")
	fmt.Println(p.Provider, p.Model)

	_, ok := llmprices.LookupProvider("ollama", "llama3")
	fmt.Println(ok)
	// Output:
	// groq 0.15 0.6
	// anthropic claude-sonnet-4-20250514
	// false
}

func ExampleCost() {
	usd, ok := llmprices.Cost("claude-haiku-5-5", 10_000, 2_000)
	fmt.Printf("%v $%.4f\n", ok, usd)
	// Output: true $0.0020
}

func ExamplePrice_CostUsage() {
	p, _ := llmprices.Lookup("claude-sonnet-4-6")
	c, _ := p.CostUsage(llmprices.Usage{Input: 2_048, CacheRead: 45_000, CacheWrite5m: 3_500, Output: 1_024})
	fmt.Printf("input $%.6f, cache reads $%.4f, cache writes $%.6f, output $%.5f, total $%.6f\n",
		c.Input, c.CacheRead, c.CacheWrite, c.Output, c.Total)
	// Output: input $0.006144, cache reads $0.0135, cache writes $0.013125, output $0.01536, total $0.048129
}

// A cost callback for a gateway: the framework says which provider served the request.
func ExampleCostProvider() {
	usd, ok := llmprices.CostProvider("google_genai", "gemini-2.5-flash", 12_000, 800)
	if !ok {
		return // unknown model: use your own default, never zero
	}
	fmt.Printf("$%.4f\n", usd)
	// Output: $0.0056
}
