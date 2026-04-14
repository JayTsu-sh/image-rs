use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

use crate::domain::error::DomainError;

/// HTTP-facing error wrapper. Maps `DomainError` to status codes and a
/// stable JSON shape.
#[derive(Debug)]
pub struct HttpError(pub DomainError);

impl From<DomainError> for HttpError {
    fn from(e: DomainError) -> Self { Self(e) }
}

impl IntoResponse for HttpError {
    fn into_response(self) -> Response {
        let (status, code) = match &self.0 {
            DomainError::InvalidArgument(_) => (StatusCode::BAD_REQUEST, "invalid_argument"),
            DomainError::UnsupportedFormat(_) => {
                (StatusCode::UNSUPPORTED_MEDIA_TYPE, "unsupported_format")
            }
            DomainError::PayloadTooLarge => (StatusCode::PAYLOAD_TOO_LARGE, "payload_too_large"),
            DomainError::MissingAsset(_) => (StatusCode::BAD_REQUEST, "missing_asset"),
            DomainError::Decode(_) => (StatusCode::BAD_REQUEST, "decode_failed"),
            DomainError::Encode(_) => (StatusCode::INTERNAL_SERVER_ERROR, "encode_failed"),
            DomainError::OpFailed { .. } => (StatusCode::UNPROCESSABLE_ENTITY, "op_failed"),
            DomainError::Overloaded => (StatusCode::SERVICE_UNAVAILABLE, "overloaded"),
            DomainError::Internal(_) => (StatusCode::INTERNAL_SERVER_ERROR, "internal"),
        };

        if status.is_server_error() {
            tracing::error!(error = ?self.0, "request failed");
        } else {
            tracing::warn!(error = %self.0, "request rejected");
        }

        let mut body = json!({
            "error": code,
            "message": self.0.to_string(),
        });
        if let DomainError::OpFailed { op, .. } = &self.0 {
            body["op"] = json!(op);
        }
        (status, Json(body)).into_response()
    }
}
