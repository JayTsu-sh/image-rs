//! Shared test helpers for HTTP integration tests.

#![allow(dead_code)]

use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, Response};
use bytes::{Bytes, BytesMut};
use opencv::{
    core::{Mat, Scalar, Vector, CV_8UC3, CV_8UC4},
    imgcodecs,
    prelude::*,
};
use tower::ServiceExt;

use image_rs::application::diff_image::DiffImageService;
use image_rs::application::get_image::GetImageService;
use image_rs::application::process_image::ProcessImageService;
use image_rs::config::Config;
use image_rs::infrastructure::codec_opencv::OpenCvCodec;
use image_rs::infrastructure::diff_opencv::OpenCvDiffer;
use image_rs::infrastructure::fonts_ab_glyph::AbGlyphFontProvider;
use image_rs::infrastructure::metrics::init as metrics_init;
use image_rs::infrastructure::ops_opencv::OpenCvOpExecutor;
use image_rs::infrastructure::runtime::{
    MokaMaskCache, MokaResultCache, TokioConcurrencyLimiter,
};
use image_rs::infrastructure::store_fs::FsImageStore;
use image_rs::interfaces::http::app::build_router;
use image_rs::interfaces::http::state::AppState;

pub fn build_test_app() -> Router {
    build_test_app_with_store(unique_store_dir())
}

/// Build a test app that returns both the router and the temp dir it uses
/// as the image store, so tests can drop source files into it.
pub fn build_test_app_with_temp_store() -> (Router, std::path::PathBuf) {
    let dir = unique_store_dir();
    (build_test_app_with_store(dir.clone()), dir)
}

fn unique_store_dir() -> std::path::PathBuf {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let n = COUNTER.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "image_rs_test_store_{}_{}",
        std::process::id(),
        n
    ))
}

pub fn build_test_app_with_store(store_root: std::path::PathBuf) -> Router {
    let _ = std::fs::create_dir_all(&store_root);
    let cfg = Arc::new(Config {
        bind_addr: "127.0.0.1:0".to_string(),
        log_level: "warn".to_string(),
        log_format: image_rs::config::LogFormat::Text,
        max_upload_bytes: 10 * 1024 * 1024,
        max_pixels: 16 * 1024 * 1024,
        max_concurrent_jobs: 4,
        request_timeout: Duration::from_secs(10),
        mask_cache_capacity: 16,
        // Resolve via the same fallback chain main.rs uses; on the EL9 test
        // box this picks /usr/share/fonts/dejavu-sans-fonts.
        font_dir: image_rs::config::default_font_dir(),
        image_store_root: store_root.clone(),
        result_cache_capacity: 16,
        ui_dir: std::path::PathBuf::from("./web/dist"),
    });
    let mask_cache = Arc::new(MokaMaskCache::new(cfg.mask_cache_capacity));
    let fonts = Arc::new(
        AbGlyphFontProvider::load_from_dir(&cfg.font_dir)
            .expect("font dir load"),
    );
    let codec = Arc::new(OpenCvCodec::new(cfg.max_pixels));
    let executor = Arc::new(OpenCvOpExecutor::new(fonts.clone(), mask_cache.clone()));
    let limiter = Arc::new(TokioConcurrencyLimiter::new(cfg.max_concurrent_jobs));
    let service = Arc::new(ProcessImageService::new(
        codec.clone(),
        executor,
        limiter.clone(),
    ));
    let store = Arc::new(FsImageStore::new(store_root));
    let result_cache = Arc::new(MokaResultCache::new(cfg.result_cache_capacity));
    let get_service = Arc::new(GetImageService::new(
        store,
        result_cache,
        service.clone(),
    ));
    let differ = Arc::new(OpenCvDiffer::new());
    let diff_service = Arc::new(DiffImageService::new(
        codec.clone(),
        differ,
        limiter.clone(),
    ));
    // Tests run in parallel; install_recorder may already be set in another
    // test thread. Tolerate that by ignoring the error and falling back to
    // a stub PrometheusHandle from a fresh builder (it will be a no-op
    // recorder for the test, which is what we want).
    let metrics = metrics_init().unwrap_or_else(|_| {
        metrics_exporter_prometheus::PrometheusBuilder::new()
            .build_recorder()
            .handle()
    });
    let state = AppState {
        cfg,
        service,
        get_service,
        diff_service,
        limiter,
        metrics,
        draining: Arc::new(AtomicBool::new(false)),
    };
    build_router(state)
}

// ─── Synthetic image fixtures ────────────────────────────────────────────────

