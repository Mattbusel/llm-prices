//! Calls the golden-corpus mock (generate.py serve) through rig-core's OpenAI Chat Completions and
//! Anthropic Messages models, with rig's tracing spans exported through tracing-opentelemetry,
//! and prints them as OTLP JSON.
//!
//!   OPENAI_API_KEY=test OPENAI_BASE_URL=http://127.0.0.1:8765/v1 \
//!   ANTHROPIC_API_KEY=test ANTHROPIC_BASE_URL=http://127.0.0.1:8765 \
//!   cargo run --release > ../rig.otlp.json

use opentelemetry::trace::TracerProvider as _;
use opentelemetry_proto::tonic::collector::trace::v1::ExportTraceServiceRequest;
use opentelemetry_proto::transform::common::tonic::ResourceAttributesWithSchema;
use opentelemetry_proto::transform::trace::tonic::group_spans_by_resource_and_scope;
use opentelemetry_sdk::trace::{InMemorySpanExporter, SdkTracerProvider};
use rig_core::providers::anthropic::Anthropic;
use rig_core::providers::openai::OpenAI;
use rig_core::completion::CompletionRequest;
use tracing_subscriber::prelude::*;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let exporter = InMemorySpanExporter::default();
    let provider = SdkTracerProvider::builder()
        .with_simple_exporter(exporter.clone())
        .build();
    let subscriber = tracing_subscriber::registry()
        .with(tracing_opentelemetry::layer().with_tracer(provider.tracer("rig-golden-spans")));
    tracing::subscriber::set_global_default(subscriber)?;

    let openai = OpenAI::from_env()?;
    for model in ["gpt-4o-mini", "o3"] {
        openai.chat(model).call(CompletionRequest::new("hi")).await?;
    }
    let anthropic = Anthropic::from_env()?;
    anthropic
        .completion("claude-sonnet-4-6")
        .call(CompletionRequest::new("hi").max_tokens(16))
        .await?;

    provider.force_flush()?;
    let spans = exporter.get_finished_spans()?;
    let resource_spans =
        group_spans_by_resource_and_scope(spans, &ResourceAttributesWithSchema::default());
    let request = ExportTraceServiceRequest { resource_spans };
    println!("{}", serde_json::to_string_pretty(&request)?);
    Ok(())
}
