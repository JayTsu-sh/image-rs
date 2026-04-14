//! Operation value objects.
//!
//! Each variant is a fully-validated specification — building one is the
//! single point at which inputs become trusted. The infrastructure executor
//! pattern-matches and dispatches.

use crate::domain::error::DomainError;
use crate::domain::value_objects::*;

#[derive(Debug, Clone)]
pub enum Op {
    // ── basic ──────────────────────────────────────────────────────────────
    Resize(ResizeSpec),
    Rotate(RotateSpec),
    Crop(CropSpec),
    // ── effect ─────────────────────────────────────────────────────────────
    Blur(BlurSpec),
    Sharpen(SharpenSpec),
    RoundCorner(RoundCornerSpec),
    Brightness(BrightnessSpec),
    Contrast(ContrastSpec),
    Saturation(SaturationSpec),
    Temperature(TemperatureSpec),
    AutoOrient,
    // ── watermark ──────────────────────────────────────────────────────────
    WatermarkImage(WatermarkImageSpec),
    WatermarkText(WatermarkTextSpec),
}

impl Op {
    pub fn kind(&self) -> OpKind {
        match self {
            Op::Resize(_) => OpKind::Resize,
            Op::Rotate(_) => OpKind::Rotate,
            Op::Crop(_) => OpKind::Crop,
            Op::Blur(_) => OpKind::Blur,
            Op::Sharpen(_) => OpKind::Sharpen,
            Op::RoundCorner(_) => OpKind::RoundCorner,
            Op::Brightness(_) => OpKind::Brightness,
            Op::Contrast(_) => OpKind::Contrast,
            Op::Saturation(_) => OpKind::Saturation,
            Op::Temperature(_) => OpKind::Temperature,
            Op::AutoOrient => OpKind::AutoOrient,
            Op::WatermarkImage(_) => OpKind::WatermarkImage,
            Op::WatermarkText(_) => OpKind::WatermarkText,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpKind {
    Resize,
    Rotate,
    Crop,
    Blur,
    Sharpen,
    RoundCorner,
    Brightness,
    Contrast,
    Saturation,
    Temperature,
    AutoOrient,
    WatermarkImage,
    WatermarkText,
}

impl OpKind {
    pub fn as_str(self) -> &'static str {
        match self {
            OpKind::Resize => "resize",
            OpKind::Rotate => "rotate",
            OpKind::Crop => "crop",
            OpKind::Blur => "blur",
            OpKind::Sharpen => "sharpen",
            OpKind::RoundCorner => "round_corner",
            OpKind::Brightness => "brightness",
            OpKind::Contrast => "contrast",
            OpKind::Saturation => "saturation",
            OpKind::Temperature => "temperature",
            OpKind::AutoOrient => "auto_orient",
            OpKind::WatermarkImage => "watermark_image",
            OpKind::WatermarkText => "watermark_text",
        }
    }
}

// ─── Resize ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResizeMode {
    /// Stretch to exact dimensions, ignoring aspect ratio.
    Exact,
    /// Fit inside the bounding box, preserving aspect ratio.
    Fit,
    /// Cover the bounding box, preserving aspect ratio (excess cropped).
    Fill,
}

#[derive(Debug, Clone, Copy)]
pub struct ResizeSpec {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mode: ResizeMode,
    pub interpolation: Interpolation,
}

impl ResizeSpec {
    /// Backwards-compatible constructor — defaults interpolation to `Auto`.
    pub fn new(
        width: Option<u32>,
        height: Option<u32>,
        mode: ResizeMode,
    ) -> Result<Self, DomainError> {
        Self::with_interpolation(width, height, mode, Interpolation::Auto)
    }

    pub fn with_interpolation(
        width: Option<u32>,
        height: Option<u32>,
        mode: ResizeMode,
        interpolation: Interpolation,
    ) -> Result<Self, DomainError> {
        if width.is_none() && height.is_none() {
            return Err(DomainError::invalid("resize requires width or height"));
        }
        if let Some(w) = width {
            if w == 0 || w > 16384 {
                return Err(DomainError::invalid("resize width out of range"));
            }
        }
        if let Some(h) = height {
            if h == 0 || h > 16384 {
                return Err(DomainError::invalid("resize height out of range"));
            }
        }
        Ok(Self { width, height, mode, interpolation })
    }
}

// ─── Rotate ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct RotateSpec {
    pub angle: Angle,
    pub background: Color,
}

impl RotateSpec {
    pub fn new(angle: Angle, background: Color) -> Self {
        Self { angle, background }
    }
}

// ─── Crop ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct CropSpec {
    pub rect: Rect,
}

impl CropSpec {
    pub fn new(rect: Rect) -> Self { Self { rect } }
}

