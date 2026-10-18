//! "Export all data": one zip with every screenshot (original PNGs) plus an index in JSON
//! (for other tools) and Markdown (for people). Nothing is uploaded; the zip is saved to
//! the Downloads folder and shown in Explorer.

use std::fs::File;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use chrono::{Local, TimeZone};
use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_opener::OpenerExt as _;
use zip::write::SimpleFileOptions;
use zip::{CompressionMethod, ZipWriter};

use crate::db::screenshots::{card_from_row, Card, CARD_COLUMNS};
use crate::error::{Error, Result};
use crate::state::AppState;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Entry<'a> {
    id: i64,
    category: &'a str,
    categories: Vec<&'a str>,
    title: Option<&'a str>,
    summary: Option<&'a str>,
    note: &'a str,
    text_from_screenshot: Option<&'a str>,
    tags: &'a [String],
    links: &'a [String],
    deadline: Option<String>,
    deadline_label: Option<&'a str>,
    saved_at: String,
    image: String,
}

fn local(ms: i64) -> String {
    Local.timestamp_millis_opt(ms).single().map(|t| t.format("%Y-%m-%d %H:%M").to_string()).unwrap_or_default()
}

fn all_cards(state: &AppState) -> Result<Vec<Card>> {
    let db = state.db();
    let mut stmt = db.prepare(&format!(
        "SELECT {CARD_COLUMNS} FROM screenshots s JOIN categories c ON c.id = s.category_id
         WHERE s.deleted_at IS NULL ORDER BY c.sort_order, s.created_at DESC"
    ))?;
    let rows = stmt.query_map([], card_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Writes the export zip into `dir` and returns its path.
pub fn write_export(state: &AppState, dir: &Path) -> Result<PathBuf> {
    let cards = all_cards(state)?;
    let path = dir.join(format!("ClipNest export {}.zip", Local::now().format("%Y-%m-%d %H%M")));
    let mut zip = ZipWriter::new(File::create(&path)?);
    // PNGs are already compressed; storing them is much faster and no bigger.
    let stored = SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
    let deflated = SimpleFileOptions::default().compression_method(CompressionMethod::Deflated);
    let zip_err = |e: zip::result::ZipError| Error::Invalid(format!("couldn't write the export: {e}"));

    let mut entries = Vec::with_capacity(cards.len());
    let mut buf = Vec::new();
    for c in &cards {
        let file_name = Path::new(&c.image_path).file_name().and_then(|n| n.to_str()).unwrap_or("image.png").to_owned();
        let image = format!("images/{}/{file_name}", sanitize(&c.category_name));
        if let Some(src) = crate::media::resolve(&state.data_dir, &c.image_path) {
            buf.clear();
            if File::open(src).and_then(|mut f| f.read_to_end(&mut buf)).is_ok() {
                zip.start_file(&image, stored).map_err(zip_err)?;
                zip.write_all(&buf)?;
            }
        }
        entries.push(Entry {
            id: c.id,
            category: &c.category_name,
            categories: c.categories.iter().map(|x| x.name.as_str()).collect(),
            title: c.title.as_deref(),
            summary: c.summary.as_deref(),
            note: &c.user_note,
            text_from_screenshot: c.ocr_text.as_deref(),
            tags: &c.tags,
            links: &c.urls,
            deadline: c.deadline_at.filter(|_| c.deadline_important).map(local),
            deadline_label: c.deadline_label.as_deref(),
            saved_at: local(c.created_at),
            image,
        });
    }

    zip.start_file("index.json", deflated).map_err(zip_err)?;
    zip.write_all(serde_json::to_string_pretty(&entries)?.as_bytes())?;
    zip.start_file("index.md", deflated).map_err(zip_err)?;
    zip.write_all(markdown(&entries).as_bytes())?;
    zip.finish().map_err(zip_err)?;
    tracing::info!(path = %path.display(), screenshots = entries.len(), "exported library");
    Ok(path)
}

fn markdown(entries: &[Entry]) -> String {
    let mut md =
        format!("# ClipNest export\n\nExported {}. {} screenshots.\n", Local::now().format("%-d %B %Y"), entries.len());
    let mut current = "";
    for e in entries {
        if e.category != current {
            current = e.category;
            md.push_str(&format!("\n## {current}\n"));
        }
        md.push_str(&format!("\n### {}\n\n", e.title.unwrap_or("(not filed yet)")));
        md.push_str(&format!("![]({})\n\n", e.image.replace(' ', "%20")));
        if let Some(s) = e.summary {
            md.push_str(&format!("{s}\n\n"));
        }
        if e.categories.len() > 1 {
            md.push_str(&format!("**Categories:** {}\n\n", e.categories.join(", ")));
        }
        if !e.note.is_empty() {
            md.push_str(&format!("**Your note:** {}\n\n", e.note));
        }
        if let Some(d) = &e.deadline {
            md.push_str(&format!(
                "**Deadline:** {d}{}\n\n",
                e.deadline_label.map(|l| format!(" ({l})")).unwrap_or_default()
            ));
        }
        for link in e.links {
            md.push_str(&format!("- <{link}>\n"));
        }
        if !e.tags.is_empty() {
            md.push_str(&format!("\nTags: {}\n", e.tags.join(", ")));
        }
        md.push_str(&format!("\n_Saved {}_\n", e.saved_at));
    }
    md
}

/// Keeps folder names inside the zip valid on every OS.
fn sanitize(name: &str) -> String {
    name.chars()
        .map(|c| if c.is_alphanumeric() || " -_&".contains(c) { c } else { '_' })
        .collect::<String>()
        .trim()
        .to_owned()
}

#[tauri::command]
pub async fn export_all<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<String> {
    let dir = app.path().download_dir().unwrap_or_else(|_| state.data_dir.clone());
    let path = write_export(&state, &dir)?;
    let _ = app.opener().reveal_item_in_dir(&path);
    Ok(path.display().to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folder_names_are_safe() {
        assert_eq!(sanitize("Opportunities & Deadlines"), "Opportunities & Deadlines");
        assert_eq!(sanitize("a/b:c*?"), "a_b_c__");
    }
}
