//! Turning a cropped capture into files on disk and a row in the database.
//!
//! Work is split so that pressing Enter is instant:
//! 1. `prepare` runs as soon as the selection is made, while the user types their note. It
//!    writes the thumbnail right away and the full-resolution PNG on a background thread,
//!    both into `pending/`.
//! 2. `commit` (on Enter) waits for the PNG if it isn't done yet, moves both files into
//!    the library, and inserts the row with status `queued` for the AI worker.
//! 3. `discard` (on Esc) deletes the pending files.

use std::fs;
use std::path::{Path, PathBuf};
use std::thread::JoinHandle;

use chrono::{DateTime, Utc};
use image::codecs::webp::WebPEncoder;
use image::imageops::FilterType;
use image::{ExtendedColorType, ImageEncoder, RgbaImage};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

pub const PENDING_DIR: &str = "pending";
pub const THUMB_WIDTH: u32 = 480;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    Text,
    Image,
}

impl Mode {
    fn as_str(self) -> &'static str {
        match self {
            Mode::Text => "text",
            Mode::Image => "image",
        }
    }
}

/// A capture waiting for the user to press Enter or Esc.
pub struct Prepared {
    pub width: u32,
    pub height: u32,
    /// The thumbnail bytes (WebP), also shown in the popup.
    pub thumb_webp: Vec<u8>,
    thumb_path: PathBuf,
    image_path: PathBuf,
    encode: Option<JoinHandle<Result<()>>>,
}

pub struct NewScreenshot<'a> {
    pub note: &'a str,
    pub mode: Mode,
    /// Categories picked in the popup. Empty means "Auto": the AI picks them later.
    pub category_ids: &'a [i64],
}

/// Starts writing a capture to `data_dir/pending`. Returns once the thumbnail exists;
/// the full PNG keeps encoding in the background.
pub fn prepare(data_dir: &Path, key: &str, crop: RgbaImage) -> Result<Prepared> {
    let pending = data_dir.join(PENDING_DIR);
    fs::create_dir_all(&pending)?;
    let (width, height) = crop.dimensions();

    let thumb_webp = encode_thumbnail(&crop)?;
    let thumb_path = pending.join(format!("{key}.webp"));
    fs::write(&thumb_path, &thumb_webp)?;

    let image_path = pending.join(format!("{key}.png"));
    let target = image_path.clone();
    let encode = std::thread::Builder::new()
        .name("capture-encode".into())
        .spawn(move || crop.save_with_format(&target, image::ImageFormat::Png).map_err(Error::from))?;

    Ok(Prepared { width, height, thumb_webp, thumb_path, image_path, encode: Some(encode) })
}

