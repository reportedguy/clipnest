//! ClipNest core.

pub mod ai;
pub mod capture;
pub mod commands;
pub mod db;
pub mod deadlines;
pub mod error;
pub mod export;
pub mod hotkey;
pub mod library;
pub mod logging;
pub mod media;
pub mod ocr;
pub mod platform;
pub mod presets;
pub mod queue;
pub mod settings;
pub mod state;
pub mod tray;
pub mod util;

use std::fs;

use tauri::{AppHandle, Manager, RunEvent, WindowEvent};
use tauri_plugin_autostart::MacosLauncher;
use tauri_plugin_global_shortcut::ShortcutState;

use crate::platform::Platform;
use crate::state::AppState;

/// Passed by the autostart entry so a login launch goes straight to the tray.
const MINIMIZED_ARG: &str = "--minimized";

pub fn run() {
    tauri::Builder::default()
        // Must be first: a second launch just surfaces the running instance.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| tray::show_main(app)))
        .plugin(tauri_plugin_autostart::init(MacosLauncher::LaunchAgent, Some(vec![MINIMIZED_ARG])))
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(|app, _shortcut, event| {
                    if event.state == ShortcutState::Pressed {
                        capture::start(app);
                    }
                })
                .build(),
        )
        .register_asynchronous_uri_scheme_protocol(media::PROTOCOL, |ctx, request, responder| {
            let app = ctx.app_handle().clone();
            let path = request.uri().path().to_owned();
            // Answer off the webview's thread: this reads files and takes locks.
            std::thread::spawn(move || responder.respond(media::serve(&app, &path)));
        })
        .setup(|app| {
            setup(app.handle())?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { .. } = event {
                let app = window.app_handle();
                match window.label() {
                    // Without close-to-tray, closing the main window quits the app.
                    "main" if !app.state::<AppState>().settings().close_to_tray => app.exit(0),
                    label => capture::session::on_window_close_requested(app, label),
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            commands::app_info,
            commands::list_categories,
            commands::get_settings,
            commands::update_settings,
            capture::session::popup_attach,
            capture::session::popup_ready,
            capture::session::save_capture,
            capture::session::cancel_capture,
            capture::session::close_popup,
            library::feed,
            library::filter_options,
            library::tag_options,
            library::set_card_categories,
            library::update_card_text,
            library::update_card_note,
            library::delete_card,
            library::restore_card,
            library::retry_card,
            library::copy_card_image,
            library::open_card_image,
            library::open_url,
            library::create_category,
            library::update_category,
            library::reorder_categories,
            library::delete_category,
            presets::category_presets,
            presets::apply_category_setup,
            library::storage_info,
            library::dev_fake_ai,
            queue::queue_status,
            ai::commands::ai_status,
            ai::commands::ai_profiles,
            ai::commands::ai_start_setup,
            ai::commands::ai_start_vision_setup,
            ai::commands::ai_pause_download,
            ai::commands::ai_delete,
            ai::commands::recheck_categories,
            ai::commands::refile_all,
            deadlines::upcoming_deadlines,
            deadlines::set_deadline,
            deadlines::mark_deadline_done,
            export::export_all,
        ])
        .build(tauri::generate_context!())
        .expect("failed to build Tauri application")
        .run(|app, event| match event {
            // The last window closing (main, or a capture popup) must never end the app: it
            // lives in the tray. Real quits (tray "Quit", or closing main with close-to-tray
            // off) call `exit` with an explicit code, which isn't intercepted here.
            RunEvent::ExitRequested { code: None, api, .. } => api.prevent_exit(),
            RunEvent::Exit => {
                // The job object would end it anyway; stopping first lets it exit cleanly.
                if let Some(ai) = app.try_state::<ai::Ai>() {
                    ai.stop();
                }
                tracing::info!("exiting");
            }
            _ => {}
        });
}

fn setup(app: &AppHandle) -> error::Result<()> {
    let data_dir = app.path().app_data_dir()?;
    for sub in ["images", "thumbs", "logs"] {
        fs::create_dir_all(data_dir.join(sub))?;
    }
    logging::init(&data_dir.join("logs"));
    tracing::info!(version = %app.package_info().version, data_dir = %data_dir.display(), "starting");

    let (conn, recovered) = db::backup::open_with_recovery(&data_dir)?;
    let settings = settings::load(&conn)?;
    let platform = Platform::new(app);
    capture::store::clear_pending(&data_dir);
    // Must exist before the hotkey is live: a press goes straight to the capture manager.
    app.manage(capture::CaptureManager::default());

    sync_autostart(&platform, settings.start_with_windows);
    let hotkey = hotkey::register_at_startup(&platform, &settings.hotkey);

    tray::create(app, settings.ai_paused)?;
    let resume_ai = settings.ai_download_consent;
    let resume_vision = settings.ai_vision_consent;
    let profile = settings.ai_profile.clone();
    app.manage(ai::Ai::new(&data_dir));
    let mut state = AppState::new(conn, settings, hotkey, platform, data_dir);
    state.startup_notice = recovered;
    library::purge_trash(&state);
    app.manage(state);
    // Daily snapshot of the library, off the startup path.
    let handle = app.clone();
    std::thread::spawn(move || {
        let state = handle.state::<AppState>();
        let result = db::backup::daily_backup(&state.db(), &state.data_dir);
        if let Err(e) = result {
            tracing::warn!(error = %e, "daily backup failed");
        }
    });
    ai::start_reaper(app);
    deadlines::start_scheduler(app);
    // The user already agreed to the download: finish it if it was interrupted.
    let ai_state = app.state::<ai::Ai>();
    if resume_ai && !ai_state.text_ready(&profile) {
        ai::Ai::start_download(app, profile, resume_vision);
    } else if resume_vision && !ai_state.vision_ready(&profile) {
        ai::Ai::start_download(app, profile, true);
    }
    let worker = queue::start(app);
    app.manage(worker);

    if !std::env::args().any(|a| a == MINIMIZED_ARG) {
        tray::show_main(app);
    }
    Ok(())
}

/// Makes the OS login item match the setting. Skipped for development binaries (debug
/// builds, or anything run from a cargo `target` dir) so they never register at login.
pub fn sync_autostart(platform: &Platform, enabled: bool) {
    let in_target_dir = std::env::current_exe()
        .map(|p| p.components().any(|c| c.as_os_str().eq_ignore_ascii_case("target")))
        .unwrap_or(false);
    if cfg!(debug_assertions) || in_target_dir {
        return;
    }
    let current = platform.autostart.is_enabled().unwrap_or(!enabled);
    if current != enabled {
        if let Err(e) = platform.autostart.set_enabled(enabled) {
            tracing::warn!(error = %e, enabled, "couldn't update start-with-Windows");
        }
    }
}
