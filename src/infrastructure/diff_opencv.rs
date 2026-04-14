//! OpenCV-backed `ImageDiffer`. Computes pixel diff between two images
//! and renders it according to the requested mode.

use opencv::{
    core::{self, Mat, Scalar},
    imgproc,
    prelude::*,
};

use crate::application::ports::ImageDiffer;
use crate::domain::error::DomainError;
use crate::domain::image::ImageBuffer;
use crate::domain::value_objects::{DiffMode, DiffSpec};
use crate::infrastructure::OpenCvImage;

pub struct OpenCvDiffer;

impl OpenCvDiffer {
    pub fn new() -> Self { Self }
}

impl Default for OpenCvDiffer {
    fn default() -> Self { Self::new() }
}

impl ImageDiffer for OpenCvDiffer {
    fn diff(
        &self,
        before: ImageBuffer,
        after: ImageBuffer,
        spec: &DiffSpec,
    ) -> Result<ImageBuffer, DomainError> {
        let before_img = *before.downcast::<OpenCvImage>().map_err(|b| {
            DomainError::Internal(format!("expected OpenCvImage, got {}", b.type_name()))
        })?;
        let after_img = *after.downcast::<OpenCvImage>().map_err(|b| {
            DomainError::Internal(format!("expected OpenCvImage, got {}", b.type_name()))
        })?;

        if before_img.mat.cols() != after_img.mat.cols()
            || before_img.mat.rows() != after_img.mat.rows()
        {
            return Err(DomainError::invalid(format!(
                "diff requires same dimensions: before={}x{}, after={}x{}",
                before_img.mat.cols(),
                before_img.mat.rows(),
                after_img.mat.cols(),
                after_img.mat.rows()
            )));
        }

        // Both images need a common channel layout. Drop alpha and force BGR.
        let before_bgr = to_bgr(&before_img.mat)?;
        let after_bgr = to_bgr(&after_img.mat)?;

        let mut absdiff = Mat::default();
        core::absdiff(&before_bgr, &after_bgr, &mut absdiff)
            .map_err(|e| DomainError::op("diff", e.to_string()))?;

        let result_mat = match spec.mode {
            DiffMode::Abs => absdiff,
            DiffMode::Grayscale => {
                let mut gray = Mat::default();
                imgproc::cvt_color_def(&absdiff, &mut gray, imgproc::COLOR_BGR2GRAY)
                    .map_err(|e| DomainError::op("diff", e.to_string()))?;
                // Re-expand to 3-channel for consistent encoding paths.
                let mut bgr = Mat::default();
                imgproc::cvt_color_def(&gray, &mut bgr, imgproc::COLOR_GRAY2BGR)
                    .map_err(|e| DomainError::op("diff", e.to_string()))?;
                bgr
            }
            DiffMode::Highlight => {
                // 1. Reduce to grayscale magnitude
                let mut gray = Mat::default();
                imgproc::cvt_color_def(&absdiff, &mut gray, imgproc::COLOR_BGR2GRAY)
                    .map_err(|e| DomainError::op("diff", e.to_string()))?;
                // 2. Threshold to a binary mask
                let mut mask = Mat::default();
                imgproc::threshold(
                    &gray,
                    &mut mask,
                    spec.threshold as f64,
                    255.0,
                    imgproc::THRESH_BINARY,
                )
                .map_err(|e| DomainError::op("diff", e.to_string()))?;
                // 3. Paint red on a copy of the `before` image where mask is set
                let mut result = before_bgr
                    .try_clone()
                    .map_err(|e| DomainError::op("diff", e.to_string()))?;
                let red = Scalar::new(0.0, 0.0, 255.0, 0.0); // BGR
                result
                    .set_to(&red, &mask)
                    .map_err(|e| DomainError::op("diff", e.to_string()))?;
                result
            }
        };

        // Wrap as OpenCvImage so the existing codec.encode path can consume
        // it. Source-format defaults to PNG since the diff is a derived
        // image, not a recovered original — encode side will override if the
        // request specifies a format anyway.
        Ok(ImageBuffer::new(OpenCvImage {
            mat: result_mat,
            source_format: crate::domain::value_objects::ImageFormat::Png,
            exif_orientation: 1,
        }))
    }
}

fn to_bgr(mat: &Mat) -> Result<Mat, DomainError> {
    match mat.channels() {
        3 => Ok(mat.clone()),
        4 => {
            let mut bgr = Mat::default();
            imgproc::cvt_color_def(mat, &mut bgr, imgproc::COLOR_BGRA2BGR)
                .map_err(|e| DomainError::op("diff", e.to_string()))?;
            Ok(bgr)
        }
        1 => {
            let mut bgr = Mat::default();
            imgproc::cvt_color_def(mat, &mut bgr, imgproc::COLOR_GRAY2BGR)
                .map_err(|e| DomainError::op("diff", e.to_string()))?;
            Ok(bgr)
        }
        ch => Err(DomainError::invalid(format!(
            "diff: unsupported channel count {ch}"
        ))),
    }
}
