# DX2: multi-format clipboard, open and reveal files from JS

Spec: `plans/research/QT-SURVEY.md` §2.7 (QA10, clipboard and drag data) and §2.8 (QD6, `svelters:shell`),
lane row 6 of §3. STATUS row DX2. Branch `ci/dx2`.

## As built

| Piece | Where | What |
| --- | --- | --- |
| `MimeData` | `svelters-host/src/platform.rs` | Typed data: ordered `(type, bytes)` entries, one per type. `single(ty, data)` is one WG4b drag (`drag-type`, `drag-data`); `filter(accept)`, `best(accept)` (first matching pattern wins), `get_str`, `plain_text`. |
| `type_matches`, `is_text_type` | `platform.rs` | Patterns follow WG4b's `drop-types`: `*`, an exact type, `prefix/*`. Text types (`text/*`, JSON, `+json`, XML, `+xml`, JavaScript) reach JS as strings, the rest as `Uint8Array`s. |
| `ClipboardBackend`, `ClipboardKind` | `platform.rs` | `read(kind, accept)`, `write(kind, data)`, `formats(kind)`. `Send + Sync`, shared (`Arc`): every host of a process, its Rust panels and a code editor use one clipboard. `Primary` is the Linux primary selection. |
| `MemoryClipboard` | `platform.rs` | In process, both kinds; `contents(kind)` and `writes()` make it the test sink. The default. |
| `ShellBackend`; `NoShell`, `RecordingShell`, `SystemShell` | `platform.rs` | `open_url`, `open_path`, `reveal`. Paths are absolute and must exist (`ShellError::NotFound`). `NoShell` (default) fails every call; `RecordingShell` records `ShellCall`s and starts nothing; `SystemShell` uses the `open` crate (URLs and files, WSL aware) and, for reveal, `org.freedesktop.FileManager1.ShowItems` via `dbus-send` on a thread (falls back to opening the parent directory), `open -R` on macOS, `explorer /select,` on Windows. |
| `Platform`, `HostConfig::platform`, `Host::platform()` | `platform.rs`, `core.rs`, `lib.rs` | The clipboard and shell of a host. Default `Platform::headless()` (memory clipboard, no shell), so tests and headless hosts never touch the system. `Host::platform()` is the host-side entry for EV3's paste command and Rust panels. |
| `__svelters_host` ops | `svelters-host/src/quickjs/platform.rs` | `clipboardWrite(kind, [[type, string or Uint8Array]...])`, `clipboardRead(kind, accept[])`, `clipboardFormats(kind)`, `shellOpen("url" or "path" or "reveal", target)`. Synchronous; failures throw `Error` with the backend's message. |
| `svelters:clipboard` | `packages/native-runtime/src/clipboard.js`, `types/clipboard.d.ts` | `write({type: data})` (strings, byte buffers, else JSON), `read(accept)` returns `{type, data, text(), json()}` or null, `readAll`, `formats`, `writeText`, `readText`; `{kind: 'primary'}`. Every call returns a Promise (a later async system clipboard keeps the API). Also `@svelters/native-runtime/clipboard`. |
| `svelters:shell` | `src/shell.js`, `types/shell.d.ts` | `openUrl`, `openPath`, `revealInFolder` returning Promises. Also `@svelters/native-runtime/shell`. |
| Aliases | `packages/native-runtime/package.json` | `svelters:clipboard`, `svelters:shell` in the runtime's alias table, so Vite and the Rust source loader resolve them the same way (as `svelters:embed`). Ambient typings: `"types": ["@svelters/native-runtime/platform"]`. |
| `SystemClipboard`, `platform::system()` | `svelters-egui/src/platform.rs` (feature `eframe`, `arboard`) | arboard, the library egui-winit copies and pastes with, so a `text-edit` and `svelters:clipboard` see one clipboard. The system gets `text/plain` (and `text/html` on the clipboard); other types stay in process next to the text they were written with and come back while the system text is unchanged. Data without `text/plain` puts its first text entry on the system. `run_app` (when its config is still headless) and `svelters-egui-run` install `system()`; an embedder sets `HostConfig::platform` itself. |

## Tests

- `svelters-host` `platform::tests` (4): `MimeData` order and patterns, text types, memory clipboard kinds, shell argument checks and `file_uri`.
- `svelters-egui` `tests/clipboard.rs` (3, kittest, fixture `fixtures/counter-egui/src/Clipboard.svelte`): a table copied as TSV, HTML and an app JSON type lands in the memory sink in order; its TSV pasted into a multiline `text-edit` gives 2 rows; a typed read prefers the app type; Rust-written data reads in JS by preference; reveal and open reach the `RecordingShell`, and a missing file rejects.
- `packages/native-runtime/test/platform.test.mjs` (3): encoding, preference order, primary selection, rejections, shell ops.

## DragPayload (WG4b, `ci/int-widgets`)

Written against master; `DragPayload` (`svelters-egui/src/kinds/dnd.rs` on `ci/int-widgets`) is `{ ty, data: String, source, dropped }`. `MimeData` is shaped so it carries one: `MimeData::single(p.ty, p.data)`, and the clipboard's `accept` patterns are its `drop-types` rules (`type_matches` is `dnd::accepts` per pattern). Unification once INT1 is on master: `DragPayload` holds `data: MimeData` (keeping `new(ty, data)`, `ty()` and `data()` as the first entry), `dnd::accepts` calls `platform::type_matches`, and a drop's `drop(type, data, …)` signal reports the best entry for the target's `drop-types`.

## Not done

- No clipboard change signal (`dataChanged`); a reader polls `formats()`.
- Images: `image/png` stays in process (arboard takes RGBA, not PNG); other apps do not see it.
- HTML read-back from other apps depends on arboard's platform support; text always works.
- `requestAttention` (QD6's "attention") is not in this lane: it is a viewport command, not a shell call.
- In kittest, egui's own copy (`ctx.copy_text` from a `text-edit`) does not reach the memory clipboard, and a paste is an injected `Event::Paste`; EV3's paste command reads `Host::platform().clipboard`.
