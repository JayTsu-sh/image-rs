//! Rust-side heap-profiling test that pins the data path against regression.
//!
//! ## What this measures
//!
//! `dhat::Alloc` replaces the **Rust** global allocator and tracks every
//! `Vec` / `Box` / `Bytes` / `String` allocation made from Rust code. It
//! does **not** see allocations made by OpenCV's C++ side (pixel buffers
//! from `cv::Mat`, libjpeg-turbo's scratch state, etc.) because those go
//! through the C++ `new` / `malloc` path, not Rust's allocator.
//!
//! That's actually the *useful* property: this test isolates the Rust data
//! path and asserts that no `clone()` / `to_vec()` / intermediate `Vec<u8>`
//! sneaks in along the way. The empirical baseline for a full
//! decode → resize → encode JPEG pipeline on a 200×200 image is around
//! **4.5 KB** of Rust-side heap — essentially just the Bytes refcount
//! control block, the Pipeline aggregate, OpContext, and a few small
//! result structs. The pixel buffer itself (~120 KB) lives in OpenCV's
//! C++ heap and never crosses into Rust.
//!
//! If someone adds an unnecessary `mat.data_bytes().to_vec()` or
//! `bytes.clone().to_vec()`, the budget below trips immediately.
//!
//! Each `tests/*.rs` is its own binary, so the `#[global_allocator]` here
//! is scoped to this one test and does not affect any other test or the
//! production binary.

use std::path::PathBuf;
use std::sync::Arc;

use bytes::Bytes;
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

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

fn synth_jpeg(w: i32, h: i32) -> Bytes {
    let mat = Mat::new_rows_cols_with_default(
        h,
        w,
        CV_8UC3,
        Scalar::new(80.0, 120.0, 200.0, 0.0),
    )
    .unwrap();
    let mut buf = Vector::<u8>::new();
    imgcodecs::imencode(".jpg", &mat, &mut buf, &Vector::new()).unwrap();
    Bytes::from(buf.to_vec())
}

fn build_service() -> Arc<ProcessImageService> {
    let codec = Arc::new(OpenCvCodec::new(64 * 1024 * 1024));
    let mask_cache = Arc::new(MokaMaskCache::new(16));
    let fonts = Arc::new(
        AbGlyphFontProvider::load_from_dir(&PathBuf::from(
            "/usr/share/fonts/dejavu-sans-fonts",
        ))
        .unwrap(),
    );
    let executor = Arc::new(OpenCvOpExecutor::new(fonts, mask_cache));
    let limiter = Arc::new(TokioConcurrencyLimiter::new(4));
    Arc::new(ProcessImageService::new(codec, executor, limiter))
}

fn make_cmd(img: Bytes) -> ProcessImageCommand {
    ProcessImageCommand {
        image: img,
        pipeline: Pipeline::new(
            vec![Op::Resize(
                ResizeSpec::new(Some(100), None, ResizeMode::Fit).unwrap(),
            )],
            OutputSpec::lossy(Some(ImageFormat::Jpeg), Quality::default(), false),
        )
        .unwrap(),
        context: OpContext::default(),
    }
}

#[tokio::test]
async fn single_pipeline_iteration_stays_within_zero_copy_budget() {
    let service = build_service();

    // Warmup: trigger any lazy statics, OpenCV thread-local buffers, tokio
    // worker pool, etc. before we start measuring.
    for _ in 0..3 {
        let _ = service.execute(make_cmd(synth_jpeg(200, 200))).await.unwrap();
    }

    // Reset profiler — dhat tracks max from this point forward.
    let _profiler = dhat::Profiler::new_heap();

    // The actual measured iteration.
    let img = synth_jpeg(200, 200);
    let input_len = img.len();
    let outcome = service.execute(make_cmd(img)).await.unwrap();
    let output_len = outcome.encoded.bytes.len();
    drop(outcome);

    let stats = dhat::HeapStats::get();
    let pixel_buffer = 200 * 200 * 3; // BGR
    eprintln!(
        "dhat: max_bytes={} ({}KB), curr_bytes={}, total_bytes={}, max_blocks={}",
        stats.max_bytes,
        stats.max_bytes / 1024,
        stats.curr_bytes,
        stats.total_bytes,
        stats.max_blocks
    );
    eprintln!(
        "  input={} pixels={} output={}  expected min ≈ {} bytes",
        input_len,
        pixel_buffer,
        output_len,
        input_len + pixel_buffer + output_len,
    );

    // Empirically the Rust-side peak is ~4.5 KB (refcount blocks, structs,
    // small result wrappers). 64 KB is generous slack — anything close to
    // 200x200x3 = 120 KB would mean a pixel-buffer clone snuck in.
    let budget = 64 * 1024;
    assert!(
        stats.max_bytes < budget,
        "peak heap {} bytes exceeds zero-copy budget {} bytes — \
         someone likely added a clone()/to_vec() in the data path",
        stats.max_bytes,
        budget
    );
}
