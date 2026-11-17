package genaicostprocessor

import (
	"path/filepath"
	"testing"
	"time"

	"github.com/stretchr/testify/require"
	"go.opentelemetry.io/collector/component"
	"go.opentelemetry.io/collector/confmap/confmaptest"

	"github.com/Mattbusel/llm-prices/otel/genaicostprocessor/internal/metadata"
)

func TestLoadConfig(t *testing.T) {
	cm, err := confmaptest.LoadConf(filepath.Join("testdata", "config.yaml"))
	require.NoError(t, err)

	load := func(name string) *Config {
		cfg := createDefaultConfig()
		sub, err := cm.Sub(component.NewIDWithName(metadata.Type, name).String())
		require.NoError(t, err)
		require.NoError(t, sub.Unmarshal(cfg))
		return cfg
	}

	def := load("")
	require.NoError(t, def.Validate())
	require.Equal(t, createDefaultConfig(), def)
	require.Equal(t, "", def.tableURL(), "the embedded table only")
	require.Equal(t, AttributesLLMPrices, def.Attributes, "semconv names are opt-in for a collector")

	rel := load("release")
	require.NoError(t, rel.Validate())
	require.Equal(t, DefaultLLMPricesURL, rel.tableURL())
	require.Equal(t, 6*time.Hour, rel.ReloadInterval)
	require.Equal(t, UnpricedErrorAttribute, rel.OnUnpriced)
	require.Equal(t, AttributesSemconv, rel.Attributes)
	require.True(t, rel.Breakdown && rel.OpenInference)
	require.Len(t, rel.publicKey(), 32)

	lite := load("litellm")
	require.NoError(t, lite.Validate())
	require.Equal(t, SourceLiteLLM, lite.Source)
	require.Nil(t, lite.publicKey(), "LiteLLM tables are not signed")
	require.Equal(t, []string{"gen_ai.usage.total_cost"}, lite.ReportedCostAttributes)
	require.Equal(t, time.Duration(0), lite.MaxSpanAge)
	require.Equal(t, DefaultLiteLLMURL, (&Config{Source: SourceLiteLLM}).tableURL())

	for _, name := range []string{"bad_source", "bad_unpriced", "reload_without_url", "fast_reload", "bad_key", "bad_attributes"} {
		require.Error(t, load(name).Validate(), name)
	}
}
