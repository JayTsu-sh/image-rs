//! Pipeline aggregate root.
//!
//! Owns an ordered list of operations and the desired output specification.
//! All invariants between ops live in `Pipeline::new`.

use crate::domain::error::DomainError;
use crate::domain::ops::Op;
use crate::domain::value_objects::{ImageFormat, Quality};

/// How the output should be compressed.
///
/// Modeled as a closed sum type instead of a nullable `quality + lossless`
/// boolean pair so that invalid combinations (e.g. "lossless with quality
/// 30") cannot be expressed at all. The `Pipeline::new` constructor adds
/// the remaining cross-field rule: JPEG + `Lossless` is rejected because
/// the JPEG format is lossy by definition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Compression {
    /// Lossy with the given quality factor. Valid for JPEG and WebP; a
    /// no-op for PNG (which is inherently lossless).
    Lossy(Quality),
    /// Lossless. Valid for PNG (always) and WebP (via libwebp's lossless
    /// encode path — OpenCV selects it when `IMWRITE_WEBP_QUALITY > 100`).
    /// JPEG rejects this at `Pipeline::new`.
    Lossless,
}

impl Default for Compression {
    fn default() -> Self {
        Compression::Lossy(Quality::default())
    }
}

#[derive(Debug, Clone)]
pub struct Pipeline {
    ops: Vec<Op>,
    output: OutputSpec,
}

impl Pipeline {
    pub fn new(ops: Vec<Op>, output: OutputSpec) -> Result<Self, DomainError> {
        // Cross-field invariant: alpha-producing ops require an alpha-capable
        // output format. JPEG silently flattens alpha against black, which is
        // almost never what the caller wants — fail fast instead.
        if let Some(fmt) = output.format {
            if !fmt.supports_alpha() && ops.iter().any(produces_alpha) {
                return Err(DomainError::invalid(
                    "output format does not support alpha but pipeline contains \
                     alpha-producing ops (round_corner / watermark with alpha)",
                ));
            }
        }
        // Cross-field invariant: JPEG is lossy by definition, so `Lossless`
        // is nonsensical for it. Reject rather than silently degrade.
        if let (Some(ImageFormat::Jpeg), Compression::Lossless) =
            (output.format, output.compression)
        {
            return Err(DomainError::invalid(
                "JPEG does not support lossless compression; use PNG or WebP lossless",
            ));
        }
        Ok(Self { ops, output })
    }

    /// Single-op convenience constructor used by the per-operation endpoints.
    pub fn single(op: Op, output: OutputSpec) -> Result<Self, DomainError> {
        Self::new(vec![op], output)
    }

    pub fn ops(&self) -> &[Op] { &self.ops }
    pub fn output(&self) -> &OutputSpec { &self.output }
}

/// Output specification — encoding format + compression.
#[derive(Debug, Clone, Copy)]
pub struct OutputSpec {
    /// `None` means "preserve input format".
    pub format: Option<ImageFormat>,
    pub compression: Compression,
    pub progressive: bool,
}

impl OutputSpec {
    pub fn new(
        format: Option<ImageFormat>,
        compression: Compression,
        progressive: bool,
    ) -> Self {
        Self { format, compression, progressive }
    }

    /// Convenience: lossy with an explicit quality.
    pub fn lossy(
        format: Option<ImageFormat>,
        quality: Quality,
        progressive: bool,
    ) -> Self {
        Self { format, compression: Compression::Lossy(quality), progressive }
    }
}

impl Default for OutputSpec {
    fn default() -> Self {
        Self {
            format: None,
            compression: Compression::default(),
            progressive: false,
        }
    }
}

fn produces_alpha(op: &Op) -> bool {
    matches!(op, Op::RoundCorner(_) | Op::WatermarkImage(_))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn jpeg_with_lossless_is_rejected() {
        let output = OutputSpec::new(
            Some(ImageFormat::Jpeg),
            Compression::Lossless,
            false,
        );
        let err = Pipeline::new(vec![], output).unwrap_err();
        assert!(matches!(err, DomainError::InvalidArgument(_)));
    }

    #[test]
    fn webp_lossless_is_accepted() {
        let output = OutputSpec::new(
            Some(ImageFormat::WebP),
            Compression::Lossless,
            false,
        );
        assert!(Pipeline::new(vec![], output).is_ok());
    }

    #[test]
    fn png_lossless_is_accepted() {
        let output = OutputSpec::new(
            Some(ImageFormat::Png),
            Compression::Lossless,
            false,
        );
        assert!(Pipeline::new(vec![], output).is_ok());
    }
}
