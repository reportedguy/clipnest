# Decisions

> ClipNest was called "Reported Snipping" while it was built, so older entries use that name. The app identifier `com.reportedsnipping.desktop` and its data folder keep the old name on purpose: changing them would orphan existing libraries.

Choices made where the spec left room, with reasons. Newest phase at the bottom of each section. **⚠ = needs your confirmation.**

## Spike results (Phase 1)

Test machine: Intel Core Ultra 7 155U (15 W laptop chip, 12 cores / 14 threads), integrated Intel graphics only (no discrete GPU), 32 GB RAM, Windows 11 Pro, one 1920×1080 display at 125%. AI benchmarks were capped at **4 threads** to approximate the spec's "half the cores of a mid-range laptop". RAM figures are measured peaks, compared against the 8 GB-laptop budgets.

### 1. Global hotkey Win+Shift+J: ✅ works
- `tauri-plugin-global-shortcut` registers `Super+Shift+J` on Windows 11. A real synthesized keypress reaches the handler.
- **Conflict path verified:** with another process holding Win+Shift+J via `RegisterHotKey`, registration fails cleanly. The app then registers **Ctrl+Shift+J** as a fallback, logs why, and the UI shows a notice. The stored setting is left unchanged, so the user decides.
- If the fallback also fails, status is `failed`. The UI says capture is still available from the tray (tray "Capture now" lands in Phase 2).
- No raw `RegisterHotKey` fallback is needed: the plugin uses `RegisterHotKey` internally and surfaces the error.
- ⚠️ Not tested on Windows 10 (no machine available). Win+Shift+J isn't a documented Windows 10 system shortcut, so we expect the same result. The fallback covers it if not.

### 2. Frozen-frame capture and crop coordinates: ✅ (single monitor verified; multi-monitor logic unit-tested)
- `xcap` grabs a full-resolution frame in **28–35 ms** (1080p), leaving most of the 300 ms hotkey-to-overlay budget for showing the overlay.
- The frame size equals the monitor's reported physical size, so xcap reports physical pixels at 125% scaling.
- **Design:** one overlay window per monitor. A selection is converted using *that* monitor's scale factor and cropped from *that* monitor's frame, so negative virtual-desktop origins and mixed DPI never enter the crop math (`capture/geometry.rs`, 7 unit tests including 150% scaling, fractional edges, reversed drags, and a monitor at a negative origin).
- **Decision:** a selection stays on the monitor where the drag started (no spanning across monitors). It's simpler, and it matches how Windows Snipping Tool behaves.
- ⚠️ Real multi-monitor / mixed-DPI hardware wasn't available. It's in [testing.md](testing.md) as a manual check.

### 3. Windows.Media.Ocr from Rust: ✅ fast and accurate
- Called through the `windows` crate (0.62). It works off the UI thread with no explicit COM setup.
- **Speed:** 8–12 ms per screenshot warm, about 100 ms for the first call. Far under the 1 s budget.
- **Accuracy** on rendered screenshot-like samples (CER = character error rate):

  | Sample | CER | 2× upscaled |
  |---|---|---|
  | Light UI, 16 px | 3.9% (dropped "5,00,000") | **0.4%** |
  | Dark UI, 15 px | **0.0%** | 12.5% |
  | Small 11 px | 1.0% ("and"→"ard") | **0.0%** |

  Upscaling helps light/small text but hurts dark backgrounds. **Phase 4 plan:** normalize to dark-on-light (invert when the background is dark), then upscale 2× when the text is small. Re-measure on real screenshots.
- Note: the Indian-format number "5,00,000" was dropped at 1×. Worth checking in Phase 4, given the users are in India.
- **No language pack:** `TryCreateFromUserProfileLanguages` fails, and we return `OcrUnavailable`. The UI shows a notice pointing to Windows Settings. Items still get filed (note + Image mode). This could not be simulated here because English packs are installed; it's in TESTING.md.
- Installed recognizers here: en-GB, en-US.

