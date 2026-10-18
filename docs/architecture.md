# Architecture

ClipNest is a Tauri 2 app: a Rust core that owns capture, storage and the AI, and a React UI in the system WebView. They talk through Tauri commands (`src-tauri/src/commands.rs`, `library.rs`, `ai/commands.rs`, typed on the UI side in `src/lib/ipc.ts`) and events (`library-changed`, `settings-changed`, `queue-status`, `ai-status`).

## The life of a screenshot

1. **Shortcut.** `hotkey.rs` registers the global shortcut (with a fallback when it's taken). The tray's "Capture now" does the same.
2. **Overlay.** `capture/session.rs` freezes every monitor and opens the native overlay (`platform/windows/overlay.rs`, Win32 + GDI), one window per monitor, physical pixels throughout.
3. **Note popup.** A small webview (`src/popup/`) shows the thumbnail; the user types a note and picks Text or Image mode and, optionally, categories.
4. **Save.** `capture/store.rs` writes the PNG and thumbnail and inserts a row (`db/`). The capture is safe on disk before any processing starts.
5. **Queue.** `queue.rs` processes items oldest first, survives restarts, and retries failures up to three times.
6. **OCR.** `ocr.rs` prepares the image (invert dark images, upscale small text) and runs `Windows.Media.Ocr`. Captures with almost no text switch to image mode.
7. **AI.** `ai/engine.rs` starts `llama-server` on `127.0.0.1` at below-normal priority, sends the prompt from `ai/prompt.rs` (versioned in `prompts/`) with a JSON schema, validates the answer, and stops the server after two idle minutes.
8. **Deadlines.** `deadlines/` resolves dates in the result, decides whether the note shows intent to act (`config/intent-keywords.txt` plus the model's flag), and schedules reminders.
9. **UI.** The main window reloads on `library-changed` and shows the filed card.

## Platform layer

`src-tauri/src/platform/mod.rs` defines traits for everything OS-specific: `Capturer`, `RegionSelector`, `OcrEngine`, `HotkeyProvider`, `Notifier`, `AutoStart`, plus process priority and lifetime helpers. Windows implements them all; `platform/macos/` holds stubs for a future port (capture already uses the cross-platform `xcap` crate).

## Data

One SQLite database (`library.db`) with versioned migrations in `src-tauri/migrations/`, FTS5 full-text search kept in sync by triggers, and images stored as separate files under `images/` and `thumbs/`. `db/backup.rs` snapshots the database daily (keeps 3) and restores the newest healthy copy if the database is ever unreadable.

## AI downloads

`models.json` lists the engine and models with pinned URLs, sizes, SHA-256 hashes and measured memory use, grouped into profiles (`standard`, `accurate`). `ai/download.rs` downloads resumably, checks free space and verifies the hash before use. Changing models is a data change in that file.

More detail and the measurements behind each choice: [decisions.md](decisions.md).
