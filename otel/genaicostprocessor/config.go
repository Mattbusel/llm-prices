package genaicostprocessor // import "github.com/Mattbusel/llm-prices/otel/genaicostprocessor"

import (
	"encoding/base64"
	"errors"
	"fmt"
	"time"

	"go.opentelemetry.io/collector/confmap"
)

// Price sources.
const (
	// SourceLLMPrices is the llm-prices table (https://github.com/Mattbusel/llm-prices). The
	// table embedded in the processor is used until a newer one is loaded from PricesURL.
	SourceLLMPrices = "llm-prices"
	// SourceLiteLLM is LiteLLM's model_prices_and_context_window.json.
	SourceLiteLLM = "litellm"
)

// What the processor does with a GenAI span it cannot price.
const (
	// UnpricedFlag sets llm_prices.unpriced to the reason. No cost attribute is written.
	UnpricedFlag = "flag"
	// UnpricedErrorAttribute also sets error.type to "llm_prices.unpriced" when the span has
	// no error.type yet, so backends that count errors count unpriced calls.
	UnpricedErrorAttribute = "error_attribute"
)

// Which attribute names the cost is written under.
const (
	// AttributesLLMPrices (the default) writes llm_prices.cost.amount, .currency, .source,
	// .pricing.id and .pricing.version.
	AttributesLLMPrices = "llm_prices"
	// AttributesSemconv writes gen_ai.usage.cost.amount, .currency and .source
	// (semantic-conventions-genai#443) and .pricing.id and .pricing.version (#624). The #443
	// text reserves these for a cost computed at recording time; use it for an agent collector
	// next to the application, where spans are priced as they are exported.
	AttributesSemconv = "semconv"
)

// Values of the token-count interpretation settings.
const (
	modeAuto  = "auto"
	modeTrue  = "true"
	modeFalse = "false"
)

// DefaultLLMPricesURL is the prices.json of the latest llm-prices release. Each release also
// publishes prices.json.sig, an Ed25519 signature checked against ReleasePublicKey.
const DefaultLLMPricesURL = "https://github.com/Mattbusel/llm-prices/releases/latest/download/prices.json"

// DefaultLiteLLMURL is LiteLLM's price map on its main branch.
const DefaultLiteLLMURL = "https://raw.githubusercontent.com/BerriAI/litellm/main/model_prices_and_context_window.json"

// ReleasePublicKey is the Ed25519 public key (base64) that signs prices.json in llm-prices
// releases from 1.7.0 on.
const ReleasePublicKey = "JMWqWwM3DNtY9/KgjA/DLph5IJSsEf5Mf6QB2E0q+LM="

// Config is the processor's configuration.
type Config struct {
	// Source is the price table: "llm-prices" (default) or "litellm".
	Source string `mapstructure:"source"`

	// PricesURL is where the table is loaded from: an https:// or http:// URL, a file:// URL
	// or a local path. For llm-prices it is optional (the embedded table is used until a load
	// succeeds; "release" means DefaultLLMPricesURL). For litellm it defaults to
	// DefaultLiteLLMURL.
	PricesURL string `mapstructure:"prices_url"`

	// ReloadInterval is how often PricesURL is fetched again. 0 loads it once at start.
	ReloadInterval time.Duration `mapstructure:"reload_interval"`

	// DownloadTimeout bounds each fetch of PricesURL (and its signature).
	DownloadTimeout time.Duration `mapstructure:"download_timeout"`

	// PublicKey is the base64 Ed25519 public key that must have signed the table: the
	// processor fetches PricesURL + ".sig" and rejects a table whose signature does not
	// verify. For the llm-prices source it defaults to ReleasePublicKey.
	PublicKey string `mapstructure:"public_key"`

	// SkipSignature loads an llm-prices table without checking a signature (your own fork
	// or a local file). Ignored for litellm, whose tables are not signed.
	SkipSignature bool `mapstructure:"skip_signature"`

	// Attributes is the set of names the cost is written under: "llm_prices" (default,
	// llm_prices.cost.*) or "semconv" (gen_ai.usage.cost.*). See AttributesSemconv.
	Attributes string `mapstructure:"attributes"`

	// OnUnpriced is "flag" (default) or "error_attribute". A span is never given a zero cost
	// because a price is missing.
	OnUnpriced string `mapstructure:"on_unpriced"`

	// InputTokensIncludeCache says whether the input token count already includes cache
	// reads and cache writes: "auto" (default: yes, as the GenAI semantic conventions say,
	// unless the cache counts are larger than the input count), "true" or "false".
	InputTokensIncludeCache string `mapstructure:"input_tokens_include_cache"`

	// OutputTokensIncludeReasoning says whether the output token count already includes
	// reasoning tokens: "auto" (default: yes, as the conventions say, unless the reasoning
	// count is larger), "true" or "false".
	OutputTokensIncludeReasoning string `mapstructure:"output_tokens_include_reasoning"`

	// BatchAttribute names a boolean span attribute that marks a request sent through a
	// provider's Batch API, priced at batch rates. Empty turns batch pricing off.
	BatchAttribute string `mapstructure:"batch_attribute"`

	// ReportedCostAttributes are attribute keys that hold a cost, in USD, that was present in
	// the response the client received (for example from a gateway). When one is set on a
	// span, its value is written as the cost with gen_ai.usage.cost.source = "provider".
	ReportedCostAttributes []string `mapstructure:"reported_cost_attributes"`

	// Breakdown also writes llm_prices.cost.{input,output,cache_read,cache_write} and the
	// matched row as llm_prices.price.row.
	Breakdown bool `mapstructure:"breakdown"`

	// OpenInference also writes the OpenInference cost attributes (llm.cost.prompt,
	// llm.cost.completion, llm.cost.total and their details).
	OpenInference bool `mapstructure:"openinference"`

	// MaxSpanAge is the oldest span (by end time) the processor prices. The cost conventions
	// freeze a cost at recording time; a span that ended longer ago than this is flagged
	// stale_span instead of priced against today's table. 0 turns the check off.
	MaxSpanAge time.Duration `mapstructure:"max_span_age"`

	// prevent unkeyed literal initialization
	_ struct{}
}

