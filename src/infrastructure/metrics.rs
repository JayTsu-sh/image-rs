//! Prometheus metrics — installs a global recorder once at startup and
//! returns a handle that the HTTP `/metrics` route can render on demand.

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};

pub fn init() -> anyhow::Result<PrometheusHandle> {
    let handle = PrometheusBuilder::new()
        .install_recorder()
        .map_err(|e| anyhow::anyhow!("install prometheus recorder: {e}"))?;

    // Pre-register metric descriptors so they show up at zero before any
    // request — easier to dashboard against, easier to alert on absence.
    metrics::describe_histogram!(
        "image_pipeline_duration_seconds",
        metrics::Unit::Seconds,
        "Full request duration: decode → ops → encode"
    );
    metrics::describe_histogram!(
        "image_decode_duration_seconds",
        metrics::Unit::Seconds,
        "Decode duration"
    );
    metrics::describe_histogram!(
        "image_encode_duration_seconds",
        metrics::Unit::Seconds,
        "Encode duration"
    );
    metrics::describe_histogram!(
        "image_op_duration_seconds",
        metrics::Unit::Seconds,
        "Per-op duration"
    );
    metrics::describe_counter!(
        "image_bytes_in_total",
        metrics::Unit::Bytes,
        "Input bytes received"
    );
    metrics::describe_counter!(
        "image_bytes_out_total",
        metrics::Unit::Bytes,
        "Output bytes returned"
    );
    metrics::describe_counter!(
        "image_jobs_total",
        "Total jobs partitioned by status"
    );

    Ok(handle)
}
