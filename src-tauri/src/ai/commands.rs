//! IPC commands for AI setup and maintenance. Keep `src/lib/ipc.ts` in sync.

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};

use super::{publish, Ai, AiStatus};
use crate::db::queue as q;
use crate::error::Result;
use crate::settings::{self, SettingsPatch};
use crate::state::AppState;
use crate::util::now_ms;

#[tauri::command]
pub fn ai_status(ai: State<'_, Ai>, state: State<'_, AppState>) -> AiStatus {
    ai.status(&state.settings().ai_profile)
}

/// One AI choice the user can download.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileInfo {
    id: String,
    model_name: String,
    /// Engine plus text model, i.e. what "Set up AI" downloads for this choice.
    download_bytes: u64,
    min_ram_gb: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiProfiles {
    /// This computer's installed memory, rounded to whole GB (0 if unknown).
    total_ram_gb: u32,
    /// The most capable choice that fits this computer's memory.
    recommended: String,
    /// Smallest first.
    profiles: Vec<ProfileInfo>,
}

/// The AI choices with their download sizes, and which one suits this computer.
#[tauri::command]
pub fn ai_profiles(ai: State<'_, Ai>) -> AiProfiles {
    let total_ram_gb = crate::util::total_ram_gb();
    let mut profiles: Vec<ProfileInfo> = ai
        .registry
        .profiles
        .iter()
        .map(|(id, p)| {
            let (text, _) = ai.registry.profile(id);
            ProfileInfo {
                id: id.clone(),
                model_name: text.name.clone(),
                download_bytes: ai.components(id, false).iter().map(|(d, _)| d.size).sum(),
                min_ram_gb: p.min_ram_gb,
            }
        })
        .collect();
    profiles.sort_by_key(|p| p.min_ram_gb);
    let recommended = recommend(&profiles, total_ram_gb).unwrap_or_else(|| ai.registry.default_profile.clone());
    AiProfiles { total_ram_gb, recommended, profiles }
}

/// The most demanding profile this much memory meets (`profiles` smallest first). Unknown
/// memory (0) gets the smallest.
fn recommend(profiles: &[ProfileInfo], ram_gb: u32) -> Option<String> {
    profiles.iter().rev().find(|p| ram_gb >= p.min_ram_gb).or(profiles.first()).map(|p| p.id.clone())
}

/// The user agreed to download the AI (the only network use). Remembered, so an
/// interrupted download resumes by itself after a restart.
#[tauri::command]
pub fn ai_start_setup<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<()> {
    let saved = settings::save(&state.db(), &SettingsPatch { ai_download_consent: Some(true), ..Default::default() })?;
    state.set_settings(saved.clone());
    Ai::start_download(&app, saved.ai_profile, false);
    Ok(())
}

/// One-time consent for the image model (spec 3.2: only downloaded when first needed).
#[tauri::command]
pub fn ai_start_vision_setup<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<()> {
    let saved = settings::save(&state.db(), &SettingsPatch { ai_vision_consent: Some(true), ..Default::default() })?;
    state.set_settings(saved.clone());
    Ai::start_download(&app, saved.ai_profile, true);
    Ok(())
}

#[tauri::command]
pub fn ai_pause_download<R: Runtime>(app: AppHandle<R>, ai: State<'_, Ai>, state: State<'_, AppState>) {
    ai.pause_download();
    publish(&app, &state.settings().ai_profile);
}

/// Deletes the engine and models. Captures keep working; filing waits until set up again.
#[tauri::command]
pub fn ai_delete<R: Runtime>(app: AppHandle<R>, ai: State<'_, Ai>, state: State<'_, AppState>) -> Result<()> {
    ai.delete_all()?;
    let saved = settings::save(
        &state.db(),
        &SettingsPatch { ai_download_consent: Some(false), ai_vision_consent: Some(false), ..Default::default() },
    )?;
    state.set_settings(saved.clone());
    publish(&app, &saved.ai_profile);
    crate::queue::publish_status(&app);
    Ok(())
}

/// Re-runs the AI on every screenshot with the current prompt and model. Hand-picked
/// categories and hand-edited titles/summaries are kept. Returns how many were queued.
#[tauri::command]
pub fn refile_all<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<usize> {
    let n = q::queue_refile_all(&state.db(), now_ms())?;
    crate::queue::wake(&app);
    Ok(n)
}

/// Offered after the user creates a category: asks the AI to look at existing screenshots
/// again and *add* the new category where it fits (nothing is removed, and categories the
/// user picked by hand aren't touched). Returns how many were queued.
#[tauri::command]
pub fn recheck_categories<R: Runtime>(app: AppHandle<R>, state: State<'_, AppState>) -> Result<usize> {
    let n = q::queue_recheck(&state.db(), now_ms())?;
    crate::queue::wake(&app);
    Ok(n)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn p(id: &str, min: u32) -> ProfileInfo {
        ProfileInfo { id: id.into(), model_name: String::new(), download_bytes: 0, min_ram_gb: min }
    }

    #[test]
    fn recommends_the_biggest_model_that_fits() {
        let all = [p("standard", 8), p("accurate", 16)];
        assert_eq!(recommend(&all, 32).as_deref(), Some("accurate"));
        assert_eq!(recommend(&all, 16).as_deref(), Some("accurate"));
        assert_eq!(recommend(&all, 15).as_deref(), Some("standard"));
        assert_eq!(recommend(&all, 8).as_deref(), Some("standard"));
        assert_eq!(recommend(&all, 4).as_deref(), Some("standard"));
        assert_eq!(recommend(&all, 0).as_deref(), Some("standard"));
    }
}
