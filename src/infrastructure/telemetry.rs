//! OpenTelemetry / OTLP exporter wiring.
//!
//! Installs a global tracer provider that ships spans over OTLP/HTTP to the
//! collector at `OTEL_EXPORTER_OTLP_ENDPOINT`. If the env var is not set
//! the function returns `None` and the rest of the system uses
//! `tracing-subscriber::fmt` only.
//!
//! Standard OTel env vars are honored:
//! * `OTEL_EXPORTER_OTLP_ENDPOINT`  — collector base URL (e.g. `http://otel:4318`)
//! * `OTEL_SERVICE_NAME`            — service.name resource attribute
//! * `OTEL_RESOURCE_ATTRIBUTES`     — additional resource attributes
//!
//! Service version defaults to the crate version compiled in.

use std::time::Duration;

use opentelemetry::trace::TracerProvider as _;
use opentelemetry::{KeyValue, global};
use opentelemetry_otlp::{Protocol, SpanExporter, WithExportConfig};
use opentelemetry_sdk::Resource;
use opentelemetry_sdk::runtime;
use opentelemetry_sdk::trace::TracerProvider as SdkTracerProvider;

const TRACER_NAME: &str = "image-rs";

/// Initialize OTLP if configured. Returns the tracer that should be wrapped
/// in a `tracing_opentelemetry::layer()` and added to the subscriber. If
/// OTLP is not configured this returns `None`.
pub fn try_install_otlp() -> anyhow::Result<Option<opentelemetry_sdk::trace::Tracer>> {
    let endpoint = match std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT") {
        Ok(s) if !s.is_empty() => s,
        _ => return Ok(None),
    };

    let exporter = SpanExporter::builder()
        .with_http()
        .with_endpoint(endpoint.clone())
        .with_protocol(Protocol::HttpBinary)
        .with_timeout(Duration::from_secs(5))
        .build()
        .map_err(|e| anyhow::anyhow!("build OTLP exporter: {e}"))?;

    let service_name = std::env::var("OTEL_SERVICE_NAME")
        .unwrap_or_else(|_| TRACER_NAME.to_string());

    let resource = Resource::new(vec![
        KeyValue::new("service.name", service_name),
        KeyValue::new("service.version", env!("CARGO_PKG_VERSION")),
    ]);

    let provider = SdkTracerProvider::builder()
        .with_batch_exporter(exporter, runtime::Tokio)
        .with_resource(resource)
        .build();

    let tracer = provider.tracer(TRACER_NAME);
    global::set_tracer_provider(provider);

    tracing::info!(endpoint = %endpoint, "OTLP exporter installed");
    Ok(Some(tracer))
}

/// Best-effort flush + shutdown of the global tracer provider. Call this
/// from the graceful-shutdown path so the last batch of spans is exported
/// before the process exits.
pub fn shutdown() {
    global::shutdown_tracer_provider();
}
