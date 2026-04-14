//! Integration tests for every HTTP endpoint. Each test exercises the full
//! stack: multipart parse → DTO → domain → OpenCV → encode → response.

mod common;

use axum::http::StatusCode;
use common::*;
use opencv::prelude::*;
use tower::ServiceExt;

// ─── Health ──────────────────────────────────────────────────────────────────

#[tokio::test]
async fn healthz_returns_200() {
    let app = build_test_app();
    let resp = app
        .oneshot(
            axum::http::Request::builder()
                .uri("/healthz")
                .body(axum::body::Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
}

// ─── Unified /v1/process ─────────────────────────────────────────────────────

#[tokio::test]
async fn process_unified_pipeline_jpeg_to_webp() {
    let app = build_test_app();
    let parts = vec![
        jpeg_part(synth_jpeg(400, 300)),
        json_part(
            "ops",
            r#"[
                {"op":"resize","width":200,"mode":"fit"},
                {"op":"blur","sigma":1.5},
                {"op":"contrast","value":1.1},
                {"op":"brightness","value":5}
            ]"#,
        ),
        json_part("output", r#"{"format":"webp","quality":80}"#),
    ];
    let resp = post_multipart(app, "/v1/process", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/webp");
    let body = body_bytes(resp).await;
    assert!(body.len() > 100);
    assert_eq!(&body[..4], b"RIFF");
    assert_eq!(&body[8..12], b"WEBP");

    // Decode and verify dimensions
    let decoded = decode(&body);
    assert_eq!(decoded.cols(), 200);
    assert_eq!(decoded.rows(), 150);
}

#[tokio::test]
async fn process_pipeline_with_round_corner_and_text_watermark_to_png() {
    let app = build_test_app();
    let parts = vec![
        jpeg_part(synth_jpeg(400, 300)),
        json_part(
            "ops",
            r##"[
                {"op":"round_corner","radius":24},
                {"op":"watermark_text","text":"hello","font":"DejaVuSans","size":24,
                 "color":"#ffffffff","position":"bottom_right","margin":12}
            ]"##,
        ),
        json_part("output", r#"{"format":"png"}"#),
    ];
    let resp = post_multipart(app, "/v1/process", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/png");
    let body = body_bytes(resp).await;
    assert_eq!(&body[..8], b"\x89PNG\r\n\x1a\n");
    let decoded = decode(&body);
    assert_eq!(decoded.channels(), 4); // BGRA after round_corner
}

#[tokio::test]
async fn process_invalid_alpha_op_with_jpeg_output_is_rejected() {
    let app = build_test_app();
    let parts = vec![
        jpeg_part(synth_jpeg(200, 200)),
        json_part("ops", r#"[{"op":"round_corner","radius":12}]"#),
        json_part("output", r#"{"format":"jpeg"}"#),
    ];
    let resp = post_multipart(app, "/v1/process", parts).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn decompression_bomb_is_413() {
    // Build a 5000x5000 JPEG (25 megapixels). The test config caps
    // max_pixels at 16 MP, so this must be rejected before imdecode runs.
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(5000, 5000))];
    let resp = post_multipart(app, "/v1/effect/blur?sigma=1.0", parts).await;
    assert_eq!(resp.status(), StatusCode::PAYLOAD_TOO_LARGE);
}

#[tokio::test]
async fn process_webp_lossless_json_dsl() {
    let app = build_test_app();
    let parts = vec![
        jpeg_part(synth_jpeg(200, 200)),
        json_part("ops", r#"[{"op":"resize","width":100}]"#),
        json_part("output", r#"{"format":"webp","lossless":true}"#),
    ];
    let resp = post_multipart(app, "/v1/process", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/webp");
    let body = body_bytes(resp).await;
    // WebP magic
    assert_eq!(&body[..4], b"RIFF");
    assert_eq!(&body[8..12], b"WEBP");
    // VP8L marker at offset 12 = lossless; VP8  / VP8X = lossy / extended.
    // libwebp may wrap lossless in VP8X; accept either.
    let chunk_fourcc = &body[12..16];
    assert!(
        chunk_fourcc == b"VP8L" || chunk_fourcc == b"VP8X",
        "expected lossless WebP (VP8L/VP8X), got {:?}",
        std::str::from_utf8(chunk_fourcc).unwrap_or("?"),
    );
    // Sanity: the decoded pixels match what we asked for.
    let m = decode(&body);
    assert_eq!(m.cols(), 100);
}

#[tokio::test]
async fn process_jpeg_lossless_is_rejected() {
    let app = build_test_app();
    let parts = vec![
        jpeg_part(synth_jpeg(100, 100)),
        json_part("ops", r#"[]"#),
        json_part("output", r#"{"format":"jpeg","lossless":true}"#),
    ];
    let resp = post_multipart(app, "/v1/process", parts).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn basic_format_webp_lossless_via_query() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(150, 150))];
    let resp =
        post_multipart(app, "/v1/basic/format?format=webp&lossless=true", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/webp");
    let body = body_bytes(resp).await;
    assert_eq!(&body[..4], b"RIFF");
    assert_eq!(&body[8..12], b"WEBP");
}

#[tokio::test]
async fn lossless_webp_is_larger_than_lossy() {
    // Sanity that lossless is actually lossless: for a busy image the
    // lossless output should be distinctly larger than a q=30 lossy one.
    let app = build_test_app();

    // Use a naturally-busy image (high-frequency) so JPEG q=30 really
    // shrinks it.
    let source = synth_busy_jpeg(256, 256);

    let lossy_resp = post_multipart(
        app.clone(),
        "/v1/basic/format?format=webp&quality=30",
        vec![jpeg_part(source.clone())],
    )
    .await;
    let lossy_body = body_bytes(lossy_resp).await;

    let lossless_resp = post_multipart(
        app,
        "/v1/basic/format?format=webp&lossless=true",
        vec![jpeg_part(source)],
    )
    .await;
    let lossless_body = body_bytes(lossless_resp).await;

    assert!(
        lossless_body.len() > lossy_body.len(),
        "lossless ({}) should be larger than q=30 lossy ({}) for busy image",
        lossless_body.len(),
        lossy_body.len()
    );
}

#[tokio::test]
async fn get_image_url_dsl_lossless() {
    let (app, store_dir) = build_test_app_with_temp_store();
    std::fs::write(store_dir.join("img.jpg"), synth_jpeg(200, 200)).unwrap();
    let req = axum::http::Request::builder()
        .uri("/v1/img/img.jpg?p=format,f_webp,l_1")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/webp");
    let body = body_bytes(resp).await;
    assert_eq!(&body[..4], b"RIFF");
}

#[tokio::test]
async fn process_missing_file_field_is_400() {
    let app = build_test_app();
    let parts = vec![json_part("ops", r#"[{"op":"resize","width":100}]"#)];
    let resp = post_multipart(app, "/v1/process", parts).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ─── /v1/basic/* ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn basic_resize() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(400, 300))];
    let resp = post_multipart(app, "/v1/basic/resize?width=100&mode=fit", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let body = body_bytes(resp).await;
    let m = decode(&body);
    assert_eq!(m.cols(), 100);
    assert_eq!(m.rows(), 75);
}

#[tokio::test]
async fn basic_resize_fill_crops_to_target() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(400, 300))];
    let resp =
        post_multipart(app, "/v1/basic/resize?width=200&height=200&mode=fill", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let m = decode(&body_bytes(resp).await);
    assert_eq!(m.cols(), 200);
    assert_eq!(m.rows(), 200);
}

#[tokio::test]
async fn basic_rotate_expands_canvas() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(200, 100))];
    let resp = post_multipart(app, "/v1/basic/rotate?angle=90", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let m = decode(&body_bytes(resp).await);
    assert_eq!(m.cols(), 100);
    assert_eq!(m.rows(), 200);
}

#[tokio::test]
async fn basic_crop_extracts_rect() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(400, 300))];
    let resp =
        post_multipart(app, "/v1/basic/crop?x=50&y=40&width=100&height=80", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let m = decode(&body_bytes(resp).await);
    assert_eq!(m.cols(), 100);
    assert_eq!(m.rows(), 80);
}

#[tokio::test]
async fn basic_crop_out_of_bounds_is_422() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(100, 100))];
    let resp =
        post_multipart(app, "/v1/basic/crop?x=80&y=80&width=50&height=50", parts).await;
    assert_eq!(resp.status(), StatusCode::UNPROCESSABLE_ENTITY);
}

#[tokio::test]
async fn basic_format_jpeg_to_webp() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(150, 150))];
    let resp = post_multipart(app, "/v1/basic/format?format=webp", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/webp");
    let body = body_bytes(resp).await;
    assert_eq!(&body[..4], b"RIFF");
}

// ─── /v1/effect/* ────────────────────────────────────────────────────────────

#[tokio::test]
async fn effect_blur() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(200, 200))];
    let resp = post_multipart(app, "/v1/effect/blur?sigma=3.0", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn effect_sharpen() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(200, 200))];
    let resp = post_multipart(app, "/v1/effect/sharpen?amount=0.6", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn effect_round_corner_to_png() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(200, 200))];
    let resp = post_multipart(
        app,
        "/v1/effect/round-corner?radius=20&format=png",
        parts,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let m = decode(&body_bytes(resp).await);
    assert_eq!(m.channels(), 4);
}

#[tokio::test]
async fn effect_saturation() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(120, 120))];
    let resp = post_multipart(app, "/v1/effect/saturation?factor=1.5", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    // metadata headers present
    assert_eq!(resp.headers()["x-image-width"], "120");
    assert_eq!(resp.headers()["x-image-height"], "120");
    assert!(resp.headers().contains_key("x-process-time-ms"));
}

#[tokio::test]
async fn effect_temperature() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(120, 120))];
    let resp = post_multipart(app, "/v1/effect/temperature?value=20", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn resize_with_lanczos4_interpolation() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(400, 300))];
    let resp = post_multipart(
        app,
        "/v1/basic/resize?width=200&interpolation=lanczos4",
        parts,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let m = decode(&body_bytes(resp).await);
    assert_eq!(m.cols(), 200);
}

#[tokio::test]
async fn process_response_carries_metadata_headers() {
    let app = build_test_app();
    let parts = vec![
        jpeg_part(synth_jpeg(400, 300)),
        json_part("ops", r#"[{"op":"resize","width":100}]"#),
        json_part("output", r#"{"format":"jpeg","quality":75}"#),
    ];
    let resp = post_multipart(app, "/v1/process", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["x-image-width"], "100");
    assert_eq!(resp.headers()["x-image-height"], "75");
    let bytes_hdr: usize = resp.headers()["x-image-bytes"]
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    assert!(bytes_hdr > 0);
    assert!(resp.headers().contains_key("x-process-time-ms"));
}

#[tokio::test]
async fn effect_brightness() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(100, 100))];
    let resp = post_multipart(app, "/v1/effect/brightness?value=20", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn effect_contrast() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(100, 100))];
    let resp = post_multipart(app, "/v1/effect/contrast?value=1.3", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn effect_progressive_jpeg() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(200, 200))];
    let resp =
        post_multipart(app, "/v1/effect/progressive?format=jpeg", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/jpeg");
}

#[tokio::test]
async fn effect_auto_orient_noop_for_synth_jpeg() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(100, 80))];
    let resp = post_multipart(app, "/v1/effect/auto-orient", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    let m = decode(&body_bytes(resp).await);
    assert_eq!(m.cols(), 100);
    assert_eq!(m.rows(), 80);
}

// ─── /v1/watermark/* ─────────────────────────────────────────────────────────

#[tokio::test]
async fn watermark_image_with_alpha_png() {
    let app = build_test_app();
    let main = jpeg_part(synth_jpeg(400, 300));
    let wm = file_part("watermark", "wm.png", "image/png", synth_png_rgba(60, 40));
    let parts = vec![main, wm];
    let resp = post_multipart(
        app,
        "/v1/watermark/image?asset=watermark&position=bottom_right&opacity=0.7&scale=0.2&format=png",
        parts,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let m = decode(&body_bytes(resp).await);
    // Output preserved input dimensions and is BGRA after watermark
    assert_eq!(m.cols(), 400);
    assert_eq!(m.rows(), 300);
    assert_eq!(m.channels(), 4);
}

#[tokio::test]
async fn watermark_image_missing_asset_is_400() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(200, 200))];
    let resp = post_multipart(
        app,
        "/v1/watermark/image?asset=missing",
        parts,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ─── POST /v1/diff ───────────────────────────────────────────────────────────

#[tokio::test]
async fn diff_highlight_mode_returns_image() {
    let app = build_test_app();
    let before = synth_jpeg(200, 200);
    let after = synth_busy_jpeg(200, 200); // intentionally different content
    let parts = vec![
        file_part("before", "before.jpg", "image/jpeg", before),
        file_part("after", "after.jpg", "image/jpeg", after),
    ];
    let resp = post_multipart(app, "/v1/diff?mode=highlight&format=png", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/png");
    let m = decode(&body_bytes(resp).await);
    assert_eq!(m.cols(), 200);
    assert_eq!(m.rows(), 200);
}

#[tokio::test]
async fn diff_grayscale_mode() {
    let app = build_test_app();
    let parts = vec![
        file_part("before", "a.jpg", "image/jpeg", synth_jpeg(120, 120)),
        file_part("after", "b.jpg", "image/jpeg", synth_busy_jpeg(120, 120)),
    ];
    let resp = post_multipart(app, "/v1/diff?mode=grayscale&format=png", parts).await;
    assert_eq!(resp.status(), StatusCode::OK);
}

#[tokio::test]
async fn diff_dimension_mismatch_is_400() {
    let app = build_test_app();
    let parts = vec![
        file_part("before", "a.jpg", "image/jpeg", synth_jpeg(100, 100)),
        file_part("after", "b.jpg", "image/jpeg", synth_jpeg(200, 200)),
    ];
    let resp = post_multipart(app, "/v1/diff", parts).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn diff_missing_after_field_is_400() {
    let app = build_test_app();
    let parts = vec![file_part(
        "before",
        "a.jpg",
        "image/jpeg",
        synth_jpeg(100, 100),
    )];
    let resp = post_multipart(app, "/v1/diff", parts).await;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

// ─── GET /v1/img/{key}?p=DSL ─────────────────────────────────────────────────

#[tokio::test]
async fn get_image_with_dsl_miss_then_hit() {
    let (app, store_dir) = build_test_app_with_temp_store();
    std::fs::write(store_dir.join("cat.jpg"), synth_jpeg(400, 300)).unwrap();

    // First request: cache miss
    let req = axum::http::Request::builder()
        .uri("/v1/img/cat.jpg?p=resize,w_200/format,f_webp,q_85")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["content-type"], "image/webp");
    assert_eq!(resp.headers()["x-image-cache"], "miss");
    let etag = resp.headers()["etag"].to_str().unwrap().to_string();
    assert!(etag.starts_with('"') && etag.ends_with('"'));
    let body = body_bytes(resp).await;
    let m = decode(&body);
    assert_eq!(m.cols(), 200);

    // Second request: cache hit
    let req = axum::http::Request::builder()
        .uri("/v1/img/cat.jpg?p=resize,w_200/format,f_webp,q_85")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(resp.headers()["x-image-cache"], "hit");
    assert_eq!(resp.headers()["etag"].to_str().unwrap(), etag);

    // Third request: If-None-Match matching → 304
    let req = axum::http::Request::builder()
        .uri("/v1/img/cat.jpg?p=resize,w_200/format,f_webp,q_85")
        .header("if-none-match", &etag)
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::NOT_MODIFIED);
    assert_eq!(resp.headers()["etag"].to_str().unwrap(), etag);
}

#[tokio::test]
async fn get_image_missing_key_is_400() {
    let (app, _dir) = build_test_app_with_temp_store();
    let req = axum::http::Request::builder()
        .uri("/v1/img/nonexistent.jpg?p=resize,w_100")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    // FsImageStore returns MissingAsset → HttpError maps to 400
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_image_path_traversal_rejected() {
    let (app, _dir) = build_test_app_with_temp_store();
    let req = axum::http::Request::builder()
        .uri("/v1/img/..%2fetc%2fpasswd?p=resize,w_100")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_image_invalid_dsl_is_400() {
    let (app, store_dir) = build_test_app_with_temp_store();
    std::fs::write(store_dir.join("a.jpg"), synth_jpeg(100, 100)).unwrap();
    let req = axum::http::Request::builder()
        .uri("/v1/img/a.jpg?p=levitate,a_90")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp = app.oneshot(req).await.unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn get_image_different_dsl_produces_different_etag() {
    let (app, store_dir) = build_test_app_with_temp_store();
    std::fs::write(store_dir.join("b.jpg"), synth_jpeg(200, 200)).unwrap();

    let req_a = axum::http::Request::builder()
        .uri("/v1/img/b.jpg?p=resize,w_100")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_a = app.clone().oneshot(req_a).await.unwrap();
    let etag_a = resp_a.headers()["etag"].to_str().unwrap().to_string();

    let req_b = axum::http::Request::builder()
        .uri("/v1/img/b.jpg?p=resize,w_150")
        .body(axum::body::Body::empty())
        .unwrap();
    let resp_b = app.clone().oneshot(req_b).await.unwrap();
    let etag_b = resp_b.headers()["etag"].to_str().unwrap().to_string();

    assert_ne!(etag_a, etag_b);
}

#[tokio::test]
async fn watermark_text() {
    let app = build_test_app();
    let parts = vec![jpeg_part(synth_jpeg(400, 300))];
    let resp = post_multipart(
        app,
        "/v1/watermark/text?text=hello&font=DejaVuSans&size=24&color=%23ffffff&position=bottom_right&format=png",
        parts,
    )
    .await;
    assert_eq!(resp.status(), StatusCode::OK);
    let m = decode(&body_bytes(resp).await);
    assert_eq!(m.cols(), 400);
    assert_eq!(m.rows(), 300);
    assert_eq!(m.channels(), 4);
}
