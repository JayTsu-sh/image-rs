//! OpenCV-backed `ImageCodec`. Decode via `imdecode`, encode via `imencode`
//! for all formats.
//!
//! ## Why not pure-Rust encoders?
//!
//! We evaluated swapping JPEG → `jpeg-encoder` / `mozjpeg` and WebP → `webp`
//! crate to get a true zero-copy egress (`Vec<u8>` → `Bytes::from`). Results:
//!
//! * `jpeg-encoder` (pure Rust): **1.6-1.8× slower** than OpenCV's
//!   libjpeg-turbo on 640×480 / 1920×1080 baselines (6.9 / 44.5 ms vs
//!   4.2 / 24.7 ms). No SIMD path in pure Rust.
//! * `mozjpeg` (libjpeg-turbo + extras): **10× slower** out of the box —
//!   trellis quantization + scan optimization on by default; the crate
//!   doesn't cleanly expose toggles to disable them all.
//! * `webp` crate: roughly break-even — saves OpenCV's WebP wrapper overhead
//!   but adds a BGR→RGB `cvtColor` since the `webp` Rust binding doesn't
//!   expose libwebp's native `WebPEncodeBGR` entry point.
//!
//! Conclusion: OpenCV's `imencode` already calls libjpeg-turbo and libwebp
//! at the C/SIMD layer. A Rust wrapper around them via `CvVecOwner` is
//! still effectively zero-copy (no pixel copy, no encoded-byte copy) — the
//! `unsafe Send + Sync` impls below are the small price.

use std::io::Cursor;

use bytes::Bytes;
use opencv::{
    core::{Mat, Vector},
    imgcodecs,
    prelude::*,
};

use crate::application::ports::{EncodedImage, ImageCodec};
use crate::domain::error::DomainError;
use crate::domain::image::ImageBuffer;
use crate::domain::pipeline::{Compression, OutputSpec};
use crate::domain::value_objects::ImageFormat;
use crate::infrastructure::OpenCvImage;

pub struct OpenCvCodec {
    /// Decompression-bomb guard: maximum `width * height` allowed for the
    /// decoded pixel buffer.
    max_pixels: u64,
}

impl OpenCvCodec {
    pub fn new(max_pixels: u64) -> Self {
        Self { max_pixels }
    }
}

impl ImageCodec for OpenCvCodec {
    fn decode(&self, bytes: Bytes) -> Result<ImageBuffer, DomainError> {
        let format = sniff_format(&bytes)?;
        let exif_orientation = read_exif_orientation(&bytes).unwrap_or(1);

        // Decompression-bomb guard: read width*height from the format header
        // BEFORE imdecode allocates the pixel buffer. `imagesize::blob_size`
        // is a tiny pure-Rust parser, no allocation, no decode.
        if let Ok(size) = imagesize::blob_size(&bytes) {
            let pixels = (size.width as u64).saturating_mul(size.height as u64);
            if pixels > self.max_pixels {
                tracing::warn!(
                    width = size.width,
                    height = size.height,
                    pixels,
                    max = self.max_pixels,
                    "rejected oversized image (decompression bomb guard)"
                );
                return Err(DomainError::PayloadTooLarge);
            }
        }

        // Mat header over the borrowed slice — zero-copy view of `bytes`.
        // imdecode allocates the pixel buffer (the one inevitable allocation
        // on this path); the source `Bytes` is dropped at end of scope.
        let header = Mat::from_slice::<u8>(&bytes)
            .map_err(|e| DomainError::Decode(e.to_string()))?;
        let mat = imgcodecs::imdecode(&header, imgcodecs::IMREAD_UNCHANGED)
            .map_err(|e| DomainError::Decode(e.to_string()))?;
        if mat.empty() {
            return Err(DomainError::Decode("empty image".into()));
        }

        Ok(ImageBuffer::new(OpenCvImage {
            mat,
            source_format: format,
            exif_orientation,
        }))
    }

