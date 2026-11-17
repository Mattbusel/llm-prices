package main

func Example() {
	main()
	// Output:
	// one call, gpt-4o: $0.006125 (prices from https://developers.openai.com/api/docs/pricing)
	// cache-aware, claude-sonnet-4-6: uncached $0.1335, cache write $0.1635, cache read $0.0255
	// budget: call 0 sent, spent $0.0250
	// budget: call 1 refused, worst case $0.0300 would pass $0.05 (spent $0.0250)
}
