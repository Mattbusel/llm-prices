//! Providers and provider-aware lookup.

use core::fmt;
use core::str::FromStr;

use crate::{all, resolve, Price};

/// A company that sells model access: a model maker's own API, a cloud platform that resells
/// it, or a host for open-weight models.
///
/// The same model id can cost different amounts depending on who serves it, so
/// [`Provider::lookup`] only returns rows that provider sells (with the platform fallback
/// described there). [`Provider::parse`] accepts the names SDKs and routers use, such as
/// `gemini`, `vertex_ai`, `together_ai` or `x-ai`.
///
/// ```
/// use llm_prices::Provider;
///
/// assert_eq!(Provider::parse("vertex_ai"), Some(Provider::Vertex));
/// assert_eq!("Together_AI".parse::<Provider>(), Ok(Provider::Together));
/// assert_eq!(Provider::Xai.as_str(), "xai");
/// assert!(Provider::Groq.rows().all(|p| p.provider == "groq"));
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Provider {
    /// Anthropic's API (`anthropic`; also parses `claude`).
    Anthropic,
    /// OpenAI's API (`openai`).
    OpenAi,
    /// The Gemini API on Google AI Studio (`google`; also parses `gemini`).
    Google,
    /// Google Cloud Vertex AI (`vertex`; also parses `vertex_ai`, `google-vertex`).
    Vertex,
    /// Amazon Bedrock (`bedrock`; also parses `aws`, `amazon-bedrock`).
    Bedrock,
    /// Azure OpenAI and Azure AI Foundry (`azure`; also parses `azure_openai`, `azure_ai`).
    Azure,
    /// DeepSeek's API (`deepseek`).
    DeepSeek,
    /// Mistral AI's API (`mistral`; also parses `mistralai`).
    Mistral,
    /// GroqCloud (`groq`).
    Groq,
    /// Together AI (`together`; also parses `together_ai`, `togetherai`).
    Together,
    /// Fireworks AI (`fireworks`; also parses `fireworks_ai`).
    Fireworks,
    /// xAI's API (`xai`; also parses `x-ai`, `grok`).
    Xai,
    /// Cohere's API (`cohere`).
    Cohere,
    /// OpenRouter (`openrouter`).
    OpenRouter,
}

