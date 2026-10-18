# Third-party notices

ClipNest is MIT-licensed (see [LICENSE](LICENSE)). It builds on the work below. Each item keeps its own license.

## Downloaded at runtime (not in the installer)

ClipNest downloads these only after the user agrees, from the pinned URLs in [`models.json`](models.json). Every file is checked against a fixed SHA-256 hash before use.

| Component | Used for | License | Source |
| --- | --- | --- | --- |
| llama.cpp `llama-server`, release b11669, Windows CPU x64 build | Runs the local AI model | MIT | [ggml-org/llama.cpp](https://github.com/ggml-org/llama.cpp) |
| Qwen3.5 2B, Q4_K_M GGUF | "Standard" text model | Apache-2.0 | [Qwen](https://huggingface.co/Qwen), quantized by [unsloth/Qwen3.5-2B-GGUF](https://huggingface.co/unsloth/Qwen3.5-2B-GGUF) |
| Qwen3.5 2B vision projector, F16 | Optional image understanding | Apache-2.0 | [unsloth/Qwen3.5-2B-GGUF](https://huggingface.co/unsloth/Qwen3.5-2B-GGUF) |
| Qwen3.5 4B, Q4_K_M GGUF | "More accurate" text model | Apache-2.0 | [unsloth/Qwen3.5-4B-GGUF](https://huggingface.co/unsloth/Qwen3.5-4B-GGUF) |
| Qwen3.5 4B vision projector, F16 | Optional image understanding | Apache-2.0 | [unsloth/Qwen3.5-4B-GGUF](https://huggingface.co/unsloth/Qwen3.5-4B-GGUF) |
| Microsoft Edge WebView2 Runtime | Renders the app's window (installed by the setup program on Windows 10 if missing; built into Windows 11) | Microsoft software license | [Microsoft](https://developer.microsoft.com/microsoft-edge/webview2/) |

## Bundled in the app

| Component | Used for | License | Source |
| --- | --- | --- | --- |
| Inter (variable) | Interface font | SIL Open Font License 1.1 | [rsms/inter](https://github.com/rsms/inter), via [Fontsource](https://fontsource.org/fonts/inter) |
| Bricolage Grotesque (variable) | "ClipNest" wordmark | SIL Open Font License 1.1 | [ateliertriay/bricolage](https://github.com/ateliertriay/bricolage), via [Fontsource](https://fontsource.org/fonts/bricolage-grotesque) |
| Lucide | Icons | ISC | [lucide-icons/lucide](https://github.com/lucide-icons/lucide) |
| Radix UI | Accessible menus, dialogs, popovers | MIT | [radix-ui/primitives](https://github.com/radix-ui/primitives) |
| React | UI framework | MIT | [facebook/react](https://github.com/facebook/react) |
| Tauri | App framework | MIT or Apache-2.0 | [tauri-apps/tauri](https://github.com/tauri-apps/tauri) |
| SQLite (via rusqlite, bundled) | Local database and full-text search | Public domain | [sqlite.org](https://sqlite.org/copyright.html) |

## Provided by Windows

Text recognition uses the built-in Windows OCR engine (`Windows.Media.Ocr`); nothing is shipped or downloaded for it.

## Everything else

The full dependency lists are in [`pnpm-lock.yaml`](pnpm-lock.yaml) and [`src-tauri/Cargo.lock`](src-tauri/Cargo.lock). They are almost all MIT, Apache-2.0, BSD, ISC or Zlib. A few Rust crates that Tauri uses for CSS parsing (`cssparser`, `selectors`, `dtoa-short`, `option-ext`) are MPL-2.0 and are used unmodified; `webpki-roots` is CDLA-Permissive-2.0. To list them yourself:

```bash
pnpm licenses list
cargo metadata --format-version 1 | jq -r '.packages[] | "\(.name) \(.license)"'
```
