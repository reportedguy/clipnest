//! The persistent processing queue lives in `screenshots.status`:
//! `queued` → `processing` → `done` | `failed`. Because it's just rows, it survives
//! restarts and crashes; `reset_stuck` recovers items a crash left mid-processing.

use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;

use crate::error::Result;

/// Automatic attempts before an item is marked failed (the user can always Retry).
pub const MAX_ATTEMPTS: i64 = 3;

#[derive(Debug, Clone)]
pub struct Job {
    pub id: i64,
    pub image_path: String,
    pub user_note: String,
    pub mode: String,
    pub ocr_text: Option<String>,
    pub category_locked: bool,
    pub attempts: i64,
    pub recheck: bool,
}

/// Puts items left in `processing` by a crash or forced quit back in the queue.
pub fn reset_stuck(conn: &Connection) -> Result<usize> {
    Ok(conn.execute("UPDATE screenshots SET status = 'queued' WHERE status = 'processing'", [])?)
}

/// The oldest item that can make progress now: anything that still needs OCR, plus items
/// the AI can handle (text mode needs the text model; image mode also needs vision). The
/// rest wait (status `queued`) until the models are ready.
pub fn next_job(conn: &Connection, ai_ready: bool, vision_ready: bool, skip: &[i64]) -> Result<Option<Job>> {
    let skip_json = serde_json::to_string(skip)?;
    Ok(conn
        .query_row(
            "SELECT id, image_path, user_note, mode, ocr_text, category_locked, attempts, recheck
             FROM screenshots
             WHERE status = 'queued' AND deleted_at IS NULL AND attempts < ?1
               AND (ocr_text IS NULL OR (?2 AND (mode = 'text' OR ?4)))
               AND id NOT IN (SELECT value FROM json_each(?3))
             ORDER BY created_at, id LIMIT 1",
            (MAX_ATTEMPTS, ai_ready, skip_json, vision_ready),
            |r| {
                Ok(Job {
                    id: r.get(0)?,
                    image_path: r.get(1)?,
                    user_note: r.get(2)?,
                    mode: r.get(3)?,
                    ocr_text: r.get(4)?,
                    category_locked: r.get(5)?,
                    attempts: r.get(6)?,
                    recheck: r.get(7)?,
                })
            },
        )
        .optional()?)
}

pub fn mark_processing(conn: &Connection, id: i64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE screenshots SET status = 'processing', attempts = attempts + 1, updated_at = ?2 WHERE id = ?1",
        (id, now),
    )?;
    Ok(())
}

/// Stores OCR output (and an automatic switch to Image mode, if any).
pub fn save_ocr(conn: &Connection, id: i64, text: &str, mode: &str, auto_upgraded: bool, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE screenshots SET ocr_text = ?2, mode = ?3, auto_upgraded = (auto_upgraded OR ?4), updated_at = ?5
         WHERE id = ?1",
        (id, text, mode, auto_upgraded, now),
    )?;
    Ok(())
}

/// What the AI concluded, after validation.
pub struct Filed<'a> {
    pub title: &'a str,
    pub summary: &'a str,
    /// The AI's categories, best match first (already validated, never empty).
    pub category_ids: &'a [i64],
    pub tags: &'a [String],
    pub urls: &'a [String],
    pub ai_json: &'a str,
    pub ai_model: &'a str,
}

