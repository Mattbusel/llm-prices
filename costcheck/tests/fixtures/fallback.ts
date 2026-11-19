// USD per 1M tokens
const PRICES: Record<string, { input: number; output: number }> = {
  "gpt-4o": { input: 2.5, output: 10 },
  "claude-sonnet-4-6": { input: 3, output: 15 },
};
export function price(model: string) {
  return PRICES[model] ?? { input: 3, output: 15 };
}
export function rate(model: string) {
  switch (model) {
    case "gpt-4o-mini": return { input: 0.15, output: 0.6 };
    default: return { input: 1, output: 2 };
  }
}
