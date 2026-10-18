//! The interactive capture flow: hotkey → frozen frames → native region selection →
//! note popup → save.
//!
//! The selection overlay is native (`platform::RegionSelector`) so it appears within the
//! 300 ms budget without keeping a webview resident. The note popup is a webview; it is
//! created hidden the moment the hotkey fires, so its engine warms up while the user is
//! still dragging, and it's shown once the selection's thumbnail has painted. All windows
//! are destroyed afterwards: an idle app holds no webviews (docs/decisions.md, Phase 2).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::time::Instant;

use chrono::Utc;
use serde::{Deserialize, Serialize};
use tauri::{
    AppHandle, Emitter, Manager, PhysicalPosition, PhysicalSize, Runtime, State, WebviewUrl, WebviewWindowBuilder,
};

use super::store::{self, Mode, NewScreenshot, Prepared};
use crate::error::{Error, Result};
use crate::platform::MonitorInfo;
use crate::state::AppState;

/// Broadcast after a screenshot is saved so open views can refresh.
pub const LIBRARY_CHANGED_EVENT: &str = "library-changed";
/// Sent to the popup when its capture's thumbnail is ready.
const SELECTED_EVENT: &str = "capture-selected";

/// Popup size in logical pixels.
// The card plus a see-through margin (POPUP_MARGIN, matching `p-3` in Popup.tsx) where
// its soft CSS shadow shows; the window itself has no OS shadow.
const POPUP_MARGIN: f64 = 12.0;
const POPUP_W: f64 = 400.0 + 2.0 * POPUP_MARGIN;
const POPUP_H: f64 = 280.0 + 2.0 * POPUP_MARGIN;
/// Gap between the top of the screen's work area and the popup.
const POPUP_TOP: f64 = 72.0;

#[derive(Default)]
pub struct CaptureManager {
    current: Mutex<Option<Session>>,
    next_id: AtomicU64,
}

struct Session {
    id: u64,
    selected: Option<Selected>,
}

struct Selected {
    at: Instant,
    prepared: Prepared,
}

impl CaptureManager {
    fn lock(&self) -> MutexGuard<'_, Option<Session>> {
        self.current.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The session, if `id` is still the active one (stale windows get ignored).
    fn with_session<T>(&self, id: u64, f: impl FnOnce(&mut Session) -> Result<T>) -> Result<T> {
        match self.lock().as_mut() {
            Some(s) if s.id == id => f(s),
            _ => Err(Error::Invalid("This capture is no longer active.".into())),
        }
    }
}

/// Values the popup page reads on load.
#[derive(Serialize)]
struct PageInit {
    session: u64,
}

fn popup_label(session: u64) -> String {
    format!("popup-{session}")
}

/// Entry point for the hotkey and the tray. Never blocks the caller.
pub fn start<R: Runtime>(app: &AppHandle<R>) {
    let app = app.clone();
    let started = Instant::now();
    let spawned = std::thread::Builder::new().name("capture".into()).spawn(move || {
        if let Err(e) = run(&app, started) {
            tracing::error!(error = %e, "capture failed");
            abandon(&app);
            let _ = app.state::<AppState>().platform.notifier.notify("Couldn't capture the screen", &e.to_string());
        }
    });
    if let Err(e) = spawned {
        tracing::error!(error = %e, "couldn't start capture thread");
    }
}

