use serde::{Serialize, Serializer};

pub type Result<T, E = Error> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("database error: {0}")]
    Db(#[from] rusqlite::Error),

    #[error("file error: {0}")]
    Io(#[from] std::io::Error),

    #[error("data error: {0}")]
    Json(#[from] serde_json::Error),

    #[error(transparent)]
    Tauri(#[from] tauri::Error),

    #[error("image error: {0}")]
    Image(#[from] image::ImageError),

    #[error("screen capture failed: {0}")]
    Capture(String),

    #[error("text recognition unavailable: {0}")]
    OcrUnavailable(String),

    #[error("text recognition failed: {0}")]
    Ocr(String),

    #[error("couldn't register shortcut {accelerator}: {reason}")]
    Hotkey { accelerator: String, reason: String },

    #[error("{0}")]
    Platform(String),

    #[error("{0}")]
    Invalid(String),
}

/// Commands return errors to the UI as plain user-facing strings.
impl Serialize for Error {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}
