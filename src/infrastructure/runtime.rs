//! Adapters for the non-CV ports: cache + concurrency.

use std::sync::Arc;

use moka::sync::Cache;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use crate::application::ports::{
    CacheKey, CachedResult, ConcurrencyLimiter, MaskCache, MaskKey, Permit, ResultCache,
};
use crate::domain::error::DomainError;
use crate::domain::image::ImageBuffer;

// ─── Mask cache ──────────────────────────────────────────────────────────────

/// Caches pre-rendered round-corner alpha masks (single-channel CV_8U Mat,
/// reference-counted internally by OpenCV).
pub struct MokaMaskCache {
    inner: Cache<MaskKey, Arc<opencv::core::Mat>>,
}

impl MokaMaskCache {
    pub fn new(capacity: u64) -> Self {
        Self { inner: Cache::new(capacity) }
    }
}

impl MaskCache for MokaMaskCache {
    fn get_or_compute(
        &self,
        key: MaskKey,
        compute: &mut dyn FnMut() -> Result<ImageBuffer, DomainError>,
    ) -> Result<ImageBuffer, DomainError> {
        if let Some(hit) = self.inner.get(&key) {
            // Cheap clone of the Arc — OpenCV's Mat refcount is *not* touched
            // because we share the Arc itself.
            return Ok(ImageBuffer::new(hit));
        }
        let buf = compute()?;
        // Move the freshly computed Mat behind an Arc and stash it.
        let mat = *buf
            .downcast::<opencv::core::Mat>()
            .map_err(|b| DomainError::Internal(format!(
                "mask compute returned {} not Mat", b.type_name()
            )))?;
        let arc = Arc::new(mat);
        self.inner.insert(key, arc.clone());
        Ok(ImageBuffer::new(arc))
    }
}

// ─── Concurrency limiter ─────────────────────────────────────────────────────

/// Bounded fast-fail semaphore. `try_acquire` never awaits; over-capacity
/// requests are rejected immediately so callers don't queue in user space.
pub struct TokioConcurrencyLimiter {
    sem: Arc<Semaphore>,
}

impl TokioConcurrencyLimiter {
    pub fn new(max: usize) -> Self {
        Self { sem: Arc::new(Semaphore::new(max)) }
    }

    pub fn available(&self) -> usize { self.sem.available_permits() }
}

impl ConcurrencyLimiter for TokioConcurrencyLimiter {
    fn try_acquire(&self) -> Result<Permit, DomainError> {
        match self.sem.clone().try_acquire_owned() {
            Ok(p) => Ok(Permit(Box::new(PermitGuard(p)))),
            Err(_) => Err(DomainError::Overloaded),
        }
    }
}

struct PermitGuard(#[allow(dead_code)] OwnedSemaphorePermit);

// ─── Result cache (GET endpoint) ─────────────────────────────────────────────

pub struct MokaResultCache {
    inner: Cache<CacheKey, CachedResult>,
}

impl MokaResultCache {
    pub fn new(capacity: u64) -> Self {
        Self { inner: Cache::new(capacity) }
    }
}

impl ResultCache for MokaResultCache {
    fn get(&self, key: &CacheKey) -> Option<CachedResult> {
        self.inner.get(key)
    }
    fn put(&self, key: CacheKey, value: CachedResult) {
        self.inner.insert(key, value);
    }
}
