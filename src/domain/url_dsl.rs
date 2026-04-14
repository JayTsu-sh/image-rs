//! OSS-style URL DSL parser.
//!
//! Format: ops are slash-separated; each op is `name,k_v,k_v,...` where
//! `k_v` is a key/value pair using `_` as the separator. The grammar is
//! intentionally restrictive so the result can be safely used as a cache
//! key after normalization.
//!
//! Examples:
//! * `resize,w_800,m_fit`
//! * `resize,w_400,h_300,m_fill/blur,s_2.0/format,f_webp,q_85`
//! * `rotate,a_90/round,r_24/format,f_png`
//! * `text,t_hello,c_ffffffff,p_br,s_24/format,f_png`
//!
//! Note: image-watermark cannot be expressed in URL DSL because it requires
//! binary asset upload. Use `POST /v1/process` for that.

use std::collections::HashMap;

use crate::domain::error::DomainError;
use crate::domain::ops::*;
use crate::domain::pipeline::{Compression, OutputSpec};
use crate::domain::value_objects::*;

pub fn parse(input: &str) -> Result<(Vec<Op>, OutputSpec), DomainError> {
    let mut ops = Vec::new();
    let mut output = OutputSpec::default();

    for part in input.split('/').filter(|p| !p.is_empty()) {
        let mut tokens = part.split(',');
        let name = tokens
            .next()
            .ok_or_else(|| DomainError::invalid("empty op segment"))?;
        let params: HashMap<&str, &str> = tokens
            .filter(|t| !t.is_empty())
            .map(|t| t.split_once('_').unwrap_or((t, "")))
            .collect();

        match name {
            "resize" => ops.push(parse_resize(&params)?),
            "rotate" => ops.push(parse_rotate(&params)?),
            "crop" => ops.push(parse_crop(&params)?),
            "blur" => ops.push(parse_blur(&params)?),
            "sharpen" => ops.push(parse_sharpen(&params)?),
            "round" => ops.push(parse_round(&params)?),
            "brightness" => ops.push(parse_brightness(&params)?),
            "contrast" => ops.push(parse_contrast(&params)?),
            "saturation" => ops.push(parse_saturation(&params)?),
            "temperature" => ops.push(parse_temperature(&params)?),
            "auto_orient" => ops.push(Op::AutoOrient),
            "text" => ops.push(parse_text(&params)?),
            "format" => output = parse_output(&params)?,
            other => return Err(DomainError::invalid(format!("unknown op: {other}"))),
        }
    }
    Ok((ops, output))
}

// ─── parsers ─────────────────────────────────────────────────────────────────

fn parse_u32(p: &HashMap<&str, &str>, key: &str) -> Result<Option<u32>, DomainError> {
    p.get(key)
        .map(|s| s.parse::<u32>().map_err(|_| DomainError::invalid(format!("bad {key}"))))
        .transpose()
}
fn parse_f32(p: &HashMap<&str, &str>, key: &str) -> Result<Option<f32>, DomainError> {
    p.get(key)
        .map(|s| s.parse::<f32>().map_err(|_| DomainError::invalid(format!("bad {key}"))))
        .transpose()
}
fn parse_f64(p: &HashMap<&str, &str>, key: &str) -> Result<Option<f64>, DomainError> {
    p.get(key)
        .map(|s| s.parse::<f64>().map_err(|_| DomainError::invalid(format!("bad {key}"))))
        .transpose()
}
fn parse_i32(p: &HashMap<&str, &str>, key: &str) -> Result<Option<i32>, DomainError> {
    p.get(key)
        .map(|s| s.parse::<i32>().map_err(|_| DomainError::invalid(format!("bad {key}"))))
        .transpose()
}

fn parse_resize(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let width = parse_u32(p, "w")?;
    let height = parse_u32(p, "h")?;
    let mode = match p.get("m").copied() {
        Some("exact") => ResizeMode::Exact,
        Some("fill") => ResizeMode::Fill,
        Some("fit") | None => ResizeMode::Fit,
        Some(other) => return Err(DomainError::invalid(format!("bad mode: {other}"))),
    };
    let interpolation = match p.get("i").copied() {
        Some(s) => Interpolation::parse(s)?,
        None => Interpolation::Auto,
    };
    Ok(Op::Resize(ResizeSpec::with_interpolation(
        width,
        height,
        mode,
        interpolation,
    )?))
}

fn parse_rotate(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let angle = parse_f64(p, "a")?
        .ok_or_else(|| DomainError::invalid("rotate needs a (angle)"))?;
    let bg = p
        .get("bg")
        .map(|s| Color::parse_hex(s))
        .transpose()?
        .unwrap_or(Color::rgba(0, 0, 0, 0));
    Ok(Op::Rotate(RotateSpec::new(Angle::degrees(angle)?, bg)))
}

fn parse_crop(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let x = parse_u32(p, "x")?.ok_or_else(|| DomainError::invalid("crop needs x"))?;
    let y = parse_u32(p, "y")?.ok_or_else(|| DomainError::invalid("crop needs y"))?;
    let w = parse_u32(p, "w")?.ok_or_else(|| DomainError::invalid("crop needs w"))?;
    let h = parse_u32(p, "h")?.ok_or_else(|| DomainError::invalid("crop needs h"))?;
    Ok(Op::Crop(CropSpec::new(Rect::new(x, y, w, h)?)))
}

fn parse_blur(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let sigma = parse_f32(p, "s")?.ok_or_else(|| DomainError::invalid("blur needs s"))?;
    Ok(Op::Blur(BlurSpec::new(sigma)?))
}