pub fn synth_jpeg(w: i32, h: i32) -> Bytes {
    let mat =
        Mat::new_rows_cols_with_default(h, w, CV_8UC3, Scalar::new(80.0, 120.0, 200.0, 0.0))
            .expect("alloc");
    let mut buf = Vector::<u8>::new();
    imgcodecs::imencode(".jpg", &mat, &mut buf, &Vector::new()).expect("encode");
    Bytes::from(buf.to_vec())
}

pub fn synth_busy_jpeg(w: i32, h: i32) -> Bytes {
    // A high-frequency checker + noise pattern that doesn't trivially
    // compress. Used in lossless-vs-lossy size comparison tests.
    use opencv::core::Mat_AUTO_STEP;
    let mut mat = Mat::new_rows_cols_with_default(
        h,
        w,
        CV_8UC3,
        Scalar::new(0.0, 0.0, 0.0, 0.0),
    )
    .unwrap();
    let data = mat.data_bytes_mut().unwrap();
    let _ = Mat_AUTO_STEP; // silence unused-import warning on some versions
    for y in 0..h as usize {
        for x in 0..w as usize {
            let i = (y * w as usize + x) * 3;
            let checker = ((x / 3) ^ (y / 3)) as u8;
            data[i] = checker.wrapping_mul(37).wrapping_add((x * 7) as u8);
            data[i + 1] = (checker ^ 0xAA).wrapping_mul(29);
            data[i + 2] = (x as u8).wrapping_mul(13) ^ (y as u8).wrapping_mul(11);
        }
    }
    let mut buf = Vector::<u8>::new();
    let mut params = Vector::<i32>::new();
    params.push(imgcodecs::IMWRITE_JPEG_QUALITY);
    params.push(95);
    imgcodecs::imencode(".jpg", &mat, &mut buf, &params).unwrap();
    Bytes::from(buf.to_vec())
}

pub fn synth_png_rgba(w: i32, h: i32) -> Bytes {
    let mat =
        Mat::new_rows_cols_with_default(h, w, CV_8UC4, Scalar::new(0.0, 255.0, 0.0, 200.0))
            .expect("alloc");
    let mut buf = Vector::<u8>::new();
    imgcodecs::imencode(".png", &mat, &mut buf, &Vector::new()).expect("encode");
    Bytes::from(buf.to_vec())
}

pub fn decode(bytes: &[u8]) -> Mat {
    let header = Mat::from_slice::<u8>(bytes).expect("header");
    imgcodecs::imdecode(&header, imgcodecs::IMREAD_UNCHANGED).expect("decode")
}

// ─── Multipart builder ───────────────────────────────────────────────────────

pub struct Part {
    pub name: &'static str,
    pub filename: Option<&'static str>,
    pub content_type: &'static str,
    pub data: Bytes,
}

pub fn file_part(name: &'static str, filename: &'static str, ct: &'static str, data: Bytes) -> Part {
    Part { name, filename: Some(filename), content_type: ct, data }
}

pub fn json_part(name: &'static str, json: &str) -> Part {
    Part {
        name,
        filename: None,
        content_type: "application/json",
        data: Bytes::from(json.to_string()),
    }
}

pub fn build_multipart(parts: Vec<Part>) -> (String, Bytes) {
    let boundary = format!("----imagersbnd{}", std::process::id());
    let mut body = BytesMut::new();
    for part in parts {
        body.extend_from_slice(format!("--{}\r\n", boundary).as_bytes());
        if let Some(fname) = part.filename {
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"{}\"; filename=\"{}\"\r\n",
                    part.name, fname
                )
                .as_bytes(),
            );
        } else {
            body.extend_from_slice(
                format!("Content-Disposition: form-data; name=\"{}\"\r\n", part.name).as_bytes(),
            );
        }
        body.extend_from_slice(format!("Content-Type: {}\r\n\r\n", part.content_type).as_bytes());
        body.extend_from_slice(&part.data);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{}--\r\n", boundary).as_bytes());
    (boundary, body.freeze())
}

pub async fn post_multipart(app: Router, uri: &str, parts: Vec<Part>) -> Response<Body> {
    let (boundary, body) = build_multipart(parts);
    let req = Request::builder()
        .method("POST")
        .uri(uri)
        .header(
            "content-type",
            format!("multipart/form-data; boundary={boundary}"),
        )
        .body(Body::from(body))
        .unwrap();
    app.oneshot(req).await.unwrap()
}

pub async fn body_bytes(resp: Response<Body>) -> Bytes {
    axum::body::to_bytes(resp.into_body(), usize::MAX)
        .await
        .unwrap()
}

pub fn jpeg_part(data: Bytes) -> Part {
    file_part("file", "in.jpg", "image/jpeg", data)
}

pub fn png_part(data: Bytes) -> Part {
    file_part("file", "in.png", "image/png", data)
}
