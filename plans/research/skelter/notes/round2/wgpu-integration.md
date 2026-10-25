# wgpu / winit integration for tessel's compositor thread (research notes, 2026-10-25)

Scope: what wgpu 30.0.1, winit 0.30.13 / 0.31.0-beta.4 and the platform APIs allow for a design in
which the **UI thread** (platform main thread) owns windows, events, the scene and JS, and one
**compositor thread** owns the wgpu `Device`/`Queue`, every `Surface`, and all acquires and presents. 3D
viewports and canvases get their own OS compositor layers (DComp child visuals, CAMetalLayer sublayers,
Wayland subsurfaces), each with its own wgpu `Surface` on the same `Device`.

Context read first: `docs/research/2026-10-25-own-gui-toolkit.md` §4–§8 and the notes `windows.md`,
`gtk-qt-wayland.md`, `gpu-2d-text.md` (same folder). This note does not repeat them; it checks their wgpu
claims against source and fills the gaps.

## Sources and versions

Grades: **[src]** read in source at the stated sha/tag, **[doc]** vendor or project docs, **[iss]** issue
or PR thread, **[anec]** forum anecdote, **[inf]** my inference, **[mem]** from memory, not re-verified.

| Source | Version / sha | How |
|---|---|---|
| wgpu | tag `v30.0.1` = `40f4a34` (2026-08-21) | sparse shallow clone (`wgpu`, `wgpu-core`, `wgpu-hal`, `wgpu-types`) |
| wgpu trunk | `100c142` (2026-10-25; workspace version still `30.0.0`; no v31 tag yet) | raw files + `CHANGELOG.md` |
| winit | `v0.30.13`; `v0.31.0-beta.4` = `d597ced` (2026-10-24); master `0c73728` | shallow clones |
| raw-window-metal | 1.1.0 (the version in wgpu 30.0.1's `Cargo.lock`) | crates.io tarball |
| egui-wgpu | tag `0.37.0` `crates/egui-wgpu/src/renderer.rs` | raw file |
| Zed / GPUI | `a5c4ca8` (2026-10-25), `crates/gpui_*` | sparse clone |
| Bevy | main `fd9e760` (`bevy_render/src/view/window/mod.rs`, `pipelined_rendering.rs`) | raw files |
| Firefox | `mozilla-firefox/firefox` main `efd2d1e` (`gfx/layers/NativeLayerCA.mm`, `widget/cocoa/nsCocoaWindow.mm`, `widget/gtk/WaylandSurface.cpp`) | raw files |
| Mesa | main `e7371b07` (`src/vulkan/wsi/wsi_common_wayland.c`) | raw file |
| forest-rs `frameclock` | `f12fbc0` (2026-10-16) | clone |
| forest-rs `subduction` | `bfa27d0` (2026-10-15) | clone |
| calloop 0.14.3, polling 3.11.0 | crates.io tarballs | |
| wgpu issues #2869, PR #8388, zed-industries/wgpu PR #2 | pages read 2026-10-25 | WebFetch; PR #2 diff downloaded |
| Apple, Microsoft docs | pages read 2026-10-25 | WebFetch |

Not read: the `windows` crate source (the DXGI/DComp calls are named in wgpu-hal and Microsoft docs, and
that was enough); objc2-quartz-core source beyond what wgpu and raw-window-metal call; smithay-client-toolkit
source (only its role is described); Chrome source (only [mem]).

---

## 1. Threading

### 1.1 What wgpu itself allows (all backends)

- `wgpu::Surface<'_>: Send + Sync`, asserted: `wgpu/src/api/surface.rs:268`
  (`static_assertions::assert_impl_all!(Surface<'_>: Send, Sync)`); `SurfaceTexture: Send + Sync`
  (`surface_texture.rs:17`); `SurfaceConfiguration: Send + Sync` (`surface.rs:18`). [src v30.0.1]
- So a `Surface` created on the main thread can be moved to the compositor thread, and a `SurfaceTexture`
  acquired on the compositor thread can be handed back to the main thread to be presented there (used for
  macOS resize below). [src+inf]
- `Queue::present(surface_texture)` replaced `SurfaceTexture::present()` in v30.0.0 (CHANGELOG v30.0.0,
  "#9361"). Inside, wgpu-core takes `device.command_indices.write()` around the hal present, "to prevent
  submits ... and other present calls" (`wgpu-core/src/present.rs:348-356`). So presents and submits on one
  device are serialized whatever thread calls them. [src]
- Only one acquired texture per surface: `get_current_texture` returns `AlreadyAcquired` otherwise
  (`present.rs:245-247`). [src]
- **The acquire timeout is hard-coded:** `const FRAME_TIMEOUT_MS: u32 = 1000;` (`present.rs:32`), passed to
  `hal::Surface::acquire_texture` (`present.rs:177-180`). There is no public API to acquire with a different
  timeout. With one compositor thread serving every window, **one blocking acquire on one window stalls all
  windows for up to 1 s**. [src+inf] This is the most important threading constraint for our design (§3.4).
- Surface *creation* is the platform-sensitive step:
  - `SurfaceTarget::DisplayAndWindow` doc: "On macOS/Metal: will panic if not called on the main thread"
    (`surface.rs:303`). [src]
  - winit returns `HandleError::Unavailable` for `window_handle()` off the main thread on macOS, in both
    0.30.13 (`src/platform_impl/macos/window.rs:54-62`) and 0.31.0-beta.4
    (`winit-appkit/src/window.rs:63-71`, `MainThreadMarker::new()` check). [src]
  - Rule: **create every surface on the UI thread, then move it to the compositor thread.** Bevy does the
    same: its `create_surfaces` system takes a `NonSendMarker` on macOS/iOS "to put this system on the main
    thread" (`bevy_render/src/view/window/mod.rs:368-372`), while acquire/present run in the render world on
    the render thread. [src]

### 1.2 winit: are handles and windows Send?

- winit 0.31: `pub trait Window: Any + Send + Sync + fmt::Debug` (`winit-core/src/window.rs:636`). winit 0.30:
  `Window` is `Send + Sync` [mem; 0.30 `src/window.rs:174-176` makes the rwh handle wrapper Send/Sync]. [src]
- But on macOS, most `Window` methods called off-main go through `maybe_wait_on_main`, a synchronous
  dispatch to the main queue (`winit-appkit/src/window.rs:106-113` and the `MainThreadBound` fields at
  `:26-42`). [src] **If the compositor thread calls a winit `Window` method while the UI thread is waiting on
  the compositor (resize sync), the two deadlock.** Rule: the compositor never touches winit objects; it gets
  sizes and scale factors through the Commit stream. [inf]
- raw-window-handle 0.6.2 handles are raw pointers (`NonNull`), not `Send`; wgpu takes them through
  `SurfaceTargetUnsafe` at creation time only. [src]
- winit 0.31 changelog: "Implement `Send` and `Sync` for `OwnedDisplayHandle`" (`winit/src/changelog/v0.31.md:47`),
  so the display handle needed by `InstanceDescriptor::new_with_display_handle` can live on the compositor
  thread. [src]

### 1.3 Metal / CAMetalLayer off the main thread

What wgpu-hal does (v30.0.1, `wgpu-hal/src/metal/surface.rs`):
- `configure` (`:248-341`) runs on the calling thread and sets layer properties: `setDevice`,
  `setPixelFormat`, `setFramebufferOnly`, `setColorspace`, `setMaximumDrawableCount(latency + 1)` (`:330`),
  `setDrawableSize` (`:331`), **`setAllowsNextDrawableTimeout(false)`** (`:334`), `setDisplaySyncEnabled`
  (`:337`). It does not touch `bounds` or `contentsScale`. [src]
- `dimensions()` reads `bounds` and `contentsScale` off-main and says this is "sound, as these properties are
  accessed atomically", citing wgpu PR #7692 (`:186-206`). [src]
- `acquire_texture` (`:347-406`) walks from the layer up to the hosting `NSWindow` through layer delegates
  and calls `[window occlusionState]` (`:354-371`), then `nextDrawable` (`:373-380`). **That is an AppKit
  `NSWindow`/`NSView` message sent from whatever thread acquires**, which AppKit documents as main-thread-only
  [inf: AppKit rule; the same file's `display_hdr_info` explicitly refuses off-main NSWindow access at
  `:67-92`, "off-thread access is undefined behavior"]. Trunk keeps the off-main call and adds a
  `has_been_visible` flag (trunk `metal/surface.rs:372-393`). **Patch candidate (small).**
- With `allowsNextDrawableTimeout = false`, `nextDrawable` blocks until a drawable is free, with no 1 s
  timeout; the hal ignores its `timeout` argument (`_timeout: ... // TODO`, `:349`). [src+doc
  `CAMetalLayer.allowsNextDrawableTimeout`]
- `present` (`metal/mod.rs:802-826`): if the layer's `presentsWithTransaction` was false at acquire time,
  `commandBuffer.presentDrawable` + `commit`; if true, `commit`, `waitUntilScheduled`, `drawable.present()`
  **on the calling thread**. Every present creates and commits an extra `MTLCommandBuffer` labelled
  "(wgpu internal) Present". [src]
- Surfaces from raw handles go through `raw_window_metal::Layer::from_ns_view` (`metal/mod.rs:160-190`), which
  panics off the main thread ("can only access NSView on the main thread", raw-window-metal 1.1.0
  `src/lib.rs:402-403`). It **creates a sublayer** of the view's layer and keeps its `bounds` and
  `contentsScale` in sync with the root layer through KVO observers (`src/observer.rs:81-187`; rationale in
  `lib.rs:68-128`). It also says `CAMetalLayer` is thread-safe "like most things in Core Animation" and marks
  `Layer: Send + Sync` (`lib.rs:182-189`). [src]

Apple rules:
- `presentsWithTransaction`: false (the default) presents "as quickly as possible and asynchronously to any
  Core Animation transactions" and "Core Animation doesn't guarantee that the Metal content arrives in the
  same frame as other Core Animation content". True draws "synchronously, using whichever Core Animation
  transaction is current at the time you call the drawable's present()"; commit, `waitUntilScheduled`, then
  `present()`; never `commandBuffer.present(_:)`. [doc `CAMetalLayer.presentsWithTransaction`, read 2026-10-25]
- Core Animation transactions are **per thread**. Firefox's comment: "CoreAnimation transactions are
  per-thread. They don't nest across threads. If you submit a transaction on the main thread and a
  transaction on a different thread, the two will race to the window server ... Cocoa has built-in
  synchronization mechanics that make sure that *main thread* window paints during window resizes are
  synchronized properly with the window shape change. But no such built-in synchronization exists for
  CATransactions that are triggered on a non-main thread." (`widget/cocoa/nsCocoaWindow.mm:329-347`) [src]
- Layer mutations off the main thread need an **explicit** transaction: "Mutating CALayers outside of a
  transaction can result in permanently stuck rendering, because such mutations create an implicit
  transaction which never auto-commits if the current thread does not have a native runloop."
  Firefox wraps them in `NSAnimationContext beginGrouping/endGrouping` + `setDisableActions:YES`
  (`gfx/layers/NativeLayerCA.mm:120-145`). [src]
- Apple's own "Creating a custom Metal view" sample has `RENDER_ON_MAIN_THREAD`; when 0, "rendering occurs on
  a background thread, allowing the UI to respond more quickly", driven by a `CVDisplayLink` callback that
  "never executes on the main thread". [doc, sample page read 2026-10-25] So rendering, `nextDrawable` and
  `present` off main are supported. I found no Apple statement that `drawableSize` must be set on main; the
  sample page does not say. [doc gap]
- `CAMetalDisplayLink` (macOS 14+): delegate callbacks on the run loop it is added to (`add(to:forMode:)`);
  each `Update` carries the drawable and target timestamps. [doc] frameclock_apple adds it to
  `NSRunLoop::currentRunLoop()` (`frameclock_apple/src/metal_display_link.rs:381`), i.e. any thread with a run
  loop, and warns "Use plain `present`: Core Animation asserts on the timed variants, such as
  `presentAtTime:`, for a drawable from this link" (`:117-126`). [src] wgpu cannot render into a drawable it
  did not get from `nextDrawable` itself: **patch candidate** (§8).

What others do on macOS:
- **GPUI** renders on the main thread. On `displayLayer:` (AppKit's resize/redraw callback) it sets
  `presents_with_transaction(true)`, stops the display link, renders a frame synchronously, then restores
  (`gpui_macos/src/window.rs:3323-3343`; also on `windowDidBecomeKey:` `:3196-3216`). Its Metal present then
  commits, `wait_until_scheduled`, `drawable.present()` (`gpui_apple/src/metal_renderer.rs:553-560`). [src]
- **Firefox** renders off main (compositor thread) and commits layers off main, except in a resize "danger
  zone": `SuspendAsyncCATransactions()` (from `nsCocoaWindow::Resize` and two other sites,
  `nsCocoaWindow.mm:348-365`, `:1375`, `:1982`, `:7080`) forces sync flushes of the rendering pipeline and
  makes `NativeLayerRootCA::CommitToScreen()` refuse off-main commits (`NativeLayerCA.mm:266-290`); the main
  thread commits inside its own transaction, and unsuspension is scheduled after layer display returns to the
  event loop (`nsCocoaWindow.mm:367-390`). [src]
- **Bevy** acquires and presents on its render thread; no resize synchronisation found in
  `prepare_windows` (`mod.rs:300-345`). [src]

### 1.4 DX12 / DXGI from a non-window thread

- Microsoft: deadlocks can occur when "the rendering thread is not the message-pump thread" or "the thread
  executing a DXGI API is not the same thread that created the window"; "never have the message-pump thread
  wait on the render thread when you use full-screen swap chains", because `Present1` can call
  `SetWindowPos`, which can `SendMessage` to the window's thread. [doc "DXGI overview", Multithread
  considerations, updated 2025-09-11]
