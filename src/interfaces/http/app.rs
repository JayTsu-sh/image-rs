//! Router assembly. Middleware ordering follows axum/tower best practices:
//! `ServiceBuilder` layers are applied top-down (outermost first), and the
//! per-route limits/timeout sit close to the business handlers so health
//! checks aren't subject to them.

use axum::Router;
use axum::extract::DefaultBodyLimit;
use axum::http::{Request, StatusCode};
use axum::routing::{get, post};
use tower::ServiceBuilder;
use tower_http::catch_panic::CatchPanicLayer;
use tower_http::cors::CorsLayer;
use tower_http::request_id::{
    MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer,
};
use tower_http::sensitive_headers::SetSensitiveRequestHeadersLayer;
use tower_http::services::ServeDir;
use tower_http::timeout::TimeoutLayer;
use tower_http::trace::{DefaultOnResponse, MakeSpan, TraceLayer};
use tracing::{Level, Span};

use crate::interfaces::http::routes;
use crate::interfaces::http::state::AppState;

pub fn build_router(state: AppState) -> Router {
    let business = Router::new()
        // GET endpoint with URL DSL + cache
        .route("/v1/img/{key}", get(routes::get_image))
        // unified pipeline
        .route("/v1/process", post(routes::process))
        // pixel diff between two images
        .route("/v1/diff", post(routes::diff))
        // basic
        .route("/v1/basic/resize", post(routes::resize))
        .route("/v1/basic/rotate", post(routes::rotate))
        .route("/v1/basic/crop", post(routes::crop))
        .route("/v1/basic/format", post(routes::format))
        // watermark
        .route("/v1/watermark/image", post(routes::watermark_image))
        .route("/v1/watermark/text", post(routes::watermark_text))
        // effect
        .route("/v1/effect/blur", post(routes::blur))
        .route("/v1/effect/sharpen", post(routes::sharpen))
        .route("/v1/effect/round-corner", post(routes::round_corner))
        .route("/v1/effect/contrast", post(routes::contrast))
        .route("/v1/effect/brightness", post(routes::brightness))
        .route("/v1/effect/saturation", post(routes::saturation))
        .route("/v1/effect/temperature", post(routes::temperature))
        .route("/v1/effect/progressive", post(routes::progressive))
        .route("/v1/effect/auto-orient", post(routes::auto_orient))
        // per-route limits and timeout
        .layer(DefaultBodyLimit::max(state.cfg.max_upload_bytes))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            state.cfg.request_timeout,
        ))
        .with_state(state.clone());

    let ops = Router::new()
        .route("/healthz", get(routes::healthz))
        .route("/readyz", get(routes::readyz))
        .route("/metrics", get(routes::metrics))
        .with_state(state.clone());

    // Static UI: Vite's production bundle is served from `ui_dir` at /ui/.
    // `append_index_html_on_directories` makes `/ui/` serve `index.html`.
    let ui = Router::new().nest_service(
        "/ui",
        ServeDir::new(&state.cfg.ui_dir).append_index_html_on_directories(true),
    );

    Router::new().merge(business).merge(ops).merge(ui).layer(
        ServiceBuilder::new()
            .layer(SetSensitiveRequestHeadersLayer::new(std::iter::once(
                axum::http::header::AUTHORIZATION,
            )))
            .layer(CatchPanicLayer::new())
            .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
            .layer(
                TraceLayer::new_for_http()
                    .make_span_with(AppMakeSpan)
                    .on_response(DefaultOnResponse::new().level(Level::INFO)),
            )
            .layer(PropagateRequestIdLayer::x_request_id())
            .layer(CorsLayer::permissive()),
    )
}

/// Custom span maker that lifts the `x-request-id` header (already populated
/// by `SetRequestIdLayer` upstream) into a structured span field, so every
/// log line emitted while handling the request carries the same id.
#[derive(Clone)]
struct AppMakeSpan;

impl<B> MakeSpan<B> for AppMakeSpan {
    fn make_span(&mut self, req: &Request<B>) -> Span {
        let request_id = req
            .headers()
            .get("x-request-id")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("-");
        tracing::info_span!(
            "http",
            method = %req.method(),
            uri = %req.uri(),
            request_id = %request_id,
        )
    }
}
