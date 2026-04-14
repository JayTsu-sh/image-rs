//! OpenCV-backed `OpExecutor` — concrete implementation of every domain Op.
//!
//! Each helper takes `&mut Mat` and either mutates in place (preferred when
//! `src == dst` is supported) or replaces the buffer via `*mat = new`. The
//! enclosing `OpenCvImage` wrapper is reassembled in the trait `execute`.

use std::sync::Arc;

use ab_glyph::{Font, FontArc, PxScale, ScaleFont};
use opencv::{
    core::{
        self, Mat, Point, Point2f, Rect as CvRect, Scalar, Size, Vector, CV_8UC1, CV_32FC1,
        CV_32FC4,
    },
    imgcodecs, imgproc,
    prelude::*,
};

use crate::application::ports::{
    FontProvider, MaskCache, MaskKey, OpContext, OpExecutor,
};
use crate::domain::error::DomainError;
use crate::domain::image::ImageBuffer;
use crate::domain::ops::*;
use crate::domain::value_objects::*;
use crate::infrastructure::OpenCvImage;
use crate::infrastructure::fonts_ab_glyph::AbGlyphFontHandle;

pub struct OpenCvOpExecutor {
    fonts: Arc<dyn FontProvider>,
    masks: Arc<dyn MaskCache>,
}

impl OpenCvOpExecutor {
    pub fn new(fonts: Arc<dyn FontProvider>, masks: Arc<dyn MaskCache>) -> Self {
        Self { fonts, masks }
    }
}

impl OpExecutor for OpenCvOpExecutor {
    fn execute(
        &self,
        image: ImageBuffer,
        op: &Op,
        ctx: &OpContext,
    ) -> Result<ImageBuffer, DomainError> {
        let mut img = *image.downcast::<OpenCvImage>().map_err(|b| {
            DomainError::Internal(format!("expected OpenCvImage, got {}", b.type_name()))
        })?;

        match op {
            Op::Resize(s) => resize(&mut img.mat, s)?,
            Op::Rotate(s) => rotate(&mut img.mat, s)?,
            Op::Crop(s) => crop(&mut img.mat, s)?,
            Op::Blur(s) => blur(&mut img.mat, s)?,
            Op::Sharpen(s) => sharpen(&mut img.mat, s)?,
            Op::RoundCorner(s) => round_corner(&mut img.mat, s, self.masks.as_ref())?,
            Op::Brightness(s) => brightness(&mut img.mat, s)?,
            Op::Contrast(s) => contrast(&mut img.mat, s)?,
            Op::Saturation(s) => saturation(&mut img.mat, s)?,
            Op::Temperature(s) => temperature(&mut img.mat, s)?,
            Op::AutoOrient => {
                auto_orient(&mut img.mat, img.exif_orientation)?;
                img.exif_orientation = 1;
            }
            Op::WatermarkImage(s) => watermark_image(&mut img.mat, s, ctx)?,
            Op::WatermarkText(s) => watermark_text(&mut img.mat, s, self.fonts.as_ref())?,
        }

        Ok(ImageBuffer::new(img))
    }
}

// ─── Error helper ────────────────────────────────────────────────────────────

fn opfail(op: &'static str) -> impl Fn(opencv::Error) -> DomainError {
    move |e| DomainError::OpFailed { op, message: e.to_string() }
}

// ─── Channel helpers ─────────────────────────────────────────────────────────

fn ensure_bgra(mat: &mut Mat) -> Result<(), DomainError> {
    if mat.channels() == 4 {
        return Ok(());
    }
    let code = match mat.channels() {
        3 => imgproc::COLOR_BGR2BGRA,
        1 => imgproc::COLOR_GRAY2BGRA,
        ch => return Err(DomainError::op("channels", format!("unsupported channels: {ch}"))),
    };
    let mut bgra = Mat::default();
    imgproc::cvt_color_def(mat, &mut bgra, code).map_err(opfail("channels"))?;
    *mat = bgra;
    Ok(())
}

// ─── Resize ──────────────────────────────────────────────────────────────────

