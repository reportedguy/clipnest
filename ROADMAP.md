# Roadmap

What's planned, what's being considered, and where help is welcome. Items marked **(confirmed)** are the maintainer's plans; everything else is a suggestion open for discussion. Difficulty: easy / medium / hard. Paths point to where the work would start.

Want to pick something up? Comment on its issue (or open one), and see [CONTRIBUTING.md](CONTRIBUTING.md).

## Planned (near-term)

| Item | Area | Difficulty | Where |
| --- | --- | --- | --- |
| **Code signing (confirmed).** Apply to SignPath Foundation and sign release builds, so SmartScreen stops warning. | Packaging | medium | `.github/workflows/release.yml`, [docs/releasing.md](docs/releasing.md) |
| **Hands-on hardware checks (confirmed).** Multi-monitor, mixed-DPI setups, Windows 10, and a real 8 GB laptop. | Testing | easy | [docs/testing.md](docs/testing.md) (items marked ⚠️) |
| **Choose the library location (confirmed).** Let people keep their library on another drive or a synced folder. | Core | medium | `src-tauri/src/lib.rs` (data dir), `src-tauri/src/state.rs`, Settings › Storage |
| **Import existing screenshots (confirmed).** Pick a folder of PNG/JPG files and queue them for OCR and filing. | Core | medium | `src-tauri/src/capture/store.rs`, `src-tauri/src/queue.rs` |
| **More OCR languages in Settings (confirmed).** Today OCR uses the Windows profile languages; let people choose. | Core | medium | `src-tauri/src/platform/windows/ocr.rs`, `src-tauri/src/ocr.rs` |
| **Per-category export (confirmed).** Export one category instead of the whole library. | Core | easy | `src-tauri/src/export.rs` |

## Ideas (longer-term)

| Item | Area | Difficulty | Where |
| --- | --- | --- | --- |
| **macOS (confirmed).** Most of the platform layer is already behind traits: needs a Vision-framework `OcrEngine`, an AppKit `RegionSelector`, screen-recording permission onboarding, notarization and a macOS `llama-server` entry in `models.json`. | Platform | hard | `src-tauri/src/platform/macos/`, `src-tauri/src/platform/mod.rs` |
| **Auto-update (confirmed, design only).** Opt-in update checks; must stay off by default to keep the no-network promise. | Packaging | hard | `src-tauri/tauri.conf.json`, Tauri updater plugin |
| **Mobile companion (undecided).** Read-only browsing of an exported library, without sync. | Platform | hard | `src-tauri/src/export.rs` (export format) |
| Linux support. Capture already uses `xcap`; needs an overlay, an OCR engine (e.g. Tesseract) and autostart. | Platform | hard | `src-tauri/src/platform/` |
| Optional GPU acceleration: offer Vulkan/CUDA `llama-server` builds when a supported GPU is present. | Performance | medium | `models.json`, `src-tauri/src/ai/engine.rs` |
| Duplicate detection: flag near-identical captures with a perceptual hash. | Core | medium | `src-tauri/src/capture/store.rs`, `src-tauri/src/db/` |
| Bulk actions: select several cards to recategorize, tag or delete at once. | UX | medium | `src/components/CardGrid.tsx`, `src-tauri/src/library.rs` |
| Simple markup before saving: arrow, box, highlight. | UX | hard | `src/popup/Popup.tsx`, `src-tauri/src/capture/` |
| Export deadlines as an `.ics` file, so they can be added to any calendar without a network connection. | Integrations | easy | `src-tauri/src/deadlines/`, `src-tauri/src/export.rs` |
| Markdown export per card for note apps (Obsidian, Logseq): image plus a front-matter note. | Integrations | medium | `src-tauri/src/export.rs` |
| Translate the interface; notes and filing already work in any language the model knows. | Localization | medium | `src/` (strings are inline today) |

## Help wanted (good for newcomers)

| Item | Area | Difficulty | Where |
| --- | --- | --- | --- |
| Use the system locale for dates and times instead of the hard-coded `en-IN`. | Localization | easy | `src/lib/format.ts`, `src/views/SettingsView.tsx` |
| Choosing an action from a card's "…" menu also folds an expanded card; keep it open. | UX | easy | `src/components/ScreenshotCard.tsx` (outside-click handler) |
| Clean up old `reported-snipping.*.log` files left by versions before 0.3.1 (the new log prefix isn't pruned with them). | Core | easy | `src-tauri/src/logging.rs` |
| Respect "reduce motion": turn off the card and progress-bar transitions when Windows animations are off. | Accessibility | easy | `src/index.css`, components with `transition-*` |
| Expand and collapse a focused card from the keyboard (today Enter opens the image). | Accessibility | easy | `src/components/CardGrid.tsx`, `ScreenshotCard.tsx` |
| Re-enable the jsx-a11y lint rules and fix what they find, rule by rule. | Accessibility | medium | `.oxlintrc.json`, `src/` |
| Component tests for the filter bar's row fitting and the card expand/collapse. | Testing | medium | `src/components/FilterBar.tsx`, `ScreenshotCard.tsx` (Vitest + Testing Library) |
| Add a `cargo audit` job to CI. | Testing | easy | `.github/workflows/ci.yml` |
| Large libraries: the feed reloads every loaded card after each change; load only what changed, or virtualize the grid. | Performance | medium | `src/views/Feed.tsx`, `src/components/CardGrid.tsx` |
| A short user guide in `docs/` with real screenshots of each workflow. | Docs | easy | `docs/`, `docs/images/CHECKLIST.md` |
| A winget manifest so people can `winget install ClipNest`. | Packaging | easy | new `packaging/winget/` |
| Tray item "Open last capture". | UX | easy | `src-tauri/src/tray.rs` |

Draft issues for the help-wanted items: [docs/starter-issues.md](docs/starter-issues.md).
