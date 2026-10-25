# PRESTO-win32: presto's own Win32 platform backend (presto batch 3, lane WIN32)

Date: 2026-10-25. Branch `ci/presto-win32` (from `ci/presto-base` at efe20dd5). Crate:
`crates/presto-platform-win32` (empty on other OSes: `#![cfg(target_os = "windows")]` at the root).

## What landed

| Module | What | Tests (written) |
|---|---|---|
| `shim` | presto-platform's contract as the b1 brief gives it (`AppHandler`, `PlatformCx`, `run`), plus this lane's proposal for the data types the brief names but does not spell out, and additions (below) | — |
| `app` | the event loop (OLE STA, `GetMessage`), the handler taken out while it runs with a re-entrancy queue, the message-only window for wakes, the high-resolution waitable-timer thread for `set_timer`, the modal-loop policy, deferred native modals, the TSF keystroke filter before `TranslateMessage`, `PlatformCx` | — |
| `window` | class and window procedure; system/custom/no frames (custom: caption dropped in `WM_NCCALCSIZE`, top resize band and app regions in `WM_NCHITTEST`, caption buttons get pointer events and act on release, `HTMAXBUTTON` for snap layouts); per-monitor DPI v2 (`WM_GETDPISCALEDSIZE` keeps the size in points, `WM_DPICHANGED` reports the scale before the resize); fullscreen; dark title bar and `ThemeChanged`; cursors; popups/tooltips that never activate; `WM_PAINT` validates only | styles, outer sizes, DPI scaling, `WM_SIZE` → events, caption buttons |
| `pointer` | `WM_POINTER*` for mouse (`EnableMouseInPointer`), pen and touch: QPC timestamps, `GetPointerInfoHistory`/`GetPointerPenInfoHistory` history (oldest first), pressure, tilt, button changes while another button is held, cancel; wheel from `WM_POINTERWHEEL` and the legacy wheel messages | message translation, history order, pen, wheel signs |
| `keyboard` | scancode → USB HID usage (page 7), sided modifiers and locations, layout character via `ToUnicodeEx` flag 4 (dead-key state untouched), text from the queued `WM_CHAR`s (dead keys compose through the OS), AltGr kept apart from Ctrl+Alt (synthetic left Ctrl dropped), egui key names | key names, layout mapping (AZERTY, Cyrillic fallback), HID table, AltGr, text filtering |
| `tsf` | `ITextStoreACP` + `ITfContextOwnerCompositionSink` over `TextInputClient` (UTF-16): the lock protocol with queued/refused requests while the handler runs, TIP edits as `replace`/`replace_and_mark`, the marked range settled to the composition's exact extent after each write lock, unmark on composition end, app edits reported through `sync()` (`OnTextChange`/`OnSelectionChange`), `OnLayoutChange` when the field moves, `GetTextExt` with the caret-area fallback for TIPs that mishandle `TS_E_NOLAYOUT`, `GetACPFromPoint`, input scopes as an `ITfInputScope` attribute, app-side commit/cancel | range resolution, text changes, extents, relative selections, lock protocol, input scopes |
| `imm` | `WM_IME_REQUEST`: `IMR_RECONVERTSTRING` (ATOK/Mozc reverse conversion), `IMR_DOCUMENTFEED`, `IMR_QUERYCHARPOSITION` | `RECONVERTSTRING` layout, context windows |
| `dmanip` | port of GPUI's `direct_manipulation.rs` (Apache-2.0, header + `THIRD_PARTY_NOTICES.md`): precision-touchpad pan with inertia (wheel events in pixels with gesture phases) and pinch; updated on each compositor wake and on an 8 ms timer while a gesture or its inertia runs | float comparison |
| `dcomp` | one DirectComposition device per process, per window a target, root visual, main visual (`SurfaceTarget::CompositionVisual`) and child visuals per OS layer (offset, clip, z restacking without relying on null-reference placement); `CommitHandle` (Send) for the compositor after wgpu sets content | stacking order, layer geometry |
| `frame` | the frame source: Windows 11 compositor clock (looked up at run time) with extra handles and boost, the DXGI waitable when given, `DwmFlush`; GPUI's 1 ms early-return rule; interval from `DwmGetCompositionTimingInfo` with the implausible-period fallback | clock results, intervals, timeouts |
| `drag` | OLE drag source (text, `CF_HDROP`) started after the handler returns so dispatch keeps running; per-window drop target → `FileHovered`/`FileDropped`/`FileHoverCancelled` | `DROPFILES` layout, drag results |
| `clipboard`, `cursor`, `monitor`, `hit_test`, `util` | text (CRLF on write) and file lists; CSS cursors; monitors with work areas, DPI, refresh; hit regions; QPC clock and message-time conversion | each |