impl Provider {
    /// Every provider, in declaration order.
    pub const ALL: &'static [Provider] = &[
        Provider::Anthropic,
        Provider::OpenAi,
        Provider::Google,
        Provider::Vertex,
        Provider::Bedrock,
        Provider::Azure,
        Provider::DeepSeek,
        Provider::Mistral,
        Provider::Groq,
        Provider::Together,
        Provider::Fireworks,
        Provider::Xai,
        Provider::Cohere,
        Provider::OpenRouter,
    ];

    /// The canonical lowercase name. For providers with rows in the table it is the value of
    /// [`Price::provider`] on those rows.
    pub const fn as_str(self) -> &'static str {
        match self {
            Provider::Anthropic => "anthropic",
            Provider::OpenAi => "openai",
            Provider::Google => "google",
            Provider::Vertex => "vertex",
            Provider::Bedrock => "bedrock",
            Provider::Azure => "azure",
            Provider::DeepSeek => "deepseek",
            Provider::Mistral => "mistral",
            Provider::Groq => "groq",
            Provider::Together => "together",
            Provider::Fireworks => "fireworks",
            Provider::Xai => "xai",
            Provider::Cohere => "cohere",
            Provider::OpenRouter => "openrouter",
        }
    }

    /// Parses a provider name, ignoring ASCII case and treating `_`, `-` and spaces alike.
    /// Accepts the canonical names and the spellings LiteLLM, LangChain, OpenRouter and the
    /// provider SDKs use (`gemini`, `google_genai`, `vertex_ai`, `google_vertexai`,
    /// `bedrock_converse`, `azure_openai`, `together_ai`, `x-ai`, ...). `None` for anything else.
    pub fn parse(name: &str) -> Option<Provider> {
        let mut buf = [0u8; 24];
        let name = name.trim();
        if name.len() > buf.len() {
            return None;
        }
        for (b, c) in buf.iter_mut().zip(name.bytes()) {
            *b = match c.to_ascii_lowercase() {
                b'_' | b' ' => b'-',
                c => c,
            };
        }
        let key = core::str::from_utf8(&buf[..name.len()]).ok()?;
        Some(match key {
            "anthropic" | "claude" => Provider::Anthropic,
            "openai" | "open-ai" => Provider::OpenAi,
            "google" | "gemini" | "google-ai-studio" | "google-gemini" | "ai-studio" | "google-genai" | "google-generativeai" | "genai"
            | "google-ai" | "googleai" => Provider::Google,
            "vertex" | "vertex-ai" | "vertexai" | "google-vertex" | "google-vertex-ai" | "vertex-ai-beta" | "google-vertexai" => {
                Provider::Vertex
            }
            "bedrock" | "aws" | "aws-bedrock" | "amazon-bedrock" | "bedrock-converse" | "bedrock-runtime" | "amazon" => Provider::Bedrock,
            "azure" | "azure-openai" | "azure-ai" | "azure-foundry" | "microsoft-foundry" | "azure-ai-foundry" => Provider::Azure,
            "deepseek" => Provider::DeepSeek,
            "mistral" | "mistralai" | "mistral-ai" => Provider::Mistral,
            "groq" | "groqcloud" | "groq-cloud" => Provider::Groq,
            "together" | "together-ai" | "togetherai" => Provider::Together,
            "fireworks" | "fireworks-ai" | "fireworksai" => Provider::Fireworks,
            "xai" | "x-ai" | "grok" => Provider::Xai,
            "cohere" | "cohere-chat" | "cohere-ai" => Provider::Cohere,
            "openrouter" | "open-router" => Provider::OpenRouter,
            _ => return None,
        })
    }

    /// The rows this provider sells, in table order. Empty for Vertex, Bedrock, Azure and
    /// OpenRouter, which have no rows of their own yet (see [`Provider::lookup`]).
    pub fn rows(self) -> impl Iterator<Item = &'static Price> {
        let name = self.as_str();
        all().iter().filter(move |p| p.provider == name)
    }

    /// The model makers whose own list prices [`Provider::lookup`] falls back to when this
    /// provider has no row for a model. These are platforms that charge the maker's list
    /// price on their global endpoints:
    ///
    /// - Vertex AI: Google (Gemini) and Anthropic (Claude).
    /// - Bedrock: Anthropic (Claude).
    /// - Azure: OpenAI.
    /// - OpenRouter: every model maker in the table (Anthropic, OpenAI, Google, DeepSeek,
    ///   Mistral, xAI, Cohere), since it bills the maker's price.
    ///
    /// Empty for every other provider. Regional, data-zone and provisioned endpoints on these
    /// platforms can cost more (often 10%); check the platform's pricing page for those.
    pub const fn fallback_providers(self) -> &'static [Provider] {
        match self {
            Provider::Vertex => &[Provider::Google, Provider::Anthropic],
            Provider::Bedrock => &[Provider::Anthropic],
            Provider::Azure => &[Provider::OpenAi],
            Provider::OpenRouter => &[
                Provider::Anthropic,
                Provider::OpenAi,
                Provider::Google,
                Provider::DeepSeek,
                Provider::Mistral,
                Provider::Xai,
                Provider::Cohere,
            ],
            _ => &[],
        }
    }

    /// Finds `model` among the rows this provider sells, with the same id handling as
    /// [`lookup`](crate::lookup) (aliases, any ASCII case, provider prefixes, Bedrock and
    /// Vertex forms, dated snapshots and `-latest`).
    ///
    /// When the provider has no row for the model, the rows of its
    /// [`fallback_providers`](Self::fallback_providers) are tried, so Claude on Bedrock gets
    /// Anthropic's row; the returned row's [`Price::provider`] says whose price it is. `None`
    /// when neither has the model: the table does not know what this provider charges for it,
    /// and pricing it with another provider's row would be a guess.
    ///
    /// ```
    /// use llm_prices::Provider;
    ///
    /// let p = Provider::Together.lookup("moonshotai/Kimi-K3").unwrap();
    /// assert_eq!((p.provider, p.input), ("together", Some(3.0)));
    /// // Same model on Fireworks, under Fireworks' id.
    /// assert!(Provider::Fireworks.lookup("accounts/fireworks/models/kimi-k3").is_some());
    /// // Claude on Bedrock is priced from Anthropic's row.
    /// let claude = Provider::Bedrock.lookup("global.anthropic.claude-opus-4-5-20251101-v1:0").unwrap();
    /// assert_eq!(claude.provider, "anthropic");
    /// // Groq does not serve Claude.
    /// assert!(Provider::Groq.lookup("claude-opus-4-5").is_none());
    /// ```
    pub fn lookup(self, model: &str) -> Option<&'static Price> {
        let name = self.as_str();
        resolve(model, &|p| p.provider == name).or_else(|| {
            let makers = self.fallback_providers();
            if makers.is_empty() {
                return None;
            }
            resolve(model, &|p| makers.iter().any(|m| m.as_str() == p.provider))
        })
    }
}

