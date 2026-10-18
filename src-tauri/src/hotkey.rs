//! Capture-shortcut registration with conflict handling.

use serde::Serialize;

use crate::error::{Error, Result};
use crate::platform::Platform;
use crate::settings::FALLBACK_HOTKEY;

/// Which shortcut (if any) is live, and why it differs from the setting when it does.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum HotkeyStatus {
    Active {
        accelerator: String,
    },
    /// The configured key failed but the fallback worked. The stored setting is left
    /// as is so the user sees why; the UI offers to switch permanently.
    UsingFallback {
        accelerator: String,
        wanted: String,
        reason: String,
    },
    /// Nothing could be registered; capture is available only from the tray.
    Failed {
        wanted: String,
        reason: String,
    },
}

impl HotkeyStatus {
    /// The accelerator currently registered with the OS.
    pub fn active(&self) -> Option<&str> {
        match self {
            Self::Active { accelerator } | Self::UsingFallback { accelerator, .. } => Some(accelerator),
            Self::Failed { .. } => None,
        }
    }
}

pub fn register_at_startup(platform: &Platform, wanted: &str) -> HotkeyStatus {
    match platform.hotkeys.register(wanted) {
        Ok(()) => HotkeyStatus::Active { accelerator: wanted.into() },
        Err(e) => {
            let reason = friendly_reason(&e);
            tracing::warn!(%wanted, %reason, "capture shortcut unavailable");
            if wanted != FALLBACK_HOTKEY && platform.hotkeys.register(FALLBACK_HOTKEY).is_ok() {
                HotkeyStatus::UsingFallback { accelerator: FALLBACK_HOTKEY.into(), wanted: wanted.into(), reason }
            } else {
                HotkeyStatus::Failed { wanted: wanted.into(), reason }
            }
        }
    }
}

/// Swaps the live shortcut for `new`. On failure the previous one stays registered and
/// the error tells the user why and what to try instead.
pub fn rebind(platform: &Platform, current: &HotkeyStatus, new: &str) -> Result<HotkeyStatus> {
    if let Some(old) = current.active() {
        let _ = platform.hotkeys.unregister(old);
    }
    match platform.hotkeys.register(new) {
        Ok(()) => Ok(HotkeyStatus::Active { accelerator: new.into() }),
        Err(e) => {
            if let Some(old) = current.active() {
                if let Err(restore) = platform.hotkeys.register(old) {
                    tracing::error!(%old, error = %restore, "couldn't restore previous shortcut");
                }
            }
            let hint = if new == FALLBACK_HOTKEY { String::new() } else { format!(" Try {FALLBACK_HOTKEY}.") };
            Err(Error::Hotkey { accelerator: new.into(), reason: format!("{}.{hint}", friendly_reason(&e)) })
        }
    }
}

fn friendly_reason(e: &Error) -> String {
    match e {
        Error::Hotkey { reason, .. } => {
            let lower = reason.to_lowercase();
            if lower.contains("already") || lower.contains("in use") {
                "it's already used by Windows or another app".into()
            } else {
                reason.clone()
            }
        }
        other => other.to_string(),
    }
}
