//! Font registry — loads `.ttf` / `.otf` files from a directory at startup
//! and serves them by stem name. Wraps each as `FontArc` for cheap sharing.

use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use ab_glyph::FontArc;

use crate::application::ports::{FontHandle, FontProvider};
use crate::domain::error::DomainError;

pub struct AbGlyphFontHandle {
    pub font: FontArc,
}

impl FontHandle for AbGlyphFontHandle {
    fn as_any(&self) -> &dyn std::any::Any { self }
}

pub struct AbGlyphFontProvider {
    fonts: HashMap<String, Arc<dyn FontHandle>>,
    default_name: Option<String>,
}

impl AbGlyphFontProvider {
    pub fn empty() -> Self {
        Self { fonts: HashMap::new(), default_name: None }
    }

    pub fn load_from_dir(dir: &Path) -> anyhow::Result<Self> {
        let mut fonts: HashMap<String, Arc<dyn FontHandle>> = HashMap::new();
        let mut default_name: Option<String> = None;

        if !dir.exists() {
            tracing::warn!(
                dir = %dir.display(),
                "font dir does not exist; text watermark will fail. Set IMAGE_RS_FONT_DIR \
                 to a directory containing .ttf/.otf files."
            );
            return Ok(Self { fonts, default_name });
        }

        // Collect entries first, sort by file name so the "default" font
        // (first loaded) is deterministic across reboots and across distros.
        let mut entries: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok())
            .map(|e| e.path())
            .filter(|p| {
                p.extension()
                    .and_then(|s| s.to_str())
                    .map(|s| matches!(s.to_ascii_lowercase().as_str(), "ttf" | "otf"))
                    .unwrap_or(false)
            })
            .collect();
        entries.sort();

        for path in &entries {
            let bytes = std::fs::read(path)?;
            let font = match FontArc::try_from_vec(bytes) {
                Ok(f) => f,
                Err(e) => {
                    tracing::warn!(path = %path.display(), error = %e, "skip unparseable font");
                    continue;
                }
            };
            let name = path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("font")
                .to_string();
            if default_name.is_none() {
                default_name = Some(name.clone());
            }
            fonts.insert(name, Arc::new(AbGlyphFontHandle { font }));
        }

        if fonts.is_empty() {
            tracing::warn!(
                dir = %dir.display(),
                "font dir contains no .ttf/.otf files; text watermark will fail"
            );
        } else {
            tracing::info!(
                dir = %dir.display(),
                count = fonts.len(),
                default = default_name.as_deref().unwrap_or("?"),
                "loaded font registry"
            );
        }
        Ok(Self { fonts, default_name })
    }
}

impl FontProvider for AbGlyphFontProvider {
    fn font(&self, name: &str) -> Result<Arc<dyn FontHandle>, DomainError> {
        self.fonts
            .get(name)
            .cloned()
            .ok_or_else(|| DomainError::invalid(format!("font not found: {name}")))
    }

    fn default_font(&self) -> Result<Arc<dyn FontHandle>, DomainError> {
        let name = self
            .default_name
            .as_deref()
            .ok_or_else(|| DomainError::invalid("no fonts loaded"))?;
        self.font(name)
    }
}
