# SKELTER-win32: skelter's own Win32 platform backend (skelter batch 3, lane WIN32)

Date: 2026-10-25. Branch `ci/presto-win32` (from `ci/presto-base` at efe20dd5; merged `ci/presto-base`
e7f68dd2). Crate: `crates/skelter-platform-win32` (empty on other OSes: `#![cfg(target_os = "windows")]`).
It implements `skelter_platform::{AppHandler, PlatformCx}` and exposes `Win32Platform::new().run(handler)` (and
`run(Box<dyn AppHandler>)`) for the `skelter` facade, which selects the backend (coordinator, 2026-10-25); the early
`shim` module (coded against the b1 brief before PLATFORM's skeleton) is gone.

## What landed

| Module | What | Tests (written) |
|---|---|---|
| `app` | the event loop (OLE STA, `GetMessage`); the handler taken out while it runs, with a re-entrancy queue; a message-only window for wakes, AccessKit events and timer expiry; a high-resolution waitable-timer thread for `set_timer`; the modal-loop policy (everything in the window procedure, so resizes, wakes and timers keep flowing through move/size/menu loops); `drag_window` posted so its move loop runs outside the handler; the TSF keystroke filter before `TranslateMessage`; IME events queued during OS calls and flushed after; `quit_requested` from `WM_QUERYENDSESSION`; `exiting`; the `PlatformCx` | — |
| `window` | class and window procedure; OS frames; bare popups without `decorations`; `custom_title_bar` frames: caption dropped in `WM_NCCALCSIZE`, top resize band and app regions in `WM_NCHITTEST`, caption buttons get pointer events and act on release, `HTMAXBUTTON` for snap layouts; per-monitor DPI v2 (`WM_GETDPISCALEDSIZE` keeps the size in points, `WM_DPICHANGED` reports the scale before the resize); min/max sizes; `os_compositing` (DComp tree and no redirection bitmap, else a raw HWND target); dark title bar and `ThemeChanged`; `Moved` as the content's top-left in global points; cursors; `set_mouse_capture` (cursor clipped and hidden, raw input → `raw_mouse_motion`); UI Automation through `accesskit_windows::SubclassingAdapter` (`update_a11y`, `a11y_event`); `WM_PAINT` validates only | styles, outer sizes, DPI scaling, `WM_SIZE` → events, caption buttons, IME on/off |
| `pointer` | `WM_POINTER*` for mouse (`EnableMouseInPointer`), pen and touch: QPC timestamps, `GetPointerInfoHistory`/`GetPointerPenInfoHistory` history (oldest first), pressure, tilt, button changes while another button is held, cancel; wheel from `WM_POINTERWHEEL` and the legacy wheel messages | message translation, history order, pen, wheel signs |
| `keyboard` | scancode → USB HID usage (page 7), sided modifiers and locations, layout character via `ToUnicodeEx` flag 4 (dead-key state untouched), text from the queued `WM_CHAR`s (dead keys compose through the OS), AltGr kept apart from Ctrl+Alt (synthetic left Ctrl dropped), egui key names; lone Alt and F10 (`WM_SYSKEYDOWN`/`UP`) arrive as key events and never start the menu loop (only Alt+F4 and Alt+Space go on to `DefWindowProc`) | key names, layout mapping (AZERTY, Cyrillic fallback), HID table, AltGr, text filtering |
| `shadow` | the focused field as the IME sees it: the app's own `TextInputClient` from `PlatformCx::set_text_input` (TSF and `WM_IME_REQUEST` edit it directly, reconversion and character positions included), or, after `set_ime`, a shadow `TextInputClient` over `ImeState` (area, purpose, composition rects, surrounding text) that turns edits into `Preedit`/`Commit`/`DeleteSurrounding` | composition and commit, unmark, cancel, reconversion, state updates during a composition, per-character bounds |
| `tsf` | `ITextStoreACP` + `ITfContextOwnerCompositionSink` over that field (UTF-16): the lock protocol, TIP edits as `replace`/`replace_and_mark`, the marked range settled to the composition's exact extent after each write lock, unmark on composition end, app-side changes reported through `sync()` (`OnTextChange`/`OnSelectionChange`, checked cheaply), `OnLayoutChange` when the field moves, `GetTextExt` with the caret-area fallback for TIPs that mishandle `TS_E_NOLAYOUT`, input scopes as an `ITfInputScope` attribute, `end_ime_composition` (cancel drops the preedit first) | range resolution, text changes, extents, relative selections, lock protocol, input scopes |
| `imm` | `WM_IME_REQUEST`: `IMR_RECONVERTSTRING` (ATOK/Mozc reverse conversion), `IMR_DOCUMENTFEED`, `IMR_QUERYCHARPOSITION` | `RECONVERTSTRING` layout, context windows |
| `dmanip` | port of GPUI's `direct_manipulation.rs` (Apache-2.0: header, `THIRD_PARTY_NOTICES.md`): precision-touchpad pan with inertia (wheel events in pixels with gesture phases) and pinch; updated on each compositor wake and on an 8 ms timer while a gesture or its inertia runs | float comparison |
| `dcomp` | one DirectComposition device per process; per window a target, root visual, main visual (`SurfaceTarget::CompositionVisual`) and child visuals per OS layer above it in creation order (offset, clip; `set_os_layer_bounds`); `os_layer_committer` wraps `CommitHandle` (Send) for the compositor's `set_os_layer_committer`; `begin_os_layer_resize`/`end_os_layer_resize` hold every commit (the committer's included) so a viewport's new bounds and its swap chain at the new size land in one commit (from CANVAS, merged `ci/presto-canvas` ae46a5b1) | layer geometry |
| `frame` | skelter-platform's `FrameSource` (`Win32Frames`, returned by `PlatformCx::frame_source` for the compositor's `set_frame_source`; the window keeps a handle and boosts the compositor clock while a DirectManipulation gesture or its inertia runs; `set_waitable` takes a DXGI waitable if one is ever handed over): Windows 11 compositor clock (looked up at run time) with extra handles and boost, the DXGI waitable when given, `DwmFlush`; GPUI's 1 ms early-return rule; interval from `DwmGetCompositionTimingInfo` with the implausible-period fallback | clock results, intervals, timeouts, wake → `FrameWait` |
| `drag` | per-window OLE drop target → `FileHovered`/`FileDropped`/`FileHoverCancelled`; an OLE drag source (text, `CF_HDROP`) that is ready but unreachable (the contract has no drag-source call) | `DROPFILES` layout, drag results |
| `clipboard`, `cursor`, `monitor`, `hit_test`, `util` | `skelter_platform::Clipboard` (text, CRLF on write, no primary selection) plus file lists; CSS cursors; monitors with work areas, DPI, refresh; hit regions; QPC and message times on `skelter_platform::clock` | each |

