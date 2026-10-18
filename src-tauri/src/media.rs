//! `rsmedia://` protocol: images for the webviews, served straight from Rust.
//!
//! - `thumb-<session>`: the in-progress capture's thumbnail (memory only).
//! - `lib/<relative path>`: a saved image or thumbnail under `images/` or `thumbs/` in the
//!   app data dir. Nothing outside those two folders can be read.

use std::path::{Component, Path, PathBuf};

use percent_encoding::percent_decode_str;
use tauri::http::{header, Response, StatusCode};
use tauri::{AppHandle, Manager, Runtime};

use crate::state::AppState;

pub const PROTOCOL: &str = "rsmedia";

pub fn serve<R: Runtime>(app: &AppHandle<R>, raw_path: &str) -> Response<Vec<u8>> {
    let path = percent_decode_str(raw_path.trim_start_matches('/')).decode_utf8_lossy().into_owned();
    let found = if let Some(session) = path.strip_prefix("thumb-") {
        session.parse().ok().and_then(|s| crate::capture::session::pending_thumb(app, s)).map(|b| (b, "image/webp"))
    } else if let Some(rel) = path.strip_prefix("lib/") {
        app.try_state::<AppState>().and_then(|state| library_file(&state.data_dir, rel))
    } else {
        None
    };
    let builder = Response::builder().header(header::CACHE_CONTROL, "no-store");
    match found {
        Some((bytes, mime)) => builder.header(header::CONTENT_TYPE, mime).body(bytes),
        None => builder.status(StatusCode::NOT_FOUND).body(Vec::new()),
    }
    .expect("static response parts are valid")
}

fn library_file(data_dir: &Path, rel: &str) -> Option<(Vec<u8>, &'static str)> {
    let full = resolve(data_dir, rel)?;
    let mime = match full.extension().and_then(|e| e.to_str()) {
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        _ => return None,
    };
    std::fs::read(full).ok().map(|b| (b, mime))
}

/// Joins a stored relative path onto the data dir, refusing anything that could escape
/// `images/` or `thumbs/` (absolute paths, `..`, drive prefixes).
pub fn resolve(data_dir: &Path, rel: &str) -> Option<PathBuf> {
    let rel = Path::new(rel);
    let mut parts = rel.components();
    let top = match parts.next() {
        Some(Component::Normal(t)) => t.to_str()?,
        _ => return None,
    };
    if !matches!(top, "images" | "thumbs") || !parts.all(|c| matches!(c, Component::Normal(_))) {
        return None;
    }
    Some(data_dir.join(rel))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_allows_only_library_folders() {
        let root = Path::new("C:/data");
        assert_eq!(resolve(root, "images/2026-10/a.png"), Some(root.join("images/2026-10/a.png")));
        assert!(resolve(root, "thumbs/2026-10/a.webp").is_some());
        assert!(resolve(root, "library.db").is_none());
        assert!(resolve(root, "images/../library.db").is_none());
        assert!(resolve(root, "../secret.png").is_none());
        assert!(resolve(root, "C:/Windows/x.png").is_none());
        assert!(resolve(root, "/images/a.png").is_none());
        assert!(resolve(root, "logs/x.log").is_none());
    }
}
