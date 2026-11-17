// Examples for the Go module: the cost of one call, a cache-aware cost, and a budget check.
//
//	cd examples/go && go run .
package main

import (
	"fmt"

	llmprices "github.com/Mattbusel/llm-prices/go"
)

func main() {
	oneCall()
	cacheAware()
	budgetCheck()
}

// The cost of one call from the usage an OpenAI response reports: prompt_tokens 1,200 (300
// cached) and completion_tokens 350.
func oneCall() {
	p, ok := llmprices.Lookup("gpt-4o-2024-08-06")
	if !ok {
		panic("gpt-4o not in the table")
	}
	c, _ := p.CostUsage(llmprices.Usage{Input: 900, CacheRead: 300, Output: 350})
	fmt.Printf("one call, %s: $%.6f (prices from %s)\n", p.Model, c.Total, p.SourceURL)
}

// What prompt caching saves on a 40k-token system prompt.
func cacheAware() {
	p, _ := llmprices.Lookup("claude-sonnet-4-6")
	uncached, _ := p.CostUsage(llmprices.Usage{Input: 40_500, Output: 800})
	write, _ := p.CostUsage(llmprices.Usage{Input: 500, CacheWrite5m: 40_000, Output: 800})
	read, _ := p.CostUsage(llmprices.Usage{Input: 500, CacheRead: 40_000, Output: 800})
	fmt.Printf("cache-aware, %s: uncached $%.4f, cache write $%.4f, cache read $%.4f\n", p.Model, uncached.Total, write.Total, read.Total)
}

// Refuse a call before sending it when its worst case would break the budget.
func budgetCheck() {
	const limit = 0.05
	spent := 0.0
	for i := 0; i < 10; i++ {
		worst, ok := llmprices.Cost("gemini-2.5-pro", 8_000, 2_000)
		if !ok {
			panic("no price")
		}
		if spent+worst > limit {
			fmt.Printf("budget: call %d refused, worst case $%.4f would pass $%.2f (spent $%.4f)\n", i, worst, limit, spent)
			return
		}
		actual, _ := llmprices.Cost("gemini-2.5-pro", 8_000, 1_500)
		spent += actual
		fmt.Printf("budget: call %d sent, spent $%.4f\n", i, spent)
	}
}