fn resize(mat: &mut Mat, spec: &ResizeSpec) -> Result<(), DomainError> {
    let in_w = mat.cols();
    let in_h = mat.rows();
    let (out_w, out_h) = compute_dims(spec, in_w, in_h)?;

    let interp = match spec.interpolation {
        Interpolation::Auto => {
            if out_w * out_h < in_w * in_h {
                imgproc::INTER_AREA
            } else {
                imgproc::INTER_LINEAR
            }
        }
        Interpolation::Nearest => imgproc::INTER_NEAREST,
        Interpolation::Linear => imgproc::INTER_LINEAR,
        Interpolation::Cubic => imgproc::INTER_CUBIC,
        Interpolation::Area => imgproc::INTER_AREA,
        Interpolation::Lanczos4 => imgproc::INTER_LANCZOS4,
    };

    let mut dst = Mat::default();
    if matches!(spec.mode, ResizeMode::Fill) {
        let (cx, cy, cw, ch) = compute_fill_crop(in_w, in_h, out_w, out_h);
        let cropped = Mat::roi(mat, CvRect::new(cx, cy, cw, ch))
            .map_err(opfail("resize"))?;
        imgproc::resize(
            &cropped,
            &mut dst,
            Size::new(out_w, out_h),
            0.0,
            0.0,
            interp,
        )
        .map_err(opfail("resize"))?;
    } else {
        imgproc::resize(
            mat,
            &mut dst,
            Size::new(out_w, out_h),
            0.0,
            0.0,
            interp,
        )
        .map_err(opfail("resize"))?;
    }
    *mat = dst;
    Ok(())
}

fn compute_dims(spec: &ResizeSpec, in_w: i32, in_h: i32) -> Result<(i32, i32), DomainError> {
    match spec.mode {
        ResizeMode::Exact => {
            let w = spec
                .width
                .ok_or_else(|| DomainError::invalid("exact resize needs width"))?
                as i32;
            let h = spec
                .height
                .ok_or_else(|| DomainError::invalid("exact resize needs height"))?
                as i32;
            Ok((w, h))
        }
        ResizeMode::Fit => {
            let max_w = spec.width.map(|w| w as i32).unwrap_or(i32::MAX);
            let max_h = spec.height.map(|h| h as i32).unwrap_or(i32::MAX);
            let rw = max_w as f32 / in_w as f32;
            let rh = max_h as f32 / in_h as f32;
            let r = rw.min(rh);
            let w = ((in_w as f32 * r).round() as i32).max(1);
            let h = ((in_h as f32 * r).round() as i32).max(1);
            Ok((w, h))
        }
        ResizeMode::Fill => {
            let w = spec
                .width
                .ok_or_else(|| DomainError::invalid("fill resize needs width"))?
                as i32;
            let h = spec
                .height
                .ok_or_else(|| DomainError::invalid("fill resize needs height"))?
                as i32;
            Ok((w, h))
        }
    }
}

fn compute_fill_crop(in_w: i32, in_h: i32, out_w: i32, out_h: i32) -> (i32, i32, i32, i32) {
    let target = out_w as f32 / out_h as f32;
    let src = in_w as f32 / in_h as f32;
    if src > target {
        let nw = (in_h as f32 * target).round() as i32;
        ((in_w - nw) / 2, 0, nw, in_h)
    } else {
        let nh = (in_w as f32 / target).round() as i32;
        (0, (in_h - nh) / 2, in_w, nh)
    }
}

// ─── Rotate (arbitrary angle, expanding canvas) ──────────────────────────────

