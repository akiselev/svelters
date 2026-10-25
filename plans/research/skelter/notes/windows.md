# How Windows does low-latency, retained UI: research notes for cadatomic's own framework

Written 2026-10-25 for the cadatomic GUI lane (replacing egui 0.37 with our own retained framework and
renderer; svelters/QuickJS-NG renders panel contents into a Rust-owned scene).

Conventions:
- **[doc]** = stated by a primary source (Microsoft Learn, DirectX blog, vendor source code).
- **[src]** = read directly in source code at the named commit.
- **[inf]** = my inference from cited facts; treat as a hypothesis to measure.
- **[anec]** = forum/anecdotal report.
- Each Learn page is cited with its `ms.date` / `updated_at` as served on 2026-10-25.

Source checkouts read (shallow clones, 2026-10-25):
- wgpu `gfx-rs/wgpu` @ `100c142` (2026-10-25, workspace v30.0.x; changelog top: v30.0.1 2026-08-21).
- egui `emilk/egui` @ `bf5ac0d` (2026-10-21) and tag `0.37.0` (raw files); egui 0.37 depends on `wgpu = "30.0"`, `winit = "0.30.13"`.
- winit `rust-windowing/winit` master @ `0c73728` (2026-10-24) and tag `v0.30.13` (`src/platform_impl/windows/event_loop.rs`).
- WinUI 3 `microsoft/microsoft-ui-xaml` @ `f980ab2` (2026-10-23) — the XAML core (`dxaml/xcp`) is public (MIT).
- WPF `dotnet/wpf` @ `c4610da` (2026-10-22).

---

## 0. Executive digest

1. **Present path is where egui/eframe loses the frames.** Windows' best path for a windowed app is a flip-model
   swap chain + frame-latency waitable object + max latency 1, which gets "down to 1 frame of latency" when the
   swap chain is independently flipped (DirectFlip/MPO), and degrades "gracefully" to the composed minimum
   otherwise [doc, DXGI flip-model blog 2018]. Composed (DWM) adds roughly one more vblank [inf from DWM loop docs].
2. **wgpu on Windows picks Vulkan before DX12** when both expose the same discrete GPU [src: wgpu-core adds Vulkan
   before Dx12 and sorts stably by device type]. NVIDIA's Vulkan driver on Windows routes FIFO through a
   "layered on DXGI swapchain" path by default ("Auto"), which wgpu maintainers found has "vastly different timing
   properties" and bad pacing [doc: wgpu CHANGELOG v27.0.4, issues #8310/#8354]. The native NVIDIA Vulkan path is
   reported by PresentMon as "Composed: Copy with GPU GDI" (blt into the redirection surface) [anec, Khronos forum
   2025-08]. Either way you are not on the clean DXGI waitable path. This is a likely cause of the measured ~3
   frames [inf].
3. **wgpu's DX12 backend already does the right DXGI things**: FLIP_DISCARD, FRAME_LATENCY_WAITABLE_OBJECT,
   ALLOW_TEARING flag, `SetMaximumFrameLatency(desired_maximum_frame_latency)`, waits on the waitable in
   `get_current_texture` [src]. It uses plain `Present` (no `Present1` dirty/scroll rects) [src], and can host its
   swap chain on a DirectComposition visual (`Dx12SwapchainKind::DxgiFromVisual` or your own visual via
   `SurfaceTargetUnsafe::CompositionVisual`) since v27 [src/doc].
4. **winit's Windows loop renders on the UI thread from `WM_PAINT`** (`RedrawWindow(RDW_INTERNALPAINT)`), waits in
   `MsgWaitForMultipleObjectsEx(QS_ALLINPUT, MWMO_INPUTAVAILABLE)` with a high-resolution waitable timer [src].
   So with eframe the swap-chain wait (and any JS/layout) blocks the thread that owns the input queue [inf].
5. **Every Microsoft UI stack since WPF separates a UI thread from a render/composition thread** and keeps a
   retained tree on the far side: WPF (milcore "UCE" render thread, batched channel), XAML/WinUI (UI thread does
   layout + "RenderWalk" + Commit; a high-priority render thread ticks animations/manipulations at vsync),
   DirectComposition / Windows.UI.Composition (compositor thread/process applies batches atomically per frame,
   runs independent animations, InteractionTracker, DirectManipulation) [doc/src].
6. **Scroll/pan that stays smooth while the UI thread is busy** = input routed to the compositor thread
   (DirectManipulation / InteractionTracker), output is a transform on a retained visual, content is
   pre-rendered beyond the viewport (virtual surfaces / realized items) [doc].
7. **Idle CPU ≈ 0** comes from: generated-on-demand messages (WM_PAINT/WM_MOUSEMOVE/WM_TIMER are flags, not
   queue entries), a compositor that sleeps when nothing changed, waiting on the waitable object or compositor
   clock rather than spinning, and never registering per-frame callbacks when idle [doc].

---

## 1. Win32 input and the message loop

### 1.1 Message retrieval order and generated messages
- `GetMessage` with no filter processes, in order: **sent messages → posted messages → input (hardware) messages
  and system internal events → sent messages again → `WM_PAINT` → `WM_TIMER`**. "To retrieve input messages
  before posted messages, use the wMsgFilterMin and wMsgFilterMax parameters." `GetMessage` does not remove
  `WM_PAINT`; it stays until processed (validated). [doc] <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmessage> (ms.date 2022-08-02)
- `WM_PAINT`, `WM_TIMER`, `WM_MOUSEMOVE` are **generated on demand**: `InvalidateRect` only accumulates the update
  region and sets a "painting to be done" flag; when the queue is otherwise empty, the window manager synthesizes
  one `WM_PAINT`. 100,000 invalidations can produce anywhere from 1 to 100,000 `WM_PAINT`s depending on the race.
  `WM_MOUSEMOVE` is generated "when the mouse has moved since the thread last retrieved a mouse message" — i.e.
  moves coalesce into the latest position. [doc] Raymond Chen, "Paint messages will come in as fast as you let them",
  2011-12-19 <https://devblogs.microsoft.com/oldnewthing/20111219-00/?p=8863>
