use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard, RwLock};

use rusqlite::Connection;

use crate::hotkey::HotkeyStatus;
use crate::platform::Platform;
use crate::settings::Settings;

/// Shared app state, managed by Tauri and available to every command.
pub struct AppState {
    db: Mutex<Connection>,
    settings: RwLock<Settings>,
    hotkey: Mutex<HotkeyStatus>,
    pub platform: Platform,
    /// Root for the DB, images, thumbnails and logs.
    pub data_dir: PathBuf,
    /// Shown once in the UI, e.g. after the library was recovered from a backup.
    pub startup_notice: Option<String>,
}

// Poisoned locks (a panic while held) are recovered rather than propagated: the guarded
// values stay valid, and SQLite's own transactions keep the data consistent.
impl AppState {
    pub fn new(
        db: Connection,
        settings: Settings,
        hotkey: HotkeyStatus,
        platform: Platform,
        data_dir: PathBuf,
    ) -> Self {
        Self {
            db: Mutex::new(db),
            settings: RwLock::new(settings),
            hotkey: Mutex::new(hotkey),
            platform,
            data_dir,
            startup_notice: None,
        }
    }

    pub fn db(&self) -> MutexGuard<'_, Connection> {
        self.db.lock().unwrap_or_else(|p| p.into_inner())
    }

    pub fn settings(&self) -> Settings {
        self.settings.read().unwrap_or_else(|p| p.into_inner()).clone()
    }

    pub fn set_settings(&self, s: Settings) {
        *self.settings.write().unwrap_or_else(|p| p.into_inner()) = s;
    }

    pub fn hotkey(&self) -> MutexGuard<'_, HotkeyStatus> {
        self.hotkey.lock().unwrap_or_else(|p| p.into_inner())
    }
}
