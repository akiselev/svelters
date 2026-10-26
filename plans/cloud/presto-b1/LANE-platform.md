# Lane PLATFORM: presto-platform

Branch `ci/presto-platform`. Owns `crates/presto-platform/**`. Read: `platform.md` (the strategy: winit
0.30.13 plus shims, no fork), architecture §11; notes `round2/winit.md`, `round2/ime-a11y-os.md`,
`round2/wgpu-integration.md` §1–§2, `round2/gpui-port.md` items 7–14.

Deliver (the brief's `AppHandler`/`PlatformCx`/`run` API first, within 45 min):
1. The winit 0.30.13 backend: `ApplicationHandler`, windows (`WindowDesc`: title, size, min size,
   decorations, transparent), event translation to presto-base `InputEnvelope`s with `InputSeq` and OS
   timestamps converted to presto `Time`, keys mapped to egui-named `Key`s (+ physical codes), IME events,
   wheel/touchpad phases, cursor icons, `Waker` (EventLoopProxy), timers (`WaitUntil`), `DeviceEvents::Never`
   by default (raw input only while a 3D view captures the mouse), monitors, `SurfaceTarget` (raw window +
   display handles for wgpu; create surfaces on the main thread).
2. Shims (platform.md §3.2), each behind `cfg(target_os)`:
   - Windows: `SetWindowSubclass` on winit's HWND: `WM_NCHITTEST` hit-test regions (snap layouts),
     modal-loop handling (skip the `WM_PAINT` re-arm during `WM_ENTERSIZEMOVE…EXITSIZEMOVE`, `SetTimer` for
     our timers), `GetMouseMovePointsEx` pointer history, `WM_IME_REQUEST` (reconversion, candidate
     placement), `ImmNotifyIME` composition cancel/commit, DirectComposition target on the HWND
     (`with_no_redirection_bitmap`) and `create_os_layer` via DComp child visuals (return
     `SurfaceTarget::CompositionVisual` for wgpu's `SurfaceTargetUnsafe::CompositionVisual`).
   - macOS: our app delegate hooks (Cmd-Q veto), CAMetalLayer sublayers for `create_os_layer`.
   - Wayland: a side event queue on winit's `wl_display` (smithay-client-toolkit / wayland-client):
     `wp_presentation` feedback, subsurfaces for `create_os_layer`, occlusion via the 100 ms frame-callback
     starvation rule.
3. Clipboard and primary selection (arboard, smithay-clipboard), file drops, rfd dialogs behind a feature.
4. AccessKit platform adapters directly (accesskit_windows/_macos/_unix at the versions svelters' lockfile
   has), fed by `Commit.a11y` updates the app passes in.
5. `FrameSource` implementations per OS for the compositor (Windows: DXGI waitable / compositor clock;
   macOS: CAMetalDisplayLink; Wayland: frame callbacks + presentation) — define the trait here, the
   compositor consumes it.
6. `HeadlessPlatform`: scripted input, virtual clock, no windows, for tests.

The Windows and macOS shims compile only on those OSes: CI checks them (the `presto` workflow runs on
windows-latest and macos-latest). Use the `windows` and `objc2` crates; no `unsafe` without a `// SAFETY:`.
Tests (write, don't run): key-name round trip; event translation; headless platform scripting.
