//! Deadlines: deciding which matter, storing them, the home-screen strip, and reminders.
//!
//! A deadline is *important* (shown and reminded) only when the note shows intent to act:
//! the model's `note_has_action_intent` OR the keyword check in `intent`. Others are
//! stored but stay quiet (spec 3.6). Deadlines set by hand are always important, and the
//! AI never overwrites them.

pub mod dates;
pub mod intent;

use std::time::Duration;

use chrono::{Local, NaiveDate, TimeZone};
use rusqlite::Connection;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::ai::prompt::Filing;
use crate::capture::session::LIBRARY_CHANGED_EVENT;
use crate::db::screenshots::{card_from_row, Card, CARD_COLUMNS};
use crate::error::{Error, Result};
use crate::state::AppState;
use crate::util::now_ms;

/// Marks a deadline the user set themselves.
const MANUAL: &str = "manual";
/// Overdue items stay in the strip this long before dropping off.
const OVERDUE_GRACE_MS: i64 = 7 * 24 * 60 * 60 * 1000;
const SCHEDULER_PERIOD: Duration = Duration::from_secs(60 * 60);

/// What filing found, ready to store.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub at_ms: i64,
    pub label: String,
    pub raw: String,
    pub important: bool,
}

/// Turns the AI's answer (plus the note) into a deadline, if there is one.
pub fn from_filing(f: &Filing, note: &str, reference: NaiveDate) -> Option<Found> {
    let important = f.note_has_action_intent || intent::note_shows_intent(note);
    if let Some(d) = &f.deadline {
        if let Some(date) = dates::resolve(Some(&d.raw), d.iso.as_deref(), reference) {
            return Some(Found {
                at_ms: dates::end_of_day_ms(date),
                label: d.label.trim().to_owned(),
                raw: d.raw.trim().to_owned(),
                important,
            });
        }
    }
    // The model may miss a date that's only in the note ("remind me 5 Jan").
    if important {
        if let Some(date) = dates::parse_text(note, reference).filter(|d| *d >= reference) {
            return Some(Found {
                at_ms: dates::end_of_day_ms(date),
                label: "Reminder".into(),
                raw: note.trim().to_owned(),
                important,
            });
        }
    }
    None
}

/// Stores an AI-found deadline unless the user set one by hand.
pub fn store_found(conn: &Connection, id: i64, found: Option<&Found>) -> Result<()> {
    let (at, label, raw, important) = match found {
        Some(f) => (Some(f.at_ms), Some(f.label.as_str()), Some(f.raw.as_str()), f.important),
        None => (None, None, None, false),
    };
    conn.execute(
        "UPDATE screenshots SET deadline_at = ?2, deadline_label = ?3, deadline_raw = ?4, deadline_important = ?5,
                deadline_done = 0, notified_at = NULL
         WHERE id = ?1 AND (deadline_raw IS NULL OR deadline_raw != ?6)",
        (id, at, label, raw, important, MANUAL),
    )?;
    Ok(())
}

/// Sets (or with `None`, clears) a deadline by hand. Hand-set deadlines are important.
pub fn set_manual(conn: &Connection, id: i64, deadline: Option<(NaiveDate, &str)>) -> Result<()> {
    let n = match deadline {
        Some((date, label)) => conn.execute(
            "UPDATE screenshots SET deadline_at = ?2, deadline_label = ?3, deadline_raw = ?4, deadline_important = 1,
                    deadline_done = 0, notified_at = NULL
             WHERE id = ?1 AND deleted_at IS NULL",
            (id, dates::end_of_day_ms(date), label.trim(), MANUAL),
        )?,
        None => conn.execute(
            "UPDATE screenshots SET deadline_at = NULL, deadline_label = NULL, deadline_raw = ?2, deadline_important = 0,
                    deadline_done = 0, notified_at = NULL
             WHERE id = ?1 AND deleted_at IS NULL",
            (id, MANUAL),
        )?,
    };
    if n == 0 {
        return Err(Error::Invalid("That screenshot no longer exists.".into()));
    }
    Ok(())
}

