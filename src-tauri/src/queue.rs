//! The background worker: takes queued screenshots one at a time, runs OCR (with the
//! smart fallback to Image mode), then hands them to the AI. Runs on one below-normal
//! priority thread so it never competes with what the user is doing.
//!
//! It wakes immediately when something changes (a save, Retry, resume) and on a
//! heartbeat as a safety net. All progress lives in the database, so a crash or quit at
//! any point loses nothing: see `db::queue`.

use std::sync::{Arc, Condvar, Mutex};
use std::time::Duration;

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::ai::prompt::{CategoryRef, Request};
use crate::ai::Ai;
use crate::capture::session::LIBRARY_CHANGED_EVENT;
use crate::db::categories;
use crate::db::queue::{self as q, Job};
use crate::error::{Error, Result};
use crate::ocr;
use crate::state::AppState;
use crate::util::now_ms;

pub const STATUS_EVENT: &str = "queue-status";
const HEARTBEAT: Duration = Duration::from_secs(30);

/// Handle for waking the worker; managed as Tauri state.
#[derive(Clone, Default)]
pub struct Queue {
    signal: Arc<(Mutex<bool>, Condvar)>,
}

impl Queue {
    /// Tells the worker there may be new work (or that it was resumed).
    pub fn wake(&self) {
        let (flag, cvar) = &*self.signal;
        *flag.lock().unwrap_or_else(|p| p.into_inner()) = true;
        cvar.notify_one();
    }

    fn wait(&self) {
        let (flag, cvar) = &*self.signal;
        let guard = flag.lock().unwrap_or_else(|p| p.into_inner());
        let (mut woken, _) = cvar.wait_timeout_while(guard, HEARTBEAT, |w| !*w).unwrap_or_else(|p| p.into_inner());
        *woken = false;
    }
}

/// Wakes the worker if it's running (no-op during startup).
pub fn wake<R: Runtime>(app: &AppHandle<R>) {
    if let Some(q) = app.try_state::<Queue>() {
        q.wake();
    }
}

/// Starts the worker thread. Call once, after `AppState` is managed.
pub fn start<R: Runtime>(app: &AppHandle<R>) -> Queue {
    let queue = Queue::default();
    let (app, handle) = (app.clone(), queue.clone());
    let spawned = std::thread::Builder::new().name("queue".into()).spawn(move || {
        crate::platform::lower_current_thread_priority();
        if let Ok(n) = q::reset_stuck(&app.state::<AppState>().db()) {
            if n > 0 {
                tracing::info!(items = n, "resumed items interrupted by the last exit");
            }
        }
        loop {
            drain(&app);
            publish_status(&app);
            handle.wait();
        }
    });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "couldn't start the processing queue");
    }
    queue
}

/// Works through everything that can make progress right now.
fn drain<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    // Items that failed during this pass wait for the next wake-up (no hot retry loop).
    let mut skip = Vec::new();
    loop {
        if paused_now(&state) {
            return;
        }
        let ai = app.state::<Ai>();
        let profile = state.settings().ai_profile;
        let ready = Readiness { text: ai.text_ready(&profile), vision: ai.vision_ready(&profile) };
        let job = match q::next_job(&state.db(), ready.text, ready.vision, &skip) {
            Ok(Some(job)) => job,
            Ok(None) => return,
            Err(e) => return tracing::error!(error = %e, "queue: couldn't read the next item"),
        };
        if let Err(e) = q::mark_processing(&state.db(), job.id, now_ms()) {
            return tracing::error!(error = %e, "queue: couldn't start item");
        }
        changed(app);

        if let Err(e) = process(&state, &ai, &profile, &job, ready) {
            skip.push(job.id);
            let message = friendly(&e);
            tracing::warn!(id = job.id, attempt = job.attempts + 1, error = %e, "queue: item failed");
            if let Err(db) = q::record_failure(&state.db(), job.id, &message, now_ms()) {
                tracing::error!(error = %db, "queue: couldn't record failure");
            }
        }
        changed(app);
    }
}

/// Paused by the user, or by battery saver when they allow that.
fn paused_now(state: &AppState) -> bool {
    let s = state.settings();
    s.ai_paused || (s.pause_on_battery_saver && crate::platform::battery_saver_on())
}

#[derive(Clone, Copy)]
struct Readiness {
    text: bool,
    vision: bool,
}

fn process(state: &AppState, ai: &Ai, profile: &str, job: &Job, ready: Readiness) -> Result<()> {
    let (text, mode) = match &job.ocr_text {
        Some(t) => (t.clone(), job.mode.clone()),
        None => run_ocr(state, job)?,
    };
    let needs_vision = mode == "image";
    if !ready.text || (needs_vision && !ready.vision) {
        return q::requeue_waiting(&state.db(), job.id, now_ms());
    }

    let categories: Vec<CategoryRef> = categories::list(&state.db())?
        .into_iter()
        .map(|c| CategoryRef { id: c.id, name: c.name, hint: c.hint })
        .collect();
    let misc_id = categories::misc_id(&state.db())?;
    let misc_name = categories.iter().find(|c| c.id == misc_id).map(|c| c.name.clone()).unwrap_or_default();
    let now = chrono::Local::now();
    let timezone = now.format("UTC%:z").to_string();
    let request = Request {
        note: &job.user_note,
        ocr_text: &text,
        categories: &categories,
        today: now.date_naive(),
        timezone: &timezone,
    };
    let image = if needs_vision { Some(image_path(state, job)?) } else { None };

    let filing = ai.file(profile, &request, image.as_deref(), &misc_name)?;

    let category_ids: Vec<i64> =
        filing.categories.iter().filter_map(|name| categories.iter().find(|c| &c.name == name).map(|c| c.id)).collect();
    let category_ids = if category_ids.is_empty() { vec![misc_id] } else { category_ids };
    let urls = crate::ai::urls::grounded(&filing.urls, &format!("{}\n{}", job.user_note, text));
    let model = ai.registry.profile(profile).0.id.clone();
    let ai_json = serde_json::to_string(&filing)?;
    let filed = q::Filed {
        title: &filing.title,
        summary: &filing.summary,
        category_ids: &category_ids,
        tags: &filing.tags,
        urls: &urls,
        ai_json: &ai_json,
        ai_model: &model,
    };
    let db = state.db();
    q::apply_filing(&db, job, &filed, now_ms())?;
    if !job.recheck {
        let found = crate::deadlines::from_filing(&filing, &job.user_note, now.date_naive());
        crate::deadlines::store_found(&db, job.id, found.as_ref())?;
    }
    Ok(())
}