    fn encode(
        &self,
        image: ImageBuffer,
        output: &OutputSpec,
    ) -> Result<EncodedImage, DomainError> {
        let img = *image
            .downcast::<OpenCvImage>()
            .map_err(|b| DomainError::Internal(format!(
                "expected OpenCvImage, got {}", b.type_name()
            )))?;

        let format = output.format.unwrap_or(img.source_format);
        let mat = strip_alpha_if_needed(img.mat, format)?;
        let width = mat.cols().max(0) as u32;
        let height = mat.rows().max(0) as u32;

        let mut params = Vector::<i32>::new();
        match format {
            ImageFormat::Jpeg => {
                // Pipeline::new already rejected JPEG + Lossless — this
                // match is exhaustive defensively.
                let quality = match output.compression {
                    Compression::Lossy(q) => q.value() as i32,
                    Compression::Lossless => {
                        return Err(DomainError::Encode(
                            "jpeg cannot be lossless (should have been caught by \
                             Pipeline::new)".into(),
                        ));
                    }
                };
                params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
                params.push(quality);
                if output.progressive {
                    params.push(imgcodecs::IMWRITE_JPEG_PROGRESSIVE);
                    params.push(1);
                }
            }
            ImageFormat::Png => {
                // PNG is inherently lossless; `Compression::Lossy` just
                // affects the deflate level ramp, not pixel fidelity.
                // Keep compression level at 7 for both branches — good
                // balance between size and encode speed.
                params.push(imgcodecs::IMWRITE_PNG_COMPRESSION);
                params.push(7);
            }
            ImageFormat::WebP => {
                // OpenCV WebP convention: quality > 100 ⇒ libwebp lossless
                // path. 101 is the canonical "lossless" sentinel.
                let q = match output.compression {
                    Compression::Lossy(q) => q.value() as i32,
                    Compression::Lossless => 101,
                };
                params.push(imgcodecs::IMWRITE_WEBP_QUALITY);
                params.push(q);
            }
        }

        let mut buf = Vector::<u8>::new();
        imgcodecs::imencode(format.extension(), &mat, &mut buf, &params)
            .map_err(|e| DomainError::Encode(e.to_string()))?;

        // Zero-copy egress: hand the cv::Vector<u8> to Bytes as the owner.
        let bytes = Bytes::from_owner(CvVecOwner(buf));
        Ok(EncodedImage { bytes, format, width, height })
    }
}

fn strip_alpha_if_needed(mat: Mat, format: ImageFormat) -> Result<Mat, DomainError> {
    if format.supports_alpha() || mat.channels() != 4 {
        return Ok(mat);
    }
    let mut bgr = Mat::default();
    opencv::imgproc::cvt_color_def(&mat, &mut bgr, opencv::imgproc::COLOR_BGRA2BGR)
        .map_err(|e| DomainError::Encode(e.to_string()))?;
    Ok(bgr)
}

/// Wrapper that lets `bytes::Bytes::from_owner` accept `cv::Vector<u8>`.
struct CvVecOwner(Vector<u8>);
impl AsRef<[u8]> for CvVecOwner {
    fn as_ref(&self) -> &[u8] { self.0.as_slice() }
}
// SAFETY: cv::Vector<u8> owns its C++ vector exclusively; once handed to
// Bytes::from_owner we never expose it for mutation. Concurrent immutable
// reads via the shared `[u8]` slice are safe.
unsafe impl Send for CvVecOwner {}
unsafe impl Sync for CvVecOwner {}

/// Magic-byte sniff. Cheap, no allocation.
fn sniff_format(bytes: &[u8]) -> Result<ImageFormat, DomainError> {
    if bytes.len() >= 3 && &bytes[..3] == b"\xFF\xD8\xFF" {
        return Ok(ImageFormat::Jpeg);
    }
    if bytes.len() >= 8 && &bytes[..8] == b"\x89PNG\r\n\x1a\n" {
        return Ok(ImageFormat::Png);
    }
    if bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        return Ok(ImageFormat::WebP);
    }
    Err(DomainError::UnsupportedFormat("unknown magic".into()))
}

/// Best-effort EXIF orientation read. Returns 1 (identity) on failure.
fn read_exif_orientation(bytes: &[u8]) -> Option<u16> {
    let exif = exif::Reader::new()
        .read_from_container(&mut Cursor::new(bytes))
        .ok()?;
    let field = exif.get_field(exif::Tag::Orientation, exif::In::PRIMARY)?;
    field.value.get_uint(0).map(|v| v as u16)
}