fn rotate(mat: &mut Mat, spec: &RotateSpec) -> Result<(), DomainError> {
    let w = mat.cols() as f64;
    let h = mat.rows() as f64;
    let center = Point2f::new((w / 2.0) as f32, (h / 2.0) as f32);

    let mut m = imgproc::get_rotation_matrix_2d(center, spec.angle.as_degrees(), 1.0)
        .map_err(opfail("rotate"))?;

    let theta = spec.angle.as_degrees().to_radians();
    let cos = theta.cos().abs();
    let sin = theta.sin().abs();
    // Subtract a small epsilon before ceil so axis-aligned angles (90°, 180°,
    // 270°) don't round up due to floating-point noise in cos/sin.
    const EPS: f64 = 1e-9;
    let new_w = (h * sin + w * cos - EPS).ceil().max(1.0) as i32;
    let new_h = (h * cos + w * sin - EPS).ceil().max(1.0) as i32;

    // Adjust translation so the rotated image is centered in the new canvas.
    {
        let tx = m.at_2d_mut::<f64>(0, 2).map_err(opfail("rotate"))?;
        *tx += new_w as f64 / 2.0 - w / 2.0;
    }
    {
        let ty = m.at_2d_mut::<f64>(1, 2).map_err(opfail("rotate"))?;
        *ty += new_h as f64 / 2.0 - h / 2.0;
    }

    let bg = spec.background;
    let bg_scalar = Scalar::new(bg.b as f64, bg.g as f64, bg.r as f64, bg.a as f64);

    let mut dst = Mat::default();
    imgproc::warp_affine(
        mat,
        &mut dst,
        &m,
        Size::new(new_w, new_h),
        imgproc::INTER_LINEAR,
        core::BORDER_CONSTANT,
        bg_scalar,
    )
    .map_err(opfail("rotate"))?;
    *mat = dst;
    Ok(())
}

// ─── Crop ────────────────────────────────────────────────────────────────────

fn crop(mat: &mut Mat, spec: &CropSpec) -> Result<(), DomainError> {
    let r = &spec.rect;
    let img_w = mat.cols() as u32;
    let img_h = mat.rows() as u32;
    if r.x.checked_add(r.width).map(|v| v > img_w).unwrap_or(true)
        || r.y.checked_add(r.height).map(|v| v > img_h).unwrap_or(true)
    {
        return Err(DomainError::op(
            "crop",
            format!("rect out of bounds for {img_w}x{img_h}"),
        ));
    }
    let cv_rect = CvRect::new(r.x as i32, r.y as i32, r.width as i32, r.height as i32);
    let view = Mat::roi(mat, cv_rect).map_err(opfail("crop"))?;
    // Materialize so the parent buffer can be released. This is the only
    // necessary copy on this op — the resulting buffer is strictly smaller.
    let detached = view.try_clone().map_err(opfail("crop"))?;
    *mat = detached;
    Ok(())
}

// ─── Blur (Gaussian) ─────────────────────────────────────────────────────────

fn blur(mat: &mut Mat, spec: &BlurSpec) -> Result<(), DomainError> {
    if spec.sigma <= 0.0 {
        return Ok(());
    }
    let mut dst = Mat::default();
    imgproc::gaussian_blur_def(mat, &mut dst, Size::new(0, 0), spec.sigma as f64)
        .map_err(opfail("blur"))?;
    *mat = dst;
    Ok(())
}

// ─── Sharpen (Unsharp Mask) ──────────────────────────────────────────────────

fn sharpen(mat: &mut Mat, spec: &SharpenSpec) -> Result<(), DomainError> {
    if spec.amount <= 0.0 {
        return Ok(());
    }
    let mut blurred = Mat::default();
    let sigma = spec.radius.max(0.1) as f64;
    imgproc::gaussian_blur_def(mat, &mut blurred, Size::new(0, 0), sigma)
        .map_err(opfail("sharpen"))?;

    let amount = spec.amount as f64;
    let mut dst = Mat::default();
    core::add_weighted(mat, 1.0 + amount, &blurred, -amount, 0.0, &mut dst, -1)
        .map_err(opfail("sharpen"))?;
    *mat = dst;
    Ok(())
}

// ─── Brightness / Contrast ───────────────────────────────────────────────────

fn brightness(mat: &mut Mat, spec: &BrightnessSpec) -> Result<(), DomainError> {
    let mut dst = Mat::default();
    mat.convert_to(&mut dst, -1, 1.0, spec.value as f64)
        .map_err(opfail("brightness"))?;
    *mat = dst;
    Ok(())
}

