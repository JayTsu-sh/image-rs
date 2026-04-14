//! Ports — traits the application depends on, implemented in `infrastructure`.
//!
//! All ports are synchronous; the async boundary lives in the application
//! service which dispatches CPU work to `spawn_blocking`.

use std::collections::HashMap;
use std::sync::Arc;

use bytes::Bytes;

use crate::domain::error::DomainError;
use crate::domain::image::ImageBuffer;
use crate::domain::ops::Op;
use crate::domain::pipeline::OutputSpec;

/// Decode raw bytes into a domain `ImageBuffer`, and encode an `ImageBuffer`
/// back into the requested format. Implementations are expected to perform
/// **at most one** pixel allocation on each path.
pub trait ImageCodec: Send + Sync {
    fn decode(&self, bytes: Bytes) -> Result<ImageBuffer, DomainError>;

    /// Encode according to `output`. Returns a `Bytes` whose backing storage
    /// is the encoder's own buffer (zero-copy via `Bytes::from_owner`).
    /// If `output.format` is `None`, the codec falls back to the format the
    /// image was originally decoded from (recorded inside the buffer).
    fn encode(
        &self,
        image: ImageBuffer,
        output: &OutputSpec,
    ) -> Result<EncodedImage, DomainError>;
}

pub struct EncodedImage {
    pub bytes: Bytes,
    pub format: crate::domain::value_objects::ImageFormat,
    pub width: u32,
    pub height: u32,
}

/// Apply a single op to a buffer.
///
/// Returning a fresh `ImageBuffer` lets implementations either mutate in
/// place (and return the same handle) or replace the buffer for ops whose
/// output dimensions differ (`resize`, `rotate`, `crop`).
pub trait OpExecutor: Send + Sync {
    fn execute(
        &self,
        image: ImageBuffer,
        op: &Op,
        ctx: &OpContext,
    ) -> Result<ImageBuffer, DomainError>;
}

/// Two-image diff. Validates dimensions, runs `cv::absdiff` (or a higher-
/// level overlay), and returns a new `ImageBuffer` ready for encoding.
pub trait ImageDiffer: Send + Sync {
    fn diff(
        &self,
        before: ImageBuffer,
        after: ImageBuffer,
        spec: &crate::domain::value_objects::DiffSpec,
    ) -> Result<ImageBuffer, DomainError>;
}

/// Context passed alongside each op — primarily multipart "assets" referenced
/// by the watermark op (e.g. the actual watermark PNG bytes).
#[derive(Default)]
pub struct OpContext {
    pub assets: HashMap<String, Bytes>,
}

impl OpContext {
    pub fn new(assets: HashMap<String, Bytes>) -> Self { Self { assets } }
    pub fn asset(&self, name: &str) -> Result<&Bytes, DomainError> {
        self.assets
            .get(name)
            .ok_or_else(|| DomainError::MissingAsset(name.to_string()))
    }
}

/// Provides loaded fonts by name. Loaded once at startup and shared across
/// requests via `Arc`.
pub trait FontProvider: Send + Sync {
    fn font(&self, name: &str) -> Result<Arc<dyn FontHandle>, DomainError>;
    fn default_font(&self) -> Result<Arc<dyn FontHandle>, DomainError>;
}

/// Opaque font handle. Infrastructure downcasts to its concrete type.
pub trait FontHandle: Send + Sync + std::any::Any {
    fn as_any(&self) -> &dyn std::any::Any;
}

/// Cache of pre-rendered round-corner alpha masks. Keys are `(w, h, radius)`.
pub trait MaskCache: Send + Sync {
    fn get_or_compute(
        &self,
        key: MaskKey,
        compute: &mut dyn FnMut() -> Result<ImageBuffer, DomainError>,
    ) -> Result<ImageBuffer, DomainError>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MaskKey {
    pub width: u32,
    pub height: u32,
    pub radius: u32,
}

/// CPU concurrency limiter — used to bound how many image jobs run in
/// parallel and reject the rest fast.
pub trait ConcurrencyLimiter: Send + Sync {
    fn try_acquire(&self) -> Result<Permit, DomainError>;
}

/// RAII permit — releases the slot on drop.
pub struct Permit(pub Box<dyn Send>);

// ─── GET /v1/img ports ───────────────────────────────────────────────────────

/// Source image backend for the GET endpoint. Implementations might read
/// from local FS, S3, HTTP, or any other byte-addressable store.
pub trait ImageStore: Send + Sync {
    fn get(&self, key: &str) -> Result<Bytes, DomainError>;
}

/// Cache of *processed* output images. Keyed by `(content_hash, dsl_hash)`.
pub trait ResultCache: Send + Sync {
    fn get(&self, key: &CacheKey) -> Option<CachedResult>;
    fn put(&self, key: CacheKey, value: CachedResult);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CacheKey {
    pub content_hash: [u8; 32],
    pub dsl_hash: [u8; 32],
}

impl CacheKey {
    /// Strong ETag — hex of the full key. Stable across processes because
    /// it depends only on (input bytes, DSL string), not on time.
    pub fn etag(&self) -> String {
        let mut s = String::with_capacity(2 + 64 * 2 + 2);
        s.push('"');
        for b in self.content_hash.iter().chain(self.dsl_hash.iter()) {
            use std::fmt::Write;
            write!(&mut s, "{b:02x}").unwrap();
        }
        s.push('"');
        s
    }
}

#[derive(Clone)]
pub struct CachedResult {
    pub bytes: Bytes,
    pub format: crate::domain::value_objects::ImageFormat,
    pub width: u32,
    pub height: u32,
}