- Generation can even happen with a non-empty queue if message filters exclude what's there; the generated
  message gets the current time stamp. Windows can also set the "mouse moved" flag itself (cursor recalculation),
  producing spurious `WM_MOUSEMOVE` with an unchanged position — dedupe by position. [doc] Raymond Chen,
  2013-05-23 <https://devblogs.microsoft.com/oldnewthing/20130523-00/?p=4273> and 2003-10-01
  <https://devblogs.microsoft.com/oldnewthing/20031001-00/?p=42343>
- **Lesson [inf]:** Win32's own design is "paint is the lowest-priority, coalesced, pull-based event". A
  framework should treat "redraw needed" as a dirty flag consumed once per frame, and pointer motion as
  "latest position + history", never as a queue to replay frame-by-frame.

### 1.2 Recovering coalesced mouse samples
- `GetMouseMovePointsEx`: the system retains **the last 64 mouse coordinates with time stamps**; pass the
  `WM_MOUSEMOVE` point (in screen coords) to fetch the history up to it. `GMMP_USE_HIGH_RESOLUTION_POINTS`
  returns 0..65535 absolute coordinates (tablet resolution). Quirks with negative multi-monitor coordinates
  (documented fix-up code). Returns points dispatched to other threads too. [doc]
  <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getmousemovepointsex> (2018-12-05)
- `WM_POINTER*` (Windows 8+): "If the application does not process these messages as fast as they are
  generated, some moves may be coalesced." `GetPointerInfoHistory` returns every coalesced input for the current
  message, most-recent first; history is only valid until the thread retrieves its next message. [doc]
  <https://learn.microsoft.com/en-us/windows/win32/inputmsg/wm-pointerupdate> (2024-05-06),
  <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-getpointerinfohistory>
- `EnableMouseInPointer(TRUE)` routes the mouse through `WM_POINTER`; it can be called **once per process**;
  desktop apps default to disabled (Store apps and mshtml consumers enabled). [doc]
  <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-enablemouseinpointer>
- **Lesson [inf]:** for ink/drag/lasso precision (schematic wires, PCB routing, sketch strokes) use the
  coalesced history (pointer history or GetMouseMovePointsEx); for hit-testing/hover use only the latest sample.

### 1.3 Raw input and high-resolution mice
- `WM_MOUSEMOVE` is limited to screen pixels (sub-pixel motion yields no message) but has pointer ballistics
  applied, so it is "the preferred option for pointer control". `WM_INPUT` (raw input) gives unfiltered HID
  deltas; DirectInput "internally creates a second thread to read WM_INPUT data" and is discouraged. [doc]
  <https://learn.microsoft.com/en-us/windows/win32/dxtecharts/taking-advantage-of-high-dpi-mouse-movement> (2018-05-31, updated 2025-03-11)
- Raw input: register via `RegisterRawInputDevices`; `RIDEV_INPUTSINK` for background delivery. **Buffered read**
  `GetRawInputBuffer` "is useful for high-frequency devices such as mice at 1000Hz, where multiple events may
  accumulate between message loop iterations"; `GetMessage` removes the current `WM_INPUT` first, so read it
  with `GetRawInputData` and then drain with `GetRawInputBuffer`. [doc]
  <https://learn.microsoft.com/en-us/windows/win32/inputdev/about-raw-input> (ms.date 2025-07-14, updated 2026-03-18)
- Windows Insider build 25272 throttled/merged raw input for **background** raw-input listeners because many
  background apps subscribing to raw mouse input caused stutter with 1000 Hz mice. [anec — forum repost of a
  Microsoft engineer] <https://forums.blurbusters.com/viewtopic.php?p=94982>
- winit 0.30.13 registers all mice/keyboards for raw input to produce `DeviceEvent`s
  (`register_all_mice_and_keyboards_for_raw_input`) [src]; at 1000+ Hz that is 1000+ `WM_INPUT`/s on the UI
  thread for no UI benefit [inf]. Our framework should not register raw input unless a 3D viewport is in a
  captured-mouse orbit/fly mode.

### 1.4 Wait loops and timers
- `MsgWaitForMultipleObjectsEx` returns on "new input" only: Peek/Get/GetQueueStatus/WaitMessage mark queued
  input as old, so without `MWMO_INPUTAVAILABLE` already-seen-but-unremoved input does not wake you (classic
  hang). It can also return for system events (e.g. foreground activation) with no input; then pump.
  Max handles = `MAXIMUM_WAIT_OBJECTS - 1` (63). [doc]
  <https://learn.microsoft.com/en-us/windows/win32/api/winuser/nf-winuser-msgwaitformultipleobjectsex> (2018-12-05, updated 2024-08-22)
- Timer resolution: default tick 15.625 ms (64 Hz); `Sleep(1)` can take 1–16.6 ms. Since **Windows 10 2004**,
  `timeBeginPeriod` no longer changes global resolution for processes that didn't call it; since **Windows 11**,
  a window-owning process that is fully occluded/minimized/invisible gets no better than the default
  resolution. [doc] <https://learn.microsoft.com/en-us/windows/win32/api/timeapi/nf-timeapi-timebeginperiod>;
  numbers from Bruce Dawson, 2020-10-04 <https://randomascii.wordpress.com/2020/10/04/windows-timer-resolution-the-great-rule-change/>
- `CreateWaitableTimerEx(CREATE_WAITABLE_TIMER_HIGH_RESOLUTION)` (Windows 10 1803+) gives high-resolution
  waits without changing the global period [doc via Dawson]; winit uses exactly this inside its
  `MsgWaitForMultipleObjectsEx` wait [src winit v0.30.13 event_loop.rs L642, L751].
- **Lesson [inf]:** never pace frames with Sleep/timers. Pace with the swap chain's waitable object or the
  compositor clock (§2.6), and wake for input via `MsgWaitForMultipleObjectsEx` with those handles in the
  array.

### 1.5 Threads, input queues, modal loops
- Windows (HWNDs) have thread affinity; each GUI thread has its own input queue. `AttachThreadInput` merges two
  threads' input state — "like pooling money into a joint bank account where both parties need to be present to
  withdraw"; it is "not a Get Out of Jail Free card, it's a Get Into the Same Jail card" (a hang in either
  thread freezes both). [doc] Raymond Chen 2013-06-19 <https://devblogs.microsoft.com/oldnewthing/20130619-00/?p=4043>,
  2008-08-01 <https://devblogs.microsoft.com/oldnewthing/20080801-00/?p=21393>