/// Stores the AI's result and marks the item done.
/// - Categories the user chose (in the popup, or by editing the card) are never changed.
/// - A re-check (after the user adds a category) only *adds* categories the AI now finds,
///   leaving everything else, including title and summary, alone.
pub fn apply_filing(conn: &Connection, job: &Job, f: &Filed, now: i64) -> Result<()> {
    let tx = conn.unchecked_transaction()?;
    if job.recheck {
        if !job.category_locked {
            let mut ids = crate::db::screenshots::category_ids(&tx, job.id)?;
            ids.extend(f.category_ids.iter().copied());
            crate::db::screenshots::write_categories(&tx, job.id, &ids, false, now)?;
        }
        tx.execute(
            "UPDATE screenshots SET status = 'done', error = NULL, recheck = 0, updated_at = ?2 WHERE id = ?1",
            (job.id, now),
        )?;
        tx.commit()?;
        return Ok(());
    }
    let tags = serde_json::to_string(f.tags)?;
    let urls = serde_json::to_string(f.urls)?;
    // A title/summary the user edited by hand is kept.
    tx.execute(
        "UPDATE screenshots SET
                title = CASE WHEN text_locked THEN title ELSE ?2 END,
                summary = CASE WHEN text_locked THEN summary ELSE ?3 END,
                tags_json = ?4, urls_json = ?5, ai_json = ?6, ai_model = ?7, status = 'done', error = NULL, updated_at = ?8
         WHERE id = ?1",
        (job.id, f.title, f.summary, tags, urls, f.ai_json, f.ai_model, now),
    )?;
    if !job.category_locked {
        crate::db::screenshots::write_categories(&tx, job.id, f.category_ids, false, now)?;
    }
    tx.commit()?;
    Ok(())
}

/// Re-runs the AI on every finished screenshot (e.g. after a better prompt or model).
/// Categories and titles the user set by hand are kept. Returns how many were queued.
pub fn queue_refile_all(conn: &Connection, now: i64) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE screenshots SET status = 'queued', recheck = 0, attempts = 0, error = NULL, updated_at = ?1
         WHERE status IN ('done', 'failed') AND deleted_at IS NULL",
        [now],
    )?)
}

/// Queues every finished screenshot whose categories the user hasn't fixed, so the AI can
/// add a newly created category where it fits. Returns how many were queued.
pub fn queue_recheck(conn: &Connection, now: i64) -> Result<usize> {
    Ok(conn.execute(
        "UPDATE screenshots SET status = 'queued', recheck = 1, attempts = 0, error = NULL, updated_at = ?1
         WHERE category_locked = 0 AND status = 'done' AND deleted_at IS NULL",
        [now],
    )?)
}

/// Back to `queued`, e.g. OCR is done and the AI isn't ready yet. Doesn't count as a
/// failed attempt.
pub fn requeue_waiting(conn: &Connection, id: i64, now: i64) -> Result<()> {
    conn.execute(
        "UPDATE screenshots SET status = 'queued', attempts = MAX(attempts - 1, 0), error = NULL, updated_at = ?2
         WHERE id = ?1",
        (id, now),
    )?;
    Ok(())
}

/// Records a failed attempt: back to `queued` for another try, or `failed` once
/// `MAX_ATTEMPTS` is reached. Returns whether it's now permanently failed.
pub fn record_failure(conn: &Connection, id: i64, message: &str, now: i64) -> Result<bool> {
    conn.execute(
        "UPDATE screenshots
         SET status = CASE WHEN attempts >= ?3 THEN 'failed' ELSE 'queued' END, error = ?2, updated_at = ?4
         WHERE id = ?1",
        (id, message, MAX_ATTEMPTS, now),
    )?;
    let status: String = conn.query_row("SELECT status FROM screenshots WHERE id = ?1", [id], |r| r.get(0))?;
    Ok(status == "failed")
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    /// Queued or processing.
    pub pending: i64,
    /// Of `pending`: already OCR'd, waiting only for the AI.
    pub waiting_for_ai: i64,
    /// Of `waiting_for_ai`: Image-mode items (they need the image model).
    pub waiting_for_vision: i64,
    pub failed: i64,
}

