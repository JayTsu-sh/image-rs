//! `ProcessImageService` — the single use case behind every endpoint.

use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;

use crate::application::ports::{
    ConcurrencyLimiter, EncodedImage, ImageCodec, OpContext, OpExecutor,
};
use crate::domain::error::DomainError;
use crate::domain::pipeline::Pipeline;

/// Inbound command. Built by the HTTP layer from a multipart upload.
pub struct ProcessImageCommand {
    pub image: Bytes,
    pub pipeline: Pipeline,
    pub context: OpContext,
}

/// Result + observability metadata (wall-clock time spent on the blocking
/// pool). The HTTP layer turns these into response headers so the UI can
/// display them in the status bar.
pub struct ProcessOutcome {
    pub encoded: EncodedImage,
    pub elapsed_ms: u64,
}

pub struct ProcessImageService {
    codec: Arc<dyn ImageCodec>,
    executor: Arc<dyn OpExecutor>,
    limiter: Arc<dyn ConcurrencyLimiter>,
}

impl ProcessImageService {
    pub fn new(
        codec: Arc<dyn ImageCodec>,
        executor: Arc<dyn OpExecutor>,
        limiter: Arc<dyn ConcurrencyLimiter>,
    ) -> Self {
        Self { codec, executor, limiter }
    }

    /// Execute on the blocking pool. The async boundary owns the permit and
    /// the heavy work; the reactor stays free.
    pub async fn execute(
        &self,
        cmd: ProcessImageCommand,
    ) -> Result<ProcessOutcome, DomainError> {
        // Fast-fail backpressure — never await on the permit, that would just
        // queue requests in user space and burn memory.
        let permit = self.limiter.try_acquire()?;

        let in_bytes = cmd.image.len() as u64;
        metrics::counter!("image_bytes_in_total").increment(in_bytes);

        let codec = self.codec.clone();
        let executor = self.executor.clone();

        let started = Instant::now();
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit; // released on drop, even on panic
            run_blocking(codec.as_ref(), executor.as_ref(), cmd)
        })
        .await
        .map_err(|e| DomainError::Internal(format!("join error: {e}")))?;
        let elapsed_ms = started.elapsed().as_millis() as u64;

        match &result {
            Ok(enc) => {
                metrics::counter!("image_bytes_out_total")
                    .increment(enc.bytes.len() as u64);
                metrics::counter!("image_jobs_total", "status" => "ok").increment(1);
            }
            Err(_) => {
                metrics::counter!("image_jobs_total", "status" => "err").increment(1);
            }
        }
        result.map(|encoded| ProcessOutcome { encoded, elapsed_ms })
    }
}

fn run_blocking(
    codec: &dyn ImageCodec,
    executor: &dyn OpExecutor,
    cmd: ProcessImageCommand,
) -> Result<EncodedImage, DomainError> {
    let ProcessImageCommand { image, pipeline, context } = cmd;

    let total_start = Instant::now();

    // Decode once. The Bytes handle is moved in — its refcount is decremented
    // when this scope ends; no copy of the source bytes was made along the
    // path from hyper -> here.
    let decode_start = Instant::now();
    let mut buf = codec.decode(image)?;
    metrics::histogram!("image_decode_duration_seconds")
        .record(decode_start.elapsed().as_secs_f64());

    for op in pipeline.ops() {
        let op_start = Instant::now();
        let kind = op.kind().as_str();
        buf = executor.execute(buf, op, &context).map_err(|e| match e {
            DomainError::OpFailed { .. } => e,
            other => DomainError::OpFailed {
                op: op.kind().as_str(),
                message: other.to_string(),
            },
        })?;
        metrics::histogram!("image_op_duration_seconds", "op" => kind)
            .record(op_start.elapsed().as_secs_f64());
    }

    let encode_start = Instant::now();
    let encoded = codec.encode(buf, pipeline.output())?;
    metrics::histogram!("image_encode_duration_seconds")
        .record(encode_start.elapsed().as_secs_f64());
    metrics::histogram!("image_pipeline_duration_seconds")
        .record(total_start.elapsed().as_secs_f64());

    Ok(encoded)
}