- Modal move/size: `DefWindowProc` runs its own modal loop between `WM_ENTERSIZEMOVE` and `WM_EXITSIZEMOVE`, so
  the app's main loop doesn't run; common workaround is a `SetTimer` started at ENTERSIZEMOVE and render from
  `WM_TIMER` (SDL3, Dear ImGui samples). [anec/project code] <https://discourse.libsdl.org/t/sdl-allow-the-application-to-draw-while-windows-is-in-a-modal-move-resize-loop/47255>
- **Lesson [inf]:** keep the HWND-owning thread tiny: pump messages, translate to our own input events
  (timestamped), push to a lock-free queue, and wake the frame thread. Never attach input queues. Do not render
  on that thread; DXGI/DComp presentation and DComp Commit are callable from any thread (DComp is documented
  thread-safe, §2.4). The modal-loop problem disappears if rendering is not on the message thread.

---

## 2. DWM and presentation

### 2.1 Flip model vs blt model
- Blt model copies the back buffer into the DWM redirection surface every Present; flip model shares the back
  buffers with DWM, which composes straight from them (one fewer read+write per frame). Use flip model; don't mix
  GDI or a second API into the same HWND; don't use `ScrollWindow(Ex)` with flip model. `BufferCount` 2–16.
  [doc] <https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/dxgi-flip-model> (2018-05-31, updated 2025-04-15)
- DirectFlip variants, "in order of increasing functionality": (1) DirectFlip — buffers match screen size and the
  window covers the screen; (2) DirectFlip with panel fitters — window covers screen, buffer within a
  hardware-dependent scale (e.g. 0.25x–4x); (3) **DirectFlip with MPO** — buffer within a scale factor of the
  *window*; DWM reserves a hardware plane, "scanned out and potentially stretched to an alpha-blended sub-region
  of the screen". Once DirectFlipped, "the DWM can go to sleep" → **Independent Flip**. If other content comes
  on top, DWM returns to composition, "reverse composes" on top, or keeps MPO. Query
  `IDXGIOutput6::CheckHardwareCompositionSupport`. [doc] Shawn Hargreaves, DirectX blog 2018-04-09
  <https://devblogs.microsoft.com/directx/dxgi-flip-model/> and
  <https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/for-best-performance--use-dxgi-flip-model> (2018-05-31, updated 2026-04-15)
- `FLIP_DISCARD` enables "reverse composition" (DWM scribbles on the app's buffer instead of copying it).
  `FLIP_SEQUENTIAL` preserves buffer contents (needed for dirty-rect partial redraw). [doc, same pages]
- The first GPU write to the back buffer is where the GPU stalls waiting for it; "delay this point as far into the
  frame as possible". [doc, same page] → render offscreen first, blit/compose into the back buffer last [inf].

### 2.2 Waitable swap chains: Microsoft's recommended low-latency loop
- Without waiting, Present blocks until a prior frame retires and the app reaches "a stable equilibrium where
  the game is always waiting almost a full extra frame between the time it renders and the time it presents". Fix:
  create with `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT` (cannot be toggled by ResizeBuffers), set
  `IDXGISwapChain2::SetMaximumFrameLatency` (default **1** for waitable swap chains), get
  `GetFrameLatencyWaitableObject`, and **wait on it before every frame, including the first**, then process
  input, update, render, present. Latency 2 only when CPU+GPU > one frame but each < one frame. [doc]
  <https://learn.microsoft.com/en-us/windows/uwp/gaming/reduce-latency-with-dxgi-1-3-swap-chains> (2017-02-08, updated 2025-06-10)
  Loop shape (from the DirectXLatency sample):
  `WaitForSingleObjectEx(waitable) → ProcessEvents(AllIfPresent) → Update() → Render() → Present()`.
- "When in Independent Flip mode, you can get down to **1 frame of latency** on recent versions of Windows, with
  graceful fallback to the minimum possible when composed" (a 2-frame minimum bug existed through the
  Anniversary Update, fixed in Fall Creators Update 1709). `ALLOW_TEARING` "can enable even lower latency than the
  waitable object, even in a window on systems with multi-plane overlay support." [doc, flip-model blog 2018]
- Default DXGI queue: 3 frames; "DXGI blocks on the fourth Present". Use 1–2 more buffers than intended queued
  frames. [doc] NVIDIA, "Advanced API Performance: Swap Chains", 2023-12-15
  <https://developer.nvidia.com/blog/advanced-api-performance-swap-chains/>
- Flip-model sync interval 0 = "cancel the remaining time on the previously presented frame and discard this
  frame if a newer frame is queued" (mailbox-like); 1–4 = at least n vblanks. [doc]
  <https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/nf-dxgi1_2-idxgiswapchain1-present1>
- Tearing: `DXGI_PRESENT_ALLOW_TEARING` only with sync interval 0; "recommended to always pass this tearing flag
  when using sync interval 0 if CheckFeatureSupport reports that tearing is supported and the app is in a windowed
  mode". VRR requires tearing enabled. [doc] <https://learn.microsoft.com/en-us/windows/win32/direct3ddxgi/variable-refresh-rate-displays>
  Tearing in a window only actually happens when the swap chain is in independent flip / MPO [inf from the blog's
  "even in a window on systems with MPO"]; composed windows can't tear.

### 2.3 Dirty rects and scroll rects (Present1)
- `Present1` + `DXGI_PRESENT_PARAMETERS`: dirty rects (app must fully repaint each dirty rect and touch nothing
  else; cannot assume pixels persist inside them), one scroll rect + offset (runtime blts from previous frame);
  dirty rects take priority over the scroll rect; not for blt-model or MSAA swap chains. "The DWM can then use the
  metadata to optimize presentation and pass the metadata to indirect displays and terminal servers." [doc]
  <https://learn.microsoft.com/en-us/windows/win32/api/dxgi1_2/ns-dxgi1_2-dxgi_present_parameters>
- **wgpu does not expose this**; its DX12 backend calls `IDXGISwapChain3::Present(interval, flags)` [src
  `wgpu-hal/src/dx12/mod.rs` ~L1744]. With FLIP_DISCARD the back buffer contents are undefined anyway.
- **Lesson [inf]:** dirty rects matter mainly for power/RDP, not latency. Partial redraw is better done by us:
  keep per-layer textures (retained) and re-composite, or have DComp do it (§2.4) so unchanged panels are
  never re-rendered.

