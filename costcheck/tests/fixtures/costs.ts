// Long-form records, one object per model.
export const MODELS = [
  {
    id: "claude-opus-4-7",
    contextWindow: 1_000_000,
    maxOutput: 128_000,
    inputPerMillion: 5,
    outputPerMillion: 25,
    cacheReadPerMillion: 0.5,
  },
  {
    id: "gemini-3.1-pro-preview",
    contextWindow: 1_048_576,
    inputPerMillion: 2,
    outputPerMillion: 12,
    longInputPerMillion: 4,
  },
  {
    id: "gpt-5.6-terra",
    inputPerMillion: 2,
    outputPerMillion: 10, // stale: page says 12
  },
];
