//! Structured logging to a local, daily-rotated file. Nothing is ever sent remotely.

use std::path::Path;
use std::sync::OnceLock;

use tracing_appender::non_blocking::WorkerGuard;
use tracing_appender::rolling::{Builder, Rotation};
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

/// Keeps the background log writer alive for the life of the process.
static GUARD: OnceLock<WorkerGuard> = OnceLock::new();

const KEEP_DAYS: usize = 7;

pub fn init(log_dir: &Path) {
    let file = Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix("clipnest")
        .filename_suffix("log")
        .max_log_files(KEEP_DAYS)
        .build(log_dir);

    let filter = EnvFilter::try_from_env("CLIPNEST_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let registry = tracing_subscriber::registry().with(filter);

    match file {
        Ok(appender) => {
            let (writer, guard) = tracing_appender::non_blocking(appender);
            let _ = GUARD.set(guard);
            let file_layer = fmt::layer().with_writer(writer).with_ansi(false);
            let console = cfg!(debug_assertions).then(fmt::layer);
            let _ = registry.with(file_layer).with(console).try_init();
        }
        Err(e) => {
            // Logging must never stop the app from starting.
            let _ = registry.with(fmt::layer()).try_init();
            tracing::warn!(error = %e, "file logging unavailable; logging to console only");
        }
    }
}