- wgpu calls `MakeWindowAssociation(DXGI_MWA_NO_WINDOW_CHANGES | DXGI_MWA_NO_ALT_ENTER)` for HWND swap chains
  (`dx12/mod.rs:1660-1670`) [src], and composition swap chains (`CreateSwapChainForComposition`) have no
  window association at all [inf]. So the SendMessage deadlock path is the fullscreen/mode-change one; a
  windowed composition swap chain presented from the compositor thread avoids it. [inf]
- DirectComposition: "call any method from any thread at any time"; multiple devices can share one visual
  tree (`windows.md` §2.4) [doc, via note]. So the compositor thread may own the DComp device and commit.
- wgpu's DX12 present goes to a **dedicated present queue** (`device.present_queue`, used for all
  `CreateSwapChainFor*` calls at `dx12/mod.rs:1548-1574`; configure waits for it idle, `:1490`). [src]
- GPUI on Windows renders on the UI thread and runs a separate `VSyncProvider` thread that waits on
  `DwmFlush()` and posts to the window (`gpui_windows/src/vsync.rs`, `platform.rs:449-470`); its comment notes
  `DwmFlush` and `DCompositionWaitForCompositorClock` "return very early ... when the monitor goes to sleep",
  so it falls back to `Sleep` when the wait took < 1 ms (`vsync.rs:45-52`). [src] subduction's Windows tick
  source does the same (`subduction_backend_windows/src/tick.rs:6-75`). [src]

