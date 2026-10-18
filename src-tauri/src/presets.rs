//! Starting categories by profession (`config/category-presets.json`, compiled in), offered
//! during setup and in Settings. Adding a profession is a data change: edit the file.

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Runtime, State};

use crate::capture::session::LIBRARY_CHANGED_EVENT;
use crate::db::categories::{self, CategoryInput};
use crate::db::queue as q;
use crate::error::Result;
use crate::state::AppState;
use crate::util::now_ms;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Preset {
    pub id: String,
    pub name: String,
    pub description: String,
    /// A lucide icon name; the UI maps it to a component.
    pub icon: String,
    pub categories: Vec<PresetCategory>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PresetCategory {
    pub name: String,
    pub hint: String,
}

#[derive(Deserialize)]
struct File {
    presets: Vec<Preset>,
}

pub fn all() -> Vec<Preset> {
    serde_json::from_str::<File>(include_str!("../../config/category-presets.json"))
        .expect("category-presets.json is valid (checked by tests)")
        .presets
}

#[tauri::command]
pub fn category_presets() -> Vec<Preset> {
    all()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SetupOutcome {
    added: usize,
    removed: usize,
    /// Screenshots queued to be filed again into the new categories.
    queued: usize,
}

/// Saves the categories picked in setup (`replace`: they become the whole list) or adds
/// a profession's categories from Settings (nothing removed). After a replace, screenshots
/// already filed are filed again, since the categories they were filed into changed.
#[tauri::command]
pub fn apply_category_setup<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, AppState>,
    categories: Vec<CategoryInput>,
    replace: bool,
) -> Result<SetupOutcome> {
    let now = now_ms();
    let conn = state.db();
    let r = categories::apply_setup(&conn, &categories, replace, now)?;
    let queued = if replace && (r.added > 0 || r.removed > 0) { q::queue_refile_all(&conn, now)? } else { 0 };
    drop(conn);
    if queued > 0 {
        crate::queue::wake(&app);
    }
    if let Err(e) = app.emit(LIBRARY_CHANGED_EVENT, ()) {
        tracing::warn!(error = %e, "couldn't broadcast library change");
    }
    tracing::info!(added = r.added, removed = r.removed, queued, replace, "category setup applied");
    Ok(SetupOutcome { added: r.added, removed: r.removed, queued })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::categories::{MAX_HINT_LEN, MAX_NAME_LEN};
    use std::collections::HashSet;

    #[test]
    fn presets_file_is_valid() {
        let presets = all();
        assert!(presets.len() >= 8);
        let mut ids = HashSet::new();
        for p in &presets {
            assert!(ids.insert(p.id.as_str()), "duplicate preset id {}", p.id);
            assert!(!p.name.is_empty() && !p.description.is_empty() && !p.icon.is_empty());
            assert!((5..=12).contains(&p.categories.len()), "{} has {} categories", p.id, p.categories.len());
            let mut names = HashSet::new();
            for c in &p.categories {
                assert_eq!(c.name, c.name.split_whitespace().collect::<Vec<_>>().join(" "), "spacing in {:?}", c.name);
                assert!(!c.name.is_empty() && c.name.chars().count() <= MAX_NAME_LEN, "{:?}", c.name);
                assert!(!c.hint.is_empty() && c.hint.chars().count() <= MAX_HINT_LEN, "{:?}", c.hint);
                assert!(!c.name.eq_ignore_ascii_case("miscellaneous"), "Miscellaneous is added by the app");
                assert!(names.insert(c.name.to_lowercase()), "{} lists {:?} twice", p.id, c.name);
            }
        }
    }

    #[test]
    fn every_preset_applies_cleanly() {
        for p in all() {
            let conn = crate::db::open_in_memory().unwrap();
            let items: Vec<CategoryInput> = p
                .categories
                .iter()
                .map(|c| CategoryInput {
                    name: Some(c.name.clone()),
                    hint: Some(c.hint.clone()),
                    color: Some("#181d26".into()),
                })
                .collect();
            categories::apply_setup(&conn, &items, true, 0).unwrap();
            let got: Vec<String> = categories::list(&conn).unwrap().into_iter().map(|c| c.name).collect();
            let mut want: Vec<String> = p.categories.iter().map(|c| c.name.clone()).collect();
            want.push("Miscellaneous".into());
            assert_eq!(got, want, "preset {}", p.id);
        }
    }
}