/// Platforms whose rows (none yet) only apply when the caller names the platform.
pub(crate) fn is_platform(provider: &str) -> bool {
    matches!(provider, "vertex" | "bedrock" | "azure" | "openrouter")
}

/// Finds `model` as sold by `provider`: [`Provider::lookup`] for a [`Provider`] or a
/// provider name such as `"groq"`, `"together_ai"` or `"vertex_ai"`. `None` when the name is
/// not a known provider or the table has no price for the model there.
///
/// ```
/// use llm_prices::{lookup_provider, Provider};
///
/// let mistral = lookup_provider("mistral", "mistral-large-latest").unwrap();
/// assert_eq!(mistral.input, Some(0.7));
/// let same = lookup_provider(Provider::Mistral, "mistral/mistral-large-4").unwrap();
/// assert!(core::ptr::eq(mistral, same));
/// assert!(lookup_provider("not-a-provider", "gpt-4o").is_none());
/// ```
pub fn lookup_provider<P: AsRef<str>>(provider: P, model: &str) -> Option<&'static Price> {
    Provider::parse(provider.as_ref())?.lookup(model)
}

impl AsRef<str> for Provider {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl fmt::Display for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The error from parsing a [`Provider`] from a name it does not know.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub struct UnknownProvider;

impl fmt::Display for UnknownProvider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("unknown LLM provider name")
    }
}

impl std::error::Error for UnknownProvider {}

impl FromStr for Provider {
    type Err = UnknownProvider;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Provider::parse(s).ok_or(UnknownProvider)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip() {
        for &p in Provider::ALL {
            assert_eq!(Provider::parse(p.as_str()), Some(p));
            assert_eq!(Provider::parse(&p.as_str().to_ascii_uppercase()), Some(p));
            assert_eq!(p.to_string(), p.as_str());
        }
    }

    #[test]
    fn every_table_provider_parses() {
        for p in all() {
            let kind = p.provider_kind().unwrap_or_else(|| panic!("{} has no Provider", p.provider));
            assert_eq!(kind.as_str(), p.provider);
            assert!(!is_platform(p.provider), "platform rows would change plain lookup order");
        }
    }

    #[test]
    fn aliases() {
        for (name, want) in [
            ("gemini", Provider::Google),
            ("vertex_ai", Provider::Vertex),
            ("Vertex AI", Provider::Vertex),
            ("aws", Provider::Bedrock),
            ("azure_openai", Provider::Azure),
            ("mistralai", Provider::Mistral),
            ("together_ai", Provider::Together),
            ("fireworks_ai", Provider::Fireworks),
            ("x-ai", Provider::Xai),
            ("open_router", Provider::OpenRouter),
            ("google_genai", Provider::Google),
            ("google-generativeai", Provider::Google),
            ("google_vertexai", Provider::Vertex),
            ("bedrock_converse", Provider::Bedrock),
            ("azure_ai_foundry", Provider::Azure),
            ("GroqCloud", Provider::Groq),
            ("cohere_ai", Provider::Cohere),
        ] {
            assert_eq!(Provider::parse(name), Some(want), "{name}");
        }
        for bad in ["", "deepseek-ai", "meta-llama", "qwen", "ollama", "huggingface", "a-very-long-provider-name-that-is-not-real"] {
            assert_eq!(Provider::parse(bad), None, "{bad}");
            assert_eq!(bad.parse::<Provider>(), Err(UnknownProvider));
        }
    }
}
