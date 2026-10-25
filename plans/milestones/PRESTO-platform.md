# PRESTO-platform: presto-platform (presto batch 1, lane PLATFORM)

Status: wave-1 deliverables landed on `ci/presto-platform` (2026-10-25). The brief and lane file are
`plans/cloud/presto-b1/BRIEF.md` and `LANE-platform.md`. The final sha and CI run ids are in the
coordinator report; the last section of this file lists what CI said.

## What landed (`crates/presto-platform/**`)

| Module | What |
|---|---|
| `api.rs` | The brief's `AppHandler` / `PlatformCx` (signatures kept), plus `WindowDesc`, `SurfaceTarget`, `OsLayerDesc`, `OsLayerCommitter`, `ImeState`, `HitRegion`/`HitKind`/`hit_test`, `Monitor`, `A11yEvent`, `Waker` and `PlatformError`. |
| `backend.rs` | `run()`: the winit 0.30.13 `ApplicationHandler`. It covers windows (title, size, min/max, position, decorations, transparent, resizable, maximized, app id), events translated into `InputEnvelope`s with `InputSeq`, OS time and receive time, coalesced cross-thread wakes through `EventLoopProxy`, timers through `WaitUntil`, and `DeviceEvents::Never` except while `set_mouse_capture` is on (then raw `MouseMotion` goes to `AppHandler::raw_mouse_motion`). It also has cursors, IME on/off with cursor area and purpose, monitors, `surface_target`, the clipboard, drag_window (a caption press outside Windows starts the move) and the a11y hookup. |
| `translate.rs`, `keys.rs` | winit events → presto events in window points: buttons bitmask, pointer history, wheel lines vs pixels with phases, touch, gestures, IME, files, window events. Logical keys get egui names (egui-winit 0.37 tables, attributed; the physical key is the fallback for non-Latin layouts). Physical keys get USB HID usages (`page << 16 | usage`). |
| `os/windows.rs` | `SetWindowSubclass` on winit's HWND. Hit testing: `WM_NCHITTEST` from the hit regions (HTCAPTION, HTMIN/MAX/CLOSE for snap layouts, HTSYSMENU); caption-button clicks become `SC_*` commands. Modal loops: during size/move and menu loops a `SetTimer` posts timer ticks, and `WM_PAINT` is validated instead of forwarded. Pointer history comes from `GetMouseMovePointsEx` (display points, 16-bit wrap handled, tick → presto time). `WM_IME_REQUEST` answers `IMR_QUERYCHARPOSITION` (from `ImeState.composition_rects`, else `area`) and `IMR_DOCUMENTFEED` (from `ImeState.surrounding`). Composition commit and cancel go through `ImmNotifyIME`. OS timestamps come from `GetMessageTime`. With `os_compositing`, DirectComposition is used: no redirection bitmap, a DComp device (`DCompositionCreateDevice2`), a target, a root and a content visual (`surface_target` → `CompositionVisual`), and child visuals for `create_os_layer`. The work area comes from `GetMonitorInfoW`. |
| `os/macos.rs` | Cmd-Q veto: `applicationShouldTerminate:` is added to winit's `WinitApplicationDelegate` class at run time; it returns `NSTerminateCancel` and routes to `AppHandler::quit_requested`. OS layers are `CAMetalLayer` sublayers of winit's NSView layer (flipped/unflipped host handled, relaid out on resize and scale change, no implicit animations). OS timestamps come from `NSEvent.timestamp`. The work area comes from `NSScreen.visibleFrame`. |
| `os/wayland.rs` | A side `Connection` on winit's `wl_display` (one per process) with its own queues. OS layers are `wl_subsurface`s (desync, empty input region, `wp_viewporter` size, else `set_buffer_scale`). `WaylandFrameSource` uses frame callbacks on the toplevel plus `wp_presentation` feedback (clock from `clock_id`), and reports `Occluded(true)` after 100 ms without a requested callback and `Occluded(false)` when callbacks resume. X11 falls back to the generic shim. |
| `os/generic.rs` | What winit alone can do (other OSes, X11). |
| `frame/` | `FrameSource` trait (`Send + Sync`: `wait(timeout)`, `before_present(frame)`, `take_presented()`, `interval()`), `FrameWait`, `Presented`. `IntervalSource` (X11/fallback, estimated). Windows `DwmSource`: the compositor clock looked up at run time (Windows 10 still loads), else `DwmFlush`, with GPUI's under-1-ms rule, targets from `DwmGetCompositionTimingInfo`. macOS `DisplayLinkSource`: `CVDisplayLink` through the C API, targets from `inOutputTime.hostTime`. `PlatformCx::frame_source(window)` picks the right one. |
| `clipboard.rs` | `Clipboard` trait with `ClipboardKind::{Clipboard, Primary}`. `SystemClipboard`: smithay-clipboard on Wayland (winit's display), arboard elsewhere (X11 primary through `LinuxClipboardKind::Primary`). `MemoryClipboard` for tests and as the fallback. |
| `a11y.rs` | AccessKit adapters used directly: accesskit_windows 0.35.1 / accesskit_macos 0.27.1 subclassing adapters and accesskit_unix 0.24.0. Activation, action and deactivation go to `AppHandler::a11y_event`; `PlatformCx::update_a11y` feeds `Commit.a11y` updates. Adapters attach before the window is first shown. |
| `dialogs.rs` | Feature `dialogs`: rfd file dialogs (pick file/files/folder, save) and message boxes. They are blocking. |
| `headless.rs` | `HeadlessPlatform`: timed script (`Scripted::{Input, Window, Wake, A11y, QuitRequested}`), `VirtualClock`, timers fired between events, coalesced wakes, window records, a log of every call (`PlatformCall`), `run`/`run_until`. |
| `clock.rs` | The process-wide presto clock (`Instant` epoch), `to_instant`/`from_instant`, `time_from_os`, `OsClock`. |

Tests (written, not run): key-name round trip, logical/physical fallback, HID usages distinct; event
translation (points, buttons, history, wheel, window events, IME, touch ids, cursors); headless scripting
(ordering, timers, wakes, exit, run_until, quit veto, resize records); clipboard; interval source; clock.

## Additions beyond the brief's API (all additive)

- `PlatformCx`: `set_os_layer_bounds`, `remove_os_layer`, `os_layer_committer`, `end_ime_composition`,
  `set_mouse_capture`, `set_title`, `set_visible`, `drag_window`, `update_a11y`, `window_scale`,
  `frame_source`. `create_window` panics with the OS error when the OS refuses (the brief's signature
  returns a bare `WindowId`, and a dead id would be a fake success). The backend also has
  `try_create_window`, which is not on the trait yet (see Q2).
- `AppHandler` (default impls): `raw_mouse_motion`, `a11y_event`, `quit_requested`, `exiting`.
- `WindowDesc.os_compositing` (default true): DirectComposition on Windows, ignored elsewhere.
- `ImeState { area, purpose, composition_rects, surrounding }`.

presto-base: **no changes.**

## Cross-lane edits

- `THIRD_PARTY_NOTICES.md`: one table for the egui-winit key tables ported into `src/keys.rs` (the brief
  requires each ported source there).
- `Cargo.lock`: presto-platform's dependencies.

## Stubbed or left out (honestly absent, nothing fakes success)

- **Windows:** no TSF (IMM32 through winit only); no `IMR_RECONVERTSTRING` (reconversion needs the
  `TextInputClient` replace path, which comes with the own backend); no DirectManipulation; no OLE drag
  source. Caption-button hover and press visuals get no client events: `WM_NCMOUSEMOVE` over our
  HTMIN/MAX/CLOSE regions is not forwarded to the app yet. The DXGI waitable belongs to the compositor
  (`as_hal::<Dx12>()`), not here.
- **macOS:** `CAMetalDisplayLink` (CVDisplayLink is used instead; it follows the active displays rather
  than the window's screen); mouse coalescing; `presentsWithTransaction` live resize (compositor and wgpu
  patch side); `end_ime_composition` falls back to toggling IME off and on for cancel, and returns
  `false` for commit.
- **Wayland:** no `fifo-v1`/`commit-timing-v1`; no tear-off experiment (D7); no work areas. A one-frame
  race with winit's own surface commits remains (platform.md §4).
- **Everywhere:** no maximize/minimize state events (winit #2334); OS timestamps only on Windows and macOS
  (Wayland and X11 use receive time); `WheelEvent.momentum` is always `None` (winit 0.30 has none);
  `Modifiers::ALT_GR` is never set (winit 0.30 reports AltGr as Ctrl+Alt on Windows); muda, ashpd and
  notify-rust are not added.
- The occlusion a `FrameSource` reports reaches the compositor only. Forwarding it to the UI as
  `WindowEvent::Occluded` is the compositor lane's job.

## Not run

- No tests were run (by the batch rule). `cargo check -p presto-platform --all-targets` passed locally
  for `x86_64-unknown-linux-gnu`, `x86_64-pc-windows-msvc` and `aarch64-apple-darwin` (cross-target
  check, no linking), and with `--features dialogs` on Linux.
- Nothing ran on hardware: DComp layers, snap layouts, modal-loop timers, pointer history, IME requests,
  Cmd-Q veto, CAMetalLayer sublayer geometry (flipped vs unflipped host), Wayland subsurfaces and
  presentation feedback, AccessKit with screen readers. These checks go to REVISIT
  (platform.md §8).

## Questions for the coordinator (each with a recommendation)

1. **Who forwards frame-source occlusion to the UI?** Recommendation: the compositor turns
   `FrameWait::Occluded(b)` into `ToUi` (a new `ToUi::Occluded { window, occluded }` in presto-base),
   rather than the platform polling the source.
2. **Should `create_window` return a `Result`?** Recommendation: add `try_create_window` to `PlatformCx`
   (additive) and keep `create_window` as the panicking convenience.
3. **Is DComp on by default on Windows?** `WindowDesc::default().os_compositing = true` needs the
   compositor to create the window surface from `SurfaceTarget::CompositionVisual` on DX12, and to call
   `os_layer_committer().commit()` after configuring a surface (wgpu calls `SetContent` but not `Commit`).
   Recommendation: keep it on; the compositor lane falls back to `os_compositing: false` when the backend
   is not DX12.
4. **macOS frame source:** CVDisplayLink now, `CAMetalDisplayLink` together with wgpu patch P3?
   Recommendation: yes, keep CVDisplayLink until P3 lands.
