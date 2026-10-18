//! Tauri IPC commands. Keep `src/lib/ipc.ts` in sync with these signatures.

use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

use crate::db::{self, categories::Category};
use crate::error::Result;
use crate::hotkey::HotkeyStatus;
use crate::settings::{self, Settings, SettingsPatch};
use crate::state::AppState;

pub const SETTINGS_CHANGED_EVENT: &str = "settings-changed";

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    version: String,
    data_dir: String,
    schema_version: i64,
    ocr_languages: Vec<String>,
    hotkey: HotkeyStatus,
    notice: Option<String>,
}

#[tauri::command]
pub fn app_info<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<AppInfo> {
    Ok(AppInfo {
        version: app.package_info().version.to_string(),
        data_dir: state.data_dir.display().to_string(),
        schema_version: db::schema_version(&state.db())?,
        ocr_languages: state.platform.ocr.languages(),
        hotkey: state.hotkey().clone(),
        notice: state.startup_notice.clone(),
    })
}

#[tauri::command]
pub fn list_categories(state: State<'_, AppState>) -> Result<Vec<Category>> {
    db::categories::list(&state.db())
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Settings {
    state.settings()
}

#[tauri::command]
pub fn update_settings<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    patch: SettingsPatch,
) -> Result<Settings> {
    patch.validate()?;
    if let Some(p) = &patch.ai_profile {
        if !app.state::<crate::ai::Ai>().registry.profiles.contains_key(p) {
            return Err(crate::error::Error::Invalid("Unknown AI model choice.".into()));
        }
    }
    let before = state.settings();

    // Apply OS side effects first so a failure (e.g. the shortcut is taken) leaves the
    // stored setting unchanged and the UI can show why.
    if let Some(new_key) = patch.hotkey.as_deref() {
        let mut status = state.hotkey();
        *status = if status.active() == Some(new_key) {
            // Already live (e.g. adopting the fallback permanently): nothing to re-register.
            HotkeyStatus::Active { accelerator: new_key.into() }
        } else {
            crate::hotkey::rebind(&state.platform, &status, new_key)?
        };
    }
    if let Some(on) = patch.start_with_windows.filter(|on| *on != before.start_with_windows) {
        crate::sync_autostart(&state.platform, on);
    }

    let saved = settings::save(&state.db(), &patch)?;
    state.set_settings(saved.clone());
    if saved.ai_profile != before.ai_profile {
        // A different model: stop the running one; fetch the new one if AI is set up.
        let ai = app.state::<crate::ai::Ai>();
        ai.stop();
        if saved.ai_download_consent {
            crate::ai::Ai::start_download(&app, saved.ai_profile.clone(), false);
        }
        crate::ai::publish(&app, &saved.ai_profile);
        crate::queue::wake(&app);
    }
    if saved.ai_paused != before.ai_paused {
        crate::tray::sync_pause(&app, saved.ai_paused);
        crate::queue::wake(&app); // resumes work, or republishes the paused status
    }
    emit_settings_changed(&app, &saved);
    Ok(saved)
}

/// Lets every open window (main, popup) react to settings changes made anywhere.
pub fn emit_settings_changed<R: Runtime>(app: &AppHandle<R>, s: &Settings) {
    if let Err(e) = app.emit(SETTINGS_CHANGED_EVENT, s) {
        tracing::warn!(error = %e, "couldn't broadcast settings change");
    }
}