fn contrast(mat: &mut Mat, spec: &ContrastSpec) -> Result<(), DomainError> {
    let mut dst = Mat::default();
    mat.convert_to(&mut dst, -1, spec.value as f64, 0.0)
        .map_err(opfail("contrast"))?;
    *mat = dst;
    Ok(())
}

// ─── Saturation (HSV-space S-channel scale) ──────────────────────────────────

fn saturation(mat: &mut Mat, spec: &SaturationSpec) -> Result<(), DomainError> {
    if (spec.factor - 1.0).abs() < 1e-6 {
        return Ok(());
    }

    // OpenCV's BGR2HSV expects 3-channel input. If we have BGRA we split off
    // the alpha, do the conversion on BGR, then re-attach.
    let had_alpha = mat.channels() == 4;
    let alpha = if had_alpha {
        let mut channels = Vector::<Mat>::new();
        core::split(mat, &mut channels).map_err(opfail("saturation"))?;
        let mut bgr_v = Vector::<Mat>::new();
        bgr_v.push(channels.get(0).map_err(opfail("saturation"))?);
        bgr_v.push(channels.get(1).map_err(opfail("saturation"))?);
        bgr_v.push(channels.get(2).map_err(opfail("saturation"))?);
        let mut bgr = Mat::default();
        core::merge(&bgr_v, &mut bgr).map_err(opfail("saturation"))?;
        let alpha = channels.get(3).map_err(opfail("saturation"))?;
        *mat = bgr;
        Some(alpha)
    } else {
        None
    };

    // BGR → HSV
    let mut hsv = Mat::default();
    imgproc::cvt_color_def(mat, &mut hsv, imgproc::COLOR_BGR2HSV)
        .map_err(opfail("saturation"))?;

    // Split, scale S channel, merge back.
    let mut hsv_channels = Vector::<Mat>::new();
    core::split(&hsv, &mut hsv_channels).map_err(opfail("saturation"))?;

    let s = hsv_channels.get(1).map_err(opfail("saturation"))?;
    let mut new_s = Mat::default();
    s.convert_to(&mut new_s, -1, spec.factor as f64, 0.0)
        .map_err(opfail("saturation"))?;
    hsv_channels.set(1, new_s).map_err(opfail("saturation"))?;

    let mut new_hsv = Mat::default();
    core::merge(&hsv_channels, &mut new_hsv).map_err(opfail("saturation"))?;

    // HSV → BGR
    let mut bgr_out = Mat::default();
    imgproc::cvt_color_def(&new_hsv, &mut bgr_out, imgproc::COLOR_HSV2BGR)
        .map_err(opfail("saturation"))?;

    if let Some(alpha) = alpha {
        // Re-attach the original alpha channel.
        let mut bgr_channels = Vector::<Mat>::new();
        core::split(&bgr_out, &mut bgr_channels).map_err(opfail("saturation"))?;
        let mut merged = Vector::<Mat>::new();
        merged.push(bgr_channels.get(0).map_err(opfail("saturation"))?);
        merged.push(bgr_channels.get(1).map_err(opfail("saturation"))?);
        merged.push(bgr_channels.get(2).map_err(opfail("saturation"))?);
        merged.push(alpha);
        let mut out = Mat::default();
        core::merge(&merged, &mut out).map_err(opfail("saturation"))?;
        *mat = out;
    } else {
        *mat = bgr_out;
    }
    Ok(())
}

// ─── Temperature (R/B channel scale) ─────────────────────────────────────────