pub fn counts(conn: &Connection) -> Result<Counts> {
    Ok(conn.query_row(
        "SELECT
           count(*) FILTER (WHERE status IN ('queued', 'processing')),
           count(*) FILTER (WHERE status = 'queued' AND ocr_text IS NOT NULL),
           count(*) FILTER (WHERE status = 'queued' AND ocr_text IS NOT NULL AND mode = 'image'),
           count(*) FILTER (WHERE status = 'failed')
         FROM screenshots WHERE deleted_at IS NULL",
        [],
        |r| {
            Ok(Counts {
                pending: r.get(0)?,
                waiting_for_ai: r.get(1)?,
                waiting_for_vision: r.get(2)?,
                failed: r.get(3)?,
            })
        },
    )?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;
    use crate::db::screenshots::tests::insert;

    fn status(conn: &Connection, id: i64) -> String {
        conn.query_row("SELECT status FROM screenshots WHERE id = ?1", [id], |r| r.get(0)).unwrap()
    }

    #[test]
    fn oldest_first_and_skips_deleted_and_exhausted() {
        let conn = open_in_memory().unwrap();
        let old = insert(&conn, 1, "old", 10);
        let newer = insert(&conn, 1, "newer", 20);
        let gone = insert(&conn, 1, "gone", 5);
        crate::db::screenshots::soft_delete(&conn, gone, 1).unwrap();
        assert_eq!(next_job(&conn, false, false, &[]).unwrap().unwrap().id, old);
        assert_eq!(next_job(&conn, false, false, &[old]).unwrap().unwrap().id, newer);
        conn.execute("UPDATE screenshots SET attempts = ?1 WHERE id = ?2", (MAX_ATTEMPTS, old)).unwrap();
        assert_eq!(next_job(&conn, false, false, &[]).unwrap().unwrap().id, newer);
    }

    #[test]
    fn without_ai_only_items_needing_ocr_are_picked() {
        let conn = open_in_memory().unwrap();
        let ocrd = insert(&conn, 1, "a", 10);
        save_ocr(&conn, ocrd, "some text", "text", false, 0).unwrap();
        assert!(next_job(&conn, false, false, &[]).unwrap().is_none());
        assert_eq!(next_job(&conn, true, false, &[]).unwrap().unwrap().id, ocrd);
        assert_eq!(counts(&conn).unwrap(), Counts { pending: 1, waiting_for_ai: 1, waiting_for_vision: 0, failed: 0 });
    }

    #[test]
    fn crash_recovery_and_failure_policy() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 1, "a", 0);
        mark_processing(&conn, id, 1).unwrap();
        assert_eq!(reset_stuck(&conn).unwrap(), 1);
        assert_eq!(status(&conn, id), "queued");

        // The interrupted run counts as an attempt, so an item that crashes the app every
        // time is parked after MAX_ATTEMPTS instead of crash-looping forever.
        for attempt in 2..=MAX_ATTEMPTS {
            mark_processing(&conn, id, 1).unwrap();
            let failed = record_failure(&conn, id, "boom", 2).unwrap();
            assert_eq!(failed, attempt == MAX_ATTEMPTS, "attempt {attempt}");
        }
        assert_eq!(status(&conn, id), "failed");
        assert_eq!(counts(&conn).unwrap().failed, 1);
    }

    #[test]
    fn waiting_for_ai_does_not_burn_attempts() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 1, "a", 0);
        for _ in 0..5 {
            mark_processing(&conn, id, 1).unwrap();
            requeue_waiting(&conn, id, 2).unwrap();
        }
        let attempts: i64 =
            conn.query_row("SELECT attempts FROM screenshots WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert_eq!(attempts, 0);
    }

    #[test]
    fn image_items_wait_for_vision() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 1, "a", 0);
        save_ocr(&conn, id, "", "image", true, 0).unwrap();
        assert_eq!(counts(&conn).unwrap().waiting_for_vision, 1);
        assert!(next_job(&conn, true, false, &[]).unwrap().is_none());
        assert_eq!(next_job(&conn, true, true, &[]).unwrap().unwrap().id, id);
    }

    fn filed<'a>(category_ids: &'a [i64], tags: &'a [String]) -> Filed<'a> {
        Filed { title: "T", summary: "S", category_ids, tags, urls: &[], ai_json: "{}", ai_model: "m" }
    }

    fn cats(conn: &Connection, id: i64) -> Vec<i64> {
        crate::db::screenshots::category_ids(conn, id).unwrap()
    }

    #[test]
    fn apply_respects_locked_categories() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 2, "a", 0);
        conn.execute("UPDATE screenshots SET category_locked = 1 WHERE id = ?1", [id]).unwrap();
        let job = next_job(&conn, false, false, &[]).unwrap().unwrap();
        apply_filing(&conn, &job, &filed(&[3, 4], &["x".into()]), 1).unwrap();
        let (status, title): (String, String) = conn
            .query_row("SELECT status, title FROM screenshots WHERE id = ?1", [id], |r| Ok((r.get(0)?, r.get(1)?)))
            .unwrap();
        assert_eq!((status.as_str(), title.as_str()), ("done", "T"));
        assert_eq!(cats(&conn, id), [2]);
    }

    #[test]
    fn apply_files_into_several_categories_and_indexes_tags() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 6, "a", 0);
        let job = next_job(&conn, false, false, &[]).unwrap().unwrap();
        apply_filing(&conn, &job, &filed(&[3, 1], &["Fonts".into(), "design".into()]), 1).unwrap();
        assert_eq!(cats(&conn, id), [3, 1]); // primary = the AI's best match
        let tags: Vec<String> = conn
            .prepare("SELECT tag FROM screenshot_tags WHERE screenshot_id = ?1 ORDER BY tag")
            .unwrap()
            .query_map([id], |r| r.get(0))
            .unwrap()
            .collect::<rusqlite::Result<_>>()
            .unwrap();
        assert_eq!(tags, ["design", "fonts"]);
    }

    #[test]
    fn recheck_only_adds_categories_and_keeps_text() {
        let conn = open_in_memory().unwrap();
        let in_misc = insert(&conn, 6, "a", 0);
        let in_ideas = insert(&conn, 1, "b", 1);
        let locked = insert(&conn, 2, "c", 2);
        conn.execute("UPDATE screenshots SET status = 'done', title = 'Original', ocr_text = ''", []).unwrap();
        conn.execute("UPDATE screenshots SET category_locked = 1 WHERE id = ?1", [locked]).unwrap();
        assert_eq!(queue_recheck(&conn, 3).unwrap(), 2); // the user's locked choice isn't touched
        conn.execute(
            "INSERT INTO categories (id, name, color, sort_order, created_at) VALUES (7, 'Project X', '#000000', 9, 0)",
            [],
        )
        .unwrap();
        for _ in 0..2 {
            let job = next_job(&conn, true, true, &[]).unwrap().unwrap();
            assert!(job.recheck);
            apply_filing(&conn, &job, &filed(&[7], &[]), 4).unwrap();
        }
        assert_eq!(cats(&conn, in_misc), [7]); // Miscellaneous replaced
        assert_eq!(cats(&conn, in_ideas), [1, 7]); // added, not moved
        assert_eq!(cats(&conn, locked), [2]);
        let title: String =
            conn.query_row("SELECT title FROM screenshots WHERE id = ?1", [in_ideas], |r| r.get(0)).unwrap();
        assert_eq!(title, "Original");
    }

    #[test]
    fn refile_keeps_hand_edited_text_and_categories() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 2, "a", 0);
        conn.execute("UPDATE screenshots SET status = 'done', ocr_text = ''", []).unwrap();
        crate::db::screenshots::update_text(&conn, id, "My title", "My summary", 1).unwrap();
        crate::db::screenshots::set_categories(&conn, id, &[3], true, 1).unwrap();
        assert_eq!(queue_refile_all(&conn, 2).unwrap(), 1);
        let job = next_job(&conn, true, true, &[]).unwrap().unwrap();
        apply_filing(&conn, &job, &filed(&[1], &["new".into()]), 3).unwrap();
        let (title, summary, tags): (String, String, String) = conn
            .query_row("SELECT title, summary, tags_json FROM screenshots WHERE id = ?1", [id], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .unwrap();
        assert_eq!((title.as_str(), summary.as_str()), ("My title", "My summary"));
        assert_eq!(tags, r#"["new"]"#); // tags still refresh
        assert_eq!(cats(&conn, id), [3]);
    }

    #[test]
    fn auto_upgrade_flag_is_sticky() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 1, "a", 0);
        save_ocr(&conn, id, "", "image", true, 0).unwrap();
        save_ocr(&conn, id, "", "image", false, 0).unwrap();
        let up: bool =
            conn.query_row("SELECT auto_upgraded FROM screenshots WHERE id = ?1", [id], |r| r.get(0)).unwrap();
        assert!(up);
    }
}
