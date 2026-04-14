//! Local-filesystem source image backend for `GET /v1/img/{key}`.

use std::path::{Path, PathBuf};

use bytes::Bytes;

use crate::application::ports::ImageStore;
use crate::domain::error::DomainError;

pub struct FsImageStore {
    root: PathBuf,
}

impl FsImageStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn safe_join(&self, key: &str) -> Result<PathBuf, DomainError> {
        // Cheap, conservative path-traversal guard. Reject anything that
        // could escape the root: leading separators, '..', NUL.
        if key.is_empty() {
            return Err(DomainError::invalid("empty key"));
        }
        if key.contains("..") || key.contains('\0') {
            return Err(DomainError::invalid("invalid characters in key"));
        }
        let trimmed = key.trim_start_matches('/');
        let candidate = self.root.join(trimmed);
        // After joining, canonicalize and verify the result is still inside
        // root. If canonicalize fails (file doesn't exist), fall back to
        // lexical containment check.
        let root_real = self.root.canonicalize().unwrap_or_else(|_| self.root.clone());
        match candidate.canonicalize() {
            Ok(real) if real.starts_with(&root_real) => Ok(real),
            Ok(_) => Err(DomainError::invalid("key escapes store root")),
            Err(_) => {
                // File missing — check lexically.
                if Self::is_inside(&candidate, &root_real) {
                    Ok(candidate)
                } else {
                    Err(DomainError::invalid("key escapes store root"))
                }
            }
        }
    }

    fn is_inside(path: &Path, root: &Path) -> bool {
        path.components().count() >= root.components().count()
            && path.starts_with(root)
    }
}

impl ImageStore for FsImageStore {
    fn get(&self, key: &str) -> Result<Bytes, DomainError> {
        let path = self.safe_join(key)?;
        match std::fs::read(&path) {
            Ok(v) => Ok(Bytes::from(v)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                Err(DomainError::MissingAsset(key.to_string()))
            }
            Err(e) => Err(DomainError::Internal(format!("fs read {key}: {e}"))),
        }
    }
}
