package pricing

// Price is USD per 1M tokens.
type Price struct {
	Input  float64
	Output float64
	Cached float64
}

var Table = map[string]Price{
	"gpt-4.1-mini": {Input: 0.40, Output: 1.60, Cached: 0.10},
	"gpt-4.1-nano": {Input: 0.10, Output: 0.40, Cached: 0.025},
	"o4-mini":      {Input: 1.10, Output: 4.40, Cached: 0.55}, // stale cached: page says 0.275
}
