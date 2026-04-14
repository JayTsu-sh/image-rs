//! Domain value objects. Constructors enforce invariants; no setters.

use crate::domain::error::DomainError;

// ─── Dimensions ──────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Dimensions {
    pub width: u32,
    pub height: u32,
}

impl Dimensions {
    pub fn new(width: u32, height: u32) -> Result<Self, DomainError> {
        if width == 0 || height == 0 {
            return Err(DomainError::invalid("dimensions must be > 0"));
        }
        if width > 16384 || height > 16384 {
            return Err(DomainError::invalid("dimensions exceed 16384"));
        }
        Ok(Self { width, height })
    }
}

// ─── Rect ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl Rect {
    pub fn new(x: u32, y: u32, width: u32, height: u32) -> Result<Self, DomainError> {
        if width == 0 || height == 0 {
            return Err(DomainError::invalid("rect dimensions must be > 0"));
        }
        Ok(Self { x, y, width, height })
    }
}

// ─── Anchor ──────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Anchor {
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

// ─── Color ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Color {
    pub const WHITE: Color = Color { r: 255, g: 255, b: 255, a: 255 };
    pub const BLACK: Color = Color { r: 0, g: 0, b: 0, a: 255 };

    pub fn rgba(r: u8, g: u8, b: u8, a: u8) -> Self {
        Self { r, g, b, a }
    }

    /// Parses `#rgb` / `#rgba` / `#rrggbb` / `#rrggbbaa`.
    pub fn parse_hex(s: &str) -> Result<Self, DomainError> {
        let s = s.strip_prefix('#').unwrap_or(s);
        let parse = |i| u8::from_str_radix(&s[i..i + 2], 16);
        let parse1 = |i| {
            let c = &s[i..i + 1];
            u8::from_str_radix(&format!("{c}{c}"), 16)
        };
        let map_err = |_| DomainError::invalid(format!("invalid color hex: {s}"));
        match s.len() {
            3 => Ok(Self::rgba(
                parse1(0).map_err(map_err)?,
                parse1(1).map_err(map_err)?,
                parse1(2).map_err(map_err)?,
                255,
            )),
            4 => Ok(Self::rgba(
                parse1(0).map_err(map_err)?,
                parse1(1).map_err(map_err)?,
                parse1(2).map_err(map_err)?,
                parse1(3).map_err(map_err)?,
            )),
            6 => Ok(Self::rgba(
                parse(0).map_err(map_err)?,
                parse(2).map_err(map_err)?,
                parse(4).map_err(map_err)?,
                255,
            )),
            8 => Ok(Self::rgba(
                parse(0).map_err(map_err)?,
                parse(2).map_err(map_err)?,
                parse(4).map_err(map_err)?,
                parse(6).map_err(map_err)?,
            )),
            _ => Err(DomainError::invalid(format!("invalid color hex: {s}"))),
        }
    }
}

// ─── Opacity ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Opacity(f32);

impl Opacity {
    pub fn new(v: f32) -> Result<Self, DomainError> {
        if !(0.0..=1.0).contains(&v) || v.is_nan() {
            return Err(DomainError::invalid("opacity must be in [0,1]"));
        }
        Ok(Self(v))
    }
    pub fn value(self) -> f32 { self.0 }
}

// ─── Angle ───────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Angle(f64);

impl Angle {
    pub fn degrees(d: f64) -> Result<Self, DomainError> {
        if d.is_nan() || d.is_infinite() {
            return Err(DomainError::invalid("angle must be finite"));
        }
        Ok(Self(d.rem_euclid(360.0)))
    }
    pub fn as_degrees(self) -> f64 { self.0 }
}

// ─── Quality ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Quality(u8);

impl Quality {
    pub fn new(q: u8) -> Result<Self, DomainError> {
        if q == 0 || q > 100 {
            return Err(DomainError::invalid("quality must be in 1..=100"));
        }
        Ok(Self(q))
    }
    pub fn value(self) -> u8 { self.0 }
}

impl Default for Quality {
    fn default() -> Self { Self(82) }
}

// ─── DiffMode ────────────────────────────────────────────────────────────────

/// How to render the difference between two images.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffMode {
    /// Per-pixel `|before - after|`, returned as an RGB image.
    Abs,
    /// Same as `Abs` but flattened to grayscale (returned as 3-channel for
    /// consistent encoding).
    Grayscale,
    /// Overlay the changed regions in red on top of `before`. Pixels whose
    /// per-channel diff exceeds the threshold are highlighted.
    Highlight,
}

impl DiffMode {
    pub fn parse(s: &str) -> Result<Self, DomainError> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "abs" => DiffMode::Abs,
            "grayscale" | "gray" => DiffMode::Grayscale,
            "highlight" => DiffMode::Highlight,
            other => {
                return Err(DomainError::invalid(format!("invalid diff mode: {other}")));
            }
        })
    }
}

#[derive(Debug, Clone, Copy)]
pub struct DiffSpec {
    pub mode: DiffMode,
    /// Per-channel threshold below which differences are ignored (Highlight
    /// mode only). 0..=255.
    pub threshold: u8,
}

impl DiffSpec {
    pub fn new(mode: DiffMode, threshold: u8) -> Self {
        Self { mode, threshold }
    }
}

// ─── Interpolation ───────────────────────────────────────────────────────────

/// Sampling kernel for resize / warp.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Interpolation {
    /// AREA when shrinking, LINEAR when enlarging — best general default.
    Auto,
    Nearest,
    Linear,
    Cubic,
    Area,
    Lanczos4,
}

impl Default for Interpolation {
    fn default() -> Self {
        Interpolation::Auto
    }
}

impl Interpolation {
    pub fn as_str(self) -> &'static str {
        match self {
            Interpolation::Auto => "auto",
            Interpolation::Nearest => "nearest",
            Interpolation::Linear => "linear",
            Interpolation::Cubic => "cubic",
            Interpolation::Area => "area",
            Interpolation::Lanczos4 => "lanczos4",
        }
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        Ok(match s.to_ascii_lowercase().as_str() {
            "auto" => Interpolation::Auto,
            "nearest" | "nn" => Interpolation::Nearest,
            "linear" | "bilinear" => Interpolation::Linear,
            "cubic" | "bicubic" => Interpolation::Cubic,
            "area" => Interpolation::Area,
            "lanczos4" | "lanczos" => Interpolation::Lanczos4,
            other => {
                return Err(DomainError::invalid(format!(
                    "invalid interpolation: {other}"
                )));
            }
        })
    }
}

// ─── ImageFormat ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Jpeg,
    Png,
    WebP,
}

impl ImageFormat {
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => ".jpg",
            ImageFormat::Png => ".png",
            ImageFormat::WebP => ".webp",
        }
    }

    pub fn content_type(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "image/jpeg",
            ImageFormat::Png => "image/png",
            ImageFormat::WebP => "image/webp",
        }
    }

    pub fn supports_alpha(self) -> bool {
        !matches!(self, ImageFormat::Jpeg)
    }

    pub fn parse(s: &str) -> Result<Self, DomainError> {
        match s.to_ascii_lowercase().as_str() {
            "jpg" | "jpeg" => Ok(Self::Jpeg),
            "png" => Ok(Self::Png),
            "webp" => Ok(Self::WebP),
            other => Err(DomainError::UnsupportedFormat(other.to_string())),
        }
    }
}
