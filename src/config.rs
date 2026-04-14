//! Service configuration. Loaded once at startup.

use std::path::{Path, PathBuf};
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct Config {
    pub bind_addr: String,
    pub log_level: String,
    pub log_format: LogFormat,
    pub max_upload_bytes: usize,
    /// Decompression-bomb guard: cap on `width * height` of the decoded
    /// pixel buffer. Checked from the source header *before* `imdecode`
    /// allocates anything.
    pub max_pixels: u64,
    pub max_concurrent_jobs: usize,
    pub request_timeout: Duration,
    pub mask_cache_capacity: u64,
    /// Directory the font registry scans at startup. If
    /// `IMAGE_RS_FONT_DIR` is unset, the resolver tries a fallback chain
    /// of common system font directories — see `default_font_dir`.
    pub font_dir: PathBuf,
    /// Root for the GET /v1/img/{key} endpoint's source image lookup.
    pub image_store_root: PathBuf,
    /// Capacity (in entries, not bytes) for the result cache that fronts
    /// the GET endpoint.
    pub result_cache_capacity: u64,
    /// Static file root served at `/ui/`. Points at the Vite production
    /// build output (`web/dist`).
    pub ui_dir: PathBuf,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

impl LogFormat {
    fn parse(s: &str) -> Self {
        match s.to_ascii_lowercase().as_str() {
            "json" => LogFormat::Json,
            _ => LogFormat::Text,
        }
    }
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let font_dir = match std::env::var("IMAGE_RS_FONT_DIR") {
            Ok(s) => PathBuf::from(s),
            Err(_) => default_font_dir(),
        };
        Ok(Self {
            bind_addr: env_or("IMAGE_RS_BIND", "0.0.0.0:8080"),
            log_level: env_or("IMAGE_RS_LOG", "info,tower_http=info"),
            log_format: LogFormat::parse(&env_or("IMAGE_RS_LOG_FORMAT", "text")),
            max_upload_bytes: env_or("IMAGE_RS_MAX_UPLOAD", "20971520").parse()?,
            // 64 megapixels — covers 8K (33 MP) with margin and rejects
            // anything that would require > ~256 MB BGRA scratch.
            max_pixels: env_or("IMAGE_RS_MAX_PIXELS", "67108864").parse()?,
            max_concurrent_jobs: env_or(
                "IMAGE_RS_MAX_CONCURRENCY",
                &(num_cpus::get() * 2).to_string(),
            )
            .parse()?,
            request_timeout: Duration::from_secs(
                env_or("IMAGE_RS_REQUEST_TIMEOUT_SECS", "30").parse()?,
            ),
            mask_cache_capacity: env_or("IMAGE_RS_MASK_CACHE", "256").parse()?,
            font_dir,
            image_store_root: PathBuf::from(env_or("IMAGE_RS_IMAGE_STORE", "./images")),
            result_cache_capacity: env_or("IMAGE_RS_RESULT_CACHE", "1024").parse()?,
            ui_dir: PathBuf::from(env_or("IMAGE_RS_UI_DIR", "./web/dist")),
        })
    }
}

/// Common system font directories, in priority order. The first one that
/// exists *and* contains at least one `.ttf`/`.otf` file wins. The local
/// `./fonts` is checked first so a developer can override system fonts by
/// dropping a font into the project directory.
const FONT_DIR_CANDIDATES: &[&str] = &[
    "./fonts",
    "/usr/share/fonts/truetype/dejavu",       // Debian / Ubuntu
    "/usr/share/fonts/truetype/noto",         // Debian / Ubuntu CJK
    "/usr/share/fonts/dejavu-sans-fonts",     // RHEL / Fedora / Alma / Rocky
    "/usr/share/fonts/google-noto-cjk",       // RHEL / Fedora CJK
    "/usr/share/fonts/dejavu",                // Arch
    "/usr/share/fonts/noto-cjk",              // Arch CJK
    "/usr/share/fonts/TTF",                   // openSUSE / generic
    "/Library/Fonts",                         // macOS
    "/usr/share/fonts",                       // last-resort scan root
];

pub fn default_font_dir() -> PathBuf {
    first_usable_font_dir(FONT_DIR_CANDIDATES).unwrap_or_else(|| PathBuf::from("./fonts"))
}

fn first_usable_font_dir<P: AsRef<Path>>(candidates: &[P]) -> Option<PathBuf> {
    for c in candidates {
        let p = c.as_ref();
        if dir_has_font(p) {
            return Some(p.to_path_buf());
        }
    }
    None
}

fn dir_has_font(p: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(p) else {
        return false;
    };
    for entry in rd.flatten() {
        if let Some(ext) = entry.path().extension().and_then(|s| s.to_str()) {
            if matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "otf") {
                return true;
            }
        }
    }
    false
}

fn env_or(key: &str, default: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| default.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn first_usable_font_dir_picks_first_match() {
        let tmp = std::env::temp_dir();
        let test_dir = tmp.join("image_rs_font_test");
        let _ = std::fs::remove_dir_all(&test_dir);
        std::fs::create_dir_all(&test_dir).unwrap();
        std::fs::write(test_dir.join("fake.ttf"), b"not a real font").unwrap();

        let candidates = vec![
            PathBuf::from("/this/path/definitely/does/not/exist"),
            test_dir.clone(),
            PathBuf::from("/another/missing/path"),
        ];
        assert_eq!(first_usable_font_dir(&candidates), Some(test_dir.clone()));
        std::fs::remove_dir_all(&test_dir).ok();
    }

    #[test]
    fn first_usable_font_dir_skips_dir_without_fonts() {
        let tmp = std::env::temp_dir();
        let empty = tmp.join("image_rs_empty_dir_test");
        let _ = std::fs::remove_dir_all(&empty);
        std::fs::create_dir_all(&empty).unwrap();
        std::fs::write(empty.join("not-a-font.txt"), b"hello").unwrap();
        let candidates = vec![empty.clone()];
        assert_eq!(first_usable_font_dir(&candidates), None);
        std::fs::remove_dir_all(&empty).ok();
    }

    #[test]
    fn first_usable_font_dir_returns_none_when_all_missing() {
        let candidates: Vec<PathBuf> = vec![
            PathBuf::from("/nonexistent/a"),
            PathBuf::from("/nonexistent/b"),
        ];
        assert_eq!(first_usable_font_dir(&candidates), None);
    }
}