### 2.4 DirectComposition (DComp)
- Architecture: `dcomp.dll` (client), kernel object database in win32k, composition engine `dwmcore.dll` inside
  `dwm.exe`; one engine for all apps; each window can host 2 visual trees (below/above child windows). The tree
  is retained; changes are batched and applied atomically on `Commit`. [doc]
  <https://learn.microsoft.com/en-us/windows/win32/directcomp/architecture-and-components> (2018-05-31, updated 2025-03-11)
- Frame loop of the composition engine (verbatim steps): estimate next vblank → retrieve all pending batches →
  process → **update all animations using the estimated time** → find dirty regions → re-compose dirty regions →
  present (flip) → if nothing composed, wait for a batch → wait for next vblank. "The composition engine produces
  one frame for each vertical blank"; batches committed before frame start are included, later ones wait a frame;
  when idle "the composition thread sleeps while waiting for a new batch", and on a new batch it "wakes up but
  immediately goes back to sleep until the next vertical blank". It publishes frame presentation times so apps
  can choose animation sampling times. [doc, same page]
- API is thread-safe ("call any method from any thread at any time"); properties are write-only (no getters,
  because independent animations may be bound). Multiple devices can share one visual tree so two threads can
  commit independently — the documented example is a **touch thread owning a transform visual while the UI thread
  owns the rest** for "low-latency touch input". All changes in one Commit land in one frame; later commits in the
  same frame are not guaranteed to. `IDCompositionSurface::BeginDraw/EndDraw` + Commit synchronize content and
  property updates. [doc] <https://learn.microsoft.com/en-us/windows/win32/directcomp/basic-concepts>
- Content types: video-memory bitmaps (DComp surfaces, virtual surfaces), DXGI swap chains for composition
  (windowless; `CreateSwapChainForComposition`), window bitmaps. Per-channel-alpha mode lets DComp blend
  **ClearType** coverage against the background, falling back to grayscale automatically when transformed. [doc]
  <https://learn.microsoft.com/en-us/windows/win32/directcomp/bitmap-surfaces>
- `IDCompositionSurface::BeginDraw(updateRect)` = incremental update of a sub-rectangle; returns an offset into
  (possibly an atlas) surface; previous contents are **not** provided inside the update rect; first draw of a
  non-virtual surface must cover it; only one surface per device may be in BeginDraw at a time; update takes
  effect on EndDraw + Commit. [doc] <https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nf-dcomp-idcompositionsurface-begindraw>
- `IDCompositionVirtualSurface` = sparsely allocated surface with `Resize` and `Trim` (discard pixels outside
  trim rects) — the primitive for huge scrollable content rendered in tiles. [doc]
  <https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nn-dcomp-idcompositionvirtualsurface>
- `WaitForCommitCompletion` waits for the engine to process the previous Commit; `DwmFlush` "blocks the caller
  until the next call to a Present method, when all of the DirectX surface updates that are currently outstanding
  have been made" (only the caller's changes). [doc] <https://learn.microsoft.com/en-us/windows/win32/api/dcomp/nf-dcomp-idcompositiondevice-waitforcommitcompletion>,
  <https://learn.microsoft.com/en-us/windows/win32/api/dwmapi/nf-dwmapi-dwmflush>
- Microsoft now recommends Windows.UI.Composition over DComp "for apps on Windows 10" (banner on all DComp pages)
  [doc]; DComp remains the C-level API that Chromium uses: its source comments say DirectComposition is preferred
  even without hardware overlays "because it allows bypassing DWM blitting by using a flip mode swap chain"
  [src via search snippet] <https://chromium.googlesource.com/chromium/src/+/main/ui/gl/direct_composition_support.h>
- **How DComp lets content update without redrawing the window [doc+inf]:** each panel/viewport is its own visual
  with its own surface or swap chain; DWM recomposes only dirty regions; moving/scrolling/clipping a visual is a
  property change (no app rendering); a 3D viewport swap chain presents on its own cadence without touching the
  panels' surfaces; a caret or hover highlight can be a tiny separate visual.

### 2.5 Composition swapchain API (Presentation Manager)
- "Spiritual successor to the DXGI swapchain"; Windows 11 build 22000.194+ with WDDM 2.0; direct scanout and
  iflip need **WDDM 3.0** and surfaces allocated "displayable". Register up to **31** presentation buffers (any
  sizes/formats); bind to `presentation surfaces` attached to DComp/WinComp visuals via a composition surface
  handle (`DCompositionCreateSurfaceHandle`); presents can carry a **target time** (interrupt-time) and content
  rate; multiple timed presents can be queued; per-buffer "available" events and a present-retiring fence;
  statistics queue (512–1024 entries, ~5 s). If several presents are ready, all but the latest are skipped.
  [doc] <https://learn.microsoft.com/en-us/windows/win32/comp_swapchain/comp-swapchain> (2021-09-10, updated 2025-03-25)
- **Lesson [inf]:** this is the Windows analogue of `VK_EXT_present_timing` (target present time) and is mostly for
  media/precise pacing. For UI latency the target-time feature lets us do "render as late as possible for vblank
  N" safely. It is D3D11-centric (surfaces created as displayable D3D11 textures); not reachable through wgpu
  without interop. Keep as a later option.

### 2.6 Compositor clock (Windows 11)
- `DCompositionWaitForCompositorClock(count, handles, timeout)` replaces `IDXGIOutput::WaitForVBlank`: a
  display-independent heartbeat (DXGI always uses the primary monitor's cadence, which breaks windows on a 144 Hz
  secondary + 60 Hz primary), can also wait on app event handles, and is a wait (not an event) so the system can
  turn off vblank interrupts when nobody waits. `DCompositionBoostCompositorClock` requests Dynamic Refresh Rate
  boost (e.g. 60→120 Hz) during latency-sensitive interaction (inking, touch-pan). `DCompositionGetFrameId /
  GetStatistics / GetTargetStatistics` give per-target composition stats; `IDCompositionDevice::GetFrameStatistics
  .nextEstimatedFrameTime` gives the next compositor frame time. Intended for "rendering threads, separate from
  general-purpose UI threads". [doc] <https://learn.microsoft.com/en-us/windows/win32/directcomp/compositor-clock/compositor-clock> (2021-09-10, updated 2025-03-13)
