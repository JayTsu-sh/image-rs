use thiserror::Error;

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("unsupported format: {0}")]
    UnsupportedFormat(String),

    #[error("payload too large")]
    PayloadTooLarge,

    #[error("missing asset: {0}")]
    MissingAsset(String),

    #[error("decode failed: {0}")]
    Decode(String),

    #[error("encode failed: {0}")]
    Encode(String),

    #[error("operation `{op}` failed: {message}")]
    OpFailed { op: &'static str, message: String },

    #[error("server overloaded")]
    Overloaded,

    #[error("internal error: {0}")]
    Internal(String),
}

impl DomainError {
    pub fn invalid(msg: impl Into<String>) -> Self {
        Self::InvalidArgument(msg.into())
    }
    pub fn op(op: &'static str, msg: impl Into<String>) -> Self {
        Self::OpFailed { op, message: msg.into() }
    }
}