fn run<R: Runtime>(app: &AppHandle<R>, started: Instant) -> Result<()> {
    let mgr = app.state::<CaptureManager>();
    // The hotkey goes live a moment before setup finishes; a press in that window is ignored.
    let Some(state) = app.try_state::<AppState>() else { return Ok(()) };

    // One capture at a time. A second press while the popup is open brings it back to
    // the front instead of throwing away the note being typed.
    let id = {
        let mut current = mgr.lock();
        if let Some(s) = current.as_ref() {
            if s.selected.is_some() {
                if let Some(w) = app.get_webview_window(&popup_label(s.id)) {
                    let _ = w.set_focus();
                }
            }
            return Ok(());
        }
        let id = mgr.next_id.fetch_add(1, Ordering::Relaxed) + 1;
        *current = Some(Session { id, selected: None });
        id
    };

    let frames = state.platform.capturer.freeze_all()?;
    let frozen_ms = started.elapsed().as_millis() as u64;

    // Building a webview blocks for ~450 ms while WebView2 starts, so the popup is built
    // on its own thread while the overlay is already up and the user is dragging.
    let popup = {
        let app = app.clone();
        std::thread::Builder::new().name("capture-popup".into()).spawn(move || create_popup(&app, id))?
    };

    let mut on_shown = || {
        tracing::info!(ms = started.elapsed().as_millis() as u64, frozen_ms, "hotkey to overlay visible");
    };
    let selection = state.platform.selector.select(&frames, &mut on_shown);
    let popup = popup.join().map_err(|_| Error::Capture("popup thread crashed".into()));
    let Some(region) = selection? else {
        abandon(app);
        tracing::info!(session = id, "capture cancelled");
        return Ok(());
    };
    popup??;

    let at = Instant::now();
    let frame = &frames[region.frame];
    let r = region.rect;
    let crop = image::imageops::crop_imm(&frame.image, r.x, r.y, r.width, r.height).to_image();
    let monitor = frame.monitor.clone();
    drop(frames); // the full-screen frames can be 100+ MB on multi-4K setups

    // Millisecond timestamp: session ids restart at 1 each launch, and a rename onto an
    // existing name would silently replace that file.
    let key = format!("{}-{id}", Utc::now().format("%Y%m%dT%H%M%S%3f"));
    let prepared = store::prepare(&state.data_dir, &key, crop)?;
    mgr.with_session(id, |s| {
        s.selected = Some(Selected { at, prepared });
        Ok(())
    })?;
    tracing::info!(width = r.width, height = r.height, ms = at.elapsed().as_millis() as u64, "region selected");

    let label = popup_label(id);
    if let Some(w) = app.get_webview_window(&label) {
        position_popup(app, &w, &monitor)?;
    }
    app.emit_to(label.as_str(), SELECTED_EVENT, ())?;
    Ok(())
}

/// Creates the popup hidden; its page loads while the user is still selecting.
fn create_popup<R: Runtime>(app: &AppHandle<R>, session: u64) -> Result<()> {
    let init = serde_json::to_string(&PageInit { session })?;
    WebviewWindowBuilder::new(app, popup_label(session), WebviewUrl::App("popup.html".into()))
        .initialization_script(format!("window.__RS_CAPTURE__ = {init};"))
        .title("Save screenshot")
        .visible(false)
        .decorations(false)
        .transparent(true)
        .shadow(false)
        .resizable(false)
        .skip_taskbar(true)
        .always_on_top(true)
        .inner_size(POPUP_W, POPUP_H)
        .build()?;
    Ok(())
}

/// Top-center of the work area of the monitor that was captured, where the eye already is.
fn position_popup<R: Runtime>(app: &AppHandle<R>, window: &tauri::WebviewWindow<R>, on: &MonitorInfo) -> Result<()> {
    let target = app
        .available_monitors()?
        .into_iter()
        .find(|m| m.position().x == on.x && m.position().y == on.y)
        .or(app.primary_monitor()?);
    if let Some(m) = target {
        let scale = m.scale_factor();
        let (w, h) = ((POPUP_W * scale).round() as u32, (POPUP_H * scale).round() as u32);
        let top = (POPUP_TOP * scale).round() as i32;
        let area = m.work_area();
        let x = area.position.x + (area.size.width as i32 - w as i32) / 2;
        let y = area.position.y + top;
        window.set_position(PhysicalPosition::new(x, y))?;
        window.set_size(PhysicalSize::new(w, h))?;
    }
    Ok(())
}

