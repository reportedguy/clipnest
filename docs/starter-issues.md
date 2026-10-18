# Starter issues

Drafts for the first GitHub issues, taken from the "Help wanted" part of [ROADMAP.md](../ROADMAP.md). Create them on GitHub (Issues › New issue) or with the GitHub CLI, for example:

```bash
gh issue create --title "Use the system locale for dates and times" --label "good first issue,help wanted,enhancement" --body-file issue.md
```

Labels used: `good first issue`, `help wanted`, `enhancement`, `bug`, `accessibility`, `localization`, `performance`, `testing`, `docs`, `packaging`.

---

## 1. Use the system locale for dates and times

**Labels:** good first issue, help wanted, localization

Dates and times are formatted with a hard-coded `en-IN` locale: `src/lib/format.ts` (`timeAgo`, `dueLabel`) and the reminder hour list in `src/views/SettingsView.tsx`. Use the user's locale instead (`undefined` as the locale argument, or `navigator.language`).

**Done when:** a Windows set to English (US) shows "Oct 14, 2026"-style dates; `src/lib/format.test.ts` still passes (its "beyond two weeks" test only checks the year).

## 2. Keep an expanded card open when using its "…" menu

**Labels:** good first issue, help wanted, bug

`ScreenshotCard` folds an expanded card on any pointer-down outside it. The "…" menu renders in a portal outside the card, so choosing a menu item folds the card. Ignore clicks inside Radix menu and dialog portals (for example by checking `[data-radix-popper-content-wrapper]` or the menu content's ref).

**Where:** `src/components/ScreenshotCard.tsx`, the `useEffect` that listens for `pointerdown`.

## 3. Clean up log files from before the rename

**Labels:** good first issue, help wanted

Since 0.3.1 logs are `clipnest.YYYY-MM-DD.log`. Older versions wrote `reported-snipping.YYYY-MM-DD.log`, which the 7-day rotation no longer prunes. On startup, delete `reported-snipping.*.log` files older than 7 days in the logs folder.

**Where:** `src-tauri/src/logging.rs`. Add a unit test using a temp folder.

## 4. Respect "reduce motion"

**Labels:** good first issue, help wanted, accessibility

When Windows animation effects are off, WebView2 reports `prefers-reduced-motion: reduce`. Turn off transitions (card chevrons, progress bars, switches) in that case.

**Where:** `src/index.css` (a `@media (prefers-reduced-motion: reduce)` block), components using `transition-*` classes.

## 5. Expand a focused card with the keyboard

**Labels:** good first issue, help wanted, accessibility

Cards can be focused and moved between with the arrow keys; Enter opens the image. Add a key (Space is a natural fit) that expands and collapses the focused card, and announce the state with `aria-expanded`.

**Where:** `src/components/CardGrid.tsx` (`onKeyDown`), `src/components/ScreenshotCard.tsx` (`expanded` state, which would need to be controllable).

## 6. Add `cargo audit` to CI

**Labels:** good first issue, help wanted, testing

Run [cargo-audit](https://github.com/rustsec/rustsec/tree/main/cargo-audit) on every pull request so known-vulnerable Rust dependencies are caught. Today it reports only informational warnings for Linux-only crates; keep those as warnings.

**Where:** `.github/workflows/ci.yml` (for example with `rustsec/audit-check`).

## 7. Tray item "Open last capture"

**Labels:** good first issue, help wanted, enhancement

Add a tray menu item that opens the main window with the most recent screenshot enlarged.

**Where:** `src-tauri/src/tray.rs` (menu), an event the UI listens for in `src/App.tsx` (which already owns the enlarged-card state).

## 8. A winget manifest

**Labels:** good first issue, help wanted, packaging

Let people install with `winget install ClipNest`. Write the manifest for the latest release and document how to update it per release.

**Where:** new `packaging/winget/`, [docs/releasing.md](releasing.md). See the [winget-pkgs](https://github.com/microsoft/winget-pkgs) contribution guide.

## 9. A short user guide with screenshots

**Labels:** good first issue, help wanted, docs

Write `docs/user-guide.md`: capturing, notes and modes, categories and tags, search, deadlines, export. Use the screenshots listed in `docs/images/CHECKLIST.md`, taken from a demo library with no personal data.

## 10. Re-enable accessibility lint rules

**Labels:** help wanted, accessibility

oxlint's `jsx-a11y` plugin is off because it flagged intentional patterns at once. Turn it back on in `.oxlintrc.json`, then fix real findings or disable individual rules with a written reason, one rule per pull request.

## 11. Component tests for the filter bar and cards

**Labels:** help wanted, testing

Add React Testing Library to the Vitest setup and test: the category row shows at most two rows' worth of pills and puts the rest behind Filter; selected pills always stay visible; a card expands on click and folds on an outside click.

**Where:** `src/components/FilterBar.tsx`, `src/components/ScreenshotCard.tsx`. Layout measurement needs a fake `offsetWidth`; jsdom doesn't lay out.

## 12. Don't reload every card after each change

**Labels:** help wanted, performance

After any library change the feed fetches every loaded card again (`src/views/Feed.tsx`, `load` with `shown.current`). With thousands of cards this gets slow. Options: refetch only the changed card (the event could carry its id), or virtualize the grid.

**Where:** `src/views/Feed.tsx`, `src/components/CardGrid.tsx`, `src-tauri/src/library.rs` (`changed`).

## 13. Per-category export

**Labels:** help wanted, enhancement

"Export all data" writes the whole library. Add "Export this category" (from the category's Filter or Settings › Categories) that writes the same zip format for one category.

**Where:** `src-tauri/src/export.rs`, `src/components/CategoryManager.tsx`.

## 14. Export deadlines as a calendar file

**Labels:** help wanted, enhancement

Write upcoming deadlines to an `.ics` file in Downloads, so they can be imported into any calendar app. No network needed.

**Where:** `src-tauri/src/deadlines/`, `src-tauri/src/export.rs`.