### 1.5 Vulkan on Wayland / X11 from another thread

- Mesa's Wayland WSI creates its **own event queue** per display connection
  (`wl_display_create_queue_with_name`, `wsi_common_wayland.c:1621`) and proxy wrappers
  (`wl_proxy_create_wrapper(wl_display)`, `:1627`); present does `wl_surface_attach`, damage, optional
  `wp_fifo` barrier, `wl_surface_commit` on the surface wrapper (`:3371-3527`). [src] libwayland-client is
  thread-safe with per-queue dispatch [mem], so `vkQueuePresentKHR` on the compositor thread while the UI
  thread dispatches winit's queue is supported.
- **The real hazard is semantic, not memory safety.** `wl_surface.commit` applies *all* pending state of that
  surface, whoever queued it. winit's configure handler (UI thread) sets `xdg_surface.set_window_geometry`
  and, with fractional scaling, `wp_viewport.set_destination` (`winit-wayland/src/window/state.rs:481-492`) and
  then deliberately does not commit: "configure demands wl_surface::commit, however winit doesn't commit on
  behalf of the users ... thus it'll ask users to redraw instead" (`winit-wayland/src/state.rs:346-353`). [src]
  If the compositor thread's next Vulkan present is still an old-size frame, that commit latches the new
  geometry/viewport with the old buffer: one stretched (viewporter) or mis-sized frame. [inf]
- Firefox serialises all `wl_surface` state through a per-surface mutex (`WaylandSurfaceLock`,
  `widget/gtk/WaylandSurface.cpp`, e.g. `Commit(WaylandSurfaceLock*...)` at `:607-625`) and draws content into
  a `wl_subsurface` of the toplevel, desync by choice (`:449-461`). [src]
- X11: Mesa's X11 WSI uses xcb (thread-safe) [mem]; window size changes come as `ConfigureNotify` on the UI
  thread; no compositor sync protocol in winit [mem]. Accept full redraws and an occasional stretched frame.

### 1.6 Resize synchronisation, per OS (what the evidence supports)

| OS | Mechanism | Evidence |
|---|---|---|
| macOS | During live resize, `presentsWithTransaction = YES`, render the new size and `present()` **on the main thread** inside the transaction that carries the window resize; suspend off-main CA commits until the main thread returns to its event loop | Apple doc (above); Firefox `nsCocoaWindow.mm:329-390`; GPUI `window.rs:3323-3343` |
| Windows (DComp) | On the size message, the UI thread asks the compositor for a new-size frame and waits for its `Present` (and `IDCompositionDevice::Commit` when visual properties changed) before returning from the message | Anecdote: with `CreateSwapChainForComposition`, calling `ResizeBuffers` from `WM_NCCALCSIZE` and presenting before returning removed edge flicker; top/left-edge drags still showed gaps [anec gamedev.net topic 708865]. DComp atomic batches per `Commit` [doc, windows.md §2.4] |
| Wayland | The commit that follows `ack_configure` must carry a buffer of the new size; viewport subsurfaces in **sync** mode during resize so their commits latch with the parent's | winit `state.rs:346-353`; `wl_subsurface` sync semantics and "wp_fifo ignored on sync subsurfaces" (`gtk-qt-wayland.md` §3.1) |
| Bevy | Nothing special; it reconfigures from the extracted size each frame and re-acquires on `Outdated` (`mod.rs:300-345`) | [src] |

---

## 2. Creating surfaces for OS layers (wgpu 30.0.1)

### 2.1 `SurfaceTargetUnsafe` variants (`wgpu/src/api/surface.rs:383-464`)

| Variant | cfg gate | Payload | Notes |
|---|---|---|---|
| `RawHandle { raw_display_handle: Option<RawDisplayHandle>, raw_window_handle: RawWindowHandle }` | none | rwh 0.6 handles | display may be omitted if given in `InstanceDescriptor::display`; if both, "their values _must_ be identical" (`:389-390`), else `CreateSurfaceError::MismatchingDisplayHandle` (`wgpu-core/src/instance.rs:205-220`) |
| `Drm { fd, plane, connector_id, width, height, refresh_rate }` | `#[cfg(drm)]` | | not relevant |
| `CoreAnimationLayer(*mut c_void)` | `#[cfg(metal)]` | a `CAMetalLayer*` | → `Instance::create_surface_metal` → `hal::metal::Instance::create_surface_from_layer` (`wgpu-core/src/instance.rs:328-357`, `metal/mod.rs:145-147`); asserts the object is a `CAMetalLayer` and retains it (`metal/surface.rs:46-49`) |
| `CompositionVisual(*mut c_void)` | `#[cfg(dx12)]` | an `IDCompositionVisual*` | "Its refcount will be incremented internally" (`:442`); hal `create_surface_from_visual` (`dx12/mod.rs:551-563`) |
| `SurfaceHandle(*mut c_void)` | `#[cfg(dx12)]` | a DComp surface handle | `CreateSwapChainForCompositionSurfaceHandle` via `IDXGIFactoryMedia`; lifetime not managed (`:446-455`, `dx12/mod.rs:565-582`) |
| `SwapChainPanel(*mut c_void)` | `#[cfg(dx12)]` | `ISwapChainPanelNative*` | WinUI (`dx12/mod.rs:584-600`) |

So **`CompositionVisual` is gated only by the `dx12` cfg** (the DX12 backend being compiled), not by a cargo
feature of its own. [src] (Answers the open check in the main doc §8.2.)

### 2.2 DX12 backend options (`wgpu-types/src/backend.rs`)

```rust
pub struct Dx12BackendOptions {               // :536-558
    pub shader_compiler: Dx12Compiler,
    pub presentation_system: Dx12SwapchainKind,       // DxgiFromHwnd (default) | DxgiFromVisual
    pub latency_waitable_object: Dx12UseFrameLatencyWaitableObject, // None | Wait (default) | DontWait
    pub force_shader_model: ForceShaderModelToken,
    pub agility_sdk: Option<Dx12AgilitySDK>,
}
```
- `Dx12SwapchainKind::DxgiFromVisual`: "creates a single `IDCompositionVisual` over the entire window ... If a
  user wants to manage the composition tree themselves, they should create their own device and composition,
  and pass the relevant visual down via `SurfaceTargetUnsafe::CompositionVisual`" (`:675-692`). Env var
  `WGPU_DX12_PRESENTATION_SYSTEM` = `visual` / `hwnd` (`:694-712`). [src] For tessel we need our own tree, so
  `CompositionVisual`, not `DxgiFromVisual`. (`DxgiFromVisual` creates the DComp device with
  `DCompositionCreateDevice2` and `CreateTargetForHwnd(hwnd, false)`, `dx12/dcomp.rs:39-100`.)
