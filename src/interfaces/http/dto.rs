//! HTTP DTOs and the anti-corruption layer that converts them into domain
//! value objects. Domain types stay free of `serde`.

use serde::Deserialize;

use crate::domain::error::DomainError;
use crate::domain::ops::*;
use crate::domain::pipeline::{Compression, OutputSpec};
use crate::domain::value_objects::*;

// ─── Output ──────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Deserialize)]
pub struct OutputDto {
    pub format: Option<String>,
    pub quality: Option<u8>,
    /// When true the output is encoded losslessly. Only valid for PNG
    /// (always) and WebP. JPEG + lossless is rejected by `Pipeline::new`.
    pub lossless: Option<bool>,
    pub progressive: Option<bool>,
}

impl TryFrom<OutputDto> for OutputSpec {
    type Error = DomainError;
    fn try_from(d: OutputDto) -> Result<Self, Self::Error> {
        let format = match d.format.as_deref() {
            Some(s) => Some(ImageFormat::parse(s)?),
            None => None,
        };
        let compression = if d.lossless.unwrap_or(false) {
            Compression::Lossless
        } else {
            let quality = match d.quality {
                Some(q) => Quality::new(q)?,
                None => Quality::default(),
            };
            Compression::Lossy(quality)
        };
        Ok(OutputSpec::new(
            format,
            compression,
            d.progressive.unwrap_or(false),
        ))
    }
}

// ─── Anchor / ResizeMode (shared with query handlers) ────────────────────────

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AnchorDto {
    TopLeft,
    Top,
    TopRight,
    Left,
    Center,
    Right,
    BottomLeft,
    Bottom,
    BottomRight,
}

impl From<AnchorDto> for Anchor {
    fn from(d: AnchorDto) -> Self {
        match d {
            AnchorDto::TopLeft => Anchor::TopLeft,
            AnchorDto::Top => Anchor::Top,
            AnchorDto::TopRight => Anchor::TopRight,
            AnchorDto::Left => Anchor::Left,
            AnchorDto::Center => Anchor::Center,
            AnchorDto::Right => Anchor::Right,
            AnchorDto::BottomLeft => Anchor::BottomLeft,
            AnchorDto::Bottom => Anchor::Bottom,
            AnchorDto::BottomRight => Anchor::BottomRight,
        }
    }
}

pub fn parse_anchor(s: &str) -> Result<Anchor, DomainError> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "top_left" | "topleft" | "tl" => Anchor::TopLeft,
        "top" | "t" => Anchor::Top,
        "top_right" | "topright" | "tr" => Anchor::TopRight,
        "left" | "l" => Anchor::Left,
        "center" | "c" => Anchor::Center,
        "right" | "r" => Anchor::Right,
        "bottom_left" | "bottomleft" | "bl" => Anchor::BottomLeft,
        "bottom" | "b" => Anchor::Bottom,
        "bottom_right" | "bottomright" | "br" => Anchor::BottomRight,
        other => return Err(DomainError::invalid(format!("invalid anchor: {other}"))),
    })
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResizeModeDto {
    Exact,
    Fit,
    Fill,
}

impl From<ResizeModeDto> for ResizeMode {
    fn from(d: ResizeModeDto) -> Self {
        match d {
            ResizeModeDto::Exact => ResizeMode::Exact,
            ResizeModeDto::Fit => ResizeMode::Fit,
            ResizeModeDto::Fill => ResizeMode::Fill,
        }
    }
}

