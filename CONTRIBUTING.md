# Contributing to ClipNest

Thanks for helping. Bug reports, fixes, docs, tests and features are all welcome. By taking part you agree to the [Code of Conduct](CODE_OF_CONDUCT.md).

## Development setup

You need Windows 10 or 11 (the app is Windows-only for now):

- [Rust](https://rustup.rs), stable MSVC toolchain
- [Visual Studio 2022 Build Tools](https://visualstudio.microsoft.com/visual-cpp-build-tools/), "Desktop development with C++" workload and a Windows SDK
- [Node.js](https://nodejs.org) 22+ and [pnpm](https://pnpm.io) 11

```bash
git clone https://github.com/<your-username>/clipnest.git
cd clipnest
pnpm install
pnpm tauri dev --config '{"identifier":"com.clipnest.dev"}'
```

Keep the clone in a short path (for example `C:\dev\clipnest`): Rust build folders are deep, and Windows' 260-character path limit otherwise breaks the build with `LNK1104: cannot open file`.

The `--config` flag gives your dev build its own data folder, so it never touches a library from an installed copy. The AI model download (1.2 GB or more) is only needed to work on filing; Settings › Developer in dev builds can fill queued screenshots with sample AI results instead.

Project layout and how the parts fit together: [docs/architecture.md](docs/architecture.md). Why things are the way they are, with measurements: [docs/decisions.md](docs/decisions.md).

## Before you open a pull request

Run the same checks as CI:

```bash
pnpm lint
pnpm format:check     # `pnpm format` fixes it
pnpm typecheck
pnpm test
cd src-tauri
cargo fmt --check     # `cargo fmt` fixes it
cargo clippy --all-targets -- -D warnings
cargo test
```

For UI or capture changes, also try the change in the running app, ideally in both light and dark theme. [docs/testing.md](docs/testing.md) lists the manual checks.

## Branches and commits

- Branch from `main`, named by type: `feat/tag-import`, `fix/overlay-dpi`, `docs/faq`.
- Use [Conventional Commits](https://www.conventionalcommits.org): `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`. Keep the subject under about 72 characters; explain *why* in the body when it isn't obvious.
- One logical change per pull request. Smaller is easier to review.

## Pull-request checklist

- [ ] The checks above pass.
- [ ] New logic has a test where practical (Rust: next to the code in `#[cfg(test)]`; UI helpers: `*.test.ts` beside the file).
- [ ] User-visible changes are noted under "Unreleased" in [CHANGELOG.md](CHANGELOG.md).
- [ ] No personal data, local paths or secrets in code, tests or screenshots.
- [ ] Screenshots for UI changes, light and dark if both are affected.

## Code style

- Formatting is automatic (Prettier for the UI, rustfmt for Rust); don't hand-format.
- Comments explain why, not what. Match the surrounding code's naming and comment density.
- Keep ClipNest local-first: no new network calls, telemetry or accounts. Anything that would send data off the computer needs an issue and discussion first.
- The UI follows the design tokens in `src/index.css`. Use the tokens rather than new raw colors.

## Proposing a feature

Open a [feature request](https://github.com/reportedguy/clipnest/issues/new?template=feature_request.yml) describing the problem first, before writing much code. [ROADMAP.md](ROADMAP.md) lists what's planned and where help is wanted.

## Reporting a bug

Open a [bug report](https://github.com/reportedguy/clipnest/issues/new?template=bug_report.yml) with your Windows version, ClipNest version (Settings › About), steps to reproduce, and what you expected. Logs are in `%APPDATA%\com.reportedsnipping.desktop\logs\`; check them for anything private before attaching.

Security problems: please don't open a public issue. See [SECURITY.md](SECURITY.md).

## Good first issues

Issues labelled [`good first issue`](https://github.com/reportedguy/clipnest/labels/good%20first%20issue) are small and well scoped. Comment on one to claim it, and ask questions in the issue if anything is unclear.