fn image_path(state: &AppState, job: &Job) -> Result<std::path::PathBuf> {
    crate::media::resolve(&state.data_dir, &job.image_path)
        .filter(|p| p.exists())
        .ok_or_else(|| Error::Invalid("The image file is missing.".into()))
}

/// OCR plus the smart fallback: too little text means the meaning is visual, so the item
/// is upgraded to Image mode (and marked, so the card can say so). Returns (text, mode).
fn run_ocr(state: &AppState, job: &Job) -> Result<(String, String)> {
    let image = image::open(image_path(state, job)?)?.to_rgba8();
    let text = match ocr::recognize(state.platform.ocr.as_ref(), &image) {
        Ok(r) => r.text,
        // No language pack: file it from the note and the picture alone.
        Err(Error::OcrUnavailable(reason)) => {
            tracing::warn!(%reason, "OCR unavailable; continuing without text");
            String::new()
        }
        Err(e) => return Err(e),
    };
    let upgrade = job.mode == "text" && ocr::meaningful_chars(&text) < ocr::MIN_MEANINGFUL_CHARS;
    let mode = if upgrade { "image" } else { job.mode.as_str() };
    let text = text.trim();
    q::save_ocr(&state.db(), job.id, text, mode, upgrade, now_ms())?;
    Ok((text.to_owned(), mode.to_owned()))
}

/// What the user sees on a failed card.
fn friendly(e: &Error) -> String {
    match e {
        Error::Invalid(m) => m.clone(),
        Error::Image(_) => "The image couldn't be opened.".into(),
        Error::Ocr(_) => "Couldn't read the text in this screenshot.".into(),
        Error::Io(_) => "Couldn't read the file. Is the disk full or the file in use?".into(),
        other => other.to_string(),
    }
}

fn changed<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.emit(LIBRARY_CHANGED_EVENT, ());
    publish_status(app);
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QueueStatus {
    #[serde(flatten)]
    pub counts: q::Counts,
    pub paused: bool,
    /// Paused because battery saver is on (not by the user).
    pub battery_saver: bool,
    pub ai_ready: bool,
}

pub fn status<R: Runtime>(app: &AppHandle<R>, state: &AppState) -> Result<QueueStatus> {
    let ai_ready = app.try_state::<Ai>().is_some_and(|ai| ai.text_ready(&state.settings().ai_profile));
    let s = state.settings();
    let battery_saver = !s.ai_paused && s.pause_on_battery_saver && crate::platform::battery_saver_on();
    Ok(QueueStatus { counts: q::counts(&state.db())?, paused: s.ai_paused || battery_saver, battery_saver, ai_ready })
}

/// Pushes the queue state to the UI badge and the tray tooltip.
pub fn publish_status<R: Runtime>(app: &AppHandle<R>) {
    let Some(state) = app.try_state::<AppState>() else { return };
    let Ok(s) = status(app, &state) else { return };
    let _ = app.emit(STATUS_EVENT, &s);
    crate::tray::set_status(app, &tooltip(&s));
}

fn tooltip(s: &QueueStatus) -> String {
    let n = s.counts.pending;
    let plural = |n: i64| if n == 1 { "screenshot" } else { "screenshots" };
    match () {
        _ if n == 0 => "ClipNest".into(),
        _ if s.battery_saver => format!("ClipNest · AI paused for battery saver · {n} {} waiting", plural(n)),
        _ if s.paused => format!("ClipNest · AI paused · {n} {} waiting", plural(n)),
        _ if s.counts.waiting_for_ai == n && !s.ai_ready => {
            format!("ClipNest · {n} {} waiting for the AI model", plural(n))
        }
        _ if s.counts.waiting_for_vision == n => {
            format!("ClipNest · {n} {} waiting for the image model", plural(n))
        }
        _ => format!("ClipNest · Processing {n} {}", plural(n)),
    }
}

#[tauri::command]
pub fn queue_status<R: Runtime>(app: AppHandle<R>, state: tauri::State<'_, AppState>) -> Result<QueueStatus> {
    status(&app, &state)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(pending: i64, waiting: i64, paused: bool) -> QueueStatus {
        QueueStatus {
            counts: q::Counts { pending, waiting_for_ai: waiting, waiting_for_vision: 0, failed: 0 },
            paused,
            battery_saver: false,
            ai_ready: false,
        }
    }

    #[test]
    fn tooltip_reads_naturally() {
        assert_eq!(tooltip(&s(0, 0, false)), "ClipNest");
        assert_eq!(tooltip(&s(3, 1, false)), "ClipNest · Processing 3 screenshots");
        assert_eq!(tooltip(&s(1, 0, false)), "ClipNest · Processing 1 screenshot");
        assert_eq!(tooltip(&s(2, 2, false)), "ClipNest · 2 screenshots waiting for the AI model");
        assert_eq!(tooltip(&s(2, 0, true)), "ClipNest · AI paused · 2 screenshots waiting");
    }
}
