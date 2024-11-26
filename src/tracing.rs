use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{propagation::TextMapPropagator, KeyValue};
use opentelemetry_otlp::WithExportConfig;
use opentelemetry_sdk::trace::{Config, Sampler, TracerProvider};
use opentelemetry_sdk::{propagation::TraceContextPropagator, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{filter::EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

pub trait ToTraceLevel {
    fn to_trace_level(&self) -> tracing::Level;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TracingConfig {
    host: String,
    port: u16,
    service_name: String,
}

impl Default for TracingConfig {
    fn default() -> Self {
        Self {
            host: "localhost".to_string(),
            port: 4317,
            service_name: "bria-dev".to_string(),
        }
    }
}

pub fn init_tracer(config: TracingConfig) -> anyhow::Result<()> {
    let tracing_endpoint = format!("http://{}:{}", config.host, config.port);
    let service_name = config.service_name;
    println!("Sending traces to {tracing_endpoint}");

    let exporter = opentelemetry_otlp::SpanExporter::builder()
        .with_tonic()
        .with_endpoint(tracing_endpoint)
        .build()?;

    let provider_config = Config::default()
        .with_sampler(Sampler::AlwaysOn)
        .with_resource(Resource::new(vec![KeyValue::new(
            "service.name",
            service_name.clone(),
        )]));

    let provider = TracerProvider::builder()
        .with_batch_exporter(exporter, opentelemetry_sdk::runtime::Tokio)
        .with_config(provider_config)
        .build();
    let tracer = provider.tracer(service_name);

    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let fmt_layer = fmt::layer().json();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info,sqlx=warn,sqlx_ledger=info"))
        .unwrap();
    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .with(telemetry)
        .try_init()?;

    Ok(())
}

pub fn extract_tracing_data() -> HashMap<String, String> {
    let mut tracing_data = HashMap::new();
    let propagator = TraceContextPropagator::new();
    let context = Span::current().context();
    propagator.inject_context(&context, &mut tracing_data);
    tracing_data
}

pub fn inject_tracing_data(span: &Span, tracing_data: &HashMap<String, String>) {
    let propagator = TraceContextPropagator::new();
    let context = propagator.extract(tracing_data);
    span.set_parent(context);
}

pub fn insert_error_fields(level: tracing::Level, error: impl std::fmt::Display) {
    Span::current().record("error", tracing::field::display("true"));
    Span::current().record("error.level", tracing::field::display(level));
    Span::current().record("error.message", tracing::field::display(error));
}

pub async fn record_error<
    T,
    E: std::fmt::Display + ToTraceLevel,
    F: FnOnce() -> R,
    R: std::future::Future<Output = Result<T, E>>,
>(
    func: F,
) -> Result<T, E> {
    let result = func().await;
    if let Err(ref e) = result {
        insert_error_fields(e.to_trace_level(), e);
    }
    result
}
