//! image-rs — OpenCV-backed image processing microservice (DDD layout).
//!
//! Layers:
//! * [`domain`]         — pure business model, no infra dependencies.
//! * [`application`]    — use cases + ports (traits) implemented by infra.
//! * [`infrastructure`] — adapters: OpenCV, ab_glyph, moka, tokio.
//! * [`interfaces`]     — HTTP delivery via axum.

pub mod application;
pub mod config;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
