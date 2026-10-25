# PRESTO-win32: presto's own Win32 platform backend (presto batch 3, lane WIN32)

Date: 2026-10-25. Branch `ci/presto-win32` (from `ci/presto-base` at efe20dd5; merged `ci/presto-base`
7a947d45). Crate: `crates/presto-platform-win32` (empty on other OSes: `#![cfg(target_os = "windows")]`).
It implements `presto_platform::{AppHandler, PlatformCx}` and exposes `run(Box<dyn AppHandler>)`; the early
`shim` module (coded against the b1 brief before PLATFORM's skeleton) is gone.

## What landed

| Module | What | Tests (written) |
|---|---|---|
| `app` | the event loop (OLE STA, `GetMessage`); the handler taken out while it runs, with a re-entrancy queue; a message-only window for wakes, AccessKit events and timer expiry; a high-resolution waitable-timer thread for `set_timer`; the modal-loop policy (everything in the window procedure, so resizes, wakes and timers keep flowing through move/size/menu loops); `drag_window` posted so its move loop runs outside the handler; the TSF keystroke filter before `TranslateMessage`; IME events queued during OS calls and flushed after; `quit_requested` from `WM_QUERYENDSESSION`; `exiting`; the `PlatformCx` | — |
| `window` | class and window procedure; OS frames, or (`decorations: false`) custom frames: caption dropped in `WM_NCCALCSIZE`, top resize band and app regions in `WM_NCHITTEST`, caption buttons get pointer events and act on release, `HTMAXBUTTON` for snap layouts; per-monitor DPI v2 (`WM_GETDPISCALEDSIZE` keeps the size in points, `WM_DPICHANGED` reports the scale before the resize); min/max sizes; `os_compositing` (DComp tree and no redirection bitmap, else a raw HWND target); dark title bar and `ThemeChanged`; cursors; `set_mouse_capture` (cursor clipped and hidden, raw input → `raw_mouse_motion`); UI Automation through `accesskit_windows::SubclassingAdapter` (`update_a11y`, `a11y_event`); `WM_PAINT` validates only | styles, outer sizes, DPI scaling, `WM_SIZE` → events, caption buttons, IME on/off |
| `pointer` | `WM_POINTER*` for mouse (`EnableMouseInPointer`), pen and touch: QPC timestamps, `GetPointerInfoHistory`/`GetPointerPenInfoHistory` history (oldest first), pressure, tilt, button changes while another button is held, cancel; wheel from `WM_POINTERWHEEL` and the legacy wheel messages | message translation, history order, pen, wheel signs |
| `keyboard` | scancode → USB HID usage (page 7), sided modifiers and locations, layout character via `ToUnicodeEx` flag 4 (dead-key state untouched), text from the queued `WM_CHAR`s (dead keys compose through the OS), AltGr kept apart from Ctrl+Alt (synthetic left Ctrl dropped), egui key names | key names, layout mapping (AZERTY, Cyrillic fallback), HID table, AltGr, text filtering |
| `shadow` | the contract has no text-input-client hook: the field arrives as `ImeState` (area, purpose, composition rects, surrounding text) and edits go back as `ImeEvent`s. The shadow is a `TextInputClient` over that state plus the composition, turning edits into `Preedit`/`Commit`/`DeleteSurrounding` | composition and commit, unmark, cancel, reconversion, state updates during a composition, per-character bounds |
| `tsf` | `ITextStoreACP` + `ITfContextOwnerCompositionSink` over the shadow (UTF-16): the lock protocol, TIP edits as `replace`/`replace_and_mark`, the marked range settled to the composition's exact extent after each write lock, unmark on composition end, app-side changes reported through `sync()` (`OnTextChange`/`OnSelectionChange`, checked cheaply), `OnLayoutChange` when the field moves, `GetTextExt` with the caret-area fallback for TIPs that mishandle `TS_E_NOLAYOUT`, input scopes as an `ITfInputScope` attribute, `end_ime_composition` (cancel drops the preedit first) | range resolution, text changes, extents, relative selections, lock protocol, input scopes |
| `imm` | `WM_IME_REQUEST`: `IMR_RECONVERTSTRING` (ATOK/Mozc reverse conversion), `IMR_DOCUMENTFEED`, `IMR_QUERYCHARPOSITION` | `RECONVERTSTRING` layout, context windows |
| `dmanip` | port of GPUI's `direct_manipulation.rs` (Apache-2.0: header, `THIRD_PARTY_NOTICES.md`): precision-touchpad pan with inertia (wheel events in pixels with gesture phases) and pinch; updated on each compositor wake and on an 8 ms timer while a gesture or its inertia runs | float comparison |
| `dcomp` | one DirectComposition device per process; per window a target, root visual, main visual (`SurfaceTarget::CompositionVisual`) and child visuals per OS layer above it in creation order (offset, clip; `set_os_layer_bounds`); `CommitHandle` (Send) for the compositor after wgpu sets content | layer geometry |
| `frame` | the frame source (inherent `wait`/`wait_with`/`interval`/`set_boost`; presto-platform's `FrameSource` trait was not written yet): Windows 11 compositor clock (looked up at run time) with extra handles and boost, the DXGI waitable when given, `DwmFlush`; GPUI's 1 ms early-return rule; interval from `DwmGetCompositionTimingInfo` with the implausible-period fallback | clock results, intervals, timeouts |
| `drag` | per-window OLE drop target → `FileHovered`/`FileDropped`/`FileHoverCancelled`; an OLE drag source (text, `CF_HDROP`) that is ready but unreachable (the contract has no drag-source call) | `DROPFILES` layout, drag results |
| `clipboard`, `cursor`, `monitor`, `hit_test`, `util` | `presto_platform::Clipboard` (text, CRLF on write, no primary selection) plus file lists; CSS cursors; monitors with work areas, DPI, refresh; hit regions; QPC and message times on `presto_platform::clock` | each |

## What I ran

Locally only `cargo check`/`cargo clippy -p presto-platform-win32 --target x86_64-pc-windows-msvc
--all-targets` (the msvc std installs in the container; nothing links): clean (clippy's only warnings are in
presto-platform). **No tests run locally**; CI's presto workflow runs them on windows-latest. CI run
40222161859 (first push) was green on all three OSes; later runs are in the coordinator report.

## Not done / stubbed (no fake successes)

- **`os_layer_committer`** returns `None`: `OsLayerCommitter`'s field is `pub(crate)` with no public
  constructor, so a backend crate cannot build one (question 3).
- **Drag source, popup/tooltip windows, fullscreen, pointer capture for tab tear-off**: the contract has no
  calls for them.
- **IMM32 composition fallback** when TSF fails to start: none (TSF exists on every supported Windows). Stray
  `WM_CHAR`s become `ImeEvent::Commit`.
- **Legacy mouse messages**: not handled; mouse input relies on `EnableMouseInPointer` (always succeeds on the
  first call in a process; if the host disabled it first, mouse moves and clicks would not arrive).
- **`create_window` failure** returns an id every other call treats as unknown (`surface_target` →
  `SurfaceTarget::None`); the signature has no error.
- **IME edits away from the caret** (a TIP replacing text that does not contain the selection) cannot be
  expressed as `ImeEvent`s; the shadow commits the text only.
- **Pointer prediction**: not here.

## Questions for the coordinator (each with a recommendation)

1. **IME data model.** With no client hook, TSF and `WM_IME_REQUEST` see only `ImeState.surrounding` (a window
   around the caret). Recommendation: keep it, and have the editing kinds always fill `surrounding` (≥ 1 KiB
   each side) and `composition_rects`; reconversion and document feed need them.
2. **`ImeEvent::DeleteSurrounding` with a selection**: this backend emits it before `Commit`, meaning "delete
   these bytes before and after the selection, then commit replaces the selection". Recommendation: state that
   order in presto-base's doc comment so the editing kinds agree.
3. **`OsLayerCommitter` needs a public constructor** (`OsLayerCommitter::new(f)`, like `Waker::new`).
   Recommendation: add it in presto-platform; this crate then returns one wrapping `dcomp::CommitHandle`.
4. **The DXGI waitable** comes from wgpu-hal's swap chain on the compositor thread;
   `Win32FrameSource::set_waitable` takes it. Recommendation: the consolidation wires it when presto-platform's
   `FrameSource` trait lands, implementing that trait with `Win32FrameSource`'s inherent methods.

## Checks that need Windows hardware (REVISIT)

`DCompositionWaitForCompositorClock`'s result encoding (`WAIT_OBJECT_0 + count` = tick, from the docs); the
custom frame's shadow and border on Windows 10 and 11 without `DwmExtendFrameIntoClientArea`; snap layouts
over the app-drawn maximise button; raw mouse motion and cursor clipping under `set_mouse_capture`; UIA with
Narrator/NVDA through the subclassing adapter; TSF with MS-IME Japanese, Chinese (Pinyin), Korean, ATOK/Mozc
reconversion; dead keys on US-International and AltGr on German/Polish layouts; DirectManipulation inertia
smoothness at the 8 ms timer; resize cleanliness from every edge with the compositor handshake.

## Cross-lane edits

`THIRD_PARTY_NOTICES.md`: a "presto-platform-win32 (Apache-2.0 port)" section for the GPUI file (merged next to
presto-render's section). `Cargo.lock`: this crate's `windows` 0.62 and `accesskit_windows` 0.35.1 entries
(versions presto-platform already uses).
