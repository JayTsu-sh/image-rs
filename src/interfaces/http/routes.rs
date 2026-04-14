//! All HTTP handlers. Each per-op endpoint constructs a single-element
//! pipeline and dispatches through the same `ProcessImageService` as the
//! unified `/v1/process` endpoint — single source of truth.

use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderName, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::Deserialize;

use crate::application::diff_image::DiffImageCommand;
use crate::application::get_image::GetImageCommand;
use crate::application::ports::OpContext;
use crate::application::process_image::{ProcessImageCommand, ProcessOutcome};
use crate::domain::error::DomainError;
use crate::domain::ops::*;
use crate::domain::pipeline::{Compression, OutputSpec, Pipeline};
use crate::domain::value_objects::*;
use crate::interfaces::http::dto::{OpDto, OutputDto, parse_anchor, parse_resize_mode};
use crate::interfaces::http::error::HttpError;
use crate::interfaces::http::extract::ImageUpload;
use crate::interfaces::http::state::AppState;

// ─── Health ──────────────────────────────────────────────────────────────────

pub async fn healthz() -> StatusCode {
    StatusCode::OK
}

pub async fn readyz(State(state): State<AppState>) -> StatusCode {
    if state.draining.load(std::sync::atomic::Ordering::Relaxed) {
        return StatusCode::SERVICE_UNAVAILABLE;
    }
    if state.limiter.available() == 0 {
        return StatusCode::TOO_MANY_REQUESTS;
    }
    StatusCode::OK
}

pub async fn metrics(State(state): State<AppState>) -> Response {
    (
        [(header::CONTENT_TYPE, "text/plain; version=0.0.4")],
        state.metrics.render(),
    )
        .into_response()
}

// ─── Shared response helper ──────────────────────────────────────────────────

const X_IMAGE_WIDTH: HeaderName = HeaderName::from_static("x-image-width");
const X_IMAGE_HEIGHT: HeaderName = HeaderName::from_static("x-image-height");
const X_IMAGE_BYTES: HeaderName = HeaderName::from_static("x-image-bytes");
const X_PROCESS_TIME_MS: HeaderName = HeaderName::from_static("x-process-time-ms");
const X_IMAGE_CACHE: HeaderName = HeaderName::from_static("x-image-cache");

fn into_image_response(outcome: ProcessOutcome) -> Response {
    let bytes_len = outcome.encoded.bytes.len();
    (
        [
            (header::CONTENT_TYPE, outcome.encoded.format.content_type().to_string()),
            (X_IMAGE_WIDTH, outcome.encoded.width.to_string()),
            (X_IMAGE_HEIGHT, outcome.encoded.height.to_string()),
            (X_IMAGE_BYTES, bytes_len.to_string()),
            (X_PROCESS_TIME_MS, outcome.elapsed_ms.to_string()),
        ],
        outcome.encoded.bytes,
    )
        .into_response()
}

async fn run(
    state: &AppState,
    upload: ImageUpload,
    ops: Vec<Op>,
    output: OutputSpec,
) -> Result<Response, HttpError> {
    let primary = upload.primary_required()?;
    let pipeline = Pipeline::new(ops, output)?;
    let cmd = ProcessImageCommand {
        image: primary,
        pipeline,
        context: OpContext::new(upload.assets),
    };
    let outcome = state.service.execute(cmd).await?;
    Ok(into_image_response(outcome))
}

// ─── Shared output query (used by every per-op endpoint) ─────────────────────

#[derive(Debug, Default, Deserialize)]
pub struct OutputQuery {
    pub format: Option<String>,
    pub quality: Option<u8>,
    /// `?lossless=true` forces the WebP / PNG encoder into its lossless
    /// path. Ignored for PNG (always lossless). Rejected for JPEG.
    pub lossless: Option<bool>,
    pub progressive: Option<bool>,
}

impl OutputQuery {
    fn into_spec(self) -> Result<OutputSpec, DomainError> {
        let format = match self.format.as_deref() {
            Some(s) => Some(ImageFormat::parse(s)?),
            None => None,
        };
        let compression = if self.lossless.unwrap_or(false) {
            Compression::Lossless
        } else {
            let quality = match self.quality {
                Some(q) => Quality::new(q)?,
                None => Quality::default(),
            };
            Compression::Lossy(quality)
        };
        Ok(OutputSpec::new(
            format,
            compression,
            self.progressive.unwrap_or(false),
        ))
    }
}

// ─── /v1/process — unified pipeline ──────────────────────────────────────────