/// Important, not-done deadlines from a week overdue onwards, soonest first.
pub fn upcoming(conn: &Connection, now: i64, limit: i64) -> Result<Vec<Card>> {
    let mut stmt = conn.prepare_cached(&format!(
        "SELECT {CARD_COLUMNS} FROM screenshots s JOIN categories c ON c.id = s.category_id
         WHERE s.deadline_at IS NOT NULL AND s.deadline_important = 1 AND s.deadline_done = 0
           AND s.deleted_at IS NULL AND s.deadline_at >= ?1
         ORDER BY s.deadline_at LIMIT ?2"
    ))?;
    let rows = stmt.query_map((now - OVERDUE_GRACE_MS, limit), card_from_row)?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Reminders due now: important, not done, not yet notified, deadline still ahead, and
/// past their reminder time (so reminders missed while the app was off catch up).
pub fn due_reminders(
    conn: &Connection,
    now: i64,
    lead_hours: u32,
    notify_hour: u8,
) -> Result<Vec<(i64, String, String, i64)>> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, COALESCE(title, user_note, ''), COALESCE(deadline_label, ''), deadline_at FROM screenshots
         WHERE deadline_at > ?1 AND deadline_important = 1 AND deadline_done = 0 AND notified_at IS NULL
           AND deleted_at IS NULL",
    )?;
    let rows = stmt
        .query_map([now], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect::<rusqlite::Result<Vec<(i64, String, String, i64)>>>()?;
    Ok(rows.into_iter().filter(|(.., at)| dates::remind_at_ms(*at, lead_hours, notify_hour) <= now).collect())
}

/// Checks for due reminders now and every hour after (spec 3.6). Each fires once.
pub fn start_scheduler<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    let _ = std::thread::Builder::new().name("deadlines".into()).spawn(move || loop {
        if let Err(e) = send_due(&app) {
            tracing::warn!(error = %e, "deadline check failed");
        }
        std::thread::sleep(SCHEDULER_PERIOD);
    });
}

fn send_due<R: Runtime>(app: &AppHandle<R>) -> Result<()> {
    let Some(state) = app.try_state::<AppState>() else { return Ok(()) };
    let s = state.settings();
    let now = now_ms();
    let due = due_reminders(&state.db(), now, s.deadline_lead_hours, s.notify_hour)?;
    for (id, title, label, at) in due {
        let when =
            Local.timestamp_millis_opt(at).single().map(|t| t.format("%a %-d %b").to_string()).unwrap_or_default();
        let heading = if label.is_empty() { format!("Deadline {when}") } else { format!("{label}: {when}") };
        match state.platform.notifier.notify(&heading, &title) {
            Ok(()) => {
                state.db().execute("UPDATE screenshots SET notified_at = ?2 WHERE id = ?1", (id, now))?;
                tracing::info!(id, "deadline reminder sent");
            }
            Err(e) => tracing::warn!(id, error = %e, "couldn't show deadline reminder"),
        }
    }
    Ok(())
}

// ---- commands ------------------------------------------------------------------------

#[tauri::command]
pub fn upcoming_deadlines(state: State<'_, AppState>) -> Result<Vec<Card>> {
    upcoming(&state.db(), now_ms(), 20)
}

/// `date` is "YYYY-MM-DD", or `None` to clear the deadline.
#[tauri::command]
pub fn set_deadline<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    id: i64,
    date: Option<String>,
    label: String,
) -> Result<()> {
    let parsed = match date.as_deref() {
        Some(d) => {
            Some(NaiveDate::parse_from_str(d, "%Y-%m-%d").map_err(|_| Error::Invalid("Pick a valid date.".into()))?)
        }
        None => None,
    };
    set_manual(&state.db(), id, parsed.map(|d| (d, label.as_str())))?;
    let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
    Ok(())
}

