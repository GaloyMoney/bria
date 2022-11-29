use opentelemetry::{propagation::TextMapPropagator, sdk::propagation::TraceContextPropagator};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tracing::Span;
use tracing_opentelemetry::OpenTelemetrySpanExt;
use tracing_subscriber::{filter::EnvFilter, fmt, layer::SubscriberExt, util::SubscriberInitExt};

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
            port: 6831,
            service_name: "bria-dev".to_string(),
        }
    }
}

pub fn init_tracer(config: TracingConfig) -> anyhow::Result<()> {
    let tracing_endpoint = format!("{}:{}", config.host, config.port);
    println!("Sending traces to {}", tracing_endpoint);
    let tracer = opentelemetry_jaeger::new_agent_pipeline()
        .with_endpoint(tracing_endpoint)
        .with_service_name(config.service_name)
        .install_simple()?;
    let telemetry = tracing_opentelemetry::layer().with_tracer(tracer);

    let fmt_layer = fmt::layer().json();
    let filter_layer = EnvFilter::try_from_default_env()
        .or_else(|_| EnvFilter::try_new("info"))
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
    Span::current().record("error", &tracing::field::display("true"));
    Span::current().record("error.level", &tracing::field::display(level));
    Span::current().record("error.message", &tracing::field::display(error));
}