fn parse_sharpen(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let amount = parse_f32(p, "a")?.ok_or_else(|| DomainError::invalid("sharpen needs a"))?;
    let radius = parse_f32(p, "r")?.unwrap_or(1.0);
    Ok(Op::Sharpen(SharpenSpec::new(amount, radius)?))
}

fn parse_round(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let radius = parse_u32(p, "r")?.ok_or_else(|| DomainError::invalid("round needs r"))?;
    Ok(Op::RoundCorner(RoundCornerSpec::new(radius)?))
}

fn parse_brightness(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let value = parse_i32(p, "v")?.ok_or_else(|| DomainError::invalid("brightness needs v"))?;
    Ok(Op::Brightness(BrightnessSpec::new(value)?))
}

fn parse_contrast(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let value = parse_f32(p, "v")?.ok_or_else(|| DomainError::invalid("contrast needs v"))?;
    Ok(Op::Contrast(ContrastSpec::new(value)?))
}

fn parse_saturation(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let factor = parse_f32(p, "f")?.ok_or_else(|| DomainError::invalid("saturation needs f"))?;
    Ok(Op::Saturation(SaturationSpec::new(factor)?))
}

fn parse_temperature(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let value = parse_i32(p, "v")?.ok_or_else(|| DomainError::invalid("temperature needs v"))?;
    Ok(Op::Temperature(TemperatureSpec::new(value)?))
}

fn parse_text(p: &HashMap<&str, &str>) -> Result<Op, DomainError> {
    let text = p.get("t")
        .ok_or_else(|| DomainError::invalid("text needs t (text)"))?
        .to_string();
    let font = p.get("f").copied().unwrap_or("default").to_string();
    let size = parse_f32(p, "s")?.unwrap_or(24.0);
    let color = p.get("c").map(|s| Color::parse_hex(s)).transpose()?
        .unwrap_or(Color::WHITE);
    let position = p.get("p").map(|s| parse_anchor_short(s)).transpose()?
        .unwrap_or(Anchor::BottomRight);
    let margin = parse_u32(p, "m")?.unwrap_or(16);
    let shadow = p.get("sh").map(|s| s == &"1" || s == &"true").unwrap_or(false);
    Ok(Op::WatermarkText(WatermarkTextSpec::new(
        text, font, size, color, position, margin, shadow,
    )?))
}

fn parse_output(p: &HashMap<&str, &str>) -> Result<OutputSpec, DomainError> {
    let format = p.get("f").map(|s| ImageFormat::parse(s)).transpose()?;
    // `l_1` / `l_true` → lossless. When absent, fall back to `q_N` (or
    // default quality).
    let lossless = p.get("l").map(|s| s == &"1" || s == &"true").unwrap_or(false);
    let compression = if lossless {
        Compression::Lossless
    } else {
        let quality = match parse_u32(p, "q")? {
            Some(q) => Quality::new(q as u8)?,
            None => Quality::default(),
        };
        Compression::Lossy(quality)
    };
    let progressive = p.get("p").map(|s| s == &"1" || s == &"true").unwrap_or(false);
    Ok(OutputSpec::new(format, compression, progressive))
}

fn parse_anchor_short(s: &str) -> Result<Anchor, DomainError> {
    Ok(match s {
        "tl" => Anchor::TopLeft,
        "t" => Anchor::Top,
        "tr" => Anchor::TopRight,
        "l" => Anchor::Left,
        "c" => Anchor::Center,
        "r" => Anchor::Right,
        "bl" => Anchor::BottomLeft,
        "b" => Anchor::Bottom,
        "br" => Anchor::BottomRight,
        other => return Err(DomainError::invalid(format!("bad anchor: {other}"))),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_simple_resize() {
        let (ops, _) = parse("resize,w_800").unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Op::Resize(_)));
    }

    #[test]
    fn parses_chain_with_format() {
        let (ops, output) =
            parse("resize,w_400,h_300,m_fill/blur,s_2.0/format,f_webp,q_85").unwrap();
        assert_eq!(ops.len(), 2);
        assert_eq!(output.format, Some(ImageFormat::WebP));
        match output.compression {
            Compression::Lossy(q) => assert_eq!(q.value(), 85),
            Compression::Lossless => panic!("expected lossy"),
        }
    }

    #[test]
    fn parses_lossless_webp() {
        let (_, output) = parse("format,f_webp,l_1").unwrap();
        assert_eq!(output.format, Some(ImageFormat::WebP));
        assert_eq!(output.compression, Compression::Lossless);
    }

    #[test]
    fn lossless_flag_overrides_quality() {
        // l_1 wins over q_50
        let (_, output) = parse("format,f_webp,q_50,l_1").unwrap();
        assert_eq!(output.compression, Compression::Lossless);
    }

    #[test]
    fn parses_text_watermark() {
        let (ops, _) = parse("text,t_hello,s_18,c_ff0000ff,p_br/format,f_png").unwrap();
        assert_eq!(ops.len(), 1);
        assert!(matches!(ops[0], Op::WatermarkText(_)));
    }

    #[test]
    fn rejects_unknown_op() {
        assert!(parse("levitate,a_45").is_err());
    }

    #[test]
    fn rejects_resize_with_no_dimensions() {
        assert!(parse("resize,m_fit").is_err());
    }

    #[test]
    fn empty_input_is_empty_pipeline() {
        let (ops, _) = parse("").unwrap();
        assert!(ops.is_empty());
    }
}
