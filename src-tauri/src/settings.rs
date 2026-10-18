//! Typed app settings persisted in the `settings` table (one JSON-encoded value per key).
//!
//! Missing keys fall back to defaults and unknown keys are ignored, so adding a setting
//! never needs a migration.

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::error::{Error, Result};

/// Default capture shortcut, in Tauri accelerator syntax ("Super" is the Windows key).
pub const DEFAULT_HOTKEY: &str = "Super+Shift+J";
/// Suggested when the default can't be registered (taken by another app or reserved by Windows).
pub const FALLBACK_HOTKEY: &str = "Ctrl+Shift+J";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Theme {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub hotkey: String,
    pub start_with_windows: bool,
    pub close_to_tray: bool,
    pub theme: Theme,
    /// How long before a deadline the reminder fires.
    pub deadline_lead_hours: u32,
    /// Local hour (0-23) at which deadline reminders are delivered.
    pub notify_hour: u8,
    pub ai_paused: bool,
    pub onboarding_done: bool,
    /// Which `models.json` profile to use ("standard" for 8 GB laptops, "accurate").
    pub ai_profile: String,
    /// The user agreed to download the AI. Remembered so an interrupted download
    /// resumes on its own after a restart.
    pub ai_download_consent: bool,
    /// Same for the image model (a separate, optional download).
    pub ai_vision_consent: bool,
    /// Hold AI work while Windows battery saver is on (spec 3.7, optional).
    pub pause_on_battery_saver: bool,
    /// Show each card's AI summary in the feed; off shows just the title (the summary
    /// opens when the card is clicked).
    pub show_summary: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            hotkey: DEFAULT_HOTKEY.into(),
            start_with_windows: true,
            close_to_tray: true,
            theme: Theme::System,
            deadline_lead_hours: 24,
            notify_hour: 9,
            ai_paused: false,
            onboarding_done: false,
            ai_profile: "standard".into(),
            ai_download_consent: false,
            ai_vision_consent: false,
            pause_on_battery_saver: true,
            show_summary: false,
        }
    }
}

/// A partial update from the UI. Only fields that are present get written.
#[derive(Debug, Default, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SettingsPatch {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hotkey: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub start_with_windows: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub close_to_tray: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub theme: Option<Theme>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deadline_lead_hours: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notify_hour: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_paused: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub onboarding_done: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_profile: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_download_consent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ai_vision_consent: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pause_on_battery_saver: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub show_summary: Option<bool>,
}

impl SettingsPatch {
    pub fn validate(&self) -> Result<()> {
        if let Some(h) = &self.hotkey {
            if h.trim().is_empty() {
                return Err(Error::Invalid("Shortcut can't be empty.".into()));
            }
        }
        if let Some(hours) = self.deadline_lead_hours {
            if !(1..=24 * 14).contains(&hours) {
                return Err(Error::Invalid("Reminder lead time must be between 1 hour and 14 days.".into()));
            }
        }
        if let Some(hour) = self.notify_hour {
            if hour > 23 {
                return Err(Error::Invalid("Reminder hour must be between 0 and 23.".into()));
            }
        }
        Ok(())
    }
}

pub fn load(conn: &Connection) -> Result<Settings> {
    let mut stmt = conn.prepare_cached("SELECT key, value FROM settings")?;
    let rows = stmt
        .query_map([], |r| Ok((r.get::<_, String>(0)?, r.get::<_, String>(1)?)))?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    // Start from defaults and overlay stored values one key at a time, so a single bad
    // value (e.g. left by a newer or older build) can't reset every other setting.
    let mut merged = serde_json::to_value(Settings::default())?;
    for (key, raw) in rows {
        let Some(previous) = merged.get(&key).cloned() else { continue }; // unknown key
        let Ok(value) = serde_json::from_str::<Value>(&raw) else {
            tracing::warn!(%key, "ignoring unreadable setting");
            continue;
        };
        merged[&key] = value;
        if serde_json::from_value::<Settings>(merged.clone()).is_err() {
            tracing::warn!(%key, "ignoring invalid setting value");
            merged[&key] = previous;
        }
    }
    Ok(serde_json::from_value(merged)?)
}

/// Writes the fields present in `patch` and returns the resulting settings.
pub fn save(conn: &Connection, patch: &SettingsPatch) -> Result<Settings> {
    patch.validate()?;
    let Value::Object(fields) = serde_json::to_value(patch)? else {
        unreachable!("SettingsPatch serializes to an object")
    };
    let tx = conn.unchecked_transaction()?;
    for (key, value) in fields {
        tx.execute(
            "INSERT INTO settings (key, value) VALUES (?1, ?2)
             ON CONFLICT (key) DO UPDATE SET value = excluded.value",
            (key, value.to_string()),
        )?;
    }
    tx.commit()?;
    load(conn)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::open_in_memory;

    #[test]
    fn fresh_db_returns_defaults() {
        let conn = open_in_memory().unwrap();
        assert_eq!(load(&conn).unwrap(), Settings::default());
    }

    #[test]
    fn patch_round_trips_and_leaves_other_fields_alone() {
        let conn = open_in_memory().unwrap();
        let s = save(&conn, &SettingsPatch { theme: Some(Theme::Dark), ai_paused: Some(true), ..Default::default() })
            .unwrap();
        assert_eq!(s.theme, Theme::Dark);
        assert!(s.ai_paused);
        assert_eq!(s.hotkey, DEFAULT_HOTKEY);
        assert_eq!(load(&conn).unwrap(), s);
    }

    #[test]
    fn invalid_patch_is_rejected_without_writing() {
        let conn = open_in_memory().unwrap();
        let bad = SettingsPatch { notify_hour: Some(24), ai_paused: Some(true), ..Default::default() };
        assert!(save(&conn, &bad).is_err());
        assert!(!load(&conn).unwrap().ai_paused);
    }

    #[test]
    fn corrupt_or_unknown_values_fall_back_per_key() {
        let conn = open_in_memory().unwrap();
        conn.execute_batch(
            r#"INSERT INTO settings VALUES ('notifyHour', '"nine"');
               INSERT INTO settings VALUES ('theme', 'not json');
               INSERT INTO settings VALUES ('someFutureKey', '1');
               INSERT INTO settings VALUES ('closeToTray', 'false');"#,
        )
        .unwrap();
        let s = load(&conn).unwrap();
        assert_eq!(s.notify_hour, 9);
        assert_eq!(s.theme, Theme::System);
        assert!(!s.close_to_tray);
    }
}
