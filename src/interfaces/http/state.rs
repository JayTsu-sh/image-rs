use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use axum::extract::FromRef;
use metrics_exporter_prometheus::PrometheusHandle;

use crate::application::diff_image::DiffImageService;
use crate::application::get_image::GetImageService;
use crate::application::process_image::ProcessImageService;
use crate::config::Config;
use crate::infrastructure::runtime::TokioConcurrencyLimiter;

/// Composition root state shared with every handler. All inner fields are
/// `Arc`-backed so cloning the state is essentially free.
#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<Config>,
    pub service: Arc<ProcessImageService>,
    pub get_service: Arc<GetImageService>,
    pub diff_service: Arc<DiffImageService>,
    pub limiter: Arc<TokioConcurrencyLimiter>,
    pub metrics: PrometheusHandle,
    pub draining: Arc<AtomicBool>,
}

impl FromRef<AppState> for Arc<ProcessImageService> {
    fn from_ref(s: &AppState) -> Self { s.service.clone() }
}

impl FromRef<AppState> for Arc<Config> {
    fn from_ref(s: &AppState) -> Self { s.cfg.clone() }
}