// ─── Blur ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct BlurSpec {
    /// Gaussian kernel sigma.
    pub sigma: f32,
}

impl BlurSpec {
    pub fn new(sigma: f32) -> Result<Self, DomainError> {
        if !(0.0..=100.0).contains(&sigma) || sigma.is_nan() {
            return Err(DomainError::invalid("blur sigma must be in [0,100]"));
        }
        Ok(Self { sigma })
    }
}

// ─── Sharpen (USM) ───────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct SharpenSpec {
    pub amount: f32,
    pub radius: f32,
}

impl SharpenSpec {
    pub fn new(amount: f32, radius: f32) -> Result<Self, DomainError> {
        if !(0.0..=5.0).contains(&amount) {
            return Err(DomainError::invalid("sharpen amount must be in [0,5]"));
        }
        if !(0.0..=10.0).contains(&radius) {
            return Err(DomainError::invalid("sharpen radius must be in [0,10]"));
        }
        Ok(Self { amount, radius })
    }
}

// ─── RoundCorner ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct RoundCornerSpec {
    pub radius: u32,
}

impl RoundCornerSpec {
    pub fn new(radius: u32) -> Result<Self, DomainError> {
        if radius == 0 || radius > 4096 {
            return Err(DomainError::invalid("round-corner radius out of range"));
        }
        Ok(Self { radius })
    }
}

// ─── Brightness / Contrast ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct BrightnessSpec {
    /// Additive offset in [-255, 255].
    pub value: i32,
}

impl BrightnessSpec {
    pub fn new(value: i32) -> Result<Self, DomainError> {
        if !(-255..=255).contains(&value) {
            return Err(DomainError::invalid("brightness must be in [-255,255]"));
        }
        Ok(Self { value })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ContrastSpec {
    /// Multiplicative gain in [0, 4]. 1.0 = identity.
    pub value: f32,
}

impl ContrastSpec {
    pub fn new(value: f32) -> Result<Self, DomainError> {
        if !(0.0..=4.0).contains(&value) {
            return Err(DomainError::invalid("contrast must be in [0,4]"));
        }
        Ok(Self { value })
    }
}

// ─── Saturation ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct SaturationSpec {
    /// HSV saturation channel multiplier in [0, 4]. 1.0 = identity, 0 = greyscale.
    pub factor: f32,
}

impl SaturationSpec {
    pub fn new(factor: f32) -> Result<Self, DomainError> {
        if !(0.0..=4.0).contains(&factor) || factor.is_nan() {
            return Err(DomainError::invalid("saturation factor must be in [0,4]"));
        }
        Ok(Self { factor })
    }
}

// ─── Temperature ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy)]
pub struct TemperatureSpec {
    /// Warm/cool shift in [-100, 100]. Positive warms (more red, less blue);
    /// negative cools. Implementation is a simple R/B channel scale, not a
    /// real Kelvin/Planck calculation — sufficient for UI sliders.
    pub value: i32,
}

impl TemperatureSpec {
    pub fn new(value: i32) -> Result<Self, DomainError> {
        if !(-100..=100).contains(&value) {
            return Err(DomainError::invalid("temperature must be in [-100,100]"));
        }
        Ok(Self { value })
    }
}

// ─── Watermark image ─────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WatermarkImageSpec {
    /// Logical name of the multipart asset that contains the watermark bytes.
    pub asset: String,
    pub position: Anchor,
    pub opacity: Opacity,
    pub margin: u32,
    pub scale: f32,
}

impl WatermarkImageSpec {
    pub fn new(
        asset: String,
        position: Anchor,
        opacity: Opacity,
        margin: u32,
        scale: f32,
    ) -> Result<Self, DomainError> {
        if asset.is_empty() {
            return Err(DomainError::invalid("watermark asset name required"));
        }
        if !(0.05..=4.0).contains(&scale) {
            return Err(DomainError::invalid("watermark scale must be in [0.05,4]"));
        }
        Ok(Self { asset, position, opacity, margin, scale })
    }
}

// ─── Watermark text ──────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WatermarkTextSpec {
    pub text: String,
    pub font: String,
    pub size: f32,
    pub color: Color,
    pub position: Anchor,
    pub margin: u32,
    pub shadow: bool,
}

impl WatermarkTextSpec {
    pub fn new(
        text: String,
        font: String,
        size: f32,
        color: Color,
        position: Anchor,
        margin: u32,
        shadow: bool,
    ) -> Result<Self, DomainError> {
        if text.is_empty() {
            return Err(DomainError::invalid("watermark text must not be empty"));
        }
        if !(4.0..=512.0).contains(&size) {
            return Err(DomainError::invalid("text size must be in [4,512]"));
        }
        Ok(Self { text, font, size, color, position, margin, shadow })
    }
}