- `Dx12UseFrameLatencyWaitableObject::DontWait`: "Create the swapchain with the
  `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT` flag and obtain a waitable handle, but do not wait for it
  before acquiring the next swapchain image. This is useful if the application wants to wait for the waitable
  object itself." (`:862-873`); env `WGPU_DX12_USE_FRAME_LATENCY_WAITABLE_OBJECT`. [src]
- Note: this option is **per instance**, not per surface. [src]

What configure does for a visual target (`dx12/mod.rs:1471-1720`):
- flags = `FRAME_LATENCY_WAITABLE_OBJECT` always, plus `ALLOW_TEARING` when supported (`:1477-1486`);
  `FLIP_DISCARD`, `DXGI_SCALING_STRETCH`, buffer count = `maximum_frame_latency + 1` (capped 16; `:1500-1501`,
  the comment above it says "minimum of 3", but the code gives 2 for latency 1).
- First configure: `CreateSwapChainForComposition(present_queue, desc)` then `visual.SetContent(&swap_chain)`
  (`:1544-1551`, `:1619-1621`). **wgpu does not call `IDCompositionDevice::Commit` for a caller-provided
  visual** (it does only for its own `VisualFromWndHandle`, `:1593-1617`). The caller commits. [src]
- Later configures (resize) take the `ResizeBuffers` path (`:1503-1521`), so the visual's content object
  stays the same and no new `SetContent`/`Commit` is needed for content. [src]
- `SetMaximumFrameLatency(config.maximum_frame_latency)` then `GetFrameLatencyWaitableObject` unless `None`
  (`:1678-1688`). [src]

### 2.3 Wayland subsurface as a wgpu surface

- Any `wl_surface` works: `RawWindowHandle::Wayland(WaylandWindowHandle::new(surface_ptr))` +
  `RawDisplayHandle::Wayland(WaylandDisplayHandle::new(display_ptr))` → hal Vulkan
  `create_surface_from_wayland(display, surface)` (`vulkan/instance.rs:438-457`, dispatch at `:894-895`). [src]
- subduction's `wayland_wgpu` example does exactly this with a `wl_surface` it created itself
  (`examples/wayland_wgpu/src/main.rs:449-498`). [src]
- To create subsurfaces on winit's connection: take winit's `RawDisplayHandle::Wayland` pointer, wrap it with
  wayland-client's foreign-display backend, create our own `EventQueue`, bind `wl_subcompositor`,
  `wp_presentation`, `wp_viewporter`, create `wl_surface` + `wl_subsurface` with the winit toplevel's
  `wl_surface` as parent. [mem: wayland-client `Backend::from_foreign_display`; smithay-client-toolkit supports
  foreign connections] Because wgpu 30 checks the display pointer against `InstanceDescriptor::display`, use
  the same `wl_display*` for every surface. [src]
- winit has no subsurface API and no `wp_presentation` (`grep` of `winit-wayland/src` at beta.4 finds none;
  `WindowExtWayland` only exposes `xdg_toplevel()`, `winit-wayland/src/lib.rs:89-96`). [src]

### 2.4 CAMetalLayer sublayer

- Create `CAMetalLayer` sublayers ourselves (main thread), add to the view's layer, set their `frame` and
  `contentsScale` on the main thread, and pass each with `SurfaceTargetUnsafe::CoreAnimationLayer(ptr)`.
  subduction's `macos_wgpu` example creates three sublayer surfaces on one device this way
  (`examples/macos_wgpu/src/main.rs:590-640`). [src]
- Do not use `RawHandle` for sublayers: raw-window-metal would add *another* sublayer and override bounds via
  KVO (`raw-window-metal lib.rs:57-66, 120-128`). [src]
- Known issues: off-main `NSWindow` access in `acquire_texture` (§1.3); `nextDrawable` hangs ~1 s after a
  `drawableSize` change were reported to Apple, whose DTS engineer recommended `CAMetalDisplayLink`
  [doc/anec, developer.apple.com forums thread 773229]; wgpu issue #8309 (occluded windows: 1 s hang in
  `nextDrawable`) is what the occlusion check works around (`metal/surface.rs:356-359`). [src]

---

## 3. Latency knobs

### 3.1 `desired_maximum_frame_latency` per backend

Doc (`wgpu-types/src/surface.rs:952-968`): Vulkan "Number of frames in the swapchain is
`desired_maximum_frame_latency + 1`, clamped"; DX12 `SetMaximumFrameLatency(n)`; Metal
`maximumDrawableCount = n + 1`; OpenGL ignored; "DX12 + Mailbox: Limits framerate to
`desired_maximum_frame_latency * Monitor Hz`"; "Vulkan/Metal + Mailbox: If this is set to `2`, limits framerate
to `2 * Monitor Hz`". [src] wgpu-core clamps it to the surface's `maximum_frame_latency` capability range
(`wgpu-core/src/device/resource.rs:5305-5310`). [src]

| Backend | Code | Effect of `latency = 1` |
|---|---|---|
| DX12 | `SetMaximumFrameLatency(1)`, 2 buffers (`dx12/mod.rs:1500-1501, 1678-1679`) | the documented low-latency DXGI loop if the app waits on the waitable before each frame |
| Vulkan | `min_image_count = latency + 1` (`vulkan/swapchain/native.rs:214`); capability range `(minImageCount-1)..=(maxImageCount-1)` (`:171`) | **on Mesa Wayland, `minImageCount` is 3 with `wp_fifo` and 4 without** (`WSI_WL_DEFAULT_NUM_IMAGES 3`, `WSI_WL_BUMPED_NUM_IMAGES 4`, `wsi_common_wayland.c:1819-1853`), so latency 1 is silently clamped to 2 or 3. Mesa's comment: latency "can only be limited through means of presentWait" (`:1842-1851`) |
| Metal | `maximumDrawableCount = 2` (`metal/surface.rs:330`) | CA clamps to 2..3 [doc] |
| GLES | ignored | |

### 3.2 DXGI waitable handle

- Exposed: `wgpu::Surface::as_hal::<Dx12>()` → `hal::dx12::Surface::waitable_handle() -> Option<HANDLE>`
  ("only valid while the swap chain is alive", `dx12/mod.rs:654-658`) and `swap_chain() ->
  Option<IDXGISwapChain3>` (`:650-652`). With `DontWait`, `acquire_texture` skips the wait (`:1734-1741`). [src]
- So the compositor thread can put every window's waitable into one `WaitForMultipleObjectsEx` together with
  its own commit event and timers, and call `get_current_texture` only for windows whose waitable fired. No
  patch needed. [src+inf]
- Frame statistics: `IDXGISwapChain::GetFrameStatistics` / `GetLastPresentCount` are callable on the raw
  `IDXGISwapChain3` from `swap_chain()`. wgpu does not call them. [src+inf] DComp's compositor clock and
  `GetFrameStatistics` are on our own DComp device. [doc, windows.md §2.6]

### 3.3 Presentation timestamps and timing extensions