pub fn parse_resize_mode(s: &str) -> Result<ResizeMode, DomainError> {
    Ok(match s.to_ascii_lowercase().as_str() {
        "exact" => ResizeMode::Exact,
        "fit" => ResizeMode::Fit,
        "fill" => ResizeMode::Fill,
        other => return Err(DomainError::invalid(format!("invalid mode: {other}"))),
    })
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InterpolationDto {
    Auto,
    Nearest,
    Linear,
    Cubic,
    Area,
    Lanczos4,
}

impl From<InterpolationDto> for Interpolation {
    fn from(d: InterpolationDto) -> Self {
        match d {
            InterpolationDto::Auto => Interpolation::Auto,
            InterpolationDto::Nearest => Interpolation::Nearest,
            InterpolationDto::Linear => Interpolation::Linear,
            InterpolationDto::Cubic => Interpolation::Cubic,
            InterpolationDto::Area => Interpolation::Area,
            InterpolationDto::Lanczos4 => Interpolation::Lanczos4,
        }
    }
}

// ─── OpDto ───────────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum OpDto {
    Resize {
        width: Option<u32>,
        height: Option<u32>,
        mode: Option<ResizeModeDto>,
        interpolation: Option<InterpolationDto>,
    },
    Rotate {
        angle: f64,
        background: Option<String>,
    },
    Crop {
        x: u32,
        y: u32,
        width: u32,
        height: u32,
    },
    Blur {
        sigma: f32,
    },
    Sharpen {
        amount: f32,
        radius: Option<f32>,
    },
    RoundCorner {
        radius: u32,
    },
    Brightness {
        value: i32,
    },
    Contrast {
        value: f32,
    },
    Saturation {
        factor: f32,
    },
    Temperature {
        value: i32,
    },
    AutoOrient,
    WatermarkImage {
        asset: String,
        position: Option<AnchorDto>,
        opacity: Option<f32>,
        margin: Option<u32>,
        scale: Option<f32>,
    },
    WatermarkText {
        text: String,
        font: Option<String>,
        size: Option<f32>,
        color: Option<String>,
        position: Option<AnchorDto>,
        margin: Option<u32>,
        shadow: Option<bool>,
    },
}

impl TryFrom<OpDto> for Op {
    type Error = DomainError;
    fn try_from(dto: OpDto) -> Result<Self, Self::Error> {
        Ok(match dto {
            OpDto::Resize {
                width,
                height,
                mode,
                interpolation,
            } => Op::Resize(ResizeSpec::with_interpolation(
                width,
                height,
                mode.map(Into::into).unwrap_or(ResizeMode::Fit),
                interpolation.map(Into::into).unwrap_or(Interpolation::Auto),
            )?),
            OpDto::Rotate { angle, background } => {
                let bg = match background.as_deref() {
                    Some(s) => Color::parse_hex(s)?,
                    None => Color::rgba(0, 0, 0, 0),
                };
                Op::Rotate(RotateSpec::new(Angle::degrees(angle)?, bg))
            }
            OpDto::Crop { x, y, width, height } => {
                Op::Crop(CropSpec::new(Rect::new(x, y, width, height)?))
            }
            OpDto::Blur { sigma } => Op::Blur(BlurSpec::new(sigma)?),
            OpDto::Sharpen { amount, radius } => {
                Op::Sharpen(SharpenSpec::new(amount, radius.unwrap_or(1.0))?)
            }
            OpDto::RoundCorner { radius } => Op::RoundCorner(RoundCornerSpec::new(radius)?),
            OpDto::Brightness { value } => Op::Brightness(BrightnessSpec::new(value)?),
            OpDto::Contrast { value } => Op::Contrast(ContrastSpec::new(value)?),
            OpDto::Saturation { factor } => Op::Saturation(SaturationSpec::new(factor)?),
            OpDto::Temperature { value } => Op::Temperature(TemperatureSpec::new(value)?),
            OpDto::AutoOrient => Op::AutoOrient,
            OpDto::WatermarkImage {
                asset,
                position,
                opacity,
                margin,
                scale,
            } => Op::WatermarkImage(WatermarkImageSpec::new(
                asset,
                position.map(Into::into).unwrap_or(Anchor::BottomRight),
                Opacity::new(opacity.unwrap_or(1.0))?,
                margin.unwrap_or(16),
                scale.unwrap_or(0.2),
            )?),
            OpDto::WatermarkText {
                text,
                font,
                size,
                color,
                position,
                margin,
                shadow,
            } => {
                let color = match color.as_deref() {
                    Some(s) => Color::parse_hex(s)?,
                    None => Color::WHITE,
                };
                Op::WatermarkText(WatermarkTextSpec::new(
                    text,
                    font.unwrap_or_else(|| "default".to_string()),
                    size.unwrap_or(24.0),
                    color,
                    position.map(Into::into).unwrap_or(Anchor::BottomRight),
                    margin.unwrap_or(16),
                    shadow.unwrap_or(false),
                )?)
            }
        })
    }
}
