//! End-to-end pipeline benchmark — measures the full
//! decode → ops → encode path through `ProcessImageService`, the same code
//! the HTTP handlers use. Run with `cargo bench --bench pipeline`.

use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
use criterion::{
    BenchmarkId, Criterion, Throughput, criterion_group, criterion_main,
};
use opencv::{
    core::{Mat, Scalar, Vector, CV_8UC3},
    imgcodecs,
    prelude::*,
};

use image_rs::application::ports::OpContext;
use image_rs::application::process_image::{ProcessImageCommand, ProcessImageService};
use image_rs::domain::ops::*;
use image_rs::domain::pipeline::{OutputSpec, Pipeline};
use image_rs::domain::value_objects::*;
use image_rs::infrastructure::codec_opencv::OpenCvCodec;
use image_rs::infrastructure::fonts_ab_glyph::AbGlyphFontProvider;
use image_rs::infrastructure::ops_opencv::OpenCvOpExecutor;
use image_rs::infrastructure::runtime::{MokaMaskCache, TokioConcurrencyLimiter};

fn synth_jpeg(w: i32, h: i32) -> Bytes {
    // A simple gradient so JPEG doesn't trivially compress to nothing.
    let mut mat = Mat::new_rows_cols_with_default(
        h,
        w,
        CV_8UC3,
        Scalar::new(0.0, 0.0, 0.0, 0.0),
    )
    .unwrap();
    let data = mat.data_bytes_mut().unwrap();
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = (y * w as usize + x) * 3;
            data[i] = (x as u8).wrapping_mul(3);
            data[i + 1] = (y as u8).wrapping_mul(5);
            data[i + 2] = ((x ^ y) as u8).wrapping_mul(7);
        }
    }
    let mut buf = Vector::<u8>::new();
    let mut params = Vector::<i32>::new();
    params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
    params.push(85);
    imgcodecs::imencode(".jpg", &mat, &mut buf, &params).unwrap();
    Bytes::from(buf.to_vec())
}

fn build_service() -> Arc<ProcessImageService> {
    let mask_cache = Arc::new(MokaMaskCache::new(64));
    let fonts = Arc::new(
        AbGlyphFontProvider::load_from_dir(&PathBuf::from(
            "/usr/share/fonts/dejavu-sans-fonts",
        ))
        .unwrap(),
    );
    let codec = Arc::new(OpenCvCodec::new(64 * 1024 * 1024));
    let executor = Arc::new(OpenCvOpExecutor::new(fonts, mask_cache));
    let limiter = Arc::new(TokioConcurrencyLimiter::new(num_cpus::get() * 2));
    Arc::new(ProcessImageService::new(codec, executor, limiter))
}

fn make_cmd(img: &Bytes, ops: Vec<Op>, format: ImageFormat) -> ProcessImageCommand {
    ProcessImageCommand {
        image: img.clone(), // refcount bump only — zero copy
        pipeline: Pipeline::new(
            ops,
            OutputSpec::lossy(Some(format), Quality::default(), false),
        )
        .unwrap(),
        context: OpContext::default(),
    }
}

fn bench_pipeline(c: &mut Criterion) {
    let runtime = tokio::runtime::Runtime::new().unwrap();
    let service = build_service();

    let resolutions: &[(i32, i32, &str)] = &[
        (640, 480, "640x480"),
        (1920, 1080, "1920x1080"),
    ];

    // ── 1. roundtrip: decode → encode (no ops) ─────────────────────────────
    {
        let mut group = c.benchmark_group("roundtrip_jpeg");
        group.sample_size(30);
        for &(w, h, label) in resolutions {
            let img = synth_jpeg(w, h);
            group.throughput(Throughput::Bytes(img.len() as u64));
            group.bench_with_input(BenchmarkId::new("jpeg", label), &img, |b, img| {
                b.iter(|| {
                    let cmd = make_cmd(img, vec![], ImageFormat::Jpeg);
                    runtime.block_on(service.execute(cmd)).unwrap()
                });
            });
        }
        group.finish();
    }

    // ── 2. resize only → JPEG / WebP ───────────────────────────────────────
    {
        let mut group = c.benchmark_group("resize_only");
        group.sample_size(30);
        for &(w, h, label) in resolutions {
            let img = synth_jpeg(w, h);
            group.throughput(Throughput::Bytes(img.len() as u64));
            group.bench_with_input(BenchmarkId::new("jpeg_to_webp_800", label), &img, |b, img| {
                b.iter(|| {
                    let cmd = make_cmd(
                        img,
                        vec![Op::Resize(
                            ResizeSpec::new(Some(800), None, ResizeMode::Fit).unwrap(),
                        )],
                        ImageFormat::WebP,
                    );
                    runtime.block_on(service.execute(cmd)).unwrap()
                });
            });
        }
        group.finish();
    }

    // ── 3. full pipeline: resize + blur + sharpen + brightness → WebP ──────
    {
        let mut group = c.benchmark_group("full_pipeline");
        group.sample_size(20);
        for &(w, h, label) in resolutions {
            let img = synth_jpeg(w, h);
            group.throughput(Throughput::Bytes(img.len() as u64));
            group.bench_with_input(BenchmarkId::new("jpeg_to_webp", label), &img, |b, img| {
                b.iter(|| {
                    let cmd = make_cmd(
                        img,
                        vec![
                            Op::Resize(
                                ResizeSpec::new(Some(1200), None, ResizeMode::Fit).unwrap(),
                            ),
                            Op::Blur(BlurSpec::new(1.0).unwrap()),
                            Op::Sharpen(SharpenSpec::new(0.4, 1.0).unwrap()),
                            Op::Brightness(BrightnessSpec::new(8).unwrap()),
                            Op::Contrast(ContrastSpec::new(1.05).unwrap()),
                        ],
                        ImageFormat::WebP,
                    );
                    runtime.block_on(service.execute(cmd)).unwrap()
                });
            });
        }
        group.finish();
    }

    // ── 4. round corner + text watermark → PNG (alpha path) ────────────────
    {
        let mut group = c.benchmark_group("watermark_text_round_corner");
        group.sample_size(20);
        for &(w, h, label) in resolutions {
            let img = synth_jpeg(w, h);
            group.throughput(Throughput::Bytes(img.len() as u64));
            group.bench_with_input(BenchmarkId::new("png", label), &img, |b, img| {
                b.iter(|| {
                    let cmd = make_cmd(
                        img,
                        vec![
                            Op::RoundCorner(RoundCornerSpec::new(24).unwrap()),
                            Op::WatermarkText(
                                WatermarkTextSpec::new(
                                    "© image-rs".to_string(),
                                    "DejaVuSans".to_string(),
                                    24.0,
                                    Color::WHITE,
                                    Anchor::BottomRight,
                                    16,
                                    true,
                                )
                                .unwrap(),
                            ),
                        ],
                        ImageFormat::Png,
                    );
                    runtime.block_on(service.execute(cmd)).unwrap()
                });
            });
        }
        group.finish();
    }
}

criterion_group!(benches, bench_pipeline);
criterion_main!(benches);