## What I ran

Locally only `cargo check`/`cargo clippy -p presto-platform-win32 --target x86_64-pc-windows-msvc --all-targets`
(the msvc std is installable in the container; nothing links), clean. **No tests run locally**; CI's presto
workflow runs them on windows-latest. CI: run 40222161859 (first push, green on all three OSes); later runs
listed in the coordinator report.

## Not done / stubbed (no fake successes)

- **AccessKit** (`WM_GETOBJECT`): not in this lane's brief; nothing answers it yet.
- **IMM32 composition fallback** when TSF fails to start: none (TSF exists on every supported Windows). Stray
  `WM_CHAR`s become `ImeEvent::Commit`.
- **Legacy mouse messages**: not handled; mouse input relies on `EnableMouseInPointer` (always succeeds on the
  first call in a process; if the host disabled it first, mouse moves and clicks would not arrive).
- **`create_window` failure** returns an id that every other call treats as unknown (`surface_target` →
  `SurfaceTarget::None`); the brief's signature has no error.
- **`OsLayerDesc::opaque`** is not passed to DComp (no visual property for it).
- **Pointer prediction**, raw input for 3D mouse capture (`DeviceEvents`): not here.
- **Window-level drag position** for drops: presto-base's file events carry no position.

## Additions for PLATFORM (proposed in `shim`)

`AppHandler::text_input(window) -> Option<&mut dyn TextInputClient>` (the IME bridge needs the client
synchronously inside OS callbacks), `AppHandler::drag_finished`; `PlatformCx::{update_os_layer, remove_os_layer,
frame_source, start_drag, set_pointer_capture, set_title, set_visible, set_window_state, end_composition}`;
types `Decorations`, `WindowKind`, `WindowState`, `HitKind`, `FrameWake`, `FrameSource`, `DragData`,
`DragResult`, `Clipboard::get_files`. `SurfaceTarget` here has `Win32` and `CompositionVisual` variants only.

## Questions for the coordinator (each with a recommendation)

1. **`TextInputClient` lacks a document length and a selection setter.** The TSF store reads the length as
   `text_for_range(0..usize::MAX)` (clients must clamp) and keeps a selection TSF set outside a composition as
   an override it reports back. Recommendation: add `fn len_utf16(&self) -> usize` and
   `fn set_selected_range(&mut self, range: Utf16Range)` to presto-base's `TextInputClient` (additive, with
   the editing kinds implementing them); I did not edit presto-base.
2. **`replace_and_mark_text_in_range`'s `new_selected` is assumed relative to the inserted text** (as GPUI and
   `NSTextInputClient` define it). Recommendation: say so in presto-base's doc comment.
3. **Who commits DComp after wgpu configures a surface?** This crate offers `dcomp::CommitHandle` (Send);
   nothing in the contract hands it to the compositor yet. Recommendation: PLATFORM adds a Windows-only
   accessor (or a generic `SurfaceTarget` "commit hook"), the consolidation wires it into presto-compositor.
4. **The DXGI waitable** comes from wgpu-hal's swap chain on the compositor thread;
   `Win32FrameSource::set_waitable` takes it. Same wiring question as 3.

## Checks that need Windows hardware (REVISIT)

`DCompositionWaitForCompositorClock`'s result encoding (`WAIT_OBJECT_0 + count` = tick, from the docs);
custom-frame shadow/border on Windows 10 vs 11 without `DwmExtendFrameIntoClientArea`; snap layouts over the
app-drawn maximise button; `SetCapture` routing of mouse pointer messages during tab tear-off; TSF with MS-IME
Japanese, Chinese (Pinyin), Korean, ATOK/Mozc reconversion; dead keys on US-International and AltGr on
German/Polish layouts; DirectManipulation inertia smoothness at 8 ms timer granularity; resize cleanliness
from every edge with the compositor handshake.

## Cross-lane edits

`THIRD_PARTY_NOTICES.md`: an "Apache-2.0 ported source (presto)" section with the GPUI row (other lanes porting
GPUI files add rows there). `Cargo.lock`: the `windows`/`windows-core` 0.62 entries for this crate.
