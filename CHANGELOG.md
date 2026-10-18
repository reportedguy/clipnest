# Changelog

All notable changes to ClipNest are listed here. The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and the project uses [Semantic Versioning](https://semver.org/spec/v2.0.0.html). Versions before 0.3.1 were shared privately, and those before 0.3.0 under the working name "Reported Snipping". Only 0.3.1 and later are published on GitHub.

## [Unreleased]

## [0.3.1] - 2026-10-19

First public release.

### Added
- MIT license, third-party notices, contributor guide, code of conduct, security policy, issue and pull-request templates.
- CI on every push and pull request (lint, formatting, types, unit tests, UI build, rustfmt, clippy, Rust tests) and a release workflow that builds the installer for version tags.
- Prettier, oxlint, rustfmt and EditorConfig configurations; UI unit tests with Vitest.

### Changed
- Internal names finished moving to ClipNest: the crate, npm package, log files (`clipnest.YYYY-MM-DD.log`) and the log-level variable (`CLIPNEST_LOG`, was `RS_LOG`).

### Removed
- An unused `get_card` command.

## [0.3.0] - 2026-10-18

### Added
- The app is now called ClipNest. Existing libraries carry over.
- The window draws its own title bar: logo, a menu for Settings and theme, a reminders bell, search and window buttons.
- Onboarding's "Set up the AI" step reads the computer's memory, recommends Standard or More accurate, and unlocks the next step once the model is downloaded.
- The tag row follows the selected categories.
- "Show summary on cards" setting.

### Changed
- Clearer welcome screen; "Set up the AI" now comes before "Try it now".
- Home: reminders above the filters, categories in up to two rows and one row of tags, each with a Filter search for the rest. Fits are measured in one pass, so thousands of tags stay fast.
- Cards show the screenshot, categories and title; a click reveals the summary, tags and original note and links (the recognized-text section was removed). Clicking outside folds the card.
- Redesigned deadline strip that reads in light and dark themes.

## [0.2.1] - 2026-10-18

### Added
- Profession presets in onboarding and Settings, with category and tag capacity checks.

### Changed
- New logo. The capture overlay matches it: white dashed selection with a white and a brand-blue corner mark.
- The filter bar handles many categories; category color dots stay visible in dark mode.

## [0.2.0] - 2026-10-16

### Added
- A screenshot can belong to up to three categories, plus free-form tags.
- Home feed with category and tag filter pills.

### Changed
- Titles weigh the note and the screenshot (prompt v3).
- Capture polish: popup at the top center, send button in the note field.

### Fixed
- Models with the same file name overwrote each other.

## [0.1.0] - 2026-10-16

### Added
- Capture: global shortcut, native frozen-screen overlay on every monitor, note popup, instant save.
- Windows OCR and a persistent background queue.
- Local AI filing with llama.cpp and Qwen models (title, summary, category), plus image mode for screenshots with little text.
- Deadlines with reminders, and full-text search.
- Daily backups with automatic recovery, "Export all data", pause on battery saver, Windows installer.

[Unreleased]: https://github.com/reportedguy/clipnest/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/reportedguy/clipnest/releases/tag/v0.3.1
