use rusqlite::{Connection, ErrorCode, OptionalExtension};
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub hint: Option<String>,
    pub color: String,
    pub icon: Option<String>,
    pub sort_order: i64,
    pub is_system: bool,
    pub item_count: i64,
    /// Thumbnail of the newest screenshot, for the home tile preview.
    pub latest_thumb: Option<String>,
}

pub const MAX_NAME_LEN: usize = 40;
pub const MAX_HINT_LEN: usize = 160;

/// All categories in display order, with how many screenshots each holds.
pub fn list(conn: &Connection) -> Result<Vec<Category>> {
    let mut stmt = conn.prepare_cached(
        "SELECT c.id, c.name, c.hint, c.color, c.icon, c.sort_order, c.is_system,
                (SELECT count(*) FROM screenshot_categories sc JOIN screenshots s ON s.id = sc.screenshot_id
                 WHERE sc.category_id = c.id AND s.deleted_at IS NULL),
                (SELECT s.thumb_path FROM screenshot_categories sc JOIN screenshots s ON s.id = sc.screenshot_id
                 WHERE sc.category_id = c.id AND s.deleted_at IS NULL
                 ORDER BY s.created_at DESC, s.id DESC LIMIT 1)
         FROM categories c
         ORDER BY c.sort_order, c.id",
    )?;
    let rows = stmt.query_map([], |r| {
        Ok(Category {
            id: r.get(0)?,
            name: r.get(1)?,
            hint: r.get(2)?,
            color: r.get(3)?,
            icon: r.get(4)?,
            sort_order: r.get(5)?,
            is_system: r.get(6)?,
            item_count: r.get(7)?,
            latest_thumb: r.get(8)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

pub fn misc_id(conn: &Connection) -> Result<i64> {
    conn.query_row("SELECT id FROM categories WHERE is_system = 1 ORDER BY id LIMIT 1", [], |r| r.get(0))
        .optional()?
        .ok_or_else(|| Error::Invalid("Miscellaneous category is missing".into()))
}

#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CategoryInput {
    pub name: Option<String>,
    /// `Some("")` clears the hint.
    pub hint: Option<String>,
    pub color: Option<String>,
}

fn clean_name(name: &str) -> Result<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    if name.is_empty() {
        return Err(Error::Invalid("Give the category a name.".into()));
    }
    if name.chars().count() > MAX_NAME_LEN {
        return Err(Error::Invalid(format!("Keep category names under {MAX_NAME_LEN} characters.")));
    }
    Ok(name)
}

fn clean_hint(hint: &str) -> Result<Option<String>> {
    let hint = hint.trim();
    if hint.chars().count() > MAX_HINT_LEN {
        return Err(Error::Invalid(format!("Keep hints under {MAX_HINT_LEN} characters.")));
    }
    Ok((!hint.is_empty()).then(|| hint.to_owned()))
}

fn clean_color(color: &str) -> Result<String> {
    let ok = color.len() == 7 && color.starts_with('#') && color[1..].chars().all(|c| c.is_ascii_hexdigit());
    ok.then(|| color.to_ascii_lowercase()).ok_or_else(|| Error::Invalid("Pick a color.".into()))
}

/// Translates a UNIQUE violation into a friendly message.
fn name_conflict(e: rusqlite::Error, name: &str) -> Error {
    match e.sqlite_error_code() {
        Some(ErrorCode::ConstraintViolation) => {
            Error::Invalid(format!("There's already a category called \"{name}\"."))
        }
        _ => e.into(),
    }
}

/// Adds a category just before Miscellaneous (which always stays last).
pub fn create(conn: &Connection, input: &CategoryInput, now: i64) -> Result<i64> {
    let tx = conn.unchecked_transaction()?;
    let id = insert(&tx, input, now)?;
    tx.commit()?;
    Ok(id)
}

/// `create` without its own transaction.
fn insert(conn: &Connection, input: &CategoryInput, now: i64) -> Result<i64> {
    let name = clean_name(input.name.as_deref().unwrap_or_default())?;
    let hint = clean_hint(input.hint.as_deref().unwrap_or_default())?;
    let color = clean_color(input.color.as_deref().unwrap_or("#181d26"))?;
    let order: i64 =
        conn.query_row("SELECT COALESCE(MAX(sort_order), -1) + 1 FROM categories WHERE is_system = 0", [], |r| {
            r.get(0)
        })?;
    conn.execute("UPDATE categories SET sort_order = sort_order + 1 WHERE is_system = 1", [])?;
    conn.execute(
        "INSERT INTO categories (name, hint, color, sort_order, is_system, created_at) VALUES (?1, ?2, ?3, ?4, 0, ?5)",
        (&name, &hint, &color, order, now),
    )
    .map_err(|e| name_conflict(e, &name))?;
    Ok(conn.last_insert_rowid())
}

pub fn update(conn: &Connection, id: i64, input: &CategoryInput) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    if let Some(name) = &input.name {
        let name = clean_name(name)?;
        tx.execute("UPDATE categories SET name = ?2 WHERE id = ?1", (id, &name)).map_err(|e| match e {
            // The trigger guarding Miscellaneous raises an ABORT with this message.
            rusqlite::Error::SqliteFailure(_, Some(ref m)) if m.contains("system category") => {
                Error::Invalid("Miscellaneous can't be renamed.".into())
            }
            other => name_conflict(other, &name),
        })?;
    }
    if let Some(hint) = &input.hint {
        tx.execute("UPDATE categories SET hint = ?2 WHERE id = ?1", (id, clean_hint(hint)?))?;
    }
    if let Some(color) = &input.color {
        tx.execute("UPDATE categories SET color = ?2 WHERE id = ?1", (id, clean_color(color)?))?;
    }
    tx.commit()?;
    Ok(())
}

/// Sets display order from `ids` (all user categories). Miscellaneous stays last.
pub fn reorder(conn: &Connection, ids: &[i64]) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    for (i, id) in ids.iter().enumerate() {
        tx.execute("UPDATE categories SET sort_order = ?2 WHERE id = ?1 AND is_system = 0", (id, i as i64))?;
    }
    tx.execute("UPDATE categories SET sort_order = ?1 WHERE is_system = 1", [ids.len() as i64])?;
    tx.commit()?;
    Ok(())
}

/// Deletes a category: it's removed from every screenshot (including trashed ones), and a
/// screenshot left with no category goes to Miscellaneous (unlocked, so a later re-check
/// can file it again). Returns how many screenshots had it.
pub fn delete(conn: &Connection, id: i64, now: i64) -> Result<usize> {
    let misc = misc_id(conn)?;
    if id == misc {
        return Err(Error::Invalid("Miscellaneous can't be deleted.".into()));
    }
    let tx = conn.unchecked_transaction()?;
    let affected = remove(&tx, id, misc, now)?;
    tx.commit()?;
    Ok(affected)
}

/// `delete` without its own transaction or the Miscellaneous check.
fn remove(tx: &Connection, id: i64, misc: i64, now: i64) -> Result<usize> {
    let affected = tx.execute("DELETE FROM screenshot_categories WHERE category_id = ?1", [id])?;
    tx.execute(
        "INSERT INTO screenshot_categories (screenshot_id, category_id)
         SELECT s.id, ?1 FROM screenshots s
         WHERE NOT EXISTS (SELECT 1 FROM screenshot_categories sc WHERE sc.screenshot_id = s.id)",
        [misc],
    )?;
    // Re-point primaries that referenced it at the card's next category.
    tx.execute(
        "UPDATE screenshots SET
           category_locked = CASE WHEN EXISTS (SELECT 1 FROM screenshot_categories sc WHERE sc.screenshot_id = screenshots.id
                                               AND sc.category_id != ?2) THEN category_locked ELSE 0 END,
           category_id = (SELECT sc.category_id FROM screenshot_categories sc JOIN categories c ON c.id = sc.category_id
                          WHERE sc.screenshot_id = screenshots.id ORDER BY c.sort_order LIMIT 1),
           updated_at = ?3
         WHERE category_id = ?1",
        (id, misc, now),
    )?;
    if tx.execute("DELETE FROM categories WHERE id = ?1", [id])? == 0 {
        return Err(Error::Invalid("That category no longer exists.".into()));
    }
    Ok(affected)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupResult {
    pub added: usize,
    pub removed: usize,
}

/// Applies a list of categories chosen in setup (or from a profession in Settings), in
/// the order given. A category whose name already exists (any case) is kept, with the new
/// hint if one is given; others are created. With `replace`, categories not in the list
/// are deleted (their screenshots move to Miscellaneous, like `delete`) and the list's
/// order becomes the display order. Without it, nothing is removed or reordered: missing
/// ones are added before Miscellaneous. Miscellaneous itself always stays, last.
pub fn apply_setup(conn: &Connection, items: &[CategoryInput], replace: bool, now: i64) -> Result<SetupResult> {
    let misc = misc_id(conn)?;
    let existing: Vec<(i64, String, bool)> =
        list(conn)?.into_iter().map(|c| (c.id, c.name.to_lowercase(), c.is_system)).collect();

    // Validate everything first so a bad entry changes nothing; drop repeats and Misc.
    let mut wanted: Vec<(String, &CategoryInput)> = Vec::new();
    for item in items {
        let name = clean_name(item.name.as_deref().unwrap_or_default())?;
        if let Some(hint) = &item.hint {
            clean_hint(hint)?;
        }
        let key = name.to_lowercase();
        let is_misc = existing.iter().any(|(id, n, _)| *id == misc && *n == key);
        if !is_misc && !wanted.iter().any(|(k, _)| *k == key) {
            wanted.push((key, item));
        }
    }

    let tx = conn.unchecked_transaction()?;
    let mut result = SetupResult::default();
    if replace {
        for (id, name, is_system) in &existing {
            if !is_system && !wanted.iter().any(|(k, _)| k == name) {
                remove(&tx, *id, misc, now)?;
                result.removed += 1;
            }
        }
    }
    let mut order = Vec::with_capacity(wanted.len());
    for (key, item) in &wanted {
        match existing.iter().find(|(_, n, is_system)| n == key && !is_system) {
            Some((id, _, _)) => {
                if let Some(hint) = item.hint.as_deref().filter(|h| !h.trim().is_empty()) {
                    tx.execute("UPDATE categories SET hint = ?2 WHERE id = ?1", (id, clean_hint(hint)?))?;
                }
                order.push(*id);
            }
            None => {
                order.push(insert(&tx, item, now)?);
                result.added += 1;
            }
        }
    }
    if replace {
        for (i, id) in order.iter().enumerate() {
            tx.execute("UPDATE categories SET sort_order = ?2 WHERE id = ?1", (id, i as i64))?;
        }
        tx.execute("UPDATE categories SET sort_order = ?1 WHERE is_system = 1", [order.len() as i64])?;
    }
    tx.commit()?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::db::screenshots::tests::insert;

    fn names(conn: &Connection) -> Vec<String> {
        list(conn).unwrap().into_iter().map(|c| c.name).collect()
    }

    #[test]
    fn seeds_default_categories_with_misc_last() {
        let conn = open_in_memory().unwrap();
        assert_eq!(
            names(&conn),
            ["Ideas", "Learning", "Resources", "Opportunities & Deadlines", "Watch Later", "Miscellaneous"]
        );
        let cats = list(&conn).unwrap();
        assert!(cats.last().unwrap().is_system);
        assert_eq!(cats.iter().filter(|c| c.is_system).count(), 1);
    }

    #[test]
    fn miscellaneous_cannot_be_deleted_or_renamed() {
        let conn = open_in_memory().unwrap();
        let misc = misc_id(&conn).unwrap();
        assert!(delete(&conn, misc, 0).is_err());
        let err = update(&conn, misc, &CategoryInput { name: Some("Other".into()), ..Default::default() });
        assert!(err.unwrap_err().to_string().contains("can't be renamed"));
        // Recoloring and hints are allowed.
        update(
            &conn,
            misc,
            &CategoryInput { color: Some("#000000".into()), hint: Some("odds and ends".into()), ..Default::default() },
        )
        .unwrap();
        // The raw triggers also hold if the app layer is bypassed.
        assert!(conn.execute("DELETE FROM categories WHERE is_system = 1", []).is_err());
    }

    #[test]
    fn create_goes_before_misc_and_rejects_duplicates() {
        let conn = open_in_memory().unwrap();
        let input = CategoryInput {
            name: Some("  Fonts   I like ".into()),
            hint: Some("typography".into()),
            color: Some("#AABBCC".into()),
        };
        create(&conn, &input, 0).unwrap();
        let n = names(&conn);
        assert_eq!(&n[n.len() - 2..], ["Fonts I like", "Miscellaneous"]);
        let dup = create(&conn, &CategoryInput { name: Some("fonts i LIKE".into()), ..Default::default() }, 0);
        assert!(dup.unwrap_err().to_string().contains("already a category"));
        assert!(create(&conn, &CategoryInput { name: Some("   ".into()), ..Default::default() }, 0).is_err());
        assert!(create(
            &conn,
            &CategoryInput { name: Some("X".into()), color: Some("red".into()), ..Default::default() },
            0
        )
        .is_err());
    }

    #[test]
    fn delete_moves_items_to_misc_and_unlocks_them() {
        let conn = open_in_memory().unwrap();
        let a = insert(&conn, 1, "a", 0);
        insert(&conn, 1, "b", 0);
        conn.execute("UPDATE screenshots SET category_locked = 1 WHERE id = ?1", [a]).unwrap();
        assert_eq!(delete(&conn, 1, 5).unwrap(), 2);
        let misc = list(&conn).unwrap().into_iter().find(|c| c.is_system).unwrap();
        assert_eq!(misc.item_count, 2);
        let locked: bool =
            conn.query_row("SELECT category_locked FROM screenshots WHERE id = ?1", [a], |r| r.get(0)).unwrap();
        assert!(!locked); // a re-check can file them again
    }

    #[test]
    fn delete_keeps_a_cards_other_categories() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 1, "a", 0);
        crate::db::screenshots::set_categories(&conn, id, &[1, 3], true, 1).unwrap();
        delete(&conn, 1, 5).unwrap();
        assert_eq!(crate::db::screenshots::category_ids(&conn, id).unwrap(), [3]);
        let (primary, locked): (i64, bool) = conn
            .query_row("SELECT category_id, category_locked FROM screenshots WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .unwrap();
        assert_eq!((primary, locked), (3, true)); // still the user's own choice
    }

    fn input(name: &str, hint: &str) -> CategoryInput {
        CategoryInput { name: Some(name.into()), hint: Some(hint.into()), color: Some("#a8d8c4".into()) }
    }

    #[test]
    fn setup_replace_keeps_matches_adds_new_and_removes_the_rest() {
        let conn = open_in_memory().unwrap();
        let card = insert(&conn, 4, "hackathon", 0); // in Opportunities & Deadlines
        let r = apply_setup(
            &conn,
            &[
                input("Lecture Notes", "class notes"),
                input("ideas", "project ideas"),
                input("Lecture notes", "dup"),
                input("Miscellaneous", ""),
            ],
            true,
            5,
        )
        .unwrap();
        assert_eq!(r, SetupResult { added: 1, removed: 4 });
        // Kept "Ideas" under its existing name, in the new order, Misc last.
        assert_eq!(names(&conn), ["Lecture Notes", "Ideas", "Miscellaneous"]);
        let ideas = list(&conn).unwrap().into_iter().find(|c| c.name == "Ideas").unwrap();
        assert_eq!((ideas.id, ideas.hint.as_deref()), (1, Some("project ideas")));
        // The card whose category went away is now in Miscellaneous.
        assert_eq!(crate::db::screenshots::category_ids(&conn, card).unwrap(), [misc_id(&conn).unwrap()]);
    }

    #[test]
    fn setup_add_only_never_removes_or_reorders() {
        let conn = open_in_memory().unwrap();
        let r = apply_setup(&conn, &[input("Typography", "fonts"), input("Learning", "")], false, 5).unwrap();
        assert_eq!(r, SetupResult { added: 1, removed: 0 });
        assert_eq!(
            names(&conn),
            [
                "Ideas",
                "Learning",
                "Resources",
                "Opportunities & Deadlines",
                "Watch Later",
                "Typography",
                "Miscellaneous"
            ]
        );
        // An empty hint doesn't wipe the existing one.
        let learning = list(&conn).unwrap().into_iter().find(|c| c.name == "Learning").unwrap();
        assert_eq!(learning.hint.as_deref(), Some("knowledge, explanations, tips, how-tos"));
    }

    #[test]
    fn setup_with_a_bad_entry_changes_nothing() {
        let conn = open_in_memory().unwrap();
        let before = names(&conn);
        assert!(apply_setup(&conn, &[input("Fine", ""), input("   ", "")], true, 5).is_err());
        assert!(apply_setup(&conn, &[input(&"x".repeat(41), "")], true, 5).is_err());
        assert_eq!(names(&conn), before);
        // An empty list in replace mode leaves only Miscellaneous ("set up my own" later).
        apply_setup(&conn, &[], true, 5).unwrap();
        assert_eq!(names(&conn), ["Miscellaneous"]);
    }

    #[test]
    fn reorder_keeps_misc_last() {
        let conn = open_in_memory().unwrap();
        reorder(&conn, &[5, 4, 3, 2, 1]).unwrap();
        assert_eq!(
            names(&conn),
            ["Watch Later", "Opportunities & Deadlines", "Resources", "Learning", "Ideas", "Miscellaneous"]
        );
    }

    #[test]
    fn counts_and_preview_ignore_trashed_items() {
        let conn = open_in_memory().unwrap();
        insert(&conn, 2, "old", 10);
        let newest = insert(&conn, 2, "new", 20);
        conn.execute("UPDATE screenshots SET thumb_path = 'newest.webp' WHERE id = ?1", [newest]).unwrap();
        let learning = |conn: &Connection| list(conn).unwrap().into_iter().find(|c| c.id == 2).unwrap();
        assert_eq!((learning(&conn).item_count, learning(&conn).latest_thumb.as_deref()), (2, Some("newest.webp")));
        crate::db::screenshots::soft_delete(&conn, newest, 30).unwrap();
        assert_eq!((learning(&conn).item_count, learning(&conn).latest_thumb.as_deref()), (1, Some("t.webp")));
    }
}
