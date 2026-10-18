# Security policy

## Supported versions

Security fixes go into the latest release. Please update to it before reporting.

## Reporting a vulnerability

Please report security problems privately, not in a public issue:

1. Preferred: use GitHub's [private vulnerability reporting](https://github.com/reportedguy/clipnest/security/advisories/new) (Security tab › "Report a vulnerability").
2. Or email **reportjatin@gmail.com** with "ClipNest security" in the subject.

Include what you found, how to reproduce it, the ClipNest and Windows versions, and the impact you expect. You'll get a reply within 7 days. Once a fix is released, you'll be credited in the release notes unless you'd rather not be.

## Scope

Especially relevant for ClipNest:

- Anything that sends screenshots, notes or other library data off the computer.
- Bypassing the SHA-256 verification of the AI engine and model downloads (`models.json`, `src-tauri/src/ai/download.rs`).
- Reading files outside the library through the app's media protocol (`src-tauri/src/media.rs`).
- Code execution through crafted screenshots, notes, links or AI output.

Out of scope: the SmartScreen warning on unsigned installers (known, see the README), and issues that need someone already running code as your Windows user.