pub async fn process(
    State(state): State<AppState>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let ops_dto: Vec<OpDto> = match &upload.ops_json {
        Some(b) => serde_json::from_slice(b)
            .map_err(|e| HttpError(DomainError::invalid(format!("invalid ops json: {e}"))))?,
        None => Vec::new(),
    };
    let output: OutputSpec = match &upload.output_json {
        Some(b) => {
            let dto: OutputDto = serde_json::from_slice(b)
                .map_err(|e| HttpError(DomainError::invalid(format!("invalid output json: {e}"))))?;
            dto.try_into()?
        }
        None => OutputSpec::default(),
    };
    let ops: Vec<Op> = ops_dto
        .into_iter()
        .map(Op::try_from)
        .collect::<Result<_, _>>()?;
    run(&state, upload, ops, output).await
}

// ─── /v1/basic/* ─────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct ResizeQuery {
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub mode: Option<String>,
    pub interpolation: Option<String>,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn resize(
    State(state): State<AppState>,
    Query(q): Query<ResizeQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let mode = match q.mode.as_deref() {
        Some(s) => parse_resize_mode(s)?,
        None => ResizeMode::Fit,
    };
    let interpolation = match q.interpolation.as_deref() {
        Some(s) => Interpolation::parse(s)?,
        None => Interpolation::Auto,
    };
    let op = Op::Resize(ResizeSpec::with_interpolation(
        q.width,
        q.height,
        mode,
        interpolation,
    )?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct RotateQuery {
    pub angle: f64,
    pub background: Option<String>,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn rotate(
    State(state): State<AppState>,
    Query(q): Query<RotateQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let bg = match q.background.as_deref() {
        Some(s) => Color::parse_hex(s)?,
        None => Color::rgba(0, 0, 0, 0),
    };
    let op = Op::Rotate(RotateSpec::new(Angle::degrees(q.angle)?, bg));
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct CropQuery {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn crop(
    State(state): State<AppState>,
    Query(q): Query<CropQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let op = Op::Crop(CropSpec::new(Rect::new(q.x, q.y, q.width, q.height)?));
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

pub async fn format(
    State(state): State<AppState>,
    Query(q): Query<OutputQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    if q.format.is_none() {
        return Err(HttpError(DomainError::invalid(
            "format query parameter is required",
        )));
    }
    run(&state, upload, vec![], q.into_spec()?).await
}

// ─── /v1/watermark/* ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct WatermarkImageQuery {
    pub asset: Option<String>,
    pub position: Option<String>,
    pub opacity: Option<f32>,
    pub margin: Option<u32>,
    pub scale: Option<f32>,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn watermark_image(
    State(state): State<AppState>,
    Query(q): Query<WatermarkImageQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let asset = q.asset.unwrap_or_else(|| "watermark".to_string());
    if !upload.assets.contains_key(&asset) {
        return Err(HttpError(DomainError::MissingAsset(asset)));
    }
    let position = match q.position.as_deref() {
        Some(s) => parse_anchor(s)?,
        None => Anchor::BottomRight,
    };
    let op = Op::WatermarkImage(WatermarkImageSpec::new(
        asset,
        position,
        Opacity::new(q.opacity.unwrap_or(1.0))?,
        q.margin.unwrap_or(16),
        q.scale.unwrap_or(0.2),
    )?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct WatermarkTextQuery {
    pub text: String,
    pub font: Option<String>,
    pub size: Option<f32>,
    pub color: Option<String>,
    pub position: Option<String>,
    pub margin: Option<u32>,
    pub shadow: Option<bool>,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn watermark_text(
    State(state): State<AppState>,
    Query(q): Query<WatermarkTextQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let color = match q.color.as_deref() {
        Some(s) => Color::parse_hex(s)?,
        None => Color::WHITE,
    };
    let position = match q.position.as_deref() {
        Some(s) => parse_anchor(s)?,
        None => Anchor::BottomRight,
    };
    let op = Op::WatermarkText(WatermarkTextSpec::new(
        q.text,
        q.font.unwrap_or_else(|| "default".to_string()),
        q.size.unwrap_or(24.0),
        color,
        position,
        q.margin.unwrap_or(16),
        q.shadow.unwrap_or(false),
    )?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

// ─── /v1/effect/* ────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
pub struct BlurQuery {
    pub sigma: f32,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn blur(
    State(state): State<AppState>,
    Query(q): Query<BlurQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let op = Op::Blur(BlurSpec::new(q.sigma)?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct SharpenQuery {
    pub amount: f32,
    pub radius: Option<f32>,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn sharpen(
    State(state): State<AppState>,
    Query(q): Query<SharpenQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let op = Op::Sharpen(SharpenSpec::new(q.amount, q.radius.unwrap_or(1.0))?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct RoundCornerQuery {
    pub radius: u32,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn round_corner(
    State(state): State<AppState>,
    Query(q): Query<RoundCornerQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let op = Op::RoundCorner(RoundCornerSpec::new(q.radius)?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct ContrastQuery {
    pub value: f32,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn contrast(
    State(state): State<AppState>,
    Query(q): Query<ContrastQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let op = Op::Contrast(ContrastSpec::new(q.value)?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct BrightnessQuery {
    pub value: i32,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn brightness(
    State(state): State<AppState>,
    Query(q): Query<BrightnessQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let op = Op::Brightness(BrightnessSpec::new(q.value)?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct SaturationQuery {
    pub factor: f32,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn saturation(
    State(state): State<AppState>,
    Query(q): Query<SaturationQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let op = Op::Saturation(SaturationSpec::new(q.factor)?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

#[derive(Debug, Deserialize)]
pub struct TemperatureQuery {
    pub value: i32,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn temperature(
    State(state): State<AppState>,
    Query(q): Query<TemperatureQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    let op = Op::Temperature(TemperatureSpec::new(q.value)?);
    run(&state, upload, vec![op], q.output.into_spec()?).await
}

pub async fn progressive(
    State(state): State<AppState>,
    Query(mut q): Query<OutputQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    q.progressive = Some(true);
    run(&state, upload, vec![], q.into_spec()?).await
}

pub async fn auto_orient(
    State(state): State<AppState>,
    Query(q): Query<OutputQuery>,
    upload: ImageUpload,
) -> Result<Response, HttpError> {
    run(&state, upload, vec![Op::AutoOrient], q.into_spec()?).await
}

// ─── POST /v1/diff — pixel diff between two images ──────────────────────────

#[derive(Debug, Deserialize)]
pub struct DiffQuery {
    pub mode: Option<String>,
    pub threshold: Option<u8>,
    #[serde(flatten)]
    pub output: OutputQuery,
}

pub async fn diff(
    State(state): State<AppState>,
    Query(q): Query<DiffQuery>,
    mut upload: ImageUpload,
) -> Result<Response, HttpError> {
    // The diff endpoint expects two named multipart fields: `before` and
    // `after`. The default `file` field is unused, but we still allow it to
    // be sent (some HTTP clients always include `file`).
    let before = upload
        .take_named("before")
        .or_else(|| upload.primary.clone())
        .ok_or_else(|| HttpError(DomainError::invalid("missing 'before' field")))?;
    let after = upload
        .take_named("after")
        .ok_or_else(|| HttpError(DomainError::invalid("missing 'after' field")))?;

    let mode = match q.mode.as_deref() {
        Some(s) => DiffMode::parse(s)?,
        None => DiffMode::Highlight,
    };
    let threshold = q.threshold.unwrap_or(10);
    let spec = DiffSpec::new(mode, threshold);

    let outcome = state
        .diff_service
        .execute(DiffImageCommand {
            before,
            after,
            spec,
            output: q.output.into_spec()?,
        })
        .await?;
    Ok(into_image_response(outcome))
}

// ─── GET /v1/img/{key}?p=DSL — cached, ETag-aware ────────────────────────────

#[derive(Debug, Deserialize)]
pub struct GetImageQuery {
    /// OSS-style DSL: `resize,w_800/blur,s_2/format,f_webp,q_85`
    #[serde(default)]
    pub p: String,
}

pub async fn get_image(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Query(q): Query<GetImageQuery>,
    headers: HeaderMap,
) -> Result<Response, HttpError> {
    let outcome = state
        .get_service
        .execute(GetImageCommand { key, dsl: q.p })
        .await?;
    let etag = outcome.cache_key.etag();

    // If-None-Match → 304
    if let Some(if_none_match) = headers.get(header::IF_NONE_MATCH) {
        if let Ok(s) = if_none_match.to_str() {
            if s == etag || s == "*" {
                return Ok((
                    StatusCode::NOT_MODIFIED,
                    [(header::ETAG, etag.clone())],
                )
                    .into_response());
            }
        }
    }

    let bytes_len = outcome.result.bytes.len();
    Ok((
        [
            (header::CONTENT_TYPE, outcome.result.format.content_type().to_string()),
            (header::ETAG, etag),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_string(),
            ),
            (X_IMAGE_CACHE, if outcome.cached { "hit" } else { "miss" }.to_string()),
            (X_IMAGE_WIDTH, outcome.result.width.to_string()),
            (X_IMAGE_HEIGHT, outcome.result.height.to_string()),
            (X_IMAGE_BYTES, bytes_len.to_string()),
            (X_PROCESS_TIME_MS, outcome.elapsed_ms.to_string()),
        ],
        outcome.result.bytes,
    )
        .into_response())
}
