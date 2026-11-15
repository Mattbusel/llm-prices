// Facts that are not prices: page URLs, the API id for each name the pricing page uses,
// and where the id was read. Prices never live here; they come from the parsed pages.

export const SOURCES = {
  anthropic: {
    page: 'https://platform.claude.com/docs/en/about-claude/pricing',
    fetch: 'https://platform.claude.com/docs/en/about-claude/pricing.md',
  },
  openai: {
    page: 'https://developers.openai.com/api/docs/pricing',
    fetch: 'https://developers.openai.com/api/docs/pricing.md',
  },
  google: {
    page: 'https://ai.google.dev/gemini-api/docs/pricing',
    fetch: 'https://ai.google.dev/gemini-api/docs/pricing.md.txt',
  },
  deepseek: {
    page: 'https://api-docs.deepseek.com/quick_start/pricing',
    fetch: 'https://api-docs.deepseek.com/quick_start/pricing/',
  },
};

// Providers with no parser. Their rows in prices.json were read by hand from these pages
// (each row's checked date says when); scripts/update.mjs keeps them as they are, and the drift
// check only uses them when they are named in --providers.
export const MANUAL_SOURCES = {
  mistral: 'https://docs.mistral.ai/inference/pricing',
  xai: 'https://docs.x.ai/developers/pricing',
  cohere: 'https://cohere.com/pricing',
  groq: 'https://console.groq.com/docs/models',
  together: 'https://docs.together.ai/docs/serverless/models',
  fireworks: 'https://docs.fireworks.ai/serverless/pricing',
};

// Anthropic's pricing page lists display names, not API ids. Ids come from
// platform.claude.com/docs/en/about-claude/models/overview, the per-model overview pages,
// and the model-deprecations page (for retired dated ids).
// Claude Mythos models are limited-availability (verified organizations only) and are left out
// on purpose. Claude Haiku 3.5 left the pricing page on 2026-11-06; scripts/update.mjs keeps
// its last listed row, marked retired.
export const ANTHROPIC_IDS = {
  'Claude Fable 5.2': { model: 'claude-fable-5-2' },
  'Claude Fable 5.1': { model: 'claude-fable-5-1' },
  'Claude Fable 5': { model: 'claude-fable-5' },
  'Claude Opus 5.5': { model: 'claude-opus-5-5' },
  'Claude Opus 5': { model: 'claude-opus-5' },
  'Claude Opus 4.8': { model: 'claude-opus-4-8' },
  'Claude Opus 4.7': { model: 'claude-opus-4-7' },
  'Claude Opus 4.6': { model: 'claude-opus-4-6' },
  'Claude Opus 4.5': { model: 'claude-opus-4-5', aliases: ['claude-opus-4-5-20251101'] },
  'Claude Opus 4.1': { model: 'claude-opus-4-1-20250805' },
  'Claude Opus 4': { model: 'claude-opus-4-20250514' },
  'Claude Sonnet 5.5': { model: 'claude-sonnet-5-5' },
  'Claude Sonnet 5': { model: 'claude-sonnet-5' },
  'Claude Sonnet 4.6': { model: 'claude-sonnet-4-6' },
  'Claude Sonnet 4.5': { model: 'claude-sonnet-4-5-20250929' },
  'Claude Sonnet 4': { model: 'claude-sonnet-4-20250514' },
  'Claude Haiku 5.5': { model: 'claude-haiku-5-5' },
  'Claude Haiku 4.5': { model: 'claude-haiku-4-5', aliases: ['claude-haiku-4-5-20251001'] },
};

export const ANTHROPIC_SKIP = ['Claude Mythos 5.2', 'Claude Mythos 5.1', 'Claude Mythos 5'];

// Gemini sections whose prices are per text token and comparable with other LLM rows.
// Image, audio, TTS, live, music and embedding sections are skipped.
export const GEMINI_TEXT_MODELS = [
  'gemini-4-carbon',
  'gemini-4-argon',
  'gemini-4-hydrogen',
  'gemini-3.8-flash',
  'gemini-3.6-flash',
  'gemini-3.5-flash-lite',
  'gemini-3.1-flash-lite',
  'gemini-3.1-pro-preview',
  'gemini-3-flash-preview',
  'gemini-2.5-pro',
  'gemini-2.5-flash',
  'gemini-2.5-flash-lite',
];

// Legacy DeepSeek model names that the pricing page no longer lists. DeepSeek's change log
// (https://api-docs.deepseek.com/updates, 2026-04-24) says deepseek-chat and deepseek-reasoner
// point to the non-thinking and thinking modes of deepseek-v4-flash, and the pricing page says
// deepseek-v4-flash is served and billed as deepseek-flash. They become aliases of that row so a
// lookup of either name gets the Flash price.
export const DEEPSEEK_LEGACY_ALIASES = ['deepseek-chat', 'deepseek-reasoner'];
export const DEEPSEEK_LEGACY_SOURCE = 'https://api-docs.deepseek.com/updates';