- wgpu exposes `Adapter::get_presentation_timestamp()` (CPU clock correlation only; Vulkan uses
  `CLOCK_MONOTONIC` on unix, `vulkan/adapter.rs:3217-3230`). [src] #2869: "None of this (outside of getting
  cpu-side presentation timestamps) is implemented yet" (cwfitzgerald, 2023-08-15). [iss]
- Vulkan `VK_GOOGLE_display_timing`: feature `Features::VULKAN_GOOGLE_DISPLAY_TIMING`
  (`vulkan/adapter.rs:1049-1050, 1428-1430`); hal `Surface::set_next_present_time(vk::PresentTimeGOOGLE)` pushes
  `VkPresentTimesInfoGOOGLE` on the next present (`vulkan/mod.rs:250-277`, `swapchain/native.rs:596-611`). Reading
  results: "`wgpu-hal` does *not* provide a way to use that API - you should manually access this through
  [`ash`]" (`vulkan/mod.rs:253-254`), using `raw_native_swapchain()` (`:236-248`). [src]
- **Trunk (unreleased, after v30.0.1)** adds generic hooks (CHANGELOG trunk "Unreleased"):
  - `wgpu_hal::vulkan::Surface::set_next_present_chain(*mut c_void)`: caller-provided `pNext` chain on the
    next `VkPresentInfoKHR` (#9847; trunk `vulkan/mod.rs:292-327`, spliced at `swapchain/native.rs:653-657`).
  - `set_next_swapchain_create_chain` and `vulkan::Queue::set_next_submit_chain` (#10095), plus swapchain
    create flags (#10304, per #2869 comments).
  - With `Adapter::open_with_callback` (exists in v30.0.1, `vulkan/adapter.rs:2995-3015`) to enable device
    extensions, this makes `VK_KHR_present_id2`/`VK_KHR_present_wait2`, `VK_EXT_present_timing` and
    **`VK_KHR_incremental_present`** usable without patching wgpu. jameysharp reported working
    `VK_EXT_present_timing` on Mesa/Intel after #10304 (2026-10-08). [src+iss]
  - Mesa's Wayland WSI implements present_wait2, present_id2 and EXT_present_timing
    (`wsi_common_wayland.c:2090-2162`) and uses `wp_presentation` internally (`:1514-1523`). [src]
- DX12: no `Present1`, no frame statistics, no timed present in wgpu (`dx12/mod.rs:1823-1845`). [src]
- Metal: `MTLDrawable.presentedTime` / `addPresentedHandler` on the drawable [doc]; wgpu-hal keeps the drawable
  inside `SurfaceTexture`, reachable via `SurfaceTexture::as_hal` [inf, not checked]. frameclock_apple converts
  `presentedTime` into feedback (`frameclock_apple/src/lib.rs:138-151`). [src]

### 3.4 Present modes and how `get_current_texture` blocks

| Backend | Modes | Acquire blocking |
|---|---|---|
| DX12 | Immediate → `Present(0, ALLOW_TEARING)`, Mailbox → `Present(0, 0)`, Fifo → `Present(1, 0)` (`dx12/mod.rs:1830-1836`) | `Wait`: `WaitForSingleObject(waitable, timeout)` (`:1423-1447`, `:1734-1741`); `DontWait`: never blocks in wgpu; Present itself may block if you present beyond the latency |
| Vulkan | per driver (`conv::map_vk_present_mode`); Mailbox on Wayland, NVIDIA; FifoRelaxed on AMD (`wgpu-types/src/surface.rs` PresentMode docs) | waits on the fence of the last submission that used this acquire semaphore (`swapchain/native.rs:445-467`), then `vkAcquireNextImageKHR(timeout)`; on Windows also an extra acquire fence (`:260-275`, `:501-514`, #8310/#8354). **`NOT_READY` is mapped to `Outdated`** (`:490-492`), so a zero-timeout poll would be misreported (patch note) |
| Metal | Fifo (`displaySyncEnabled = true`) or Immediate only; anything else `unreachable!` (`metal/surface.rs:261-265`) | `nextDrawable` with no timeout (§1.3) |
| GLES | | `present` blits a renderbuffer to the default framebuffer and `swap_buffers` (`gles/egl.rs` v30 `:1112`, trunk `:1137-1210`) |

Implication for one compositor thread [inf]:
- DX12: `DontWait` + our own multi-object wait. Solved.
- Vulkan/Wayland: only acquire when we know an image is free: after the previous frame's present-wait /
  frame callback / `wp_presentation` feedback, never on a hidden window (Qt's 100 ms frame-callback timeout
  marks it hidden, `gtk-qt-wayland.md`). Without present_wait the 1000 ms wgpu timeout is the backstop.
- Metal: acquire only in that layer's display-link callback, or when the previous drawable was presented;
  never for an occluded window (the hal returns `Occluded` itself).
- A wgpu patch for a caller-chosen acquire timeout (and `NOT_READY → Timeout`) removes the 1 s worst case.

---

## 4. Present with damage

### 4.1 Where it plumbs through (v30.0.1)

| Layer | Place | Change |
|---|---|---|
| wgpu | `Queue::present(SurfaceTexture)` (`wgpu/src/api/queue.rs:367-377`), dispatch traits, `backend/wgpu_core.rs` | add `present_with_damage(texture, &[DamageRect])` |
| wgpu-types | new `DamageRect { x, y, width, height }` | as in Zed's PR (`wgpu-types/src/lib.rs` +19 lines) |
| wgpu-core | `Queue::present` (`present.rs:303-376`) → `raw_queue.present(raw_surface, raw)` | pass `&[DamageRect]` |
| wgpu-hal trait | `Queue::present(&self, surface, texture)` (`lib.rs:1319-1323`), dyn queue wrapper | add a `damage: &[DamageRect]` param |
| Vulkan | `NativeSwapchain::present` builds `vk::PresentInfoKHR` and already pushes optional pNext (`swapchain/native.rs:591-611`) | push `VkPresentRegionsKHR { pRegions: [VkPresentRegionKHR { rectangles: [VkRectLayerKHR] }] }` when `VK_KHR_incremental_present` is enabled (adapter: advertise + enable the extension, ~6 lines) |
| DX12 | `Queue::present` → `sc.raw.Present(interval, flags)` (`dx12/mod.rs:1823-1845`) | `IDXGISwapChain1::Present1(interval, flags, &DXGI_PRESENT_PARAMETERS { DirtyRectsCount, pDirtyRects, .. })` (the `IDXGISwapChain3` already derives from `IDXGISwapChain1`) |
| GLES/EGL | `swap_buffers` (`gles/egl.rs`) | `eglSwapBuffersWithDamageKHR/EXT`, Y flipped (Zed's PR: +107/−8 lines) |
| Metal | `metal/mod.rs:802-826` | ignore (no CAMetalLayer damage API; `gpu-2d-text.md` §3.3) |

Buffer-age / preservation requirements:
- **DXGI**: "An application must update every single pixel in each rectangle that it reports ... must not
  update any pixel outside of the dirty rectangles" (`DXGI_PRESENT_PARAMETERS`, updated 2024-02-22). In the flip
  model "the runtime copies only the differences between updated areas of the previous frame and updated
  areas of the current frame onto the current back buffer"; the app must copy or re-render the intersection of
  the previous and current dirty rects, and with N buffers track all of them ("Flip model, dirty rectangles,
  scrolled areas", updated 2025-03-11). Scroll rects are flip-model only; neither works with MSAA swap chains.
  [doc] wgpu creates `FLIP_DISCARD` (`dx12/mod.rs:1538`); the Microsoft page describes
  `FLIP_SEQUENTIAL`. Whether the runtime's copy-forward applies to `FLIP_DISCARD` I did not confirm. **Verify
  on hardware; the patch may need a `FLIP_SEQUENTIAL` option.** [doc gap]
- **Vulkan**: no buffer-age query; track damage per swapchain image as GTK does (`gtk-qt-wayland.md` §1.2).
  Mesa turns the regions into `wl_surface_damage_buffer` calls (`wsi_common_wayland.c:3375-3388`). [src]
- **Simplest correct scheme for tessel** [inf]: render damage into a persistent per-surface target
  (we need it anyway for retained layers), then copy *damage ∪ damage of the last (images−1) frames* into the
  acquired image and present with the current frame's damage. No reliance on driver copy-forward; works on all
  three. Zed's PR #59441 also renders into a persistent target and copies (`gpu-2d-text.md` §3.3).

### 4.2 Status upstream

- #2869 "Extended Presentation API Investigation" (open since 2022-07-10). 2026-08-14, cwfitzgerald: "Damage
  tracking as a dedicated PR would be accepted!" and, asked about Zed's fork's `present_with_damage`, "I think
  for V1 that's fine!". 2026-08-28: points to `open_with_callback` and #9847 for extensions. [iss]
- zed-industries/wgpu PR #2 "Add damage-aware presentation support" (loadingalias, opened 2026-04-14, rebased
  on Zed's v29 branch 2026-06-29, **closed unmerged 2026-07-02** as Zed moved off its fork): Vulkan via
  `VK_KHR_incremental_present`, EGL via `EGL_EXT_swap_buffers_with_damage`; **DX12, Metal, web, noop ignore
  damage** (no `Present1`). [iss] Its diff is **~280 added / ~15 removed lines in 22 files** (my count:
  egl.rs +107, vulkan native.rs +45, wgpu-core present.rs +15/−2, backend/wgpu_core.rs +16, surface_texture.rs
  +14, wgpu-types +19, the rest 1–10 each). [src: diff downloaded from patch-diff.githubusercontent.com]
- Estimate for our upstream PR on v30/trunk [inf]: port of that diff to `Queue::present` (~300 lines) +
  DX12 `Present1` (~40 lines) + tests/docs → **~350–450 lines**. On trunk, the Vulkan part can instead be done
  *outside* wgpu with `set_next_present_chain(&VkPresentRegionsKHR)`; DX12 still needs a patch.
- **DXGI swapchain on Vulkan for Windows**: issue #8354 (open); PR #8388 "Implement DXGI Swapchain on Vulkan"
  (cwfitzgerald, opened 2025-10-21, ready for review 2026-07-08, **still open**; andyleiserson requested
  changes 2026-10-07: undocumented downgrade to native, possible interop-queue deadlock on failed submit,
  queue ownership transfer, code size, a `transmute`). Shape: new `VulkanSwapchainKind` backend option, env
  `WGPU_VULKAN_SWAPCHAIN_KIND`, a `DxgiFromHwnd` variant, D3D12 interop with dummy command buffers, "negligible
  performance impact (something on the order of 100us)", default off. [iss] The trunk hal already prepares for
  it (`raw_native_swapchain` "Returns None ... if the swapchain is a DXGI swapchain", `vulkan/mod.rs:236-248`).
  [src] Irrelevant to tessel as long as we force `Backends::DX12` on Windows (`windows.md` §3.2).

---

## 5. Sharing one Device

- Multiple surfaces + offscreen targets on one `Device`/`Queue` from one thread is the normal wgpu model;
  subduction's examples configure three layer surfaces on one device (`macos_wgpu/src/main.rs:619-640`). [src]
- Cost per extra surface per frame [src+inf]:
  - one `get_current_texture` (wgpu-core creates a `Texture` wrapper and a clear view each time,
    `present.rs:181-236`), one `Queue::present` (takes `command_indices.write()`, `:348-356`);
  - Metal: each present creates and commits an extra internal `MTLCommandBuffer` (`metal/mod.rs:806-817`);
  - Vulkan: one `vkQueuePresentKHR` per surface (wgpu never batches swapchains in one present call);
  - DX12: `Present` on the dedicated present queue.
  - **Do one `queue.submit` per compositor frame** containing all surfaces' passes, then N presents. A single
    submit can use several surface textures (hal `submit(..., surface_textures: &[&Texture], ...)`,
    `dx12/mod.rs:1774-1777`). [src]
- Only one texture may be acquired per surface at a time; the compositor holds each acquired texture only
  between encode and present. [src]
- **egui island on our device** (egui-wgpu 0.37.0 `renderer.rs`): `Renderer::new(&Device, output_color_format,
  RendererOptions { msaa_samples, depth_stencil_format, dithering, predictable_texture_filtering })`
  (`:175-215`, `:273-277`); per frame `update_texture` (`:637`), `update_buffers(&Device, &Queue, &mut
  CommandEncoder, &[ClippedPrimitive], &ScreenDescriptor) -> Vec<CommandBuffer>` (`:964-971`), `render(&mut
  RenderPass<'static>, &[ClippedPrimitive], &ScreenDescriptor)` (`:506-511`). The render pass can target any
  of our textures (an island layer texture). Our textures go into egui with
  `register_native_texture(&Device, &TextureView, FilterMode) -> TextureId` ("Texture must have the texture
  format `Rgba8Unorm`", `:805-828`) or `register_native_texture_with_sampler_options` (`:859`). [src]
  Threading [inf]: run `egui::Context::run` on the UI thread (it is our UI), ship `ClippedPrimitive`s +
  `TexturesDelta` in the Commit, and keep the `egui_wgpu::Renderer` on the compositor thread. Paint callbacks
  (`CallbackTrait`) must then be `Send + Sync`; egui-wgpu already requires that [mem, not re-checked].
- External/imported textures: wgpu 30 has `ExternalTexture` (`wgpu/src/api/external_texture.rs`, the WebGPU
  video-frame type) and hal-level imports via `Device::create_texture_from_hal`; trunk adds
  `dx12::Texture::with_plane_slice` for NV12 imports (#9551). [src] Not needed for one device; relevant later for
  video or a separate-process renderer.

---

## 6. forest-rs `frameclock` and `subduction`

- **Who/where**: `github.com/forest-rs/frameclock` (`f12fbc0`, 2026-10-16) and `github.com/forest-rs/subduction`
  (`bfa27d0`, 2026-10-15). AUTHORS: Bruce Mitchener Jr., Muhammad Ragib Hasin, no-materials, Aaron Muir
  Hamilton. Version `0.0.1`, edition 2024, Apache-2.0 OR MIT. [src] **Not on crates.io**: `frameclock` → "crate
  does not exist"; the crates.io `subduction_core` is an unrelated Ink & Switch sync protocol (0.20.0). [src:
  crates.io API, 2026-10-25] The frameclock crates exist in both repos (subduction carries a copy and
  re-exports for migration, `subduction_core/README.md`). [src]
- **frameclock** (`no_std`): "turning platform display callbacks, display timing, and frame demand into
  explicit frame plans ... decides when frame work should begin, which time a frame should sample, what
  deadline it has, and how scheduler policy should adapt after presentation feedback. The crate intentionally
  does not own windows, event loops, layer trees, renderers, swapchains" (`frameclock/README.md`). API:
  `FrameDriver::new(SchedulerConfig)`, `request(FrameDemand::{INPUT, CONTINUOUS_INPUT, ANIMATION,
  BACKGROUND})`, `begin_frame(FrameOpportunity) -> FrameBegin { result: Ready(ActiveFrame) | Wait(FrameWait::
  {NextTick, Until(t)}) | Expired | Idle }`, `submit_frame(frame, FrameSubmission)`, `FramePlan::{frame_start,
  sample_time, target_present}`, presets `predictive()/estimated()/pacing_only()`, a learned wake-latency
  ("The driver learns how late the host's wakes come and starts later frames that much earlier"). Adapters:
  `frameclock_apple` (CADisplayLink, CVDisplayLink, `CaMetalDisplayLink`), `_wayland` (`wp_presentation`
  clock, frame callbacks), `_windows` (QPC), `_x11` (Present), `_web`; plus `mediaclock` and `_footfall`
  tracing. Example `winit_paced_redraw` drives `ControlFlow::WaitUntil`. [src]
  - Caveat: `frameclock_apple`'s CADisplayLink/CVDisplayLink adapters deliver ticks **on the main thread**
    (`ca_display_link.rs:29, 164-179`; `cv_display_link.rs:57-113`, "Each tick is ... queued for the main
    thread"); only `metal_display_link.rs` uses the current run loop (`:381`). [src]
- **subduction**: retained layer tree + presenter contract (`subduction_core`: "retained layer tree, dirty
  evaluation, transform math, output policy, tracing hooks, and backend presenter contract"), backends for
  DComp (`subduction_backend_windows`: "manages compositing only (transforms, opacity, clips, visibility,
  topology). Visuals are property-only ... Applications attach GPU content by obtaining the visual via
  `DCompPresenter::visual_for` and calling `SetContent`"), CALayer/CAMetalLayer (`_apple`; CALayer mutations
  "Must be called on the main thread", `calayer.rs:192`), Wayland (`_wayland`: frame-callback ticks, optional
  `wp_presentation`, owned-queue or embedded-state modes), X11, Android, web, and a wgpu fallback compositor
  (`_wgpu`, on **wgpu 29**: workspace `wgpu = "29"`). [src]
- **Adoptable?** [inf]
  - `frameclock` core: yes as a dependency candidate for the compositor's pacing logic (no_std, no
    threading assumptions, does exactly §5.3's "latch − margin − estimates" job). Pin a git sha; it is
    0.0.1, unpublished, two weeks old. Its Apple adapters assume main-thread ticks, so we would write the
    compositor-thread adapter (CAMetalDisplayLink on the compositor's run loop) or feed it `FrameTick`s
    ourselves. Evaluate against our §5.3 before committing.
  - `subduction`: read as a reference for the per-OS layer plumbing (DComp visual management, Wayland
    subsurface/presentation code, CALayer transactions), not as a dependency: its tree overlaps our own
    property trees, the Apple backend is main-thread-only (we want compositor-thread commits), and it is on
    wgpu 29.

---

## 7. winit integration of our frame clock

- **Proxy wake-up latency** [src beta.4]: Windows `PostMessageW(target_window, USER_EVENT_MSG_ID)`
  (`winit-win32/src/event_loop.rs:934-936`) wakes `MsgWaitForMultipleObjectsEx(... QS_ALLINPUT,
  MWMO_INPUTAVAILABLE)` (`:859`); Wayland `calloop::ping` (eventfd) (`winit-wayland/src/event_loop/proxy.rs:15-17`);
  macOS a run-loop source [mem]. All are sub-millisecond kernel wakes [inf]; the latency that matters is how long
  the UI thread is busy, not the proxy.
- **`ControlFlow::WaitUntil` precision** [src]:
  - Windows: a `CREATE_WAITABLE_TIMER_HIGH_RESOLUTION` timer in the message wait (`event_loop.rs:755-761`).
  - Wayland/X11: calloop 0.14.3 → polling 3.11 epoll backend, which arms a **timerfd** for the timeout
    (`polling src/epoll.rs:177-222`), so ns-resolution deadlines subject to timer slack (default 50 µs) [mem
    for slack].
  - macOS: a `CFRunLoopTimer` whose next fire date is set (`winit-appkit/src/observer.rs:11-60`); CF timers can
    be coalesced or delayed by App Nap [mem].
  - None of these is a frame clock. In our design the **compositor thread** owns pacing (waitables, display
    link, `wp_presentation`, frame callbacks) and wakes the UI thread with a proxy when a UI frame should
    start; the UI thread uses `WaitUntil` only for JS timers. [inf]
- **`pre_present_notify`**: "Android / iOS / X11 / Web / Windows / macOS / Orbital: Unsupported. Wayland:
  Schedules a frame callback to throttle `WindowEvent::RedrawRequested`" (`winit-core/src/window.rs:767-792`;
  0.30.13 `src/window.rs:636`). [src] We don't need it: winit's RedrawRequested is not our frame trigger. Since
  winit adds that frame callback to *its* wl_surface state, which our compositor's commit would latch, calling
  it from the UI thread while the compositor presents is racy; leave it unused. [inf]
- **`request_redraw`** on Windows is `RedrawWindow` → `WM_PAINT` → `RedrawRequested` "in sync with any WM_PAINT
  messages" (`winit-core/src/window.rs:732-757`). Keep handling `RedrawRequested` (an OS expose) by asking the
  compositor for a frame; never render there. [src+inf]
- **Running the event loop while another thread presents** [inf]: winit just needs its thread back.
  `ApplicationHandler::proxy_wake_up` (0.31; `user_event` in 0.30) drains a lock-free queue of compositor →
  UI messages (frame-start, presented-feedback for rAF timestamps, resize-done). The resize handshake is the
  only place the UI thread blocks on the compositor, with a bounded wait (≤ ~1 refresh) inside the
  `SurfaceResized`/`Resized` handler.
- **winit 0.31 changes that matter** (`winit/src/changelog/v0.31.md`):
  - `ApplicationHandler::can_create_surfaces()` split from `resumed` and "should ... be the only place to
    create render surfaces" (`:229-235`); `run_app` takes `impl ApplicationHandler` (`:238-239`); `ActiveEventLoop`
    is `dyn` (`:217`).
  - `EventLoopProxy::send_event` → `wake_up()`, `user_event` → `proxy_wake_up`; data goes through your own
    channel (`:220-227`). Fits our design.
  - `Resized` → `SurfaceResized`, `inner_size` → `surface_size` (`:251-267`).
  - Pointer overhaul: `PointerMoved/PointerEntered/PointerLeft/PointerButton` with `PointerKind`,
    `PointerSource`, `ButtonSource`, `FingerId`, `primary`, `position`; `Touch` folded in; pen on Wayland,
    Windows, Web (`:186-188, 210, 270-291`). Still no coalesced/history samples (no `GetPointerInfoHistory`),
    so §7.3's pointer history stays ours.
  - IME: `Window::request_ime_update` "to atomically apply set of IME changes", `Ime::DeleteSurrounding`, more
    `ImePurpose`, `ImeHints` (`:206-209`); `set_ime_allowed/cursor_area/purpose` deprecated (`:333`).
  - `Window: Send + Sync` trait object (§1.2); `OwnedDisplayHandle: Send + Sync` (`:47`).
  - Still absent at beta.4: frame clock, presentation feedback, subsurfaces (`gtk-qt-wayland.md` §3.7 confirms
    for master `0c73728`).

---

## 8. Recommended design (summary of the above, [inf] unless cited)

### 8.1 Common

1. UI thread creates winit windows and, in `can_create_surfaces`, every OS layer and every `wgpu::Surface`
   (main thread required on macOS, §1.1). It moves the `Surface`s to the compositor thread with the first
   Commit. The compositor never calls winit.
2. One `Instance` (with `new_with_display_handle(owned_display_handle)`), one `Device`/`Queue`, owned by the
   compositor. One `submit` per compositor frame, then one present per damaged surface; skip present when a
   surface has no damage.
3. The compositor's single blocking wait multiplexes: DXGI waitables (DX12 `DontWait`), Metal display-link
   callbacks (compositor thread runs a `CFRunLoop`), Wayland frame callbacks + `wp_presentation` on our own
   event queue (or Vulkan present-wait), the Commit channel and animation deadlines. It calls
   `get_current_texture` only for surfaces known to have a free image.
4. Pacing: our own (or frameclock's) scheduler on the compositor thread; it wakes the UI thread through the
   winit proxy at `latch − margin − est`.
5. Resize is the one synchronous handshake: UI → `Resize{window, size, scale, seq}` → compositor reconfigures,
   renders, presents (or hands the texture back, macOS) → `ResizeDone{seq}`; the UI waits ≤ ~1 refresh, then
   returns to the OS.

### 8.2 Per OS

- **Windows**: `Backends::DX12`; `Dx12BackendOptions { latency_waitable_object: DontWait, .. }`; our own DComp
  device (`DCompositionCreateDevice2`/3) and target owned by the compositor (DComp is thread-safe); one visual
  per window (UI) plus child visuals per viewport/canvas, each with
  `SurfaceTargetUnsafe::CompositionVisual`; after the first `configure`, the compositor calls `Commit()`
  (wgpu does not, §2.2). Latency 1; waitables from `as_hal::<Dx12>().waitable_handle()`; stats from
  `swap_chain()`. Resize: in the size message the UI thread waits for `ResizeDone` (compositor:
  `configure` → `ResizeBuffers`, render, `Present`, DComp `Commit` for changed clips/offsets). Try
  `WM_NCCALCSIZE` timing if edges flicker [anec].
- **macOS**: one `CAMetalLayer` sublayer per window plus sublayers per viewport, created and laid out
  (`frame`, `contentsScale`) by the UI thread; `CoreAnimationLayer` surfaces; compositor wraps any layer
  property change it makes in an explicit `CATransaction`/`NSAnimationContext` group. Normal frames: compositor
  acquires/presents (`presentsWithTransaction = NO`). Live resize (Firefox/GPUI model): UI thread sets
  `presentsWithTransaction = YES` *before* the compositor's next acquire, sends `Resize`, compositor configures
  (new `drawableSize`), renders, submits and **returns the `SurfaceTexture`**; the UI thread calls
  `queue.present(texture)` itself, which runs `waitUntilScheduled` + `drawable.present()` on the main thread in
  the resize transaction (`metal/mod.rs:814-822`). Compositor-side commits stay suspended until the main thread
  returns to its run loop, then `presentsWithTransaction = NO`. Pacing: `CAMetalDisplayLink` on the
  compositor's run loop once wgpu can accept its drawable (patch); until then a display link for timing plus
  `nextDrawable` through wgpu.
- **Wayland**: Vulkan; viewports and canvases as `wl_subsurface`s of winit's toplevel, created on our own event
  queue over winit's `wl_display`, wgpu surfaces from their `wl_surface`; desync normally, **sync during
  resize** so their commits latch with the parent. The toplevel's buffer is presented by the compositor
  thread. Resize race with winit's internal `set_window_geometry`/`set_destination` (§1.5): on `SurfaceResized`
  the UI thread publishes the new size; the compositor drops any frame whose size differs and renders the new
  size before its next present; UI waits for `ResizeDone`. A residual one-frame race remains until our own
  Wayland backend holds a per-surface lock around all `wl_surface` state (Firefox `WaylandSurfaceLock`).
  Latency: Mesa clamps latency 1 to 2–3 images, so bound it with present-wait (trunk chain API) or our own
  frame-callback gate before acquire.
- **X11**: winit + Vulkan, render-to-texture fallback for viewports, full presents; accept resize artefacts.

### 8.3 wgpu patches (and what needs none)

| # | Patch | Backends | Size (est.) | Needed when |
|---|---|---|---|---|
| P1 | `Queue::present_with_damage(texture, &[DamageRect])`: Vulkan `VkPresentRegionsKHR`, DX12 `Present1` dirty rects (maybe a `FLIP_SEQUENTIAL` option), EGL swap-with-damage, Metal no-op | all | ~350–450 lines (Zed's v29 version: ~280 lines, 22 files, no DX12) | M-damage milestone; upstreamable (#2869 maintainer: "would be accepted") |
| P2 | Caller-chosen acquire timeout (`get_current_texture_with_timeout(Duration)`) and Vulkan `NOT_READY` → `Timeout` | core + vulkan | ~30–60 lines | before multi-window on one compositor thread ships |
| P3 | Metal: acquire from a caller-supplied `CAMetalDrawable` (`CAMetalDisplayLink` update) | metal hal + small wgpu entry | ~40–80 lines | when we pace macOS with CAMetalDisplayLink |
| P4 | Metal: don't message `NSWindow`/`NSView` off the main thread in `acquire_texture` (make the occlusion check opt-out or let the app report occlusion) | metal hal | ~15–30 lines | before shipping macOS compositor thread |
| — | Vulkan present-timing, present-wait, incremental present via pNext chains | — | none on trunk (#9847, #10095, #10304); on 30.0.1 a backport (~150 lines) or wait for v31 | Wayland pacing |
| — | DXGI waitable + frame stats | — | none (`waitable_handle()`, `swap_chain()`) | |
| — | DComp visuals | — | none (`CompositionVisual`, `dx12` cfg only) | |
| — | DXGI-on-Vulkan (#8388) | — | not needed (force DX12) | |

Under cadatomic's fork rules, P1–P4 are one fork set on a v30.0.x tag (wgpu plus egui-wgpu's pin follows),
registered in `forks.toml`, opened upstream as separate PRs.

---

## 9. Open questions to verify on hardware

1. DXGI `Present1` dirty rects with `FLIP_DISCARD`: does the runtime still copy forward? (doc covers
   `FLIP_SEQUENTIAL`).
2. Does a composition swap chain + bounded UI-thread wait in `WM_SIZE`/`WM_NCCALCSIZE` give glitch-free
   resizes from all edges? (anecdote says top/left still gap).
3. macOS: is setting `drawableSize` from the compositor thread while the main thread changes `frame` safe in
   practice (no doc found)?
4. Mesa/NVIDIA Wayland: present-wait2 via trunk's chain API end to end through wgpu.
5. `CAMetalDisplayLink` on a non-main run loop with layers whose geometry the main thread changes.
