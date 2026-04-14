use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use anyhow::Context;
use tokio::net::TcpListener;
use tracing_subscriber::{EnvFilter, fmt, prelude::*};

use image_rs::application::diff_image::DiffImageService;
use image_rs::application::get_image::GetImageService;
use image_rs::application::process_image::ProcessImageService;
use image_rs::config::{Config, LogFormat};
use image_rs::infrastructure::{
    codec_opencv::OpenCvCodec,
    diff_opencv::OpenCvDiffer,
    fonts_ab_glyph::AbGlyphFontProvider,
    metrics::init as metrics_init,
    ops_opencv::OpenCvOpExecutor,
    runtime::{MokaMaskCache, MokaResultCache, TokioConcurrencyLimiter},
    store_fs::FsImageStore,
    telemetry,
};
use image_rs::interfaces::http::{app::build_router, state::AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cfg = Arc::new(Config::from_env().context("load config")?);
    init_tracing(&cfg).context("init tracing")?;

    // Install global Prometheus recorder before any metrics! macros run.
    let metrics_handle = metrics_init().context("init metrics")?;

    // Composition root: assemble adapters and inject them through ports.
    let mask_cache = Arc::new(MokaMaskCache::new(cfg.mask_cache_capacity));
    let fonts = Arc::new(
        AbGlyphFontProvider::load_from_dir(&cfg.font_dir)
            .context("load fonts")?,
    );
    let codec = Arc::new(OpenCvCodec::new(cfg.max_pixels));
    let executor = Arc::new(OpenCvOpExecutor::new(fonts.clone(), mask_cache.clone()));
    let limiter = Arc::new(TokioConcurrencyLimiter::new(cfg.max_concurrent_jobs));

    let service = Arc::new(ProcessImageService::new(
        codec.clone(),
        executor.clone(),
        limiter.clone(),
    ));

    // GET endpoint adapters: FS-backed source store + moka result cache.
    let store = Arc::new(FsImageStore::new(cfg.image_store_root.clone()));
    let result_cache = Arc::new(MokaResultCache::new(cfg.result_cache_capacity));
    let get_service = Arc::new(GetImageService::new(
        store,
        result_cache,
        service.clone(),
    ));

    // Diff endpoint adapter.
    let differ = Arc::new(OpenCvDiffer::new());
    let diff_service = Arc::new(DiffImageService::new(
        codec.clone(),
        differ,
        limiter.clone(),
    ));

    let draining = Arc::new(AtomicBool::new(false));
    let state = AppState {
        cfg: cfg.clone(),
        service,
        get_service,
        diff_service,
        limiter,
        metrics: metrics_handle,
        draining: draining.clone(),
    };

    let app = build_router(state);
    let addr: SocketAddr = cfg.bind_addr.parse().context("parse bind addr")?;
    let listener = TcpListener::bind(addr).await.context("bind tcp")?;
    tracing::info!(%addr, "image-rs listening");

    axum::serve(listener, app.into_make_service_with_connect_info::<SocketAddr>())
        .with_graceful_shutdown(shutdown_signal(draining))
        .await
        .context("serve")?;

    // Flush any in-flight OTLP spans before exit.
    telemetry::shutdown();
    Ok(())
}

fn init_tracing(cfg: &Config) -> anyhow::Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(&cfg.log_level));

    // Optional OTLP layer — only present when OTEL_EXPORTER_OTLP_ENDPOINT is
    // set. `Option<L>` implements `Layer`, so this composes cleanly into
    // either fmt branch below.
    let otel_layer = telemetry::try_install_otlp()?
        .map(|tracer| tracing_opentelemetry::layer().with_tracer(tracer));

    let registry = tracing_subscriber::registry().with(filter).with(otel_layer);

    match cfg.log_format {
        LogFormat::Json => {
            registry
                .with(
                    fmt::layer()
                        .json()
                        .with_current_span(true)
                        .with_span_list(false)
                        .with_target(true),
                )
                .init();
        }
        LogFormat::Text => {
            registry
                .with(fmt::layer().with_target(true).with_thread_ids(false))
                .init();
        }
    }
    Ok(())
}

async fn shutdown_signal(draining: Arc<AtomicBool>) {
    let ctrl_c = async {
        tokio::signal::ctrl_c().await.ok();
    };
    #[cfg(unix)]
    let term = async {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut s) = signal(SignalKind::terminate()) {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let term = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = term => {},
    }
    tracing::info!("shutdown signal received, draining for 15s");
    draining.store(true, Ordering::Relaxed);
    // Give load balancers time to observe /readyz returning 503.
    tokio::time::sleep(std::time::Duration::from_secs(15)).await;
}
