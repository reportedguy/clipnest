//! IPC commands for browsing and managing saved screenshots and categories.
//! Keep `src/lib/ipc.ts` in sync. Every change broadcasts `library-changed`.

use std::path::Path;

use serde::Serialize;
use tauri::image::Image;
use tauri::{AppHandle, Emitter, Runtime, State};
use tauri_plugin_clipboard_manager::ClipboardExt as _;
use tauri_plugin_opener::OpenerExt as _;

use crate::capture::session::LIBRARY_CHANGED_EVENT;
use crate::db::categories::{self, CategoryInput};
use crate::db::feed::{FeedQuery, TagCount};
use crate::db::screenshots::{self, Card};
use crate::error::{Error, Result};
use crate::state::AppState;
use crate::util::now_ms;

/// Upper bound on cards returned by one `list_cards` call.
const MAX_PAGE: i64 = 1000;
/// How long a deleted screenshot can still be restored before it's purged for good.
pub const TRASH_KEEP_MS: i64 = 24 * 60 * 60 * 1000;

fn changed<R: Runtime>(app: &AppHandle<R>) {
    if let Err(e) = app.emit(LIBRARY_CHANGED_EVENT, ()) {
        tracing::warn!(error = %e, "couldn't broadcast library change");
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FeedPage {
    cards: Vec<Card>,
    /// How many cards match in total (for "Show more" and the heading).
    total: i64,
}

/// The home feed: newest first, narrowed by category/tag filters (all must match) and
/// the header's text search.
#[tauri::command]
pub fn feed(state: State<'_, AppState>, query: FeedQuery, offset: i64, limit: i64) -> Result<FeedPage> {
    let db = state.db();
    Ok(FeedPage {
        cards: crate::db::feed::feed(&db, &query, limit.clamp(1, MAX_PAGE), offset.max(0))?,
        total: crate::db::feed::count(&db, &query)?,
    })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FilterOptions {
    categories: Vec<categories::Category>,
    tags: Vec<TagCount>,
}

/// The tags to offer for the selected categories (all tags when none are selected).
#[tauri::command]
pub fn tag_options(state: State<'_, AppState>, category_ids: Vec<i64>) -> Result<Vec<TagCount>> {
    crate::db::feed::tag_counts_in(&state.db(), &category_ids)
}

/// Everything the filter bar can offer: all categories and all tags, with counts.
#[tauri::command]
pub fn filter_options(state: State<'_, AppState>) -> Result<FilterOptions> {
    let db = state.db();
    Ok(FilterOptions { categories: categories::list(&db)?, tags: crate::db::feed::tag_counts(&db)? })
}

/// Sets a card's categories by hand. Hand-picked categories are final: the AI won't
/// change them later.
#[tauri::command]
pub fn set_card_categories<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: i64,
    category_ids: Vec<i64>,
) -> Result<()> {
    screenshots::set_categories(&state.db(), id, &category_ids, true, now_ms())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn update_card_text<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: i64,
    title: String,
    summary: String,
) -> Result<()> {
    if title.trim().is_empty() {
        return Err(Error::Invalid("The title can't be empty.".into()));
    }
    screenshots::update_text(&state.db(), id, &title, &summary, now_ms())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn update_card_note<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: i64,
    note: String,
) -> Result<()> {
    screenshots::update_note(&state.db(), id, &note, now_ms())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn delete_card<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, id: i64) -> Result<()> {
    screenshots::soft_delete(&state.db(), id, now_ms())?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn restore_card<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, id: i64) -> Result<()> {
    screenshots::restore(&state.db(), id)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn retry_card<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, id: i64) -> Result<()> {
    screenshots::requeue(&state.db(), id, now_ms())?;
    changed(&app);
    crate::queue::wake(&app);
    Ok(())
}

fn image_file(state: &AppState, id: i64) -> Result<std::path::PathBuf> {
    let card = screenshots::get(&state.db(), id)?;
    crate::media::resolve(&state.data_dir, &card.image_path)
        .filter(|p| p.exists())
        .ok_or_else(|| Error::Invalid("The image file is missing.".into()))
}

/// Copies the full-resolution image to the clipboard (only ever on explicit request).
#[tauri::command]
pub async fn copy_card_image<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, id: i64) -> Result<()> {
    let img = image::open(image_file(&state, id)?)?.to_rgba8();
    let (w, h) = img.dimensions();
    app.clipboard()
        .write_image(&Image::new_owned(img.into_raw(), w, h))
        .map_err(|e| Error::Platform(format!("couldn't copy the image: {e}")))
}

/// Opens the image in the user's default image viewer.
#[tauri::command]
pub fn open_card_image<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, id: i64) -> Result<()> {
    let path = image_file(&state, id)?;
    app.opener()
        .open_path(path.to_string_lossy(), None::<&str>)
        .map_err(|e| Error::Platform(format!("couldn't open the image: {e}")))
}