/// Drops the current session, closing its popup and discarding any pending capture.
fn abandon<R: Runtime>(app: &AppHandle<R>) {
    let session = app.state::<CaptureManager>().lock().take();
    if let Some(s) = session {
        if let Some(w) = app.get_webview_window(&popup_label(s.id)) {
            let _ = w.destroy();
        }
        if let Some(sel) = s.selected {
            store::discard(sel.prepared);
        }
    }
}

/// Handles the user closing the popup by other means (e.g. Alt+F4): same as Esc.
pub fn on_window_close_requested<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if label.starts_with("popup-") {
        abandon(app);
    }
}

// ---------------------------------------------------------------------------------------
// Commands (async so window operations never deadlock the webview's IPC thread on Windows)
// ---------------------------------------------------------------------------------------

/// Called by the popup page once it's listening for `capture-selected`. Returns whether
/// the selection already happened (then the page loads the thumbnail right away).
#[tauri::command]
pub async fn popup_attach<R: Runtime>(app: AppHandle<R>, session: u64) -> Result<bool> {
    app.state::<CaptureManager>().with_session(session, |s| Ok(s.selected.is_some()))
}

/// Called once the thumbnail has painted: shows and focuses the popup.
#[tauri::command]
pub async fn popup_ready<R: Runtime>(app: AppHandle<R>, session: u64) -> Result<()> {
    let selected_at = app.state::<CaptureManager>().with_session(session, |s| Ok(s.selected.as_ref().map(|x| x.at)))?;
    if let Some(w) = app.get_webview_window(&popup_label(session)) {
        w.show()?;
        w.set_focus()?;
    }
    if let Some(at) = selected_at {
        tracing::info!(ms = at.elapsed().as_millis() as u64, "selection to popup visible");
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SaveRequest {
    note: String,
    mode: Mode,
    /// Categories picked in the popup; empty means "Auto" (the AI decides).
    #[serde(default)]
    category_ids: Vec<i64>,
}

/// Saves the pending capture. Returns the new screenshot id. On failure the capture stays
/// pending so the user can retry or discard; nothing is lost.
#[tauri::command]
pub async fn save_capture<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    session: u64,
    request: SaveRequest,
) -> Result<i64> {
    let t = Instant::now();
    let mgr = app.state::<CaptureManager>();
    let id = mgr.with_session(session, |s| {
        let sel = s.selected.as_mut().ok_or_else(|| Error::Invalid("Nothing to save yet.".into()))?;
        let new = NewScreenshot { note: &request.note, mode: request.mode, category_ids: &request.category_ids };
        store::commit(&state.db(), &state.data_dir, &mut sel.prepared, &new, Utc::now())
    })?;
    mgr.lock().take();
    tracing::info!(id, ms = t.elapsed().as_millis() as u64, mode = ?request.mode, "screenshot saved");
    if let Err(e) = app.emit(LIBRARY_CHANGED_EVENT, id) {
        tracing::warn!(error = %e, "couldn't broadcast library change");
    }
    crate::queue::wake(&app);
    Ok(id)
}

/// Esc in the popup.
#[tauri::command]
pub async fn cancel_capture<R: Runtime>(app: AppHandle<R>, session: u64) -> Result<()> {
    let active = app.state::<CaptureManager>().lock().as_ref().map(|s| s.id);
    if active == Some(session) {
        abandon(&app);
        tracing::info!(session, "capture discarded");
    }
    Ok(())
}

/// Closes the popup after the "Saved" confirmation has shown.
#[tauri::command]
pub async fn close_popup<R: Runtime>(app: AppHandle<R>, session: u64) -> Result<()> {
    if let Some(w) = app.get_webview_window(&popup_label(session)) {
        w.destroy()?;
    }
    Ok(())
}

/// The in-progress capture's thumbnail (WebP), for the popup. Served by `media::serve`.
pub fn pending_thumb<R: Runtime>(app: &AppHandle<R>, session: u64) -> Option<Vec<u8>> {
    app.state::<CaptureManager>()
        .with_session(session, |s| Ok(s.selected.as_ref().map(|x| x.prepared.thumb_webp.clone())))
        .ok()
        .flatten()
}
