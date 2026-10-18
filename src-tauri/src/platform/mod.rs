//! OS abstraction layer. Business logic depends only on these traits; each OS supplies
//! implementations, and `Platform::new` picks them. Porting to macOS means filling in
//! `platform::macos` without touching the queue, DB, AI, or deadline code.

mod tauri_backed;
mod xcap_capturer;

#[cfg(windows)]
mod windows;

#[cfg(target_os = "macos")]
mod macos;

use std::sync::Arc;

use image::RgbaImage;
use serde::Serialize;
use tauri::{AppHandle, Runtime};

use crate::error::Result;

/// A display, in physical pixels on the virtual desktop. Origins can be negative
/// (monitors left of or above the primary).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MonitorInfo {
    pub id: u32,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: u32,
    pub height: u32,
    pub scale_factor: f64,
    pub is_primary: bool,
}

/// One monitor's pixels, grabbed before the overlay is shown.
pub struct FrozenFrame {
    pub monitor: MonitorInfo,
    pub image: RgbaImage,
}

pub trait Capturer: Send + Sync {
    fn monitors(&self) -> Result<Vec<MonitorInfo>>;
    /// Grabs every monitor at full physical resolution.
    fn freeze_all(&self) -> Result<Vec<FrozenFrame>>;
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct OcrResult {
    /// Recognized lines joined with '\n'.
    pub text: String,
    /// BCP-47 tag of the recognizer that produced the text.
    pub language: Option<String>,
}

pub trait OcrEngine: Send + Sync {
    /// Installed recognizer languages (BCP-47 tags). Empty means OCR can't run.
    fn languages(&self) -> Vec<String>;
    fn recognize(&self, image: &RgbaImage) -> Result<OcrResult>;
}

/// Where the user dragged: which frame, and the crop within it (physical pixels).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Region {
    pub frame: usize,
    pub rect: crate::capture::geometry::PhysicalRect,
}

/// Full-screen region selection over frozen frames. Implemented natively per OS: a
/// webview overlay can't meet the 300 ms hotkey budget without keeping WebView2 resident
/// (~150 MB idle). See docs/decisions.md, Phase 2.
pub trait RegionSelector: Send + Sync {
    /// Shows one overlay per frame and blocks the calling thread until the user drags a
    /// region (`Some`) or cancels with Esc / right-click / Alt+F4 (`None`).
    /// `on_shown` runs once, right after the overlays become visible.
    fn select(&self, frames: &[FrozenFrame], on_shown: &mut dyn FnMut()) -> Result<Option<Region>>;
}

pub trait HotkeyProvider: Send + Sync {
    /// Registers a global shortcut in Tauri accelerator syntax (e.g. "Super+Shift+J").
    /// Fails with `Error::Hotkey` if another app or the OS owns it.
    fn register(&self, accelerator: &str) -> Result<()>;
    fn unregister(&self, accelerator: &str) -> Result<()>;
}

pub trait Notifier: Send + Sync {
    fn notify(&self, title: &str, body: &str) -> Result<()>;
}

pub trait AutoStart: Send + Sync {
    fn is_enabled(&self) -> Result<bool>;
    fn set_enabled(&self, enabled: bool) -> Result<()>;
}

/// The set of OS services the rest of the app uses.
#[derive(Clone)]
pub struct Platform {
    pub capturer: Arc<dyn Capturer>,
    pub selector: Arc<dyn RegionSelector>,
    pub ocr: Arc<dyn OcrEngine>,
    pub hotkeys: Arc<dyn HotkeyProvider>,
    pub notifier: Arc<dyn Notifier>,
    pub autostart: Arc<dyn AutoStart>,
}

impl Platform {
    pub fn new<R: Runtime>(app: &AppHandle<R>) -> Self {
        Self {
            capturer: native_capturer(),
            selector: native_selector(),
            ocr: native_ocr(),
            hotkeys: Arc::new(tauri_backed::PluginHotkeys::new(app.clone())),
            notifier: Arc::new(tauri_backed::PluginNotifier::new(app.clone())),
            autostart: Arc::new(tauri_backed::PluginAutoStart::new(app.clone())),
        }
    }
}

// Capture and OCR don't need a running Tauri app, so they're constructible on their own
// (used by the spikes in `examples/` and, later, by headless tests).

pub fn native_capturer() -> Arc<dyn Capturer> {
    Arc::new(xcap_capturer::XcapCapturer)
}

#[cfg(windows)]
pub fn native_ocr() -> Arc<dyn OcrEngine> {
    Arc::new(windows::ocr::WindowsOcr)
}

#[cfg(target_os = "macos")]
pub fn native_ocr() -> Arc<dyn OcrEngine> {
    Arc::new(macos::VisionOcr)
}

#[cfg(windows)]
pub fn native_selector() -> Arc<dyn RegionSelector> {
    Arc::new(windows::overlay::GdiRegionSelector)
}

#[cfg(target_os = "macos")]
pub fn native_selector() -> Arc<dyn RegionSelector> {
    Arc::new(macos::AppKitRegionSelector)
}

/// Lowers the calling thread's CPU priority, for background work (OCR, the AI queue) that
/// must never make the user's foreground apps feel slow.
pub fn lower_current_thread_priority() {
    #[cfg(windows)]
    {
        use ::windows::Win32::System::Threading::{GetCurrentThread, SetThreadPriority, THREAD_PRIORITY_BELOW_NORMAL};
        // SAFETY: GetCurrentThread returns a pseudo-handle that is always valid for this thread.
        if let Err(e) = unsafe { SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_BELOW_NORMAL) } {
            tracing::warn!(error = %e, "couldn't lower thread priority");
        }
    }
}

/// A command for a background helper process (the AI engine): no console window, below
/// normal CPU priority.
pub fn background_command(program: &std::path::Path) -> std::process::Command {
    #[allow(unused_mut)]
    let mut cmd = std::process::Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        const BELOW_NORMAL_PRIORITY_CLASS: u32 = 0x0000_4000;
        cmd.creation_flags(CREATE_NO_WINDOW | BELOW_NORMAL_PRIORITY_CLASS);
    }
    cmd
}

/// Ties a child process's lifetime to this app: if the app exits for any reason (including
/// a crash or being killed), the OS terminates the child too, so it's never orphaned.
pub fn bind_child_to_app(child: &std::process::Child) {
    #[cfg(windows)]
    windows::job::assign(child);
    #[cfg(not(windows))]
    let _ = child;
}

/// Free bytes on the volume holding `path`, if it can be determined.
pub fn free_disk_space(path: &std::path::Path) -> Option<u64> {
    #[cfg(windows)]
    {
        windows::job::free_space(path)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        None
    }
}

/// True when the OS battery saver is on (the AI then pauses if the user allows it).
pub fn battery_saver_on() -> bool {
    #[cfg(windows)]
    {
        use ::windows::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        let mut s = SYSTEM_POWER_STATUS::default();
        // SAFETY: `s` is a valid out-parameter for the call's duration.
        unsafe { GetSystemPowerStatus(&mut s) }.is_ok() && s.SystemStatusFlag == 1
    }
    #[cfg(not(windows))]
    {
        false
    }
}