/// Opens a link from a card in the default browser. Only http(s) URLs, and only on an
/// explicit click; nothing from a screenshot is ever opened or run automatically.
#[tauri::command]
pub fn open_url<R: Runtime>(app: AppHandle<R>, url: String) -> Result<()> {
    let url = url.trim();
    let lower = url.to_ascii_lowercase();
    if !(lower.starts_with("https://") || lower.starts_with("http://")) || url.chars().any(char::is_whitespace) {
        return Err(Error::Invalid("Only web links can be opened.".into()));
    }
    app.opener().open_url(url, None::<&str>).map_err(|e| Error::Platform(format!("couldn't open the link: {e}")))
}

#[tauri::command]
pub fn create_category<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, input: CategoryInput) -> Result<i64> {
    let id = categories::create(&state.db(), &input, now_ms())?;
    changed(&app);
    Ok(id)
}

#[tauri::command]
pub fn update_category<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: i64,
    input: CategoryInput,
) -> Result<()> {
    categories::update(&state.db(), id, &input)?;
    changed(&app);
    Ok(())
}

#[tauri::command]
pub fn reorder_categories<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, ids: Vec<i64>) -> Result<()> {
    categories::reorder(&state.db(), &ids)?;
    changed(&app);
    Ok(())
}

/// Deletes a category; its screenshots move to Miscellaneous. Returns how many moved.
#[tauri::command]
pub fn delete_category<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, id: i64) -> Result<usize> {
    let moved = categories::delete(&state.db(), id, now_ms())?;
    changed(&app);
    Ok(moved)
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    data_dir: String,
    bytes: u64,
    screenshots: i64,
}

#[tauri::command]
pub async fn storage_info(state: State<'_, AppState>) -> Result<StorageInfo> {
    let screenshots =
        state.db().query_row("SELECT count(*) FROM screenshots WHERE deleted_at IS NULL", [], |r| r.get(0))?;
    Ok(StorageInfo { data_dir: state.data_dir.display().to_string(), bytes: dir_size(&state.data_dir), screenshots })
}

fn dir_size(dir: &Path) -> u64 {
    std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| match e.metadata() {
            Ok(m) if m.is_dir() => dir_size(&e.path()),
            Ok(m) => m.len(),
            Err(_) => 0,
        })
        .sum()
}

/// Startup housekeeping: permanently removes screenshots deleted more than a day ago.
pub fn purge_trash(state: &AppState) {
    match screenshots::purge_deleted(&state.db(), now_ms() - TRASH_KEEP_MS) {
        Ok(files) => {
            for rel in &files {
                if let Some(p) = crate::media::resolve(&state.data_dir, rel) {
                    let _ = std::fs::remove_file(p);
                }
            }
            if !files.is_empty() {
                tracing::info!(files = files.len(), "purged deleted screenshots");
            }
        }
        Err(e) => tracing::warn!(error = %e, "couldn't purge deleted screenshots"),
    }
}

/// Development only: fills queued screenshots with placeholder AI results so the UI can
/// be exercised without the AI pipeline. Refuses to run in release builds.
#[tauri::command]
pub fn dev_fake_ai<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<usize> {
    if !cfg!(debug_assertions) {
        return Err(Error::Invalid("Only available in development builds.".into()));
    }
    let n = state.db().execute(
        r#"UPDATE screenshots SET
             status = 'done',
             title = 'Sample title for screenshot ' || id,
             summary = 'A placeholder summary written by the development stub. The real one comes from the local AI.',
             tags_json = '["sample","dev"]',
             urls_json = '["https://example.org/page"]',
             ocr_text = COALESCE(ocr_text, 'Text read from the screenshot will appear here.')
           WHERE status = 'queued' AND deleted_at IS NULL"#,
        [],
    )?;
    changed(&app);
    Ok(n)
}
