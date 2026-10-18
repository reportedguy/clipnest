//! System tray icon and menu.

use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime, WebviewWindowBuilder};

use crate::error::Result;
use crate::settings::SettingsPatch;
use crate::state::AppState;

const MAIN_TRAY_ID: &str = "main";

/// Handles to tray items whose state mirrors settings.
pub struct TrayHandles<R: Runtime> {
    pub pause_ai: CheckMenuItem<R>,
}

pub fn create<R: Runtime>(app: &AppHandle<R>, ai_paused: bool) -> Result<()> {
    let capture = MenuItem::with_id(app, "capture", "Capture now", true, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open ClipNest", true, None::<&str>)?;
    let pause_ai = CheckMenuItem::with_id(app, "pause_ai", "Pause AI processing", true, ai_paused, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&capture, &open, &pause_ai, &separator, &quit])?;

    TrayIconBuilder::with_id(MAIN_TRAY_ID)
        .icon(app.default_window_icon().cloned().expect("bundle icon is configured"))
        .tooltip("ClipNest")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "capture" => {
                // Let the tray menu finish closing so it isn't in the frozen frame.
                let app = app.clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(250));
                    crate::capture::start(&app);
                });
            }
            "open" => show_main(app),
            "pause_ai" => toggle_pause(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = event {
                show_main(tray.app_handle());
            }
        })
        .build(app)?;

    app.manage(TrayHandles { pause_ai });
    Ok(())
}

/// Shows, un-minimizes and focuses the main window, creating it if it was closed to the
/// tray. (Closing destroys the webview so an idle app doesn't hold WebView2's memory.)
pub fn show_main<R: Runtime>(app: &AppHandle<R>) {
    let window = match app.get_webview_window("main") {
        Some(w) => w,
        None => {
            let config = app.config().app.windows.iter().find(|w| w.label == "main");
            let built = config.map(|c| WebviewWindowBuilder::from_config(app, c).and_then(|b| b.build()));
            match built {
                Some(Ok(w)) => w,
                Some(Err(e)) => return tracing::error!(error = %e, "couldn't open main window"),
                None => return tracing::error!("main window missing from tauri.conf.json"),
            }
        }
    };
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// Shows queue progress ("Processing 3 screenshots") as the tray icon's tooltip.
pub fn set_status<R: Runtime>(app: &AppHandle<R>, text: &str) {
    if let Some(tray) = app.tray_by_id(MAIN_TRAY_ID) {
        let _ = tray.set_tooltip(Some(text));
    }
}

/// Keeps the tray checkbox in step when the setting changes elsewhere (e.g. Settings screen).
pub fn sync_pause<R: Runtime>(app: &AppHandle<R>, paused: bool) {
    if let Some(h) = app.try_state::<TrayHandles<R>>() {
        let _ = h.pause_ai.set_checked(paused);
    }
}

fn toggle_pause<R: Runtime>(app: &AppHandle<R>) {
    let state = app.state::<AppState>();
    let paused = !state.settings().ai_paused;
    let saved = crate::settings::save(&state.db(), &SettingsPatch { ai_paused: Some(paused), ..Default::default() });
    match saved {
        Ok(s) => {
            state.set_settings(s);
            crate::commands::emit_settings_changed(app, &state.settings());
        }
        Err(e) => tracing::error!(error = %e, "couldn't toggle AI pause"),
    }
    // The menu toggles its own check on click; re-sync so it always reflects what was saved.
    sync_pause(app, state.settings().ai_paused);
    crate::queue::wake(app);
}