### 4. Text model on CPU via llama-server: ✅ (default stepped down to meet budgets)
llama.cpp **b11669** CPU build, `-t 4 -c 4096`, below-normal priority, JSON-schema-constrained output via `response_format` (the category is an `enum` of the user's categories, so the model *cannot* return an unknown category). 9 test cases in `scripts/bench_cases.json`: note-outranks-text, DD/MM dates, a missing year, a date without intent, a receipt that should go to Miscellaneous, and others.

| Model (Q4) | Size | Median / case | Gen t/s | JSON valid | Category | Intent | Deadline | Peak RAM (WS / private) |
|---|---|---|---|---|---|---|---|---|
| Qwen2.5-3B-Instruct (spec default) | 2.1 GB | 10.3 s | 10.0 | 9/9 | 7/9 | 8/9 | 7/9 | 3.4 GB / 1.7 GB |
| Llama 3.2 3B Instruct | 2.0 GB | 10.6 s | 9.3 | 9/9 | 7/9 | 7/9 | 9/9 | 3.8 GB / 2.0 GB |
| Gemma 3 4B-it QAT | 2.5 GB | 35.8 s | 7.2 | 9/9 | 7/9 | 7/9 | 7/9 (garbled ISO) | 5.7 GB / 3.6 GB* |
| **Qwen3.5-4B** | 2.7 GB | 22.1 s | 6.9 | 9/9 | **8/9** | **9/9** | **9/9** | 4.4 GB / 2.3 GB |
| **Qwen3.5-2B** | **1.3 GB** | **9.6 s** | **~15** | 9/9 | 7/9 | 6/9 | 8/9 | **3.0 GB / 2.5 GB*** |

\* with the vision projector loaded too.

- Every model returned schema-valid JSON 9/9. Constrained decoding works, so the "retry once, then fail" path is rarely needed.
- Model load time is 2.7–4.2 s from a warm disk cache.
- Every model misfiled the receipt (Resources instead of Miscellaneous). That's a prompt and category-hint issue, to tune in Phase 5.
- Most intent and missing-year errors are covered by design: intent = model **OR** a deterministic keyword check, and dates are resolved in Rust with chrono.
- Gemma 3 produced invalid dates such as `0303-20-27`. This confirms we must validate dates in Rust, not trust the model.
- A smaller context (2048) did **not** reduce Qwen3.5-4B's peak RAM, because the weights dominate.

### 5. Vision via llama-server + mmproj: ✅
Two visual-only screenshots: a 5-color palette and a bar chart, with note "save this for my design moodboard".

| Model | Time / image | Quality | Peak RAM (WS / private) |
|---|---|---|---|
| **Qwen3.5-2B** + mmproj F16 (0.67 GB) | **49–56 s** | Correct; read "61 million in June" off the chart | **3.0 GB / 2.5 GB** |
| Qwen3.5-4B + mmproj F16 (0.67 GB) | 95–110 s | Correct, slightly richer | 5.5 GB / 3.6 GB |
| Gemma 3 4B QAT + mmproj (0.85 GB) | 81–88 s | Correct; named the palette's hues | 5.7 GB / 3.6 GB |

### 6. Installer size and idle RAM: ✅ (after one fix)
- **NSIS installer: 3.27 MiB** (budget 30 MB).
- Idle RAM, first measurement: the app process was 10 MB private, but **WebView2 kept 171 MB private across 7 processes** even when the window was hidden to the tray. Over the 80 MB budget.
- **Fix (decision):** closing the main window *destroys* its webview, the app stays in the tray, and opening recreates it from config. Autostart launches (`--minimized`) don't create the window at all.
- **Result:** in the tray, **10 MB private / 36 MB working set, 1 process**. With the window open it's about 175 MB private (WebView2), released on close.
- The open question this raised (pre-created webview windows vs. idle RAM) was settled in Phase 2: see "Capture overlay" below.

## Model choice (confirmed 2026-10-15)

**Default: Qwen3.5-2B (Q4_K_M) as a single model for both Text and Image mode.**
- It's the only candidate within every 8 GB-laptop budget (text under 3.5 GB, vision under 5 GB, text well under 45 s, vision well under 3 min).
- It's natively multimodal, so Image mode needs only the 0.67 GB `mmproj` file, downloaded lazily as the spec asks. First-run download is **1.28 GB** (model) + **19.5 MB** (engine).
- Same category accuracy as the spec's Qwen2.5-3B, at half the latency and less RAM. The main gap versus 4B is intent detection, which the keyword check backs up.
- License: Apache-2.0.

**Optional "accurate" profile: Qwen3.5-4B** (best scores, about 22 s per screenshot), offered in Settings to machines with 16 GB+ RAM. Both are in `models.json`.

Rejected: Gemma 4 E4B (5.15 GB at Q4, too large for 8 GB). Gemma 3 4B (slowest and least accurate for text). Llama 3.2 3B (over the text RAM budget, and no vision).

## Architecture and implementation decisions (Phase 1)

- **App identifier:** `com.reportedsnipping.desktop`. Data lives in `%APPDATA%\com.reportedsnipping.desktop\` (`library.db`, `images/`, `thumbs/`, `logs/`).
- **Package manager:** pnpm (already installed). **Tailwind CSS 4** via the Vite plugin. Theme uses `data-theme` on `<html>`, so `system/light/dark` from Settings overrides the OS, and "system" tracks OS changes live.
- **No shadcn/Radix yet.** No component in Phase 1 needs one. Radix primitives (dropdown, dialog, toggle-group) arrive in Phase 3 when the popup and Settings need them.
- **SQLite:** `rusqlite` with the `bundled` feature (FTS5 included, verified by test). WAL mode, foreign keys on.
- **Timestamps** are stored as Unix epoch **milliseconds** (integers): easy to sort and compare, with chrono for display.
- **Migrations:** embedded SQL files (`src-tauri/migrations/NNNN_*.sql`), applied in order inside transactions and tracked in `schema_migrations`. Append-only.
- **Schema additions beyond the spec:**
  - `category_locked`: set when the user picks a category in the popup, so the AI never overrides an explicit choice.
  - `attempts`: caps automatic retries.
- **Miscellaneous protection is enforced in the database** (triggers block rename/delete of `is_system` categories), not only in the UI. Category names are unique case-insensitively.
- **Deleting a category:** the foreign key is `ON DELETE RESTRICT`. The app must move items to Miscellaneous first, so a bug can't silently orphan screenshots.
- **FTS:** an external-content FTS5 table over title, summary, note, OCR text, and `tags_json`, kept in sync by triggers. `unicode61 remove_diacritics 2` with 2- and 3-character prefix indexes for fast prefix search.
- **Settings:** one JSON value per key in `settings`. Missing keys fall back to defaults, and a single corrupt value falls back alone without resetting the others.
- **The hotkey is stored in Tauri accelerator syntax** (`Super+Shift+J`) and shown as `Win+Shift+J`.
- **Platform layer:** `Capturer`, `OcrEngine`, `HotkeyProvider`, `Notifier`, `AutoStart` traits in `platform/`. Capture (`xcap`), hotkeys, notifications and autostart are already cross-platform crates or plugins, so the only Windows-specific code is OCR. macOS has an `OcrEngine` stub returning "not implemented yet".
- **Autostart in development:** never registered by debug builds or by any binary running from a cargo `target\` folder. (During spike 6 a release run registered itself once. I removed that `Run` key entry and added this guard.)
- **Single instance:** a second launch focuses the running window and exits.
- **Logging:** `tracing` to `logs/clipnest.YYYY-MM-DD.log`, rotated daily, kept 7 days. Override the level with `CLIPNEST_LOG`.
- **Tray "Capture now"** is deferred to Phase 2, when capture exists, so the menu has no dead item.
- **Engine distribution:** llama.cpp is **downloaded at first run** with the model (19.5 MB, SHA256-verified), not bundled. The installer stays at about 3 MB, and engine plus model versions move together via `models.json`. The official CPU build bundles per-CPU-generation backends (Sandy Bridge through Sapphire Rapids) and picks one at runtime, so **we don't need our own AVX2 detection**.
- **Thinking mode:** Qwen3.5 defaults to a "thinking" phase. We pass `chat_template_kwargs: {enable_thinking: false}`, recorded per model in `models.json`.

## Phase 2: capture

### Capture overlay is native Win32/GDI, not a webview
Measured on a release build, app resting in the tray:

| Overlay approach | Hotkey → overlay | Idle RAM (tray) |
|---|---|---|
| Webview overlay, created on demand | 538–664 ms ❌ | 10 MB |
| Webview overlay + hidden "keep-warm" webview | 265–320 ms (borderline) | 140–170 MB ❌ |
| **Native GDI overlay** (chosen) | **46–74 ms** ✅ | **~11 MB** ✅ |

The overlay UI is simple (a frozen image, dimming, a rectangle, a size label), so native code is small (`platform/windows/overlay.rs`). It meets both budgets at once. It sits behind a new `RegionSelector` platform trait; macOS gets an AppKit implementation in the port (stubbed for now).

Side benefit: the process is per-monitor DPI aware, so native windows and mouse coordinates are already **physical pixels**, the frozen frame's own space. The logical→physical conversion (and the floating-point off-by-one it caused in the webview version) no longer exists.

### Popup
- A React webview, **created hidden the moment the hotkey fires** on its own thread, so WebView2 starts while the user is still dragging. It's shown once the thumbnail has painted, so there's no blank flash. Measured selection → popup visible: **41 ms** (it was 383 ms when created after the selection).
- Placed at the bottom-right of the captured monitor's **work area** (above the taskbar). The spec's "next to the selection if there's room" was skipped for v1, since a fixed corner is more predictable.
- A second hotkey press while the popup is open re-focuses the popup instead of starting a new capture, so the note being typed is never lost.
- The category picker is a native `<select>`, which is accessible and keyboard-friendly with no dependency.
- Closing the popup by any means (Esc, Alt+F4) discards the capture. Esc/right-click/Alt+F4 in the overlay cancels.

### Saving
- On mouse-up, the crop's thumbnail (WebP, 480 px wide, lossless) is written immediately, and the full PNG **encodes on a background thread** while the user types. Enter then only moves two files and inserts one row: **3–6 ms** measured (budget 200 ms).
- "Auto" captures are stored in **Miscellaneous** with `status = queued` until the AI files them; a chosen category sets `category_locked`.
- Files go to `images/YYYY-MM/` and `thumbs/YYYY-MM/` with millisecond timestamps. A save never overwrites an existing file (tested).
- A failed save (e.g. disk full) leaves the capture pending, so the user can retry; leftovers from a crash are cleared at startup.
- The popup's thumbnail is served from memory by a small `rsframe://` protocol (no temp-file reads).

### Test tooling note
Automated real-input tests must **never send Esc/Enter blindly**: if the target window isn't in front, the keystroke lands in whatever is (during development that was the Claude window, where Esc means "interrupt"). The test scripts now verify the foreground window first and close overlays with `WM_CLOSE` instead of Esc.

## Phase 3: UI shell

- **Screens:** Home (category tiles with a preview of the newest thumbnail and a count), Category (card grid), Settings, and a 2-step onboarding (welcome → "try it now", which waits for a real capture). The spec's third onboarding step (AI setup with download progress) is added in Phase 5, when the download exists.
- **No router library.** Three views and a back button don't justify one; navigation is a small state machine in `App.tsx`.
- **Cards** follow your sketch: image (click to enlarge) → title → summary → tags → collapsed "Original text" (your note, the OCR text, and links shown small and muted). Queued items shimmer, and failed items show the reason with a Retry link.
- **Card actions** are in a hover/focus menu: move to (submenu), edit title & summary, edit note, copy image, open in image viewer, retry AI, delete. Move and delete show an **Undo** toast.
- **Delete is a soft delete** (migration 0002 adds `deleted_at`), so Undo can't fail. Trashed items are purged with their files after 24 hours, at startup.
- **Moving a card manually locks its category**, so the AI never moves it back. Deleting a category moves its items to Miscellaneous and *unlocks* them, so "re-check Miscellaneous" can re-file them.
- **Keyboard:** cards are focusable; arrow keys move between them (row-aware); Enter enlarges; Delete removes (with undo). (Ctrl+K for search lands with search in Phase 7.)
- **Links from screenshots** open in the default browser only on an explicit click, through a command that accepts only `http(s)` URLs. Nothing from OCR is ever executed or opened automatically.
- **Images reach the UI through the app's own `rsmedia://` protocol**, which only serves files under `images/` and `thumbs/` (path traversal is rejected, with a test). Tauri's general asset protocol stays disabled, so the webview can't read arbitrary files.
- **Hotkey recorder** in Settings: press a combination; conflicts show the reason and keep the old shortcut. When the fallback is in use, a one-click "use it permanently" link is offered.
- **Settings implemented now:** categories, shortcut, start with Windows, close to tray, reminder lead time and hour, pause AI, theme, storage location and size, about/privacy. *Later:* AI/model status (Phase 5), export (Phase 8).
- **Radix UI** (menus, dialogs, alert dialog, switch) for accessible behavior; **Lucide** for icons.
- **Dev-only "sample AI results" button** (Settings › Developer, dev builds only; the command refuses in release builds) lets the card UI be reviewed before the real AI exists.

## Phase 4: OCR + persistent queue

- **The queue is the `screenshots` table** (`status`: queued → processing → done | failed), so it survives restarts and crashes with no extra storage. At startup, items left in `processing` go back to `queued`.
- **One worker thread at below-normal priority.** It wakes instantly on save, Retry, and resume (from Settings or the tray), with a 30 s heartbeat as a safety net. Pausing lets the current item finish, then stops.
- **Attempts:** an item gets 3 automatic attempts, then shows as failed with a friendly reason and a Retry action. An attempt interrupted by a crash counts too, so a screenshot that crashes the app every time can't crash-loop it. Waiting for the AI never uses up attempts.
- **OCR preparation** (from spike 3, now measured end to end): invert dark images to dark-on-light, then upscale 2× when the capture is ≤1200 px tall. Results on the spike samples: light **3.9% → 0.4%** CER, dark **0.0%** (raw upscaling had made it 12.5%), small **1.0% → 0.0%**, at 33–63 ms each.
- **Smart fallback:** fewer than 15 letters/digits (any script; punctuation and icon glyphs don't count) switches a Text-mode item to Image mode and sets `auto_upgraded`, so the card says "Auto-upgraded to Image".
- **No OCR language pack:** OCR is skipped (empty text) and the item continues on the note and image, as the spec asks. The app shows a notice explaining how to add one.
- **Until the AI exists (Phase 5)**, items stop after OCR and wait, labeled "Waiting for the AI model…" on the card and "N screenshots waiting for the AI model" in the header and tray tooltip.

## Phase 5: local AI

- **Setup is explicit.** Nothing downloads until the user clicks "Set up AI" (onboarding step 3, or Settings › AI). Consent is remembered, so an interrupted download resumes by itself at the next start. Settings › About and the setup text say this is the only network use.
- **Downloads** (`ai/download.rs`): HTTP Range resume from a `.part` file, 5 retries with exponential backoff, a free-space check (download plus a 500 MB margin), SHA-256 verification against `models.json`, then a `.verified` marker so 1.3 GB isn't re-hashed on every start. Pause keeps the partial file.
  - ureq's body timeout is a whole-body budget, not a per-read timeout, so it's scaled to the file size (≥50 KB/s assumed). A stalled connection eventually times out and the retry resumes.
- **Engine distribution:** the official llama.cpp `win-cpu-x64` zip (19.5 MB, pinned build `b11669`) is downloaded with the model and extracted to `ai/engine/<id>/`. A missing exe (e.g. quarantined by antivirus) produces a message that says so and offers a re-download.
- **Engine lifecycle** (`ai/engine.rs`): started only when the queue has AI work, on a free localhost port, with `--threads` set to half the logical cores (2–8), `--ctx-size 4096`, CPU only, one slot, no web UI. It runs at **below-normal priority with no console window** and is assigned to a **kill-on-close Job Object**, so Windows kills it if the app exits for any reason, including a crash. It's health-checked on start, and restarted if it dies; 3 deaths within 10 minutes stop restarts with an antivirus hint. It stops after **2 idle minutes** to free RAM. Its stderr goes to `ai/llama-server.log`, and the last line is shown in errors.
- **Prompting** (`prompts/categorize.v1.*`): versioned system prompt, JSON schema, and two few-shot examples (the "note outranks text" case and the "receipt → Miscellaneous" case that every model missed in the spike). The schema's `category` is an **enum of the user's own category names**, so the model can't invent one. Today's date (with weekday), the UTC offset and the India DD/MM rule are injected. Long OCR text keeps the first 60% and last 40% of 4000 characters, with an explicit "middle was cut" marker. Qwen3.5's thinking mode is disabled per model in `models.json`.
- **Validation:** the answer is parsed and normalized (title/summary trimmed and length-capped, tags lowercased, deduplicated and capped at 4, category matched case-insensitively, falling back to Miscellaneous). One retry with a stricter prompt at temperature 0, then the item fails with a reason.
- **Links aren't trusted to the model:** URLs are extracted deterministically from the note and OCR text, and a model URL is kept only if it appears verbatim in that text.
- **A user's choice wins:** a category picked in the popup, or a manual move, is never overridden by the AI.
- **Raw AI output is stored** (`ai_json`, plus `ai_model`), so Phase 7 can derive deadlines, and future features can re-use results without re-running the model.
- **Re-check Miscellaneous** (offered after adding a category): re-files unlocked Miscellaneous items, moving one **only if the AI now picks a real category**, and never touching its existing title and summary.
- **Model choice in Settings:** "Standard" (Qwen3.5-2B, 8 GB laptops) or "More accurate" (Qwen3.5-4B, 16 GB+). Switching stops the engine and downloads the other model if AI is set up.
- **Image mode** items wait until the image model (vision projector) is installed; that flow is Phase 6.
- **Measured end to end in the app** (this laptop, release-equivalent settings, debug build of the app): engine zip and the 1.28 GB model downloaded and SHA-256-verified; engine start **2.8 s**; first filing **18.9 s** (cold prompt cache), then **6–8 s** per screenshot.

## Phase 6: image mode

- **The image model is a separate, opt-in download** (the 670 MB vision projector for the same Qwen3.5 model), offered only when it's needed:
  - in the popup, when the user picks **Image** and it isn't installed (a one-line "Download" in the footer);
  - on the home screen, when screenshots were **auto-upgraded** (too little text) and are waiting for it;
  - in Settings › AI ("Add image understanding").
- **That consent is remembered too**, so an interrupted image download resumes at the next start. Found in testing: a restart mid-download had left it stuck at 620/670 MB. After the fix it resumed via HTTP Range and verified.
- Image-mode items wait (labeled "Waiting for the image model…", counted in the badge and tray tooltip) until the add-on is installed; text-mode items are never held up by them.
- **The engine restarts with the projector loaded** only when an image item comes up, and stays that way until it goes idle.
- Images are scaled to at most 1280 px on the long side before going to the model, since more pixels mean many more image tokens and much slower answers.
- **Measured:** engine with vision starts in 3.8 s; the palette test image was filed in **48 s** (budget 3 min). Title "Brand color palette", correct colors in the summary, filed into Ideas following the note.

## Phase 7: deadlines + search

- **Importance** = the model's `note_has_action_intent` **or** a keyword in the note. The list is in **`config/intent-keywords.txt`** (one entry per line, whole-word, case-insensitive; rebuild after editing). Deadlines without intent are stored (`deadline_important = 0`) but never shown or reminded.
- **Dates are resolved in Rust** (`deadlines/dates.rs`): the text the model quoted is parsed first with fixed rules (DD/MM[/YY[YY]], ISO, "12 Nov", "Nov 12th, 2026", "1st of Sept"); the model's ISO date is only a fallback and must be a real date. No year → next occurrence on or after the capture date; time 23:59 local; >30 days past or >3 years ahead → ignored; a few days overdue is kept and shown as overdue. If the model misses a date that's only in the note ("remind me 5 Jan") and intent is clear, the note itself is parsed.
- **Found in real testing:** Windows OCR (on the 2× upscaled image) read "12/11/2026" as "1 2/11/2026", so the deadline became 2 Nov. Fixed at the source: an OCR clean-up step rejoins a lone digit split from a following date or number, and the date parser applies the same repair. Re-run: **12 Nov 2026** ✔. Regression tests added.
- **Also found:** the model wrote "Rs 50,000" for "Rs 5,00,000". The prompt now tells it to copy amounts exactly and explains Indian grouping.
- **Hand-set deadlines** ("Set deadline…" on any card) are always important and never overwritten by the AI. Clearing one by hand also stops the AI from re-adding it.
- **Strip:** important, not-done deadlines from 7 days overdue onwards, soonest first, with "overdue / today / tomorrow / in N days" and a **Done** button. Clicking opens its category.
- **Reminders:** a scheduler checks at start-up and hourly. A reminder fires on the day `lead` hours before the deadline at the chosen hour (default 1 day before, 9:00); a reminder missed while the app was off fires on the next check, as long as the deadline hasn't passed. Each fires exactly once (`notified_at`).
- **Search:** FTS5 over title, summary, note, OCR text and tags. Every word must match, each as a prefix; input is tokenized and quoted, so FTS syntax can't be injected. Ranked by BM25 with weights title 6, note 5, summary 3, tags 3, OCR 1. **10,000 items: 3.1 ms (prefix) / 1.8 ms (two words)**, release build. Results reuse the card grid, labeled with their category. Ctrl+K jumps to search from any screen; Esc clears.

## Phase 8: polish and packaging

- **Database safety:** a daily snapshot (`backups/library-YYYY-MM-DD.db`, via `VACUUM INTO`, newest 3 kept). On startup the library is integrity-checked (`PRAGMA quick_check`). If it's damaged, the broken file is **kept** as `library.db.damaged-<time>`, the newest healthy backup is restored, and the home screen explains what happened. With no usable backup, a fresh library starts; the images are never touched. Tested with a deliberately corrupted file.
- **Export all data:** a zip in Downloads (revealed in Explorer) with every original PNG under `images/<category>/`, an `index.json` for tools, and a readable `index.md`. PNGs are stored uncompressed since they're already compressed.
- **Battery saver:** AI work pauses while Windows battery saver is on (setting, default on). The badge and tray say why; the 30 s heartbeat resumes work when it turns off.
- **Disk full:** a failed save keeps the capture pending in the popup with the error shown (retry or Esc); downloads check free space first, leaving a 500 MB margin.
- **Release pipeline:** `.github/workflows/release.yml` (typecheck, clippy with warnings as errors, tests, installer, SHA-256 checksums, draft release on tags) with a ready slot for SignPath signing. Details in `RELEASE.md`.

### Final measurements (this laptop: Core Ultra 7 155U, integrated graphics, Windows 11)

| Budget (spec) | Target | Measured |
|---|---|---|
| Hotkey → overlay | < 300 ms | **52–74 ms** (release, app in tray) |
| Save on Enter | < 200 ms | **3–6 ms** |
| "Saved" confirmation | < 800 ms | 550 ms |
| Idle RAM, model unloaded | < 80 MB | **11 MB private / 38 MB working set** (1 process) |
| Installer | < ~30 MB | **4.8 MB** |
| Text-mode filing | < ~45 s | **6–8 s** warm, 19 s cold (Qwen3.5-2B, 4 threads) |
| Image-mode filing | < ~3 min | **48 s** |
| AI peak RAM, text / vision | < 3.5 / 5 GB | **3.0 GB** working set for both (spike 4–5) |
| Search, 10 000 items | fast | **3 ms** |

⚠ Not measured on a real 8 GB machine (this one has 32 GB). The AI was limited to 4 threads to approximate one, and RAM was measured as peak working set. Worth a final check on the target hardware.

## Design system (after v1): Airtable-style editorial

Adopted the supplied Airtable DESIGN.md, adapted for a desktop app:
- **Tokens** in `src/index.css`: white canvas, `#181d26` ink and primary, `#333840` body, `#dddddd` hairlines, link `#1b61c9`, focus `#458fff`, and the eight signature surfaces (coral, forest, navy, cream, peach, mint, yellow, mustard). Radii: 12px actions and big surfaces, 10px cards, 6px inputs, circles for icon buttons. Pill shapes aren't used (the doc reserves them for pricing).
- **Type:** Haas Grotesk is licensed and can't be shipped, so Inter (variable, display optical size for headings) is bundled offline, as the doc recommends. Display headings are weight 400 and everything else is 400/500; nothing is bold.
- **Color lives in surfaces:** empty category tiles are full signature blocks; Upcoming deadlines is a cream callout band; onboarding uses a forest signature card; urgent due chips are coral and others cream. Default category colors moved to the signature palette (migration 0005, only for colors the user never changed).
- **Buttons:** near-black primary and white hairline secondary, with pressed states and keyboard focus rings but no hover styling (the doc's no-hover rule). Card action menus still *appear* on hover or focus, since that's behavior, not styling.
- **Dark mode** (original requirement: follow the OS) is derived from the doc's own dark surfaces (#181d26 / #1d1f25).
- The native capture overlay's size label uses the same ink color.

## Multi-category screenshots and the home feed (2026-10-16)

User-requested change: screenshots can belong to several categories, categories are easy to create, and home is one filterable feed instead of category folders.

**Decisions (agreed with the user):** several categories per card (AI picks 1–3); selected filters must **all** match; the whole screenshot is visible in its frame (no cropping); text search stays **separate** (header) from the category/tag filter search.

- **Data:** migration 0006 adds `screenshot_categories` (many-to-many) and an indexed `screenshot_tags` mirror of `tags_json`, kept in sync by triggers. `screenshots.category_id` stays as the card's **primary** category (the AI's best match, shown first). Keeping it avoids rebuilding the table and its FTS triggers. Existing libraries upgrade in place (tested, and verified on the real library).
- **Rules** (one function, `set_categories`): duplicates dropped; no category means Miscellaneous; Miscellaneous never sits beside a real category; the first is primary; a hand-made choice is **locked** and the AI won't change it.
- **AI:** prompt **v2** returns `categories` (1–3, best first), with the schema restricting items to the user's category names. A third few-shot example teaches the project use case (a font saved for "Portfolio Redesign" goes to that project category **and** Resources). Validation mirrors the DB rules. Verified with the real model: the hackathon screenshot got `["Opportunities & Deadlines", "Learning"]` in 28 s cold. The 2B model sometimes adds a weak second category; hand edits fix it permanently.
- **Creating categories:** a "+ New category" pill in the filter bar (name, color from the signature palette, optional hint for the AI); "Create “…”" while editing a card's categories; and the same inside the capture popup's category picker. Settings › Categories remains for rename, recolor, reorder and delete.
- **Re-check after creating a category:** a toast offers "Check your existing screenshots for it?". This re-runs the AI on finished, unlocked screenshots and **only adds** categories, never removing anything or touching titles and summaries. It runs in the background at roughly 6–30 s per screenshot.
- **Deleting a category** removes it from its cards; cards left with none go to Miscellaneous; hand-locked cards that keep another category stay locked.
- **Feed:** every screenshot newest first, 4 per row at ≥1024 px (3/2/1 below), whole image shown with `object-contain` on a soft surface. Loads 60 at a time as you scroll. The deadline band shows only when nothing is filtered. Clicking a deadline opens the screenshot enlarged (there are no category pages to jump to anymore).
- **Filter bar** (sticky): "All", every category with counts, then the 8 most-used tags (plus any selected tag outside the top 8), then "+ New category", with the pills wrapping onto further lines rather than hiding behind a scrollbar. At the right, **Filter** opens a searchable multi-select over every category and tag, with counts and "Clear all". Pills are fully rounded because the user asked for pills; the design doc otherwise reserves pill shapes for pricing.
- **Performance at 10,000 screenshots** (release): all 0.7 ms; 2 categories 8.5 ms; category + 2 tags + text 16 ms.
- **Popup:** the category dropdown became a multi-select picker with create; Esc inside the open picker closes the picker, never the capture.
- Removed: category pages, the home tile grid, the old search results view, and the `list_cards`, `search`, `move_card` and `recheck_misc` commands (replaced by `feed`, `filter_options`, `set_card_categories` and `recheck_categories`).

## Titles that weigh the note and the screenshot (2026-10-16)

Problem (found by the user): titles were based on the screenshot even when the note said what it was for. In the real library, "setup hermes again with new code" was titled "Hermes app icon screenshot". The old prompt applied "the note matters most" only to categories.

**What changed (prompt v3):**
- **`note_purpose` comes first in the answer** (the JSON schema puts it before `title`, and grammar-constrained output follows that order). The model has to state, in a few words, what the note says the person wants, *then* write the title from it. Wording changes alone barely moved the 2B model; this ordering did.
- **The note is now the last thing in the message**, labeled "The person's note (most important)". Previously it sat before a long block of OCR text, and small models weigh what they read last most.
- Rules: the title starts from the purpose when there is one and names the subject from the screenshot; an empty or aside note ("test", "just checking") means the screenshot leads; never "Screenshot of…". The summary says why it was saved when the note says so.
- Two new few-shot examples: a purpose that must lead (the Hermes case) and an aside note where the screenshot leads.
- Image mode no longer tells the model to just "use what it shows"; the note still decides the purpose.

**Results on the real library (Qwen3.5-2B):** Hermes → "Set up Hermes again with the new code"; "remember deadline to register is 5 dec" → "Register for India Case Study 2027"; aside notes correctly titled from the screenshot. On two *unseen* combinations: "use these colors for the mobile app redesign" → "Color palette for mobile app redesign" ✅; "share this with my team" → purpose only in the summary ⚠. Better, but the small model is still not perfect at this; the "More accurate" (4B) model should do better.

**Also:**
- **Hand-edited titles and summaries are now kept** (`text_locked`, migration 0007). Previously a Retry would have overwritten them.
- **Settings › AI › "Re-file all with the latest AI"** re-runs the AI on every screenshot (keeping hand-picked categories and edited text), so existing cards benefit from prompt improvements.
