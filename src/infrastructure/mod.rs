//! Infrastructure adapters — concrete implementations of application ports.

pub mod codec_opencv;
pub mod diff_opencv;
pub mod fonts_ab_glyph;
pub mod metrics;
pub mod ops_opencv;
pub mod runtime;
pub mod store_fs;
pub mod telemetry;

/// Concrete buffer that flows through the OpenCV adapters. Wraps the pixel
/// matrix together with metadata that survives the pipeline (source format
/// for encode-fallback, raw EXIF orientation for `AutoOrient`).
pub struct OpenCvImage {
    pub mat: opencv::core::Mat,
    pub source_format: crate::domain::value_objects::ImageFormat,
    /// EXIF orientation tag (1..=8). 1 = identity. After AutoOrient runs we
    /// reset this to 1 so subsequent ops don't double-rotate.
    pub exif_orientation: u16,
}