fn temperature(mat: &mut Mat, spec: &TemperatureSpec) -> Result<(), DomainError> {
    if spec.value == 0 {
        return Ok(());
    }
    let factor = spec.value as f64 / 200.0; // -0.5..0.5
    let r_scale = 1.0 + factor;
    let b_scale = 1.0 - factor;

    let had_alpha = mat.channels() == 4;
    let mut channels = Vector::<Mat>::new();
    core::split(mat, &mut channels).map_err(opfail("temperature"))?;

    // OpenCV BGR(A) channel order: [B, G, R, A?]
    let b = channels.get(0).map_err(opfail("temperature"))?;
    let g = channels.get(1).map_err(opfail("temperature"))?;
    let r = channels.get(2).map_err(opfail("temperature"))?;

    let mut new_b = Mat::default();
    b.convert_to(&mut new_b, -1, b_scale, 0.0)
        .map_err(opfail("temperature"))?;
    let mut new_r = Mat::default();
    r.convert_to(&mut new_r, -1, r_scale, 0.0)
        .map_err(opfail("temperature"))?;

    let mut out_channels = Vector::<Mat>::new();
    out_channels.push(new_b);
    out_channels.push(g);
    out_channels.push(new_r);
    if had_alpha {
        out_channels.push(channels.get(3).map_err(opfail("temperature"))?);
    }

    let mut out = Mat::default();
    core::merge(&out_channels, &mut out).map_err(opfail("temperature"))?;
    *mat = out;
    Ok(())
}

// ─── Auto-orient (apply EXIF orientation tag) ────────────────────────────────

fn auto_orient(mat: &mut Mat, orientation: u16) -> Result<(), DomainError> {
    if orientation <= 1 {
        return Ok(());
    }
    let mut dst = Mat::default();
    match orientation {
        2 => {
            core::flip(mat, &mut dst, 1).map_err(opfail("auto_orient"))?;
        }
        3 => {
            core::rotate(mat, &mut dst, core::ROTATE_180).map_err(opfail("auto_orient"))?;
        }
        4 => {
            core::flip(mat, &mut dst, 0).map_err(opfail("auto_orient"))?;
        }
        5 => {
            let mut tmp = Mat::default();
            core::rotate(mat, &mut tmp, core::ROTATE_90_CLOCKWISE)
                .map_err(opfail("auto_orient"))?;
            core::flip(&tmp, &mut dst, 1).map_err(opfail("auto_orient"))?;
        }
        6 => {
            core::rotate(mat, &mut dst, core::ROTATE_90_CLOCKWISE)
                .map_err(opfail("auto_orient"))?;
        }
        7 => {
            let mut tmp = Mat::default();
            core::rotate(mat, &mut tmp, core::ROTATE_90_COUNTERCLOCKWISE)
                .map_err(opfail("auto_orient"))?;
            core::flip(&tmp, &mut dst, 1).map_err(opfail("auto_orient"))?;
        }
        8 => {
            core::rotate(mat, &mut dst, core::ROTATE_90_COUNTERCLOCKWISE)
                .map_err(opfail("auto_orient"))?;
        }
        _ => {
            return Err(DomainError::op(
                "auto_orient",
                format!("invalid orientation: {orientation}"),
            ));
        }
    }
    *mat = dst;
    Ok(())
}

// ─── Round corner (alpha mask, cached) ───────────────────────────────────────

fn round_corner(
    mat: &mut Mat,
    spec: &RoundCornerSpec,
    cache: &dyn MaskCache,
) -> Result<(), DomainError> {
    ensure_bgra(mat)?;
    let w = mat.cols() as u32;
    let h = mat.rows() as u32;
    let radius = spec.radius.min(w / 2).min(h / 2);
    if radius == 0 {
        return Ok(());
    }

    let key = MaskKey { width: w, height: h, radius };
    let mask_buf = cache.get_or_compute(key, &mut || compute_round_mask(w, h, radius))?;
    let mask_arc = *mask_buf.downcast::<Arc<Mat>>().map_err(|b| {
        DomainError::Internal(format!("mask cache returned wrong type: {}", b.type_name()))
    })?;
    let mask: &Mat = mask_arc.as_ref();

    // alpha_new = alpha * mask / 255
    let mut channels = Vector::<Mat>::new();
    core::split(mat, &mut channels).map_err(opfail("round_corner"))?;
    let alpha = channels.get(3).map_err(opfail("round_corner"))?;
    let mut new_alpha = Mat::default();
    core::multiply(&alpha, mask, &mut new_alpha, 1.0 / 255.0, -1)
        .map_err(opfail("round_corner"))?;
    channels.set(3, new_alpha).map_err(opfail("round_corner"))?;

    let mut out = Mat::default();
    core::merge(&channels, &mut out).map_err(opfail("round_corner"))?;
    *mat = out;
    Ok(())
}

