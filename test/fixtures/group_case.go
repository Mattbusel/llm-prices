package pricing

func rates(model string) (float64, float64) {
	switch model {
	case "gpt-5", "gpt-5.1":
		return Rates{Input: 1.25, Output: 10.0}
	case "gpt-5-mini":
		return Rates{Input: 0.25, Output: 2.0}
	}
	return Rates{}
}
