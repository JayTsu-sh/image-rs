//! Domain layer.
//!
//! Pure business model. Must NOT depend on opencv, axum, tokio, hyper, serde,
//! or any other infrastructure crate. Errors are domain errors; types are
//! ubiquitous-language value objects and aggregates.

pub mod error;
pub mod image;
pub mod ops;
pub mod pipeline;
pub mod url_dsl;
pub mod value_objects;

pub use error::DomainError;
pub use image::ImageBuffer;
pub use ops::{Op, OpKind};
pub use pipeline::{Compression, OutputSpec, Pipeline};
pub use value_objects::*;