fn compute_round_mask(w: u32, h: u32, r: u32) -> Result<ImageBuffer, DomainError> {
    let w = w as i32;
    let h = h as i32;
    let r = r as i32;
    let mut mask = Mat::zeros(h, w, CV_8UC1)
        .map_err(opfail("round_corner"))?
        .to_mat()
        .map_err(opfail("round_corner"))?;

    // Two overlapping rectangles for the central cross
    imgproc::rectangle(
        &mut mask,
        CvRect::new(r, 0, w - 2 * r, h),
        Scalar::all(255.0),
        -1,
        imgproc::LINE_8,
        0,
    )
    .map_err(opfail("round_corner"))?;
    imgproc::rectangle(
        &mut mask,
        CvRect::new(0, r, w, h - 2 * r),
        Scalar::all(255.0),
        -1,
        imgproc::LINE_8,
        0,
    )
    .map_err(opfail("round_corner"))?;

    // Four anti-aliased corner circles
    for (cx, cy) in [
        (r, r),
        (w - r - 1, r),
        (r, h - r - 1),
        (w - r - 1, h - r - 1),
    ] {
        imgproc::circle(
            &mut mask,
            Point::new(cx, cy),
            r,
            Scalar::all(255.0),
            -1,
            imgproc::LINE_AA,
            0,
        )
        .map_err(opfail("round_corner"))?;
    }
    Ok(ImageBuffer::new(mask))
}

// ─── Watermark (image) ───────────────────────────────────────────────────────

fn watermark_image(
    mat: &mut Mat,
    spec: &WatermarkImageSpec,
    ctx: &OpContext,
) -> Result<(), DomainError> {
    let asset = ctx.asset(&spec.asset)?;
    let header = Mat::from_slice::<u8>(asset).map_err(opfail("watermark_image"))?;
    let mut wm = imgcodecs::imdecode(&header, imgcodecs::IMREAD_UNCHANGED)
        .map_err(opfail("watermark_image"))?;
    if wm.empty() {
        return Err(DomainError::op("watermark_image", "empty watermark image"));
    }

    ensure_bgra(mat)?;
    ensure_bgra(&mut wm)?;

    // Scale watermark by spec.scale (relative to main image width).
    let img_w = mat.cols();
    let img_h = mat.rows();
    let target_w = ((img_w as f32) * spec.scale).round() as i32;
    let factor = target_w as f32 / wm.cols() as f32;
    let target_h = (wm.rows() as f32 * factor).round().max(1.0) as i32;

    let wm_scaled = if target_w != wm.cols() || target_h != wm.rows() {
        let mut s = Mat::default();
        imgproc::resize(
            &wm,
            &mut s,
            Size::new(target_w.max(1), target_h),
            0.0,
            0.0,
            imgproc::INTER_AREA,
        )
        .map_err(opfail("watermark_image"))?;
        s
    } else {
        wm
    };

    let (x, y) = anchor_xy(
        spec.position,
        img_w,
        img_h,
        wm_scaled.cols(),
        wm_scaled.rows(),
        spec.margin as i32,
    );
    // Clamp to image bounds.
    let x0 = x.max(0);
    let y0 = y.max(0);
    let w = (wm_scaled.cols()).min(img_w - x0);
    let h = (wm_scaled.rows()).min(img_h - y0);
    if w <= 0 || h <= 0 {
        return Ok(());
    }
    let dst_rect = CvRect::new(x0, y0, w, h);
    let src_rect = CvRect::new(x0 - x, y0 - y, w, h);

    alpha_blend(mat, &wm_scaled, dst_rect, src_rect, spec.opacity.value())?;
    Ok(())
}