var _ confmap.Validator = (*Config)(nil)

func createDefaultConfig() *Config {
	return &Config{
		Source:                       SourceLLMPrices,
		Attributes:                   AttributesLLMPrices,
		DownloadTimeout:              30 * time.Second,
		OnUnpriced:                   UnpricedFlag,
		InputTokensIncludeCache:      modeAuto,
		OutputTokensIncludeReasoning: modeAuto,
		BatchAttribute:               "llm_prices.batch",
		MaxSpanAge:                   15 * time.Minute,
	}
}

// Validate checks the configuration.
func (c *Config) Validate() error {
	var errs []error
	switch c.Source {
	case SourceLLMPrices, SourceLiteLLM:
	default:
		errs = append(errs, fmt.Errorf("source %q: want %q or %q", c.Source, SourceLLMPrices, SourceLiteLLM))
	}
	switch c.Attributes {
	case AttributesLLMPrices, AttributesSemconv:
	default:
		errs = append(errs, fmt.Errorf("attributes %q: want %q or %q", c.Attributes, AttributesLLMPrices, AttributesSemconv))
	}
	switch c.OnUnpriced {
	case UnpricedFlag, UnpricedErrorAttribute:
	default:
		errs = append(errs, fmt.Errorf("on_unpriced %q: want %q or %q", c.OnUnpriced, UnpricedFlag, UnpricedErrorAttribute))
	}
	for name, v := range map[string]string{
		"input_tokens_include_cache":      c.InputTokensIncludeCache,
		"output_tokens_include_reasoning": c.OutputTokensIncludeReasoning,
	} {
		switch v {
		case modeAuto, modeTrue, modeFalse:
		default:
			errs = append(errs, fmt.Errorf("%s %q: want auto, true or false", name, v))
		}
	}
	if c.ReloadInterval < 0 || c.MaxSpanAge < 0 || c.DownloadTimeout < 0 {
		errs = append(errs, errors.New("reload_interval, download_timeout and max_span_age must not be negative"))
	}
	if c.ReloadInterval > 0 && c.ReloadInterval < time.Minute {
		errs = append(errs, fmt.Errorf("reload_interval %s: the shortest is 1m", c.ReloadInterval))
	}
	if c.ReloadInterval > 0 && c.PricesURL == "" && c.Source == SourceLLMPrices {
		errs = append(errs, errors.New("reload_interval needs prices_url (use prices_url: release for the latest llm-prices release)"))
	}
	if c.PublicKey != "" {
		if k, err := base64.StdEncoding.DecodeString(c.PublicKey); err != nil || len(k) != 32 {
			errs = append(errs, errors.New("public_key: want a base64 Ed25519 public key (32 bytes)"))
		}
	}
	for _, k := range c.ReportedCostAttributes {
		if k == "" {
			errs = append(errs, errors.New("reported_cost_attributes: empty key"))
		}
	}
	return errors.Join(errs...)
}

// tableURL is the URL or path the table is loaded from, "" for the embedded table only.
func (c *Config) tableURL() string {
	switch {
	case c.PricesURL == "release":
		return DefaultLLMPricesURL
	case c.PricesURL == "" && c.Source == SourceLiteLLM:
		return DefaultLiteLLMURL
	}
	return c.PricesURL
}

// publicKey is the key a loaded table must be signed with, nil when none is checked.
func (c *Config) publicKey() []byte {
	key := c.PublicKey
	if key == "" && c.Source == SourceLLMPrices && !c.SkipSignature {
		key = ReleasePublicKey
	}
	if key == "" || (c.Source == SourceLLMPrices && c.SkipSignature) {
		return nil
	}
	k, _ := base64.StdEncoding.DecodeString(key)
	return k
}
