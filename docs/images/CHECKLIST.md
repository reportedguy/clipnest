# README screenshots

The README uses these files. They were taken from a demo library of fictional pages, filed by the real OCR and AI. To retake them, capture from a library with realistic but non-personal content (a separate data folder works: `pnpm tauri dev --config '{"identifier":"com.clipnest.demo"}'`). Use a window about 1400×900, PNG, and crop to the window. Check nothing private is visible (names, emails, chats, the taskbar).

| File | What it should show |
| --- | --- |
| `home-light.png` | Home screen in light theme: title bar, the upcoming-deadlines strip with one or two items, two rows of category pills, the tag row, and a grid of 6–8 filed cards. |
| `home-dark.png` | The same view in dark theme. |
| `capture-overlay.png` | The capture overlay mid-drag over a web page: dimmed screen, white dashed selection, the white and blue "+" corners. |
| `note-popup.png` | The note popup right after a capture, with a typed note such as "apply by Friday" and the Text/Image switch visible. |
| `card-expanded.png` | Two cards opened: title, summary, due chip and tags. |
| `onboarding-ai.png` | Onboarding's "Set up the AI" step: detected memory, both model choices, the "Recommended for your computer" badge. |
| `settings.png` | Settings, showing the Categories section. |
| `social-preview.png` (optional) | 1280×640 image for GitHub's social preview: logo, "ClipNest", tagline, one screenshot. Upload it under the repository's Settings › General › Social preview. |
