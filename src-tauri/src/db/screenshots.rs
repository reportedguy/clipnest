//! Screenshot ("card") queries. Soft-deleted rows (`deleted_at` set) are invisible
//! everywhere except `restore` and `purge_deleted`.

use rusqlite::{Connection, OptionalExtension, Row};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Card {
    pub id: i64,
    pub category_id: i64,
    pub category_name: String,
    pub category_color: String,
    pub image_path: String,
    pub thumb_path: String,
    pub width: u32,
    pub height: u32,
    pub user_note: String,
    pub ocr_text: Option<String>,
    pub title: Option<String>,
    pub summary: Option<String>,
    pub tags: Vec<String>,
    pub urls: Vec<String>,
    pub mode: String,
    pub auto_upgraded: bool,
    pub category_locked: bool,
    pub status: String,
    pub error: Option<String>,
    pub deadline_at: Option<i64>,
    pub deadline_label: Option<String>,
    pub deadline_done: bool,
    /// Shown in the strip and reminded (intent to act, or set by hand).
    pub deadline_important: bool,
    pub created_at: i64,
    /// Every category the card is in, primary (best match) first.
    pub categories: Vec<CardCategory>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CardCategory {
    pub id: i64,
    pub name: String,
    pub color: String,
}

pub(crate) const CARD_COLUMNS: &str =
    "s.id, s.category_id, c.name, c.color, s.image_path, s.thumb_path, s.width, s.height,
     s.user_note, s.ocr_text, s.title, s.summary, s.tags_json, s.urls_json, s.mode, s.auto_upgraded,
     s.category_locked, s.status, s.error, s.deadline_at, s.deadline_label, s.deadline_done, s.created_at,
     s.deadline_important,
     (SELECT json_group_array(json_object('id', x.id, 'name', x.name, 'color', x.color)) FROM (
         SELECT c2.id, c2.name, c2.color FROM screenshot_categories sc JOIN categories c2 ON c2.id = sc.category_id
         WHERE sc.screenshot_id = s.id ORDER BY c2.id != s.category_id, c2.sort_order, c2.id) x)";

pub(crate) fn card_from_row(r: &Row) -> rusqlite::Result<Card> {
    let json_list = |i: usize| -> rusqlite::Result<Vec<String>> {
        let raw: String = r.get(i)?;
        Ok(serde_json::from_str(&raw).unwrap_or_default())
    };
    Ok(Card {
        id: r.get(0)?,
        category_id: r.get(1)?,
        category_name: r.get(2)?,
        category_color: r.get(3)?,
        image_path: r.get(4)?,
        thumb_path: r.get(5)?,
        width: r.get(6)?,
        height: r.get(7)?,
        user_note: r.get(8)?,
        ocr_text: r.get(9)?,
        title: r.get(10)?,
        summary: r.get(11)?,
        tags: json_list(12)?,
        urls: json_list(13)?,
        mode: r.get(14)?,
        auto_upgraded: r.get(15)?,
        category_locked: r.get(16)?,
        status: r.get(17)?,
        error: r.get(18)?,
        deadline_at: r.get(19)?,
        deadline_label: r.get(20)?,
        deadline_done: r.get(21)?,
        created_at: r.get(22)?,
        deadline_important: r.get(23)?,
        categories: serde_json::from_str(&r.get::<_, String>(24)?).unwrap_or_default(),
    })
}

pub fn get(conn: &Connection, id: i64) -> Result<Card> {
    conn.query_row(
        &format!(
            "SELECT {CARD_COLUMNS} FROM screenshots s JOIN categories c ON c.id = s.category_id
             WHERE s.id = ?1 AND s.deleted_at IS NULL"
        ),
        [id],
        card_from_row,
    )
    .optional()?
    .ok_or_else(not_found)
}

fn not_found() -> Error {
    Error::Invalid("That screenshot no longer exists.".into())
}

fn touch(conn: &Connection, sql: &str, params: impl rusqlite::Params) -> Result<()> {
    match conn.execute(sql, params)? {
        0 => Err(not_found()),
        _ => Ok(()),
    }
}

/// Moves a card. A manual move is final: the AI won't move it again.
/// Replaces a card's categories. Rules: duplicates are dropped; no categories means
/// Miscellaneous; Miscellaneous never sits alongside a real category; the first category
/// becomes the primary one. `locked` marks a choice the AI must never override.
pub fn set_categories(conn: &Connection, id: i64, ids: &[i64], locked: bool, now: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    write_categories(&tx, id, ids, locked, now)?;
    tx.commit()?;
    Ok(())
}

/// `set_categories` without its own transaction, for callers already inside one.
pub(crate) fn write_categories(conn: &Connection, id: i64, ids: &[i64], locked: bool, now: i64) -> Result<()> {
    let misc = crate::db::categories::misc_id(conn)?;
    let mut chosen: Vec<i64> = Vec::new();
    for &c in ids {
        if !chosen.contains(&c) {
            chosen.push(c);
        }
    }
    if chosen.len() > 1 {
        chosen.retain(|&c| c != misc);
    }
    if chosen.is_empty() {
        chosen.push(misc);
    }
    let n = conn.execute(
        "UPDATE screenshots SET category_id = ?2, category_locked = (category_locked OR ?3), updated_at = ?4
         WHERE id = ?1 AND deleted_at IS NULL",
        (id, chosen[0], locked, now),
    )?;
    if n == 0 {
        return Err(not_found());
    }
    conn.execute("DELETE FROM screenshot_categories WHERE screenshot_id = ?1", [id])?;
    for c in &chosen {
        conn.execute("INSERT INTO screenshot_categories (screenshot_id, category_id) VALUES (?1, ?2)", (id, c))
            .map_err(|_| Error::Invalid("One of those categories no longer exists.".into()))?;
    }
    Ok(())
}

/// Current category ids of a card, primary first.
pub fn category_ids(conn: &Connection, id: i64) -> Result<Vec<i64>> {
    let mut stmt = conn.prepare_cached(
        "SELECT sc.category_id FROM screenshot_categories sc JOIN screenshots s ON s.id = sc.screenshot_id
         JOIN categories c ON c.id = sc.category_id
         WHERE sc.screenshot_id = ?1 ORDER BY sc.category_id != s.category_id, c.sort_order",
    )?;
    let rows = stmt.query_map([id], |r| r.get(0))?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn update_text(conn: &Connection, id: i64, title: &str, summary: &str, now: i64) -> Result<()> {
    touch(
        conn,
        "UPDATE screenshots SET title = ?2, summary = ?3, text_locked = 1, updated_at = ?4
         WHERE id = ?1 AND deleted_at IS NULL",
        (id, title.trim(), summary.trim(), now),
    )
}

pub fn update_note(conn: &Connection, id: i64, note: &str, now: i64) -> Result<()> {
    touch(
        conn,
        "UPDATE screenshots SET user_note = ?2, updated_at = ?3 WHERE id = ?1 AND deleted_at IS NULL",
        (id, note.trim(), now),
    )
}

pub fn soft_delete(conn: &Connection, id: i64, now: i64) -> Result<()> {
    touch(conn, "UPDATE screenshots SET deleted_at = ?2 WHERE id = ?1 AND deleted_at IS NULL", (id, now))
}

pub fn restore(conn: &Connection, id: i64) -> Result<()> {
    touch(conn, "UPDATE screenshots SET deleted_at = NULL WHERE id = ?1 AND deleted_at IS NOT NULL", [id])
}

/// Puts a card back in the AI queue (e.g. after a failure, or "Retry AI").
pub fn requeue(conn: &Connection, id: i64, now: i64) -> Result<()> {
    touch(
        conn,
        "UPDATE screenshots SET status = 'queued', error = NULL, attempts = 0, updated_at = ?2
         WHERE id = ?1 AND deleted_at IS NULL",
        (id, now),
    )
}

/// Permanently removes rows deleted before `cutoff` and returns their files' relative
/// paths so the caller can delete them.
pub fn purge_deleted(conn: &Connection, cutoff: i64) -> Result<Vec<String>> {
    let tx = conn.unchecked_transaction()?;
    let files = {
        let mut stmt = tx.prepare("SELECT image_path, thumb_path FROM screenshots WHERE deleted_at < ?1")?;
        let rows = stmt.query_map([cutoff], |r| Ok([r.get::<_, String>(0)?, r.get::<_, String>(1)?]))?;
        rows.collect::<rusqlite::Result<Vec<_>>>()?.into_iter().flatten().collect()
    };
    tx.execute("DELETE FROM screenshots WHERE deleted_at < ?1", [cutoff])?;
    tx.commit()?;
    Ok(files)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::db::open_in_memory;

    pub(crate) fn insert(conn: &Connection, category_id: i64, note: &str, created_at: i64) -> i64 {
        conn.execute(
            "INSERT INTO screenshots (category_id, image_path, thumb_path, width, height, user_note, created_at, updated_at)
             VALUES (?1, 'i.png', 't.webp', 10, 10, ?2, ?3, ?3)",
            (category_id, note, created_at),
        )
        .unwrap();
        let id = conn.last_insert_rowid();
        conn.execute(
            "INSERT INTO screenshot_categories (screenshot_id, category_id) VALUES (?1, ?2)",
            (id, category_id),
        )
        .unwrap();
        id
    }

    #[test]
    fn soft_delete_hides_and_restore_brings_back() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 1, "n", 0);
        soft_delete(&conn, id, 100).unwrap();
        assert!(get(&conn, id).is_err());
        assert!(soft_delete(&conn, id, 100).is_err()); // already deleted
        restore(&conn, id).unwrap();
        assert_eq!(get(&conn, id).unwrap().id, id);
    }

    #[test]
    fn purge_removes_only_old_deletions_and_reports_files() {
        let conn = open_in_memory().unwrap();
        let old = insert(&conn, 1, "old", 0);
        let recent = insert(&conn, 1, "recent", 0);
        soft_delete(&conn, old, 100).unwrap();
        soft_delete(&conn, recent, 500).unwrap();
        let files = purge_deleted(&conn, 200).unwrap();
        assert_eq!(files, ["i.png", "t.webp"]);
        restore(&conn, recent).unwrap(); // still there
        assert!(restore(&conn, old).is_err()); // gone for good
    }

    #[test]
    fn set_categories_rules_and_edits_persist() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 6, "n", 0);
        // Misc is dropped next to real categories; duplicates collapse; first is primary.
        set_categories(&conn, id, &[2, 6, 3, 2], true, 1).unwrap();
        assert_eq!(category_ids(&conn, id).unwrap(), [2, 3]);
        let card = get(&conn, id).unwrap();
        assert_eq!(card.categories.iter().map(|c| c.name.as_str()).collect::<Vec<_>>(), ["Learning", "Resources"]);
        // Nothing chosen means Miscellaneous.
        set_categories(&conn, id, &[], true, 1).unwrap();
        assert_eq!(category_ids(&conn, id).unwrap(), [6]);
        assert!(set_categories(&conn, id, &[999], true, 1).is_err());
        set_categories(&conn, id, &[2], true, 1).unwrap();
        update_text(&conn, id, "  New title ", "Short summary.", 2).unwrap();
        update_note(&conn, id, " remind me ", 3).unwrap();
        let c = get(&conn, id).unwrap();
        assert_eq!((c.category_id, c.category_locked), (2, true));
        assert_eq!((c.title.as_deref(), c.user_note.as_str()), (Some("New title"), "remind me"));
    }

    #[test]
    fn requeue_resets_failure() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 1, "n", 0);
        conn.execute("UPDATE screenshots SET status = 'failed', error = 'x', attempts = 3 WHERE id = ?1", [id])
            .unwrap();
        requeue(&conn, id, 5).unwrap();
        let c = get(&conn, id).unwrap();
        assert_eq!((c.status.as_str(), c.error), ("queued", None));
    }
}
