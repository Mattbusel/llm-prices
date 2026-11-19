// Things that mention models and numbers but are not prices. None of these may be flagged.
export const SUPPORTED = ["gpt-4o", "gpt-4.1", "claude-opus-5-5", "gemini-2.5-pro"];

export const LIMITS = {
  "gpt-4o": { contextWindow: 128000, maxTokens: 16384 },
  "claude-opus-5-5": { context_window: 1000000, max_output_tokens: 128000 },
};

export const usage = { model: "claude-sonnet-5-5", input_tokens: 1200, output_tokens: 350 };

export const defaults = { model: "gemini-2.5-flash", temperature: 0.7, top_p: 0.95 };

// gpt-4.1 was released in 2025 with a 1M context.
export const retries = { "gpt-5": 3, timeoutSeconds: 30 };