- **Lesson [inf]:** our frame thread should block on `{swap-chain waitable, compositor clock, input-arrived event,
  JS-result event}` in one wait; boost the clock while a drag/pan/inking gesture is active.

### 2.7 Presentation modes as PresentMon reports them
- "Composed: Flip" = windowed flip model composed by DWM; "Hardware: Independent Flip" / "Hardware Composed:
  Independent Flip" = bypasses composition, the latter on an MPO plane; "Composed: Copy with GPU GDI" = legacy
  blt into a GDI-shared redirection surface. [doc quoted on forums from PresentMon docs]
  <https://forums.blurbusters.com/viewtopic.php?p=88397>
- PresentMon metrics (current README): MsBetweenPresents, MsUntilDisplayed, DisplayLatency ("time between frame
  submission and scan out"), **MsClickToPhotonLatency** ("time between mouse click input and display"),
  **MsAllInputToPhotonLatency**, MsInPresentAPI, CPUBusy, CPUWait, GPULatency. [doc]
  <https://github.com/GameTechDev/PresentMon/blob/main/README-CaptureApplication.md>
- Composed vs iflip latency numbers are anecdotal: one user measured 48 ms windowed Composed:Flip vs 32 ms
  borderless Independent Flip in a game; another saw no change at 100 Hz. [anec]
  <https://forums.blurbusters.com/viewtopic.php?p=88577> (search summary). From the documented DWM loop, composed
  presentation adds up to one vblank (DWM picks up the flip at frame start and presents at the next vblank)
  [inf from §2.4 and the flip-model timeline in dxgi-flip-model].

### 2.8 Latency budget at 120 Hz (8.33 ms) [inf, assembled from the above]
| Stage | Ideal | eframe today (likely) |
|---|---|---|
| Input arrives → app sees it | <1 ms (thread waiting in MsgWait) | up to 1 frame if blocked in present/wait on UI thread |
| JS handler | 0 (not on the critical path) | next frame (≈8.3 ms) |
| Build + render | <2–4 ms, rendered just before deadline | full rebuild every frame |
| Queue to display | 1 frame (waitable, latency 1) iflip; +1 vblank composed | Vulkan-on-DXGI layer / GDI copy: 2+ frames |
Target: ~1.5–2 frames click-to-photon composed (12–17 ms at 120 Hz), ~1–1.5 frames in iflip.

---

## 3. What wgpu (v30, HEAD 100c142) and winit actually do on Windows

### 3.1 DX12 backend [src `wgpu-hal/src/dx12/mod.rs`]
- `configure`: flags always include `FRAME_LATENCY_WAITABLE_OBJECT`, plus `ALLOW_TEARING` when supported (so present
  modes can change without recreating); `SwapEffect = FLIP_DISCARD`; buffer count = `maximum_frame_latency + 1`
  (comment cites NVIDIA's 1–2 extra buffers); `SetMaximumFrameLatency(config.maximum_frame_latency)`;
  `GetFrameLatencyWaitableObject` unless `Dx12UseFrameLatencyWaitableObject::None`.
- Creation paths: `CreateSwapChainForHwnd` (default `Dx12SwapchainKind::DxgiFromHwnd`),
  `CreateSwapChainForComposition` (`DxgiFromVisual`, wgpu creates a DComp device/target/visual; or the caller's own
  visual via `SurfaceTargetUnsafe::CompositionVisual`), `CreateSwapChainForCompositionSurfaceHandle`, and
  SwapChainPanel. Env var `WGPU_DX12_PRESENTATION_SYSTEM`. Added in v27.0.0 (2025-10-01) per CHANGELOG
  ("Builtin Support for DXGI swapchains on top of DirectComposition Visuals in DX12", #8125).
- `acquire_texture` waits on the waitable (`Dx12UseFrameLatencyWaitableObject::Wait`, the default) with the
  caller's timeout; `DontWait` (added by #7400) lets the app wait itself — **the hook we need to put the wait in our
  own multi-handle wait**.
- Present modes: Immediate → `Present(0, ALLOW_TEARING)`; Mailbox → `Present(0, 0)`; Fifo → `Present(1, 0)`.
  No `Present1`, no dirty rects, no frame statistics, no timed present.
- `desired_maximum_frame_latency` docs: DX12 → `SetMaximumFrameLatency(n)`; Vulkan → images = n+1; "Choose 1 ... For
  applications like GUIs doing a small amount of GPU work each frame that need low latency, this is a reasonable
  choice." Default 2. [src `wgpu-types/src/surface.rs` ~L804–838]

### 3.2 Vulkan backend on Windows [src `wgpu-hal/src/vulkan/swapchain/native.rs`]
- `min_image_count = maximum_frame_latency + 1`. On Windows only, an extra fence on `vkAcquireNextImageKHR` "to
  throttle acquisition on Windows ... very important to avoid bad frame pacing when the Vulkan driver is using a
  DXGI swapchain" (refs #8310/#8354). No `VK_KHR_present_wait`/`present_id` use.
- Issue #8310 (2025-10-06, closed 2025-10-23 by #8420): NVIDIA's "Auto" setting routes FIFO/FIFO_RELAXED through a
  layer on top of DXGI; other modes use the native swapchain; 25 fps instead of 75. Issue #8354 (opened
  2025-10-15, still open as of 2026-09-22): plan to implement a DXGI swapchain directly for the Vulkan backend to
  get the frame-latency waitable object, frame statistics and pacing. [doc] <https://github.com/gfx-rs/wgpu/issues/8310>,
  <https://github.com/gfx-rs/wgpu/issues/8354>
- NVIDIA control panel "Vulkan/OpenGL present method": Auto / Prefer native / Prefer layered on DXGI swapchain;
  layered "can add minor memory and performance overhead"; PresentMon shows native windowed as "Composed: Copy with
  GPU GDI" and layered as "Independent Flip". [anec, 2025-08] <https://community.khronos.org/t/vulkan-present-mode-preferred-native-vs-layered-on-dxgi-swapchain/112055>
- **Backend choice:** wgpu-core adds Vulkan then DX12 (`try_add_hal(Vulkan)` before `Dx12`) and sorts adapters by
  device type with a stable sort, so a discrete NVIDIA GPU exposed by both is returned via **Vulkan** first
  [src `wgpu-core/src/instance.rs` ~L126–130, ~L560–590; inf about which wins]. egui-wgpu doesn't override this.
  **Force `Backends::DX12` on Windows.**

### 3.3 eframe/egui-wgpu 0.37 defaults [src tag 0.37.0]
- eframe sets `SurfaceConfig::LOW_LATENCY` = `PresentMode::AutoVsync` + `desired_maximum_frame_latency: Some(1)`
  (`crates/eframe/src/epi.rs` L492); egui-wgpu raises latency to ≥2 while resizing (`winit.rs` ~L125).

### 3.4 winit Windows event loop [src v0.30.13 and master 0c73728]
- Wait: `MsgWaitForMultipleObjectsEx(n, [high-res waitable timer], timeout, QS_ALLINPUT, MWMO_INPUTAVAILABLE)`;
  "We must use QS_ALLINPUT to wake on accessibility messages." Drains with `PeekMessageW(PM_REMOVE)` loop.
- Redraw: `request_redraw` → `RedrawWindow(hwnd, RDW_INTERNALPAINT)`; `RedrawRequested` is emitted from
  `WM_PAINT` ("RedrawRequested is emitted in sync with any WM_PAINT messages", `winit-core/src/window.rs` ~L750).
  So redraw is naturally after input in GetMessage order (good), but it runs on the HWND thread (bad for us).
- No `GetMouseMovePointsEx` / pointer history use; WM_POINTER handled for pen/touch.
- **Lesson [inf]:** keep winit (or our own thin Win32 layer) only as the message-pump thread; do not render inside
  `RedrawRequested`. If we keep winit, run it on the main thread and run the frame loop on another thread holding
  the `Surface`; wgpu surfaces are `Send + Sync`.

---

## 4. Windows.UI.Composition / WinUI 3

### 4.1 Visual layer
- "Retained-mode API for graphics, effects, and animations ... rendered in a smooth, glitch-free manner independent
  of the app's UI thread." `Visual` is "a lightweight, thread-agile proxy in the app process for visual state in the
  compositor". Animations (KeyFrame, Expression — "mathematical relationships between different properties,
  including user input") "run independently from the UI thread". In WinUI 3 (Microsoft.UI.Composition) the
  compositor "runs entirely within a Windows App SDK app and has access only to pixels that it drew"; external
  content (SwapChainPanel, MediaPlayerElement, WebView2, Mica) is handed to the system compositor and drawn
  **below** with a hole cut — no transparency/effects over it. [doc]
  <https://learn.microsoft.com/en-us/windows/apps/develop/composition/visual-layer> (2026-03-16, updated 2026-07-14)
- **Lesson [inf]:** the "hole punch" is exactly how a wgpu 3D viewport should coexist with our UI layer on Windows:
  viewport swap chain in its own DComp visual; overlays (gizmos, tooltips) in visuals above it.

### 4.2 InteractionTracker and redirection
- `InteractionTracker` is a state machine (Idle / Interacting / Inertia / CustomAnimation) whose Position/Scale
  drive ExpressionAnimations on the compositor; "built to utilize the new Animation engine that operates on an
  independent thread at 60 FPS". `VisualInteractionSource` defines the hit-test visual and redirection; by default
  pointer input goes to the UI thread and **Precision Touchpad** input goes to the tracker; touch/pen need
  `TryRedirectForManipulation` on PointerPressed. [doc]
  <https://learn.microsoft.com/en-us/windows/apps/develop/composition/interaction-tracker-manipulations> (2026-03-16)
- `VisualInteractionSourceRedirectionMode`: Off; CapableTouchpadOnly; **PointerWheelOnly** and
  **CapableTouchpadAndPointerWheel** (added 1803 / SDK 17134) — mouse wheel handled on the compositor. [doc]
  <https://learn.microsoft.com/en-us/uwp/api/windows.ui.composition.interactions.visualinteractionsourceredirectionmode>
- WinUI's new `ScrollPresenter`/`ScrollView` is built on InteractionTracker (owner callbacks ValuesChanged,
  InertiaStateEntered, ...) [src `controls/dev/ScrollPresenter/ScrollPresenter.h` @f980ab2]; the classic
  `ScrollViewer` uses DirectManipulation via `CompositorDirectManipulationViewport` [src
  `dxaml/xcp/core/compositor/`]. Learn says a XAML ScrollViewer's own scrolling is "bound to the UI thread" for
  input-driven effects unless you use its manipulation property set [doc]
  <https://learn.microsoft.com/en-us/windows/apps/develop/composition/input-driven-animations>

### 4.3 DirectManipulation (Win32)
- "Direct Manipulation processing occurs on a separate, independent thread from the UI thread", with "off-thread
  input processing, optional off-thread input hit testing, and input/output prediction"; outputs transforms applied
  through a DComp wrapper (`DCompDirectManipulationCompositor`); app calls `SetContact` on WM_POINTERDOWN;
  keyboard/mouse can be fed via `ProcessInput`; inertia after release. [doc]
  <https://learn.microsoft.com/en-us/previous-versions/windows/desktop/directmanipulation/direct-manipulation-portal> (archived, 2018-05-31)
- Browsers use DirectManipulation for precision-touchpad smooth scrolling on Win32 [inf; not verified here].

### 4.4 XAML's frame pipeline (from WinUI 3 source, @f980ab2)
- Threads: one UI thread + one **render (compositor) thread** "at high priority" (`compositorscheduler.cpp`,
  "Run the render thread at high priority"; initializes COM "for DirectManipulation support").
- The render thread ticks the animation clock, then queues a UI-thread tick; it "never runs faster than the
  refresh rate of the primary display", and sleeps until the next requested time or more work
  (`compositorscheduler.cpp` ~L292–425). "It is important that this happens after ticking the clock so that the UI
  thread sees up-to-date values."
- UI-thread tick priority (`UIThreadScheduler.cpp` ~L118–126): **tick requests made outside a tick (from input or
  async handlers) are high priority — "processed before any additional input messages"; requests made during a
  tick (next animation frame, dependent animations, CompositionTarget.Rendering) are low priority — "deferred until
  all input is processed."** Requests within a frame are coalesced and the render thread isn't woken until the
  frame's requests are combined.
- UI tick (`xcpcore.cpp` `CCoreServices::Tick` L4338, `NWDrawTree` ~L6285–6760): timing manager tick → event
  queue → `CLayoutManager::UpdateLayout` (possibly several times) → **RenderWalk** (only if needed: dirty tree) →
  submit frame / Commit. Comment at ~L7263: "the main device Commit() of the content changes can take a long time
  (> 1 vBlank) to return. If the UI thread holds the lock this entire time, it prevents the render thread from
  ticking any active animations or manipulations. It is more preferable to allow for de-synchronization at this
  point than to glitch the render thread."
- Independent vs dependent animations: keyframe animations of opacity, RenderTransform, Clip, Canvas.Left/Top,
  brush color run on the composition thread; Width/Height (layout-affecting) are dependent, disabled by default,
  and stutter when the UI thread is busy; CompositionTarget.Rendering handlers force UI work every frame and
  prevent idle. [doc] <https://learn.microsoft.com/en-us/windows/apps/develop/performance/optimize-animations-and-media> (2026-08-11)

---

## 5. WPF

- Architecture: managed PresentationCore/Framework + unmanaged **milcore** (composition engine). The visual tree
  is mirrored into unmanaged "composition nodes", "only accessible through a messaging protocol"; "the entire tree
  of visuals and drawing instructions is cached ... This enables the system to repaint at high refresh rates
  without the composition system blocking on callbacks to user code." Painter's algorithm, not GDI clipping.
  Layout is two-phase Measure/Arrange; input from User32 → WPF raw input → dispatcher → preview (tunnel) + bubble.
  [doc] <https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/wpf-architecture> (2017-03-30, updated 2025-08-27)
- Render thread: `WpfGfx/core/uce/crossthreadcomposition.cpp` — "cross-thread composition device that allows for
  deferred execution of the partition commands"; the channel submits batches that are enqueued and processed by a
  worker thread (partition manager). The device layer is `IDirect3DDevice9Ex` (`hw/d3ddevice.h`). [src dotnet/wpf @c4610da]
- **Dispatcher priorities** (high→low): Send 10, Normal 9, DataBind 8, **Render 7**, Loaded 6, **Input 5**,
  Background 4, ContextIdle 3, ApplicationIdle 2, SystemIdle 1, Inactive 0. Note **Render is above Input**, and
  Normal/DataBind are above Render. The dispatcher "does not throttle the execution of an operation". [doc]
  <https://learn.microsoft.com/en-us/dotnet/api/system.windows.threading.dispatcherpriority> (2025-07-01)
- `MediaContext` (`PresentationCore/System/Windows/Media/MediaContext.cs` @c4610da):
  - Renders at Render priority, but "if something at render priority takes more than a frame it will block input from
    ever being processed"; it posts an Input-priority marker and, if input has been blocked for
    `MaxTicksWithoutInput = TicksPerSecond / 2` (**500 ms**), it schedules the render at Input priority (L609–631,
    L2889).
  - Frame: `TimeManager.Tick()` → render callbacks (layout) → `CompositionTarget.Rendering` → loop while timing is dirty
    (cap 153 iterations, else "infinite tick loop") → `Render()` sends the updated scene to the UCE (L1759–1853).
  - Commits the channel "only after the next vblank has occurred"; waits for the UCE "Presented" notification
    before rendering the next frame; at most one commit per vblank interval (L525–870).
- Performance lessons [doc]: layout pass recursion — any child change can trigger a new pass; build trees
  **top-down** (150-deep tree: 11 ms vs 366 ms bottom-up to build; 96 vs 454 ms incl. render); update a Transform
  instead of replacing it to avoid layout. <https://learn.microsoft.com/en-us/dotnet/desktop/wpf/advanced/optimizing-performance-layout-and-design>
- WPF lessons I take [inf]: (1) a retained render thread alone does not give low latency — WPF's UI thread still
  does layout+render-tree generation, and anything at Normal priority (most app code, bindings) starves Render and
  Input; (2) the 500 ms input-starvation escape hatch is far too coarse; (3) D3D9Ex + its own composition meant
  "airspace" problems with HWND/DirectX interop; (4) per-frame managed→native message marshalling of the scene is
  the cost model to avoid — send deltas.

---

## 6. Direct2D / DirectWrite

- **Geometry realizations** (Windows 8.1+): D2D normally re-tessellates every geometry every frame; a realization
  caches the tessellation (points, not pixels), is device-dependent, created with a flattening tolerance (DIPs);
  translation/rotation are free, scaling eventually reveals facets. Recommended scheme for zooming apps: recreate at
  each 2x scale change, created with `maxZoomFactor = 2` headroom; for very fast zoom, recreate only after the
  gesture ends. [doc] <https://learn.microsoft.com/en-us/windows/win32/direct2d/geometry-realizations-overview> (2018-05-31)
- D2D performance guidance [doc] <https://learn.microsoft.com/en-us/windows/win32/direct2d/improving-direct2d-performance>:
  reuse resources (GPU resource create/destroy is expensive); atlas small bitmaps ("keep most bitmaps to at least
  64 KB and limit the number of bitmaps that are smaller than 4 KB"); avoid `Flush`; three static-content caches
  (full-scene color bitmap, per-primitive A8 mask + FillOpacityMask, geometry realizations); prefer specific
  primitives over generic geometry; axis-aligned clips over layers; minimize D2D↔D3D switches; reuse
  `IDWriteTextLayout` (DrawText rebuilds a layout every call); "Set the text antialias mode to GRAYSCALE ... comparable
  to ClearType but is much faster."
- DirectWrite layout caches glyph positions: "a large performance gain is possible by reusing the same layout object
  for multiple draw calls". D2D uses "alpha correction" instead of gamma correction so it does not need to read the
  destination pixels for text. Grayscale populates destination alpha correctly (needed for transparent/composited
  layers). [doc] <https://learn.microsoft.com/en-us/windows/win32/direct2d/direct2d-and-directwrite>
- ClearType in composition: DComp's per-channel-alpha mode blends subpixel coverage with a text color against the
  background and falls back to grayscale when not pixel/axis aligned [doc §2.4].
- **Lessons [inf]:** for schematic/PCB canvases cache tessellations per zoom octave (realization-style), not
  rasterizations; text shaping/layout cached per (string, font, width) and glyph rasters in an atlas; default to
  grayscale AA for text on GPU layers (subpixel only on opaque, pixel-aligned layers).

---

## 7. Latency tooling and "Reflex-like" ideas

- PresentMon metrics as in §2.7; it reports the present mode per frame, so it is the first tool to tell
  "Composed: Flip" vs "Independent Flip" vs "Copy with GPU GDI". [doc]
- NVIDIA Reflex: "Reflex Low Latency mode ... synchronizes CPU and GPU pipelines for just-in-time rendering";
  latency markers break PC latency into input, simulation, render submission, driver, render queue, GPU render
  stages. [doc] <https://developer.nvidia.com/reflex> — the idea: start CPU work as late as possible so input is
  sampled just before submission; keep the render queue empty.
- Vulkan equivalents: `VK_NV_low_latency2` (Reflex for Vulkan) [unverified here]; `VK_EXT_present_timing`
  (target present time, per-present timing; depends on `VK_KHR_present_id2`, calibrated timestamps; NVIDIA drivers
  first) [doc] <https://docs.vulkan.org/features/latest/features/proposals/VK_EXT_present_timing.html>,
  <https://www.khronos.org/blog/vk-ext-present-timing-the-journey-to-state-of-the-art-frame-pacing-in-vulkan>.
  wgpu exposes neither.
- **Apply to apps [inf]:** (1) sample input as late as possible: after the waitable fires, drain input, then build;
  (2) when render is cheap, delay the start ("render just in time") using the compositor's
  `nextEstimatedFrameTime` minus a measured render budget; (3) measure our own click-to-photon via PresentMon and
  an in-app trace: input timestamp (message time / pointer `PerformanceCount`) → frame build → Present → DXGI frame
  statistics `SyncQPCTime`.

---

## 8. Recommendations for cadatomic's framework on Windows [inf unless marked]

### Threads
1. **Message thread** (process main thread, owns HWNDs): pump with `MsgWaitForMultipleObjectsEx(..., QS_ALLINPUT,
   MWMO_INPUTAVAILABLE)`; translate to our event types with QPC timestamps and pointer history; push to a queue;
   signal the frame thread's event. Never renders, never runs JS, never blocks on GPU.
2. **Frame/render thread** (high priority, like XAML's render thread): waits on {swap-chain waitable (wgpu
   `Dx12UseFrameLatencyWaitableObject::DontWait` then our own wait), input event, JS-commit event, compositor clock};
   on wake: drain input, run Rust-side hit-testing and immediate visual feedback (hover, press, drag, scroll offsets,
   caret), apply any scene deltas JS has committed, layout only dirty subtrees, record GPU work for dirty layers,
   present.
3. **JS thread** (QuickJS-NG): receives input events as messages, runs Svelte handlers, emits scene deltas as one
   batch per microtask flush (like DComp Commit / WPF channel batch); the frame thread applies whatever batches have
   arrived before its deadline and never waits for JS. Model it on XAML's high/low-priority ticks: a JS commit
   triggered by input wakes the frame thread immediately ("high priority"); JS animation-frame callbacks are
   low priority and yield to input.
4. Optional **3D viewport threads**: each viewport its own swap chain/visual, presenting at its own cadence.

### Present path on Windows
5. Force **DX12** (`Backends::DX12`) on Windows; don't let wgpu pick Vulkan (driver DXGI-layer pacing, GDI-copy
   composition). Re-measure with PresentMon: expect "Composed: Flip" or "Independent Flip", never "Copy with GPU GDI".