fn alpha_blend(
    dst: &mut Mat,
    src_bgra: &Mat,
    dst_rect: CvRect,
    src_rect: CvRect,
    opacity: f32,
) -> Result<(), DomainError> {
    let mut dst_roi = Mat::roi_mut(dst, dst_rect).map_err(opfail("watermark_image"))?;
    let src_view = Mat::roi(src_bgra, src_rect).map_err(opfail("watermark_image"))?;

    // Split src into channels and grab alpha.
    let mut src_ch = Vector::<Mat>::new();
    core::split(&src_view, &mut src_ch).map_err(opfail("watermark_image"))?;
    let src_alpha = src_ch.get(3).map_err(opfail("watermark_image"))?;

    // alpha_f = src_alpha * opacity / 255 → CV_32FC1 in [0,1]
    let mut alpha_f = Mat::default();
    src_alpha
        .convert_to(&mut alpha_f, CV_32FC1, opacity as f64 / 255.0, 0.0)
        .map_err(opfail("watermark_image"))?;

    // Replicate to 4 channels.
    let alpha_4 = {
        let mut v = Vector::<Mat>::new();
        v.push(alpha_f.clone());
        v.push(alpha_f.clone());
        v.push(alpha_f.clone());
        v.push(alpha_f);
        let mut m = Mat::default();
        core::merge(&v, &mut m).map_err(opfail("watermark_image"))?;
        m
    };

    let mut dst_f = Mat::default();
    dst_roi
        .convert_to(&mut dst_f, CV_32FC4, 1.0, 0.0)
        .map_err(opfail("watermark_image"))?;
    let mut src_f = Mat::default();
    src_view
        .convert_to(&mut src_f, CV_32FC4, 1.0, 0.0)
        .map_err(opfail("watermark_image"))?;

    // result = dst*(1-α) + src*α
    let mut one_minus = Mat::default();
    core::subtract(&Scalar::all(1.0), &alpha_4, &mut one_minus, &Mat::default(), -1)
        .map_err(opfail("watermark_image"))?;
    let mut a = Mat::default();
    core::multiply(&dst_f, &one_minus, &mut a, 1.0, -1).map_err(opfail("watermark_image"))?;
    let mut b = Mat::default();
    core::multiply(&src_f, &alpha_4, &mut b, 1.0, -1).map_err(opfail("watermark_image"))?;
    let mut sum = Mat::default();
    core::add(&a, &b, &mut sum, &Mat::default(), -1).map_err(opfail("watermark_image"))?;

    let mut out = Mat::default();
    sum.convert_to(&mut out, opencv::core::CV_8UC4, 1.0, 0.0)
        .map_err(opfail("watermark_image"))?;
    out.copy_to(&mut dst_roi).map_err(opfail("watermark_image"))?;
    Ok(())
}

// ─── Watermark (text — CJK via ab_glyph) ─────────────────────────────────────

fn watermark_text(
    mat: &mut Mat,
    spec: &WatermarkTextSpec,
    fonts: &dyn FontProvider,
) -> Result<(), DomainError> {
    let handle = fonts
        .font(&spec.font)
        .or_else(|_| fonts.default_font())?;
    let concrete = handle
        .as_any()
        .downcast_ref::<AbGlyphFontHandle>()
        .ok_or_else(|| DomainError::Internal("font handle is not ab_glyph".into()))?;
    let font = &concrete.font;

    let (text_w, text_h) = measure_text(font, spec.size, &spec.text);
    if text_w == 0 || text_h == 0 {
        return Ok(());
    }

    ensure_bgra(mat)?;
    let img_w = mat.cols();
    let img_h = mat.rows();
    let stride = (img_w * 4) as usize;

    let (origin_x, origin_y) = anchor_xy(
        spec.position,
        img_w,
        img_h,
        text_w,
        text_h,
        spec.margin as i32,
    );

    let data = mat
        .data_bytes_mut()
        .map_err(opfail("watermark_text"))?;

    if spec.shadow {
        let shadow = Color::rgba(0, 0, 0, ((spec.color.a as u16 * 180) / 255) as u8);
        rasterize_text(
            data, stride, img_w, img_h, &spec.text, font, spec.size,
            origin_x + 2, origin_y + 2, shadow,
        );
    }
    rasterize_text(
        data, stride, img_w, img_h, &spec.text, font, spec.size,
        origin_x, origin_y, spec.color,
    );
    Ok(())
}

