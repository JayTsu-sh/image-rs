//! `GetImageService` — use case behind `GET /v1/img/{key}?p=...`.
//!
//! Pipeline:
//!
//!   1. parse the DSL string (pure domain)
//!   2. compute the cache key from `(sha256(source), sha256(dsl))`
//!   3. cache lookup → return cached bytes if present (with ETag)
//!   4. otherwise: source.get → ProcessImageService.execute → cache.put → return

use std::sync::Arc;

use sha2::{Digest, Sha256};

use crate::application::ports::{
    CacheKey, CachedResult, ImageStore, OpContext, ResultCache,
};
use crate::application::process_image::{ProcessImageCommand, ProcessImageService};
use crate::domain::error::DomainError;
use crate::domain::pipeline::Pipeline;
use crate::domain::url_dsl;

pub struct GetImageService {
    store: Arc<dyn ImageStore>,
    cache: Arc<dyn ResultCache>,
    processor: Arc<ProcessImageService>,
}

pub struct GetImageCommand {
    pub key: String,
    pub dsl: String,
}

pub struct GetImageOutcome {
    pub cache_key: CacheKey,
    pub cached: bool,
    pub elapsed_ms: u64,
    pub result: CachedResult,
}

impl GetImageService {
    pub fn new(
        store: Arc<dyn ImageStore>,
        cache: Arc<dyn ResultCache>,
        processor: Arc<ProcessImageService>,
    ) -> Self {
        Self { store, cache, processor }
    }

    pub async fn execute(
        &self,
        cmd: GetImageCommand,
    ) -> Result<GetImageOutcome, DomainError> {
        // 1. fetch the source bytes (cheap I/O — could be in tokio's blocking
        //    pool if backend is slow; FS impl uses std::fs::read which is
        //    fine for local disk).
        let source = self.store.get(&cmd.key)?;

        // 2. compute the cache key — sha256(source) + sha256(normalized dsl)
        let content_hash = sha256(&source);
        let normalized_dsl = normalize_dsl(&cmd.dsl);
        let dsl_hash = sha256(normalized_dsl.as_bytes());
        let cache_key = CacheKey { content_hash, dsl_hash };

        // 3. cache lookup
        if let Some(hit) = self.cache.get(&cache_key) {
            metrics::counter!("image_get_cache_total", "result" => "hit").increment(1);
            return Ok(GetImageOutcome {
                cache_key,
                cached: true,
                elapsed_ms: 0,
                result: hit,
            });
        }
        metrics::counter!("image_get_cache_total", "result" => "miss").increment(1);

        // 4. parse + run the pipeline
        let (ops, output) = url_dsl::parse(&cmd.dsl)?;
        let pipeline = Pipeline::new(ops, output)?;
        let outcome = self
            .processor
            .execute(ProcessImageCommand {
                image: source,
                pipeline,
                context: OpContext::default(),
            })
            .await?;

        let cached = CachedResult {
            bytes: outcome.encoded.bytes,
            format: outcome.encoded.format,
            width: outcome.encoded.width,
            height: outcome.encoded.height,
        };
        self.cache.put(cache_key, cached.clone());
        Ok(GetImageOutcome {
            cache_key,
            cached: false,
            elapsed_ms: outcome.elapsed_ms,
            result: cached,
        })
    }
}

fn sha256(bytes: &[u8]) -> [u8; 32] {
    let mut h = Sha256::new();
    h.update(bytes);
    h.finalize().into()
}

/// Trim whitespace and any leading/trailing slashes so cosmetically
/// different DSL strings hash to the same key.
fn normalize_dsl(s: &str) -> String {
    s.trim().trim_matches('/').to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn etag_is_stable_for_same_inputs() {
        let k1 = CacheKey { content_hash: [1; 32], dsl_hash: [2; 32] };
        let k2 = CacheKey { content_hash: [1; 32], dsl_hash: [2; 32] };
        assert_eq!(k1.etag(), k2.etag());
        // 2 surrounding quotes + 64 bytes × 2 hex chars each = 130
        assert_eq!(k1.etag().len(), 130);
    }

    #[test]
    fn etag_changes_with_dsl() {
        let k1 = CacheKey { content_hash: [1; 32], dsl_hash: [2; 32] };
        let k2 = CacheKey { content_hash: [1; 32], dsl_hash: [3; 32] };
        assert_ne!(k1.etag(), k2.etag());
    }
}