/// "Done": removes the deadline from the strip and stops its reminder.
#[tauri::command]
pub fn mark_deadline_done<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>, id: i64) -> Result<()> {
    state.db().execute("UPDATE screenshots SET deadline_done = 1 WHERE id = ?1", [id])?;
    let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai::prompt::Deadline;
    use crate::db::open_in_memory;
    use crate::db::screenshots::tests::insert;

    fn today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 10, 15).unwrap()
    }

    fn filing(intent: bool, deadline: Option<(&str, Option<&str>)>) -> Filing {
        Filing {
            title: "T".into(),
            summary: "S".into(),
            categories: vec!["Ideas".into()],
            note_purpose: String::new(),
            tags: vec![],
            urls: vec![],
            note_has_action_intent: intent,
            deadline: deadline.map(|(raw, iso)| Deadline {
                raw: raw.into(),
                iso: iso.map(Into::into),
                label: "registration closes".into(),
            }),
        }
    }

    #[test]
    fn importance_needs_intent_from_model_or_keywords() {
        let f = filing(false, Some(("12/11/2026", None)));
        assert!(!from_filing(&f, "", today()).unwrap().important); // stored, but quiet
        assert!(from_filing(&f, "register for this", today()).unwrap().important); // keyword
        assert!(from_filing(&filing(true, Some(("12/11/2026", None))), "", today()).unwrap().important); // model
        assert!(from_filing(&filing(true, None), "", today()).is_none());
    }

    #[test]
    fn note_only_dates_are_caught_when_intent_is_clear() {
        let found = from_filing(&filing(false, None), "remind me 5 Jan", today()).unwrap();
        assert!(found.important);
        assert_eq!(found.at_ms, dates::end_of_day_ms(NaiveDate::from_ymd_opt(2027, 1, 5).unwrap()));
        assert!(from_filing(&filing(false, None), "notes from 5 Jan", today()).is_none());
        // no intent
    }

    #[test]
    fn manual_deadlines_win_and_strip_lists_important_ones() {
        let conn = open_in_memory().unwrap();
        let a = insert(&conn, 1, "a", 0);
        let b = insert(&conn, 1, "b", 0);
        let quiet = insert(&conn, 1, "c", 0);
        let now = dates::end_of_day_ms(today());
        let soon = Found { at_ms: now + 86_400_000, label: "x".into(), raw: "16/10".into(), important: true };
        let later = Found { at_ms: now + 5 * 86_400_000, ..soon.clone() };
        store_found(&conn, a, Some(&later)).unwrap();
        store_found(&conn, b, Some(&soon)).unwrap();
        store_found(&conn, quiet, Some(&Found { important: false, ..soon.clone() })).unwrap();
        let ids: Vec<i64> = upcoming(&conn, now, 10).unwrap().iter().map(|c| c.id).collect();
        assert_eq!(ids, [b, a]); // soonest first; the quiet one isn't shown

        // A hand-set deadline isn't overwritten by a later AI run.
        set_manual(&conn, a, Some((NaiveDate::from_ymd_opt(2026, 12, 1).unwrap(), "my date"))).unwrap();
        store_found(&conn, a, None).unwrap();
        let label: String =
            conn.query_row("SELECT deadline_label FROM screenshots WHERE id = ?1", [a], |r| r.get(0)).unwrap();
        assert_eq!(label, "my date");

        // Done removes it from the strip.
        conn.execute("UPDATE screenshots SET deadline_done = 1 WHERE id = ?1", [b]).unwrap();
        assert_eq!(upcoming(&conn, now, 10).unwrap().len(), 1);
    }

    #[test]
    fn reminders_fire_once_and_catch_up() {
        let conn = open_in_memory().unwrap();
        let id = insert(&conn, 1, "a", 0);
        let deadline = dates::end_of_day_ms(NaiveDate::from_ymd_opt(2026, 11, 12).unwrap());
        store_found(
            &conn,
            id,
            Some(&Found { at_ms: deadline, label: "closes".into(), raw: "12/11".into(), important: true }),
        )
        .unwrap();
        let remind = dates::remind_at_ms(deadline, 24, 9);
        assert!(due_reminders(&conn, remind - 1, 24, 9).unwrap().is_empty());
        assert_eq!(due_reminders(&conn, remind, 24, 9).unwrap().len(), 1);
        // App was off at 9:00; started at 15:00 the same day: still sent (catch-up).
        assert_eq!(due_reminders(&conn, remind + 6 * 3_600_000, 24, 9).unwrap().len(), 1);
        // Once notified, never again; and nothing after the deadline has passed.
        conn.execute("UPDATE screenshots SET notified_at = 1 WHERE id = ?1", [id]).unwrap();
        assert!(due_reminders(&conn, remind, 24, 9).unwrap().is_empty());
        conn.execute("UPDATE screenshots SET notified_at = NULL WHERE id = ?1", [id]).unwrap();
        assert!(due_reminders(&conn, deadline + 1, 24, 9).unwrap().is_empty());
    }
}