fn measure_text(font: &FontArc, px: f32, text: &str) -> (i32, i32) {
    let scaled = font.as_scaled(PxScale::from(px));
    let mut width = 0.0_f32;
    let mut last: Option<ab_glyph::GlyphId> = None;
    for c in text.chars() {
        let g = scaled.scaled_glyph(c);
        if let Some(prev) = last {
            width += scaled.kern(prev, g.id);
        }
        width += scaled.h_advance(g.id);
        last = Some(g.id);
    }
    let height = (scaled.ascent() + scaled.descent().abs()).ceil() as i32;
    (width.ceil() as i32, height)
}

#[allow(clippy::too_many_arguments)]
fn rasterize_text(
    data: &mut [u8],
    stride: usize,
    img_w: i32,
    img_h: i32,
    text: &str,
    font: &FontArc,
    px: f32,
    origin_x: i32,
    origin_y: i32,
    color: Color,
) {
    let scaled = font.as_scaled(PxScale::from(px));
    let ascent = scaled.ascent();
    let mut caret = 0.0_f32;
    let mut last_id: Option<ab_glyph::GlyphId> = None;
    for c in text.chars() {
        let g = scaled.scaled_glyph(c);
        if let Some(prev) = last_id {
            caret += scaled.kern(prev, g.id);
        }
        let advance = scaled.h_advance(g.id);
        let glyph_id = g.id;
        if let Some(outlined) = scaled.outline_glyph(g) {
            let bb = outlined.px_bounds();
            outlined.draw(|gx, gy, v| {
                let px_i = origin_x + (caret + bb.min.x) as i32 + gx as i32;
                let py_i = origin_y + (ascent + bb.min.y) as i32 + gy as i32;
                if px_i < 0 || py_i < 0 || px_i >= img_w || py_i >= img_h {
                    return;
                }
                let idx = py_i as usize * stride + px_i as usize * 4;
                let src_a = v * (color.a as f32 / 255.0);
                if src_a <= 0.0 {
                    return;
                }
                let inv = 1.0 - src_a;
                data[idx] = (data[idx] as f32 * inv + color.b as f32 * src_a) as u8;
                data[idx + 1] =
                    (data[idx + 1] as f32 * inv + color.g as f32 * src_a) as u8;
                data[idx + 2] =
                    (data[idx + 2] as f32 * inv + color.r as f32 * src_a) as u8;
                let dst_a = data[idx + 3] as f32 / 255.0;
                let out_a = src_a + dst_a * (1.0 - src_a);
                data[idx + 3] = (out_a * 255.0).clamp(0.0, 255.0) as u8;
            });
        }
        caret += advance;
        last_id = Some(glyph_id);
    }
}

// ─── Anchor positioning ──────────────────────────────────────────────────────

fn anchor_xy(
    anchor: Anchor,
    img_w: i32,
    img_h: i32,
    w: i32,
    h: i32,
    margin: i32,
) -> (i32, i32) {
    let m = margin;
    match anchor {
        Anchor::TopLeft => (m, m),
        Anchor::Top => ((img_w - w) / 2, m),
        Anchor::TopRight => (img_w - w - m, m),
        Anchor::Left => (m, (img_h - h) / 2),
        Anchor::Center => ((img_w - w) / 2, (img_h - h) / 2),
        Anchor::Right => (img_w - w - m, (img_h - h) / 2),
        Anchor::BottomLeft => (m, img_h - h - m),
        Anchor::Bottom => ((img_w - w) / 2, img_h - h - m),
        Anchor::BottomRight => (img_w - w - m, img_h - h - m),
    }
}
