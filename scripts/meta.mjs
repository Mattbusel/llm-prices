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
  // AWS's own machine-readable price list (the AWS Price List bulk API) for Amazon Bedrock in
  // us-east-1; https://aws.amazon.com/bedrock/pricing/ renders its tables from the same data.
  bedrock: {
    page: 'https://aws.amazon.com/bedrock/pricing/',
    fetch: 'https://pricing.us-east-1.amazonaws.com/offers/v1.0/aws/AmazonBedrock/current/us-east-1/index.json',
  },
  // OpenRouter's own model list API, which is what https://openrouter.ai/models shows and bills.
  openrouter: {
    page: 'https://openrouter.ai/models',
    fetch: 'https://openrouter.ai/api/v1/models',
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
  perplexity: 'https://docs.perplexity.ai/docs/agent-api/models',
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

// Amazon Bedrock: the AWS price list names models by "provider|model" (its `provider` and
// `model` attributes; Amazon's own Nova models have no provider). The API ids come from each
// model's card under https://docs.aws.amazon.com/bedrock/latest/userguide/model-cards.html
// (`card` is the slug after model-card-): `model` is the in-Region bedrock-runtime id, aliases
// are the bedrock-mantle id and the US geo inference profile (us.), which bills at the source
// Region's price. Global inference profiles (global.) bill at their own prices and are left out.
export const BEDROCK_IDS = {
  'DeepSeek|DeepSeek V3.1': { model: 'deepseek.v3-v1:0', name: 'DeepSeek-V3.1', aliases: ["deepseek.v3.1"], card: 'deepseek-deepseek-v3-1' },
  'DeepSeek|DeepSeek v3.2': { model: 'deepseek.v3.2', name: 'DeepSeek V3.2', aliases: [], card: 'deepseek-deepseek-v3-2' },
  'DeepSeek|R1': { model: 'deepseek.r1-v1:0', name: 'DeepSeek-R1', aliases: ["us.deepseek.r1-v1:0"], card: 'deepseek-deepseek-r1' },
  'Google|Gemma 3 12B': { model: 'google.gemma-3-12b-it', name: 'Gemma 3 12B IT', aliases: [], card: 'google-gemma-3-12b-it' },
  'Google|Gemma 3 27B': { model: 'google.gemma-3-27b-it', name: 'Gemma 3 27B PT', aliases: [], card: 'google-gemma-3-27b-pt' },
  'Google|Gemma 3 4B': { model: 'google.gemma-3-4b-it', name: 'Gemma 3 4B IT', aliases: [], card: 'google-gemma-3-4b-it' },
  'Google|google.gemma-4-26b-a4b': { model: 'google.gemma-4-26b-a4b', name: 'Gemma 4 26B-A4B', aliases: [], card: 'google-gemma-4-26b-a4b' },
  'Google|google.gemma-4-31b': { model: 'google.gemma-4-31b', name: 'Gemma 4 31B', aliases: [], card: 'google-gemma-4-31b' },
  'Google|google.gemma-4-e2b': { model: 'google.gemma-4-e2b', name: 'Gemma 4 E2B', aliases: [], card: 'google-gemma-4-e2b' },
  'Kimi AI|Kimi K2 Thinking': { model: 'moonshot.kimi-k2-thinking', name: 'Kimi K2 Thinking', aliases: ["moonshotai.kimi-k2-thinking"], card: 'moonshot-ai-kimi-k2-thinking' },
  'Moonshot AI|Kimi K2 Thinking': { model: 'moonshot.kimi-k2-thinking', name: 'Kimi K2 Thinking', aliases: ["moonshotai.kimi-k2-thinking"], card: 'moonshot-ai-kimi-k2-thinking' },
  'Moonshot AI|Kimi K2.5': { model: 'moonshotai.kimi-k2.5', name: 'Kimi K2.5', aliases: [], card: 'moonshot-ai-kimi-k2-5' },
  'Moonshot AI|Kimi K3': { model: 'moonshotai.kimi-k3', name: 'Kimi K3', aliases: ["us.moonshotai.kimi-k3"], card: 'moonshot-ai-kimi-k3' },
  'Meta|Llama 3 70B': { model: 'meta.llama3-70b-instruct-v1:0', name: 'Llama 3 70B Instruct', aliases: [], card: 'meta-llama-3-70b-instruct' },
  'Meta|Llama 3 8B': { model: 'meta.llama3-8b-instruct-v1:0', name: 'Llama 3 8B Instruct', aliases: [], card: 'meta-llama-3-8b-instruct' },
  'Meta|Llama 3.1 405B': { model: 'meta.llama3-1-405b-instruct-v1:0', name: 'Llama 3.1 405B Instruct', aliases: ["us.meta.llama3-1-405b-instruct-v1:0"], card: 'meta-llama-3-1-405b-instruct' },
  'Meta|Llama 3.1 70B': { model: 'meta.llama3-1-70b-instruct-v1:0', name: 'Llama 3.1 70B Instruct', aliases: ["us.meta.llama3-1-70b-instruct-v1:0"], card: 'meta-llama-3-1-70b-instruct' },
  'Meta|Llama 3.1 8B': { model: 'meta.llama3-1-8b-instruct-v1:0', name: 'Llama 3.1 8B Instruct', aliases: ["us.meta.llama3-1-8b-instruct-v1:0"], card: 'meta-llama-3-1-8b-instruct' },
  'Meta|Llama 3.2 11B': { model: 'meta.llama3-2-11b-instruct-v1:0', name: 'Llama 3.2 11B Instruct', aliases: ["us.meta.llama3-2-11b-instruct-v1:0"], card: 'meta-llama-3-2-11b-instruct' },
  'Meta|Llama 3.2 1B': { model: 'meta.llama3-2-1b-instruct-v1:0', name: 'Llama 3.2 1B Instruct', aliases: ["us.meta.llama3-2-1b-instruct-v1:0"], card: 'meta-llama-3-2-1b-instruct' },
  'Meta|Llama 3.2 3B': { model: 'meta.llama3-2-3b-instruct-v1:0', name: 'Llama 3.2 3B Instruct', aliases: ["us.meta.llama3-2-3b-instruct-v1:0"], card: 'meta-llama-3-2-3b-instruct' },
  'Meta|Llama 3.2 90B': { model: 'meta.llama3-2-90b-instruct-v1:0', name: 'Llama 3.2 90B Instruct', aliases: ["us.meta.llama3-2-90b-instruct-v1:0"], card: 'meta-llama-3-2-90b-instruct' },
  'Meta|Llama 3.3 70B': { model: 'meta.llama3-3-70b-instruct-v1:0', name: 'Llama 3.3 70B Instruct', aliases: ["us.meta.llama3-3-70b-instruct-v1:0"], card: 'meta-llama-3-3-70b-instruct' },
  'Meta|Llama 4 Maverick 17B': { model: 'meta.llama4-maverick-17b-instruct-v1:0', name: 'Llama 4 Maverick 17B Instruct', aliases: ["us.meta.llama4-maverick-17b-instruct-v1:0"], card: 'meta-llama-4-maverick-17b-instruct' },
  'Meta|Llama 4 Scout 17B': { model: 'meta.llama4-scout-17b-instruct-v1:0', name: 'Llama 4 Scout 17B Instruct', aliases: ["us.meta.llama4-scout-17b-instruct-v1:0"], card: 'meta-llama-4-scout-17b-instruct' },
  'Minimax AI|MiniMax M2.5': { model: 'minimax.minimax-m2.5', name: 'MiniMax M2.5', aliases: [], card: 'minimax-minimax-m2-5' },
  'Minimax AI|Minimax M2': { model: 'minimax.minimax-m2', name: 'MiniMax M2', aliases: [], card: 'minimax-minimax-m2' },
  'Minimax AI|Minimax M2.1': { model: 'minimax.minimax-m2.1', name: 'MiniMax M2.1', aliases: [], card: 'minimax-minimax-m2-1' },
  'Mistral AI|Devstral': { model: 'mistral.devstral-2-123b', name: 'Devstral 2 123B', aliases: [], card: 'mistral-ai-devstral-2-123b' },
  'Mistral|Magistral Small 1.2': { model: 'mistral.magistral-small-2509', name: 'Magistral Small 2509', aliases: [], card: 'mistral-ai-magistral-small-2509' },
  'Mistral|Ministral 14B 3.0': { model: 'mistral.ministral-3-14b-instruct', name: 'Ministral 14B 3.0', aliases: [], card: 'mistral-ai-ministral-14b-3-0' },
  'Mistral|Ministral 3B 3.0': { model: 'mistral.ministral-3-3b-instruct', name: 'Ministral 3B', aliases: [], card: 'mistral-ai-ministral-3b' },
  'Mistral|Ministral 8B 3.0': { model: 'mistral.ministral-3-8b-instruct', name: 'Ministral 3 8B', aliases: [], card: 'mistral-ai-ministral-3-8b' },
  'Mistral|Mistral 7B': { model: 'mistral.mistral-7b-instruct-v0:2', name: 'Mistral 7B Instruct', aliases: [], card: 'mistral-ai-mistral-7b-instruct' },
  'Mistral|Mistral Large 3': { model: 'mistral.mistral-large-3-675b-instruct', name: 'Mistral Large 3', aliases: [], card: 'mistral-ai-mistral-large-3' },
  'Mistral|Mistral Large': { model: 'mistral.mistral-large-2402-v1:0', name: 'Mistral Large', aliases: [], card: 'mistral-ai-mistral-large' },
  'Mistral|Mistral Small': { model: 'mistral.mistral-small-2402-v1:0', name: 'Mistral Small', aliases: [], card: 'mistral-ai-mistral-small' },
  'Mistral|Mixtral 8x7B': { model: 'mistral.mixtral-8x7b-instruct-v0:1', name: 'Mixtral 8x7B Instruct', aliases: [], card: 'mistral-ai-mixtral-8x7b-instruct' },
  'Mistral|Pixtral Large 25.02': { model: 'mistral.pixtral-large-2502-v1:0', name: 'Pixtral Large', aliases: ["us.mistral.pixtral-large-2502-v1:0"], card: 'mistral-ai-pixtral-large' },
  'Mistral|Voxtral Mini 1.0': { model: 'mistral.voxtral-mini-3b-2507', name: 'Voxtral Mini 3B 2507', aliases: [], card: 'mistral-ai-voxtral-mini-3b-2507' },
  'Mistral|Voxtral Small 1.0': { model: 'mistral.voxtral-small-24b-2507', name: 'Voxtral Small 24B 2507', aliases: [], card: 'mistral-ai-voxtral-small-24b-2507' },
  'Nvidia|NVIDIA Nemotron 3 Super 120B A12B': { model: 'nvidia.nemotron-super-3-120b', name: 'NVIDIA Nemotron 3 Super 120B', aliases: [], card: 'nvidia-nemotron-super-3-120b' },
  'Nvidia|NVIDIA Nemotron Nano 2 VL': { model: 'nvidia.nemotron-nano-12b-v2', name: 'NVIDIA Nemotron Nano 12B v2 VL BF16', aliases: [], card: 'nvidia-nvidia-nemotron-nano-12b-v2-vl-bf16' },
  'Nvidia|NVIDIA Nemotron Nano 2': { model: 'nvidia.nemotron-nano-9b-v2', name: 'NVIDIA Nemotron Nano 9B v2', aliases: [], card: 'nvidia-nvidia-nemotron-nano-9b-v2' },
  'Nvidia|Nemotron Nano 3 30B': { model: 'nvidia.nemotron-nano-3-30b', name: 'Nemotron Nano 3 30B', aliases: [], card: 'nvidia-nemotron-nano-3-30b' },
  'OpenAI|GPT OSS Safeguard 120B': { model: 'openai.gpt-oss-safeguard-120b', name: 'GPT OSS Safeguard 120B', aliases: [], card: 'openai-gpt-oss-safeguard-120b' },
  'OpenAI|GPT OSS Safeguard 20B': { model: 'openai.gpt-oss-safeguard-20b', name: 'GPT OSS Safeguard 20B', aliases: [], card: 'openai-gpt-oss-safeguard-20b' },
  'OpenAI|gpt-oss-120b': { model: 'openai.gpt-oss-120b-1:0', name: 'gpt-oss-120b', aliases: ["openai.gpt-oss-120b"], card: 'openai-gpt-oss-120b' },
  'OpenAI|gpt-oss-20b': { model: 'openai.gpt-oss-20b-1:0', name: 'gpt-oss-20b', aliases: ["openai.gpt-oss-20b"], card: 'openai-gpt-oss-20b' },
  'Qwen|Qwen3 235B A22B 2507': { model: 'qwen.qwen3-235b-a22b-2507-v1:0', name: 'Qwen3 235B A22B 2507', aliases: ["qwen.qwen3-235b-a22b-2507"], card: 'qwen-qwen3-235b-a22b-2507' },
  'Qwen|Qwen3 32B': { model: 'qwen.qwen3-32b-v1:0', name: 'Qwen3 32B', aliases: ["qwen.qwen3-32b"], card: 'qwen-qwen3-32b' },
  'Qwen|Qwen3 Coder 30B A3B': { model: 'qwen.qwen3-coder-30b-a3b-v1:0', name: 'Qwen3-Coder-30B-A3B-Instruct', aliases: ["qwen.qwen3-coder-30b-a3b-instruct"], card: 'qwen-qwen3-coder-30b-a3b-instruct' },
  'Qwen|Qwen3 Coder 480B A35B': { model: 'qwen.qwen3-coder-480b-a35b-v1:0', name: 'Qwen3 Coder 480B A35B Instruct', aliases: ["qwen.qwen3-coder-480b-a35b-instruct"], card: 'qwen-qwen3-coder-480b-a35b-instruct' },
  'Qwen|Qwen3 Coder Next': { model: 'qwen.qwen3-coder-next', name: 'Qwen3 Coder Next', aliases: [], card: 'qwen-qwen3-coder-next' },
  'Qwen|Qwen3 Next 80B A3B': { model: 'qwen.qwen3-next-80b-a3b', name: 'Qwen3 Next 80B A3B', aliases: ["qwen.qwen3-next-80b-a3b-instruct"], card: 'qwen-qwen3-next-80b-a3b' },
  'Qwen|Qwen3 VL 235B A22B': { model: 'qwen.qwen3-vl-235b-a22b', name: 'Qwen3 VL 235B A22B', aliases: ["qwen.qwen3-vl-235b-a22b-instruct"], card: 'qwen-qwen3-vl-235b-a22b' },
  'Writer|Writer Palmyra Vision 7B': { model: 'writer.palmyra-vision-7b', name: 'Palmyra Vision 7B', aliases: [], card: 'writer-palmyra-vision-7b' },
  'Z AI|GLM 4.7 Flash': { model: 'zai.glm-4.7-flash', name: 'GLM 4.7 Flash', aliases: [], card: 'zai-glm-4-7-flash' },
  'Z AI|GLM 4.7': { model: 'zai.glm-4.7', name: 'GLM 4.7', aliases: [], card: 'zai-glm-4-7' },
  'Z AI|GLM 5': { model: 'zai.glm-5', name: 'GLM 5', aliases: [], card: 'zai-glm-5' },
  '|xai.grok-4.6': { model: 'xai.grok-4.6', name: 'Grok 4.6', aliases: [], card: 'xai-grok-4-6' },
  'xAI|Grok 4.7': { model: 'xai.grok-4.7', name: 'Grok 4.7', aliases: ["us.xai.grok-4.7"], card: 'xai-grok-4-7' },
  '|Nova 2.0 Lite': { model: 'amazon.nova-2-lite-v1:0', name: 'Nova 2 Lite', aliases: ["us.amazon.nova-2-lite-v1:0"], card: 'amazon-nova-2-lite' },
  '|Nova Lite': { model: 'amazon.nova-lite-v1:0', name: 'Nova Lite', aliases: ["us.amazon.nova-lite-v1:0"], card: 'amazon-nova-lite' },
  '|Nova Micro': { model: 'amazon.nova-micro-v1:0', name: 'Nova Micro', aliases: ["us.amazon.nova-micro-v1:0"], card: 'amazon-nova-micro' },
  '|Nova Pro': { model: 'amazon.nova-pro-v1:0', name: 'Nova Pro', aliases: ["us.amazon.nova-pro-v1:0"], card: 'amazon-nova-pro' },

};

// Price-list names with token prices but no public API id on a model card (previews and
// speech-to-speech models priced per speech token), or a model that is only sold through the
// AWS Marketplace page and has no Bedrock token price here. Skipped on purpose.
export const BEDROCK_SKIP = [
  'Anthropic|Claude 2.0',
  'Anthropic|Claude 2.1',
  'Anthropic|Claude 3 Haiku',
  'Anthropic|Claude 3 Sonnet',
  'Anthropic|Claude Instant',
  'Meta|Llama 3.1 70B Latency Optimized',
  '|Nova 2.0 Omni',
  '|Nova 2.0 Pro',
  '|Nova 2.5 Sonic',
  '|Nova Premier',
  '|Nova Pro Latency Optimized',
  '|Nova Sonic',
  '|Nova Sonic 2.0',
  '|',
];
