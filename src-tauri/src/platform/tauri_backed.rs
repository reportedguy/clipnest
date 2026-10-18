//! Platform services backed by official Tauri plugins. The plugins are themselves
//! cross-platform, so these implementations serve Windows and macOS alike.

use tauri::{AppHandle, Runtime};
use tauri_plugin_autostart::ManagerExt as _;
use tauri_plugin_global_shortcut::GlobalShortcutExt as _;
use tauri_plugin_notification::NotificationExt as _;

use super::{AutoStart, HotkeyProvider, Notifier};
use crate::error::{Error, Result};

pub struct PluginHotkeys<R: Runtime>(AppHandle<R>);

impl<R: Runtime> PluginHotkeys<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self(app)
    }
}

impl<R: Runtime> HotkeyProvider for PluginHotkeys<R> {
    fn register(&self, accelerator: &str) -> Result<()> {
        self.0
            .global_shortcut()
            .register(accelerator)
            .map_err(|e| Error::Hotkey { accelerator: accelerator.into(), reason: e.to_string() })
    }

    fn unregister(&self, accelerator: &str) -> Result<()> {
        self.0
            .global_shortcut()
            .unregister(accelerator)
            .map_err(|e| Error::Hotkey { accelerator: accelerator.into(), reason: e.to_string() })
    }
}

pub struct PluginNotifier<R: Runtime>(AppHandle<R>);

impl<R: Runtime> PluginNotifier<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self(app)
    }
}

impl<R: Runtime> Notifier for PluginNotifier<R> {
    fn notify(&self, title: &str, body: &str) -> Result<()> {
        self.0
            .notification()
            .builder()
            .title(title)
            .body(body)
            .show()
            .map_err(|e| Error::Platform(format!("couldn't show notification: {e}")))
    }
}

pub struct PluginAutoStart<R: Runtime>(AppHandle<R>);

impl<R: Runtime> PluginAutoStart<R> {
    pub fn new(app: AppHandle<R>) -> Self {
        Self(app)
    }
}

impl<R: Runtime> AutoStart for PluginAutoStart<R> {
    fn is_enabled(&self) -> Result<bool> {
        self.0.autolaunch().is_enabled().map_err(|e| Error::Platform(format!("autostart: {e}")))
    }

    fn set_enabled(&self, enabled: bool) -> Result<()> {
        let launcher = self.0.autolaunch();
        let r = if enabled { launcher.enable() } else { launcher.disable() };
        r.map_err(|e| Error::Platform(format!("autostart: {e}")))
    }
}