6. `desired_maximum_frame_latency = 1`, Fifo (vsync) by default; offer Immediate+ALLOW_TEARING for VRR displays
   (wgpu already sets the swap-chain flag).
7. Use `Dx12SwapchainKind::DxgiFromVisual` / our own DComp tree via `SurfaceTargetUnsafe::CompositionVisual`:
   UI shell visual + one visual per 3D/canvas viewport. This enables per-region updates, MPO candidates, transparent
   windows, and moving/scrolling visuals without re-rendering.
8. What to ask of wgpu (upstream PRs or a small fork): expose `Present1` dirty/scroll rects, DXGI frame statistics
   (`GetFrameStatistics`/`GetLastPresentCount`), the waitable handle itself (so `DontWait` users can put it in a
   `WaitForMultipleObjects`), and `IDXGIOutput6::CheckHardwareCompositionSupport`. Track wgpu #8354 (DXGI swapchain
   for Vulkan). Bypass wgpu for: DComp tree construction/commit and compositor-clock waits (raw `windows` crate calls
   alongside wgpu; wgpu only needs the visual).
9. Render late: after the waitable fires, sample input and render immediately; first write to the back buffer last
   (compose offscreen layers into it at the end).

### Input path
10. Hover/press/drag/scroll feedback computed in Rust on the frame thread from the retained scene's hit-test data —
    JS gets the event in parallel and its result shows when it shows (usually the next frame, invisible for state
    changes; visible only for content changes).
