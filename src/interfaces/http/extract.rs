//! Multipart upload extractor — collects the request body into named
//! `Bytes` slots without ever copying the underlying buffers.
//!
//! * `file`   — the primary image (required)
//! * `ops`    — JSON for the unified `/v1/process` pipeline (optional)
//! * `output` — JSON output spec for `/v1/process` (optional)
//! * any other named field is treated as a binary asset (e.g. a watermark)
//!   keyed by its field name.

use std::collections::HashMap;

use axum::extract::{FromRequest, Multipart, Request};
use bytes::Bytes;

use crate::domain::error::DomainError;
use crate::interfaces::http::error::HttpError;

pub struct ImageUpload {
    /// `Some` for endpoints that take a single primary image via the `file`
    /// field; `None` for endpoints like `/v1/diff` that take two named
    /// images instead. Routes that need it MUST go through
    /// `primary_required()` to get a clean 400 response.
    pub primary: Option<Bytes>,
    pub assets: HashMap<String, Bytes>,
    pub ops_json: Option<Bytes>,
    pub output_json: Option<Bytes>,
}

impl ImageUpload {
    /// Returns the primary image or a `400 missing 'file'` error.
    pub fn primary_required(&self) -> Result<Bytes, HttpError> {
        self.primary
            .clone()
            .ok_or_else(|| HttpError(DomainError::invalid("missing 'file' field")))
    }

    /// Convenience for endpoints that take two named images instead of the
    /// usual `file` + asset map (e.g. the diff endpoint takes `before` and
    /// `after`). Pulls the named field out of `assets` so the caller can
    /// treat the upload as having two primary images.
    pub fn take_named(&mut self, name: &str) -> Option<Bytes> {
        self.assets.remove(name)
    }
}

impl<S> FromRequest<S> for ImageUpload
where
    S: Send + Sync,
{
    type Rejection = HttpError;

    async fn from_request(req: Request, state: &S) -> Result<Self, Self::Rejection> {
        let mut mp = Multipart::from_request(req, state)
            .await
            .map_err(|e| HttpError(DomainError::invalid(e.to_string())))?;

        let mut primary: Option<Bytes> = None;
        let mut assets: HashMap<String, Bytes> = HashMap::new();
        let mut ops_json: Option<Bytes> = None;
        let mut output_json: Option<Bytes> = None;

        while let Some(field) = mp
            .next_field()
            .await
            .map_err(|e| HttpError(DomainError::invalid(e.to_string())))?
        {
            let name = field.name().map(|s| s.to_string());
            let bytes = field
                .bytes()
                .await
                .map_err(|_| HttpError(DomainError::PayloadTooLarge))?;

            match name.as_deref() {
                Some("file") => primary = Some(bytes),
                Some("ops") => ops_json = Some(bytes),
                Some("output") => output_json = Some(bytes),
                Some(other) => {
                    assets.insert(other.to_string(), bytes);
                }
                None => {}
            }
        }

        // `primary` is allowed to be `None` here so endpoints like /v1/diff
        // (which use named `before` / `after` fields instead) can still parse
        // the request. Routes that require it must call `primary_required`.
        Ok(Self { primary, assets, ops_json, output_json })
    }
}
