package genaicostprocessor // import "github.com/Mattbusel/llm-prices/otel/genaicostprocessor"

import (
	"context"

	"go.opentelemetry.io/collector/component"
	"go.opentelemetry.io/collector/consumer"
	"go.opentelemetry.io/collector/processor"
	"go.opentelemetry.io/collector/processor/processorhelper"

	"github.com/Mattbusel/llm-prices/otel/genaicostprocessor/internal/metadata"
)

// NewFactory returns the factory of the genaicost processor.
func NewFactory() processor.Factory {
	return processor.NewFactory(
		metadata.Type,
		func() component.Config { return createDefaultConfig() },
		processor.WithTraces(createTraces, metadata.TracesStability),
	)
}

func createTraces(ctx context.Context, set processor.Settings, cfg component.Config, next consumer.Traces) (processor.Traces, error) {
	p, err := newCostProcessor(cfg.(*Config), set.TelemetrySettings)
	if err != nil {
		return nil, err
	}
	return processorhelper.NewTraces(ctx, set, cfg, next, p.processTraces,
		processorhelper.WithCapabilities(consumer.Capabilities{MutatesData: true}),
		processorhelper.WithStart(p.start),
		processorhelper.WithShutdown(p.shutdown),
	)
}