11. Use pointer history (`GetPointerInfoHistory` with `EnableMouseInPointer`, or `GetMouseMovePointsEx`) for
    wire-drawing/inking; latest sample only for hover. Don't register raw input except in captured 3D navigation.
12. Precision touchpad and wheel: compositor-side scroll (see below); boost the compositor clock during gestures.

### Compositor-side scroll and animation
13. Make scroll offset, opacity, transform and clip **properties of retained layers** that the frame thread can
    animate without re-running layout or JS (XAML "independent animations"). Layout-affecting animations
    (width/height) are "dependent" and must be avoided or explicitly opt-in.
14. Scroll containers render content beyond the viewport (tile cache/virtual-surface style, cf.
    `IDCompositionVirtualSurface` Trim/Resize) so that a fling never waits for JS or layout; on our own frame thread
    this is equivalent to DirectManipulation/InteractionTracker. Going further (OS compositor animating our visuals
    while our process is stalled) is possible with DComp animations/InteractionTracker but not portable — keep it as
    a Windows-only optimization behind the same layer-property API.
15. Canvases (schematic/PCB): cache tessellations per zoom octave (D2D realization strategy), pan = transform only.

### Idle CPU
16. Frame thread sleeps in one wait when nothing is dirty; no per-frame callbacks registered when idle (XAML/WPF
    lesson: CompositionTarget.Rendering forces every-frame work); no timer-based polling (Win11 clamps timer
    resolution for occluded windows anyway).
