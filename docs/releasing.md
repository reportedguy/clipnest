# Releasing ClipNest (Windows)

## Build locally

```bash
pnpm install
pnpm tauri build
```

The installer is written to `src-tauri/target/release/bundle/nsis/ClipNest_<version>_x64-setup.exe` (about 5 MB). It installs per user (no admin prompt) and uses the WebView2 runtime that ships with Windows 11; on Windows 10 it downloads WebView2 if missing.

The AI engine and model are **not** in the installer. They're downloaded once, after the user agrees, from the pinned URLs and SHA-256 hashes in `models.json`.

## Releasing a new version

1. Make sure `main` is green in CI and `CHANGELOG.md` lists the changes under "Unreleased".
2. Pick the version ([SemVer](https://semver.org): fixes bump the patch, features the minor).
3. Set it in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json`, and run `cargo update -p clipnest --offline` in `src-tauri/` so `Cargo.lock` follows.
4. In `CHANGELOG.md`, rename "Unreleased" to `## [x.y.z] - YYYY-MM-DD`, add a fresh empty "Unreleased" above it, and update the links at the bottom.
5. Commit (`chore: release vx.y.z`), merge to `main`, then tag and push:

```bash
git tag vx.y.z
git push origin vx.y.z
```

6. The Release workflow builds the installer, writes `SHA256SUMS.txt`, and creates a **draft** release named "ClipNest vx.y.z" with the changelog section as its notes. It fails early if the tag doesn't match the version in `tauri.conf.json`.
7. On GitHub, open Releases, check the draft (notes, the `.exe` and `SHA256SUMS.txt`), download the installer once to test it, then press **Publish release**.

## CI

- `.github/workflows/ci.yml` runs on every push to `main` and every pull request: UI lint, formatting, types, unit tests and build on Linux; rustfmt, clippy (warnings are errors) and Rust tests on Windows.
- `.github/workflows/release.yml` runs on `v*` tags (and by hand from the Actions tab, without creating a release) and does the steps above.
- `.github/dependabot.yml` opens weekly dependency-update pull requests; merge them once CI passes.

## Code signing

### Without signing (works today)

Unsigned installers run fine, but Windows **SmartScreen** shows "Windows protected your PC / Unrecognized app" until the file has built a download reputation. Users click **More info → Run anyway**. The warning fades as more people download the same signed-or-unsigned file over time, but every new version starts over. Publishing `SHA256SUMS.txt` lets careful users verify the download.

### Free signing through SignPath Foundation

[SignPath Foundation](https://signpath.org) provides free code-signing certificates and signing to qualifying open-source projects. As of this writing, the main conditions are:

- the repository is **public** and uses an **OSI-approved open-source license** (this repo has no license yet; adding one, e.g. MIT or Apache-2.0, is the owner's decision);
- releases are built by CI from the public source (the workflow above does this);
- the project applies, is accepted, and follows their policies (e.g. signing only release builds of its own code).

Check signpath.org for the current terms before applying.

Once accepted, enable signing **without code changes**:

1. Add the SignPath GitHub action in the commented "Sign installer" step of `release.yml`, with the organization ID, project and signing-policy slugs SignPath gives you, plus a `SIGNPATH_API_TOKEN` repository secret.
2. Point the later steps (checksums, upload, release) at the signed output.

Tauri can also sign during `tauri build` via `bundle.windows.signCommand` in `tauri.conf.json` if you obtain a certificate another way.

### macOS (later)

Signing and notarization need an Apple Developer account. This is deferred with the Mac port; see ROADMAP.md.
