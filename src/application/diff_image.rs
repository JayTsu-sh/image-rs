//! `DiffImageService` — use case behind `POST /v1/diff`.
//!
//! Decodes two images, runs the configured diff, encodes the result. Same
//! permit / spawn_blocking pattern as `ProcessImageService`.

use std::sync::Arc;
use std::time::Instant;

use bytes::Bytes;

use crate::application::ports::{
    ConcurrencyLimiter, ImageCodec, ImageDiffer,
};
use crate::application::process_image::ProcessOutcome;
use crate::domain::error::DomainError;
use crate::domain::pipeline::OutputSpec;
use crate::domain::value_objects::DiffSpec;

pub struct DiffImageCommand {
    pub before: Bytes,
    pub after: Bytes,
    pub spec: DiffSpec,
    pub output: OutputSpec,
}

pub struct DiffImageService {
    codec: Arc<dyn ImageCodec>,
    differ: Arc<dyn ImageDiffer>,
    limiter: Arc<dyn ConcurrencyLimiter>,
}

impl DiffImageService {
    pub fn new(
        codec: Arc<dyn ImageCodec>,
        differ: Arc<dyn ImageDiffer>,
        limiter: Arc<dyn ConcurrencyLimiter>,
    ) -> Self {
        Self { codec, differ, limiter }
    }

    pub async fn execute(
        &self,
        cmd: DiffImageCommand,
    ) -> Result<ProcessOutcome, DomainError> {
        let permit = self.limiter.try_acquire()?;

        let codec = self.codec.clone();
        let differ = self.differ.clone();

        let started = Instant::now();
        let result = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let DiffImageCommand { before, after, spec, output } = cmd;
            let before_img = codec.decode(before)?;
            let after_img = codec.decode(after)?;
            let diffed = differ.diff(before_img, after_img, &spec)?;
            codec.encode(diffed, &output)
        })
        .await
        .map_err(|e| DomainError::Internal(format!("join error: {e}")))?;
        let elapsed_ms = started.elapsed().as_millis() as u64;

        result.map(|encoded| ProcessOutcome { encoded, elapsed_ms })
    }
}