/// Saves a prepared capture into the library and queues it for AI processing.
/// On failure the pending files are kept, so the user can retry.
pub fn commit(
    conn: &Connection,
    data_dir: &Path,
    p: &mut Prepared,
    s: &NewScreenshot,
    now: DateTime<Utc>,
) -> Result<i64> {
    if let Some(job) = p.encode.take() {
        job.join().map_err(|_| Error::Capture("image encoder crashed".into()))??;
    }

    let locked = !s.category_ids.is_empty();
    let categories: Vec<i64> =
        if locked { s.category_ids.to_vec() } else { vec![crate::db::categories::misc_id(conn)?] };
    let base = p.image_path.file_stem().and_then(|s| s.to_str()).unwrap_or("capture").to_owned();
    let month = now.format("%Y-%m").to_string();
    // Never overwrite an existing capture (rename replaces silently on Windows).
    let (image_rel, thumb_rel) = (0..)
        .map(|n| if n == 0 { base.clone() } else { format!("{base}-{n}") })
        .map(|stem| (format!("images/{month}/{stem}.png"), format!("thumbs/{month}/{stem}.webp")))
        .find(|(img, thumb)| !data_dir.join(img).exists() && !data_dir.join(thumb).exists())
        .expect("an unused name always exists");
    let image_abs = data_dir.join(&image_rel);
    let thumb_abs = data_dir.join(&thumb_rel);
    fs::create_dir_all(image_abs.parent().expect("has parent"))?;
    fs::create_dir_all(thumb_abs.parent().expect("has parent"))?;

    fs::rename(&p.image_path, &image_abs)?;
    if let Err(e) = fs::rename(&p.thumb_path, &thumb_abs) {
        let _ = fs::rename(&image_abs, &p.image_path);
        return Err(e.into());
    }

    let ts = now.timestamp_millis();
    // Row and memberships together: a bad category id must not leave a half-saved card.
    let inserted = (|| -> Result<i64> {
        let tx = conn.unchecked_transaction()?;
        tx.execute(
            "INSERT INTO screenshots (category_id, category_locked, image_path, thumb_path, width, height,
                                      user_note, mode, status, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, 'queued', ?9, ?9)",
            (categories[0], locked, &image_rel, &thumb_rel, p.width, p.height, s.note.trim(), s.mode.as_str(), ts),
        )?;
        let id = tx.last_insert_rowid();
        crate::db::screenshots::write_categories(&tx, id, &categories, locked, ts)?;
        tx.commit()?;
        Ok(id)
    })();
    match inserted {
        Ok(id) => Ok(id),
        Err(e) => {
            // Put the files back so a retry (or Esc) still finds them.
            let _ = fs::rename(&image_abs, &p.image_path);
            let _ = fs::rename(&thumb_abs, &p.thumb_path);
            Err(e)
        }
    }
}

/// Drops a prepared capture and its files.
pub fn discard(mut p: Prepared) {
    if let Some(job) = p.encode.take() {
        let _ = job.join();
    }
    let _ = fs::remove_file(&p.image_path);
    let _ = fs::remove_file(&p.thumb_path);
}

/// Removes leftovers from captures that were open when the app last exited.
pub fn clear_pending(data_dir: &Path) {
    let dir = data_dir.join(PENDING_DIR);
    if dir.exists() {
        if let Err(e) = fs::remove_dir_all(&dir) {
            tracing::warn!(error = %e, "couldn't clear pending captures");
        }
    }
}

