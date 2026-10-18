# Testing

## Automated

```bash
pnpm lint && pnpm format:check && pnpm typecheck && pnpm test   # UI: lint, formatting, types, unit tests
cd src-tauri && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test
```

The Rust tests cover migrations, FTS sync, category rules, settings, DPI and crop math, the queue, deadlines, downloads and backups. CI runs all of the above on every push and pull request.

Spikes (rerunnable; they write their output to `.spikes/`):

```bash
cd src-tauri
cargo run --release --example spike_capture
powershell -File examples/make_ocr_samples.ps1 -Out ../.spikes
cargo run --release --example spike_ocr -- ../.spikes/ocr-light.png ../.spikes/ocr-light.txt
bash ../scripts/fetch-spike-models.sh "$LOCALAPPDATA/rs-spike-models"
python -I ../scripts/bench_llm.py --server <llama-server.exe> --model <model.gguf> [--mmproj <mmproj.gguf>]
```

## Manual checklist

Status key: ✅ verified · ⏳ not yet testable (later phase) · ⚠️ needs hardware not available during development

### Phase 1: foundation
- ✅ First launch creates `%APPDATA%\com.reportedsnipping.desktop\{library.db, images, thumbs, logs}` and seeds 6 categories.
- ✅ Theme follows the OS (dark/light).
- ✅ Closing the window hides to the tray; the process keeps running and WebView2 memory is released.
- ⏳ Tray: left-click opens the window; "Pause AI processing" toggles and persists across restart; "Quit" exits. *(Code complete; not yet clicked by hand. Please check during Phase 1 review.)*
- ✅ Launching a second copy focuses the running one and exits.
- ✅ Win+Shift+J registers on Windows 11. A real keypress reaches the handler.
- ✅ If another app owns Win+Shift+J, the app falls back to Ctrl+Shift+J and says so.
- ⚠️ Win+Shift+J on Windows 10.
- ✅ `--minimized` (autostart) launch creates no window or webview (a single process, about 7 MB private).
- ✅ Start-with-Windows is never registered by dev builds (debug, or run from `target\`).

### Phase 2: capture
- ✅ Win+Shift+J shows the frozen, dimmed overlay in 46–74 ms (release build, app in tray).
- ⏳ Tray "Capture now" starts the same flow. *(Code complete; not yet clicked by hand.)*
- ✅ Overlay takes keyboard focus; Esc / right-click / Alt+F4 cancel; a click without a drag does nothing.
- ✅ Dragged region is saved at exactly the dragged physical size (600×340 drag → 600×340 PNG); live size label matches.
- ✅ Popup appears bottom-right above the taskbar with focus in the note box; Enter saves (3–6 ms) and shows "Saved" briefly; Esc discards and leaves no files.
- ✅ Saved row: Miscellaneous + queued for Auto; note, mode and size stored; FTS finds the note.
- ✅ Main window counts refresh live after a save.
- ⏳ Shift+Enter inserts a newline; choosing a category in the popup locks it. *(Code complete; not yet driven by hand.)*
- ⚠️ Multi-monitor and mixed DPI (one overlay per monitor; selection stays on the starting monitor).

### Phase 3: UI shell
- ✅ Settings › Categories renders rows with color, name, hint, reorder and delete; Miscellaneous can't be renamed or deleted (unit-tested in the DB layer too).
- ✅ Onboarding completes and is remembered.
- ⏳ By hand: add/rename/recolor/reorder/delete a category; deleting moves its items to Miscellaneous.
- ⏳ By hand: in a category, use each card action (move with Undo, edit title/summary, edit note, copy image, open in viewer, retry, delete with Undo); arrow keys, Enter and Delete on focused cards.
- ⏳ By hand: change the capture shortcut in Settings, including a taken one (should explain and keep the old one).
- ⏳ Dev build: Settings › Developer › "Fill queued screenshots with sample AI results" shows finished cards.

### Phase 4: OCR + queue
- ✅ On restart, queued items are OCR'd automatically in the background; the cards show the OCR text and wait for the AI without using up attempts.
- ✅ Header badge and tray tooltip show "N screenshots waiting for the AI model".
- ✅ Unit tests: oldest-first order, crash recovery, 3-attempt failure policy, waiting-for-AI doesn't burn attempts, sticky auto-upgrade, dark/small-image preparation, meaningful-character counting.
- ⏳ By hand: capture something with almost no text (e.g. a color swatch). It should show "Auto-upgraded to Image".
- ⏳ By hand: Pause AI in the tray, capture, and see it wait; resume and see it processed.

### Phase 5: local AI
- ✅ Unit tests: registry consistency, prompt files filled and valid, schema restricts categories, head+tail truncation (multi-byte safe), answer validation and fallback to Miscellaneous, URL extraction and grounding, download verification markers and resume accounting, locked categories survive filing, re-check only moves items out of Miscellaneous.
- ✅ Real setup in the app: engine (19.5 MB) and model (1.28 GB) downloaded and verified; 4 queued screenshots filed at 6–19 s each.
- ✅ Interrupted download resumes after a restart (image add-on: 620 MB → resumed → verified).
- ⏳ By hand: Settings › AI › Pause, then Resume.
- ⏳ By hand: add a category, accept "Re-check Miscellaneous", and see matching items move.
- ⏳ By hand: switch the model to "More accurate" and back.

### Phase 6: image mode
- ⏳ By hand: choose Image in the popup with no image model installed. The footer should offer the one-time download.
- ✅ Image-mode item filed with the vision model in 48 s (palette → "Brand color palette", correct colors).
- ⏳ By hand: capture a picture with almost no text. It should auto-upgrade and the home screen should offer the image model.

### Phase 7: deadlines + search
- ✅ Real item "register for this" + hackathon screenshot → Opportunities & Deadlines, deadline 12 Nov 2026 23:59, important, shown in the strip with Done.
- ✅ Unit tests: DD/MM parsing, month names both orders, next-occurrence years, model-date validation, keyword intent (whole words), note-only dates, manual deadlines win, strip order, reminders fire once and catch up, OCR split-digit repair, FTS query safety, ranking, trash hidden, 10k-item speed.
- ⏳ By hand: Set deadline… / Change / Remove on a card; Done in the strip.
- ⏳ By hand: a reminder toast (set a deadline for tomorrow with lead "1 day", hour = now, and restart the app).
- ⏳ By hand: search from the home screen; Ctrl+K from a category view.

### Phase 8: polish
- ✅ Final release build: installer 4.8 MB; idle 11 MB private in the tray; hotkey → overlay 52–62 ms; no autostart entry from dev builds; no orphan processes after quitting.
- ✅ Unit tests: corrupted library restored from backup (damaged file kept); fresh start without a backup; only the newest 3 backups kept.
- ⏳ By hand: Settings › Storage › Export all data. Open the zip and check images, index.json and index.md.
- ⏳ By hand: turn on Windows battery saver. The badge should say "AI paused for battery saver" and resume when it's off.
- ⏳ On a real 8 GB, CPU-only laptop: run the full flow and watch memory while filing.

### Multi-category + feed (2026-10-16)
- ✅ Real library upgraded (schema 6): every screenshot kept its category; tags indexed.
- ✅ Feed shows all screenshots, 4 per row, whole images; category chips on cards.
- ✅ Clicking a category pill filters (count shown, deadline band hidden); adding a tag narrows further (must match all); All resets.
- ✅ "+ New category" dialog opens with name, hint and palette.
- ✅ The real AI with prompt v2 filed a screenshot into two categories.
- ✅ Unit tests: set_categories rules, delete keeps other categories, recheck only adds, feed filters/paging/trash, tag counts, upgrade migration, 10k-item speed.
- ⏳ By hand: create a category, accept "Check", and see existing screenshots gain it.
- ⏳ By hand: Edit categories… on a card (tick several, create one), then confirm a re-check doesn't change it.
- ⏳ By hand: capture with the popup's category picker (several categories, and create one while capturing).
- ⏳ By hand: the Filter search popover (search, tick, Clear all).

### Remaining manual checks from the spec
- ⚠️ Multi-monitor capture with a monitor left of / above the primary (negative coordinates), and mixed DPI (100% + 150%).
- ⏳ Hotkey conflict UI and changing the hotkey in Settings.
- ⏳ Sleep/resume: hotkey still works, queue resumes.
- ✅ Kill the app mid-processing: item returns to `queued` on restart, nothing lost (unit-tested; observed after dev restarts).
- ⏳ No internet: capture, OCR, search all work; AI waits.
- ⚠️ No OCR language pack: app shows the notice; items are filed using note/image only.
- ✅ 10,000-item search stays fast (3 ms).