## What I ran

Locally only `cargo check`/`cargo clippy -p skelter-platform-win32 --target x86_64-pc-windows-msvc
--all-targets` (the msvc std installs in the container; nothing links): clean (clippy's only warnings are in
skelter-platform). **No tests run locally**; CI's skelter workflow runs them on windows-latest. CI run
40222161859 (first push) was green on all three OSes; run 40226717129 (07bb50d1) passed this crate's tests on
all three, failing only in skelter-compositor, skelter-core and skelter-widgets (other lanes).

## Not done / stubbed (no fake successes)

- **Drag source, popup/tooltip windows, fullscreen, pointer capture for tab tear-off**: not in the contract;
  consolidation 2 decides (coordinator, 2026-10-25). The OLE drag source in `drag` is ready for it.
  *Update (lane skelter-fixes, 2026-10-26):* the contract has them now. `start_drag` runs the OLE drag source
  (text and `file:` URIs only) after the handler returns and reports `AppHandler::drag_ended`; `create_popup`
  and `create_child_window` make owned windows; `set_window_state` does maximise/restore/minimise/activate.
  Still absent: fullscreen, popup grabs, `attach_window_to_drag`.
- **The DXGI frame-latency waitable**: it lives in wgpu's swap chain on the compositor side; `FrameSource`
  stays as it is (coordinator, 2026-10-25), so `Win32Frames` paces by the compositor clock or `DwmFlush` and
  only uses a waitable passed to `set_waitable`.
- **IMM32 composition fallback** when TSF fails to start: none (TSF exists on every supported Windows). Stray
  `WM_CHAR`s become `ImeEvent::Commit`.
- **Legacy mouse messages**: not handled; mouse input relies on `EnableMouseInPointer` (always succeeds on the
  first call in a process; if the host disabled it first, mouse moves and clicks would not arrive).
- **IME edits away from the caret** with `set_ime` (no client): a TIP replacing text that does not contain the
  selection cannot be expressed as `ImeEvent`s; the shadow commits the text only. `set_text_input` has no such
  limit.
- **Pointer prediction**: not here.

## Decided (coordinator, 2026-10-25)

- `OsLayerCommitter::new(f)` added to skelter-platform (cross-lane, additive); `os_layer_committer` returns one.
- `FrameSource` is not widened; Windows-only setup happens when the source is created (boost as above).
- `ImeEvent::DeleteSurrounding` documents "before `Commit`, counted from the selection's ends" (skelter-base,
  additive doc).
- `custom_title_bar` skips `DwmExtendFrameIntoClientArea` until the hardware check below.

## Checks that need Windows hardware [local] (REVISIT)

`custom_title_bar` without `DwmExtendFrameIntoClientArea` (shadow, border, caption buttons on Windows 10 and
11, with and without `os_compositing`); `DCompositionWaitForCompositorClock`'s result encoding (`WAIT_OBJECT_0 + count` = tick, from the docs); the
custom frame's shadow and border on Windows 10 and 11 without `DwmExtendFrameIntoClientArea`; snap layouts
over the app-drawn maximise button; raw mouse motion and cursor clipping under `set_mouse_capture`; UIA with
Narrator/NVDA through the subclassing adapter; TSF with MS-IME Japanese, Chinese (Pinyin), Korean, ATOK/Mozc
reconversion; dead keys on US-International and AltGr on German/Polish layouts; DirectManipulation inertia
smoothness at the 8 ms timer; resize cleanliness from every edge with the compositor handshake.

## Cross-lane edits

`crates/skelter-platform/src/api.rs`: `OsLayerCommitter::new` (additive, coordinator's decision).
`crates/skelter-base/src/event.rs`: the `DeleteSurrounding` doc comment (additive, coordinator's decision).
`THIRD_PARTY_NOTICES.md`: a "skelter-platform-win32 (Apache-2.0 port)" section for the GPUI file (merged next to
skelter-render's section). `Cargo.lock`: this crate's `windows` 0.62 and `accesskit_windows` 0.35.1 entries
(versions skelter-platform already uses).