fn encode_thumbnail(img: &RgbaImage) -> Result<Vec<u8>> {
    let thumb = if img.width() > THUMB_WIDTH {
        let h = ((u64::from(img.height()) * u64::from(THUMB_WIDTH)) / u64::from(img.width())).max(1) as u32;
        image::imageops::resize(img, THUMB_WIDTH, h, FilterType::Triangle)
    } else {
        img.clone()
    };
    let mut out = Vec::new();
    WebPEncoder::new_lossless(&mut out).write_image(
        thumb.as_raw(),
        thumb.width(),
        thumb.height(),
        ExtendedColorType::Rgba8,
    )?;
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    fn temp_dir(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("rs-store-test-{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn sample(w: u32, h: u32) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| image::Rgba([(x % 256) as u8, (y % 256) as u8, 128, 255]))
    }

    #[test]
    fn commit_auto_files_into_misc_unlocked_and_queued() {
        let dir = temp_dir("auto");
        let conn = open_in_memory().unwrap();
        let mut p = prepare(&dir, "20261015T120000-1", sample(1000, 400)).unwrap();
        let now = DateTime::parse_from_rfc3339("2026-10-15T12:00:00Z").unwrap().with_timezone(&Utc);
        let id = commit(
            &conn,
            &dir,
            &mut p,
            &NewScreenshot { note: "  watch later ", mode: Mode::Text, category_ids: &[] },
            now,
        )
        .unwrap();

        let (cat, locked, img, thumb, w, note, status): (String, bool, String, String, u32, String, String) = conn
            .query_row(
                "SELECT c.name, s.category_locked, s.image_path, s.thumb_path, s.width, s.user_note, s.status
                 FROM screenshots s JOIN categories c ON c.id = s.category_id WHERE s.id = ?1",
                [id],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?, r.get(6)?)),
            )
            .unwrap();
        assert_eq!(
            (cat.as_str(), locked, w, note.as_str(), status.as_str()),
            ("Miscellaneous", false, 1000, "watch later", "queued")
        );
        assert_eq!(img, "images/2026-10/20261015T120000-1.png");
        assert_eq!(thumb, "thumbs/2026-10/20261015T120000-1.webp");

        // Full-resolution PNG and a 480-wide thumbnail are on disk; pending is empty.
        assert_eq!(image::open(dir.join(&img)).unwrap().width(), 1000);
        assert_eq!(image::open(dir.join(&thumb)).unwrap().width(), THUMB_WIDTH);
        assert_eq!(fs::read_dir(dir.join(PENDING_DIR)).unwrap().count(), 0);
    }

    #[test]
    fn explicit_category_is_locked() {
        let dir = temp_dir("locked");
        let conn = open_in_memory().unwrap();
        let mut p = prepare(&dir, "k", sample(50, 50)).unwrap();
        let id =
            commit(&conn, &dir, &mut p, &NewScreenshot { note: "", mode: Mode::Image, category_ids: &[2] }, Utc::now())
                .unwrap();
        let (cat, locked, mode): (i64, bool, String) = conn
            .query_row("SELECT category_id, category_locked, mode FROM screenshots WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((cat, locked, mode.as_str()), (2, true, "image"));
    }

    #[test]
    fn failed_insert_keeps_pending_files_for_retry() {
        let dir = temp_dir("fail");
        let conn = open_in_memory().unwrap();
        let mut p = prepare(&dir, "k", sample(50, 50)).unwrap();
        // A category id that doesn't exist violates the foreign key.
        let err = commit(
            &conn,
            &dir,
            &mut p,
            &NewScreenshot { note: "", mode: Mode::Text, category_ids: &[999] },
            Utc::now(),
        );
        assert!(err.is_err());
        // The capture is still usable: a retry with a valid category succeeds.
        commit(&conn, &dir, &mut p, &NewScreenshot { note: "", mode: Mode::Text, category_ids: &[] }, Utc::now())
            .unwrap();
    }

    #[test]
    fn never_overwrites_an_existing_capture() {
        let dir = temp_dir("collide");
        let conn = open_in_memory().unwrap();
        let auto = |n| NewScreenshot { note: n, mode: Mode::Text, category_ids: &[] };
        let mut a = prepare(&dir, "same", sample(20, 20)).unwrap();
        let first = commit(&conn, &dir, &mut a, &auto("a"), Utc::now()).unwrap();
        let mut b = prepare(&dir, "same", sample(30, 30)).unwrap();
        let second = commit(&conn, &dir, &mut b, &auto("b"), Utc::now()).unwrap();

        let path = |id: i64| -> String {
            conn.query_row("SELECT image_path FROM screenshots WHERE id = ?1", [id], |r| r.get(0)).unwrap()
        };
        assert_ne!(path(first), path(second));
        assert!(path(second).ends_with("same-1.png"));
        assert_eq!(image::open(dir.join(path(first))).unwrap().width(), 20); // first is intact
    }

    #[test]
    fn discard_removes_files() {
        let dir = temp_dir("discard");
        let p = prepare(&dir, "k", sample(50, 50)).unwrap();
        discard(p);
        assert_eq!(fs::read_dir(dir.join(PENDING_DIR)).unwrap().count(), 0);
    }

    #[test]
    fn small_captures_are_not_upscaled() {
        let thumb = encode_thumbnail(&sample(100, 30)).unwrap();
        let decoded = image::load_from_memory(&thumb).unwrap();
        assert_eq!((decoded.width(), decoded.height()), (100, 30));
    }
}
