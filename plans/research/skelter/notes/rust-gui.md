# Rust GUI frameworks and renderers: what to learn for cadatomic's own retained framework

Research date: 2026-10-25. Written for the decision "replace egui 0.37 with our own retained GUI framework +
renderer, with svelters (Svelte 5 in QuickJS-NG) mutating a Rust-owned retained scene".

Evidence tags used on every claim:

- **[src]** read directly from the project's source at the named branch/tag on 2026-10-25 (raw.githubusercontent.com).
- **[doc]** stated in the project's own docs, README, changelog, release notes, PR/issue text or blog.
- **[rep]** reported by a third party or a commenter (not the project itself).
- **[inf]** my inference from the cited observations; alternatives named where relevant.

Versions were read from the crates.io API (`https://crates.io/api/v1/crates/<name>`) on 2026-10-25 unless stated.

---

## 0. Version table (crates.io, 2026-10-25)

| Crate | Latest | Released | Notes |
|---|---|---|---|
| gpui | 0.2.2 | 2025-10-22 | official, stale on crates.io; Zed main is far ahead |
| gpui-pre | 0.3.9 | 2026-10-12 | "gpui-pre snapshot of zed@93d7456", owner `huacnlee` (Longbridge) |
| gpui-fast | 0.1.10 | 2026-10-19 | Longbridge fork with retained mode |
| gpui-component / gpui-kit | 0.8.0 | 2026-10-19 | Longbridge |
| gpui-ce | 0.3.0 | 2026-10-20 | community fork |
| xilem / masonry / masonry_winit | 0.4.0 | 2025-10-29 | repo main still 0.4.0, last commit 2026-09-14 |
| vello (compute, now "research") | 0.11.0 | 2026-10-02 | |
| vello_gpu (formerly vello_hybrid) | 0.3.0 | 2026-10-02 | vello_hybrid 0.2.0 last on 2026-08-07 |
| vello_cpu / vello_common | 0.3.0 | 2026-10-02 | |
| glifo (text for sparse strips) | 0.4.0 | 2026-10-02 | formerly parley_draw |
| parley / fontique | 0.12.0 | 2026-10-09 | |
| kurbo | 0.13.1 | 2026-05-13 | |
| peniko | 0.6.1 | 2026-05-15 | |
| anyrender / anyrender_vello | 0.14.0 / 0.15.0 | 2026-10-03 | |
| swash | 0.2.10 | 2026-07-17 | |
| skrifa / read-fonts | 0.49.0 / 0.46.0 | 2026-10-21 | |
| harfrust | 0.15.0 | 2026-10-22 | |
| taffy | 0.15.0 | 2026-10-12 | Zed pins `=0.15.0` |
| cosmic-text | 0.19.0 | 2026-04-22 | |
| glyphon | 0.12.0 | 2026-07-09 | |
| accesskit / accesskit_winit | 0.25.1 / 0.34.1 | 2026-09-25 | |
| wgpu | 30.0.1 | 2026-08-22 | Zed pins a git rev for a Vulkan surface-drop fix |
| winit | 0.30.13 stable; 0.31.0-beta.4 | 2026-10-24 (beta.4) | |
| lyon | 1.0.19 | 2026-03-08 | |
| femtovg | 0.27.0 | 2026-08-31 | |
| tiny-skia | 0.12.0 | 2026-02-02 | |
| iced / iced_wgpu | 0.14.0 | 2025-12-07 | master is 0.15.0-dev |
| floem | 0.2.0 | 2024-11-14 | repo main far ahead |
| slint | 1.18.1 | 2026-09-21 | master is 1.19.0 |
| makepad-widgets | 1.0.0 | 2025-05-13 | repo `dev` is 2.0.0 |
| freya | 0.4.3 stable; 0.5.0-rc.10 / beta.12 | 2026-10-18 | |
| vizia | 0.4.0 | 2026-04-23 | |
| blitz-dom / blitz-paint | 0.3.0-beta.4 | 2026-10-16 | |
| dioxus-native | 0.8.0-alpha.3 | 2026-10-16 | |
| egui / eframe / egui-wgpu | 0.37.0 | 2026-10-15 | |
| rquickjs | 0.14.0 | 2026-09-18 | |

---

## 1. GPUI (Zed)

Source: `zed-industries/zed` `main`, read 2026-10-25. Licence of the `gpui` crate: Apache-2.0 [src:
`crates/gpui/Cargo.toml`].

### 1.1 Crate layout and platform layer

- GPUI is now split into a core plus per-platform crates: `gpui`, `gpui_platform`, `gpui_macos`, `gpui_apple`,
  `gpui_ios`, `gpui_windows`, `gpui_linux`, `gpui_wgpu`, `gpui_web`, `gpui_macros`, `gpui_util`,
  `gpui_shared_string`, `gpui_tokio` [src: zed `Cargo.toml` workspace members, lines 96–108].
  https://raw.githubusercontent.com/zed-industries/zed/main/Cargo.toml
- **No winit.** Each OS has its own platform layer [src: manifests below].
  - **macOS:** `gpui_macos` (AppKit, `display_link.rs`, `CVHostTime`, accesskit_macos) + `gpui_apple` (Metal renderer
    `metal_renderer.rs`, `shaders.metal`, CoreText `text_system.rs`, `metal_atlas.rs`) [src: directory listings
    https://github.com/zed-industries/zed/tree/main/crates/gpui_macos/src and
    https://github.com/zed-industries/zed/tree/main/crates/gpui_apple/src].
  - **Windows:** `gpui_windows`: Direct3D 11 (`directx_renderer.rs`, `shaders.hlsl`), DirectComposition,
    DirectManipulation (`direct_manipulation.rs`), DirectWrite (`direct_write.rs`), own vsync thread (`vsync.rs`),
    accesskit_windows [src: `crates/gpui_windows/Cargo.toml` features `Win32_Graphics_Direct3D11`,
    `Win32_Graphics_DirectComposition`, `Win32_Graphics_DirectManipulation`, `Win32_Graphics_DirectWrite`; file listing
    https://github.com/zed-industries/zed/tree/main/crates/gpui_windows/src].
  - **Linux/FreeBSD:** `gpui_linux` (own Wayland client via wayland-client/protocols, own X11 via x11rb + xim,
    xkbcommon, calloop, accesskit_unix) rendering through `gpui_wgpu` [src: `crates/gpui_linux/Cargo.toml`].
  - **Web:** `gpui_web` (wasm, web-sys, `gpui_wgpu` with WebGPU/WebGL) [src: `crates/gpui_web/Cargo.toml`].
- **Blade is gone; Linux renders with wgpu.** PR #46758 "gpui: Remove blade, reimplement linux renderer with wgpu"
  (zortax) was opened 2026-01-14 and merged 2026-02-13 [doc: https://github.com/zed-industries/zed/pull/46758].
  The workspace no longer references blade [src: grep of zed `Cargo.toml`]. `gpui_wgpu` enables wgpu features
  `metal, gles, vulkan, wgsl, webgpu` on Linux [src: `crates/gpui_wgpu/Cargo.toml`].
- **Windows and macOS keep native renderers.** A Zed member (reflectronic, 2026-01-23, on PR #46758): "I don't think
  we plan to replace the Windows or macOS renderers at this time", the native renderers "probably have better
  performance and wider compatibility than WGPU" [doc: PR #46758 thread]. A community fork, nestrilabs/gpui-wgpu,
  uses one wgpu + winit backend on all platforms [rep: https://github.com/nestrilabs/gpui-wgpu].
- Windows history: Zed on Windows first used the Vulkan (Blade) path, then added a DX11 backend so it runs on
  machines/VMs without Vulkan; glyph rasterisation moved from Direct2D to DirectWrite [doc:
  https://zed.dev/blog/windows-progress-report, 2025-08-19]. Windows GA: "DirectX 11 for rendering, and DirectWrite
  for text" [doc: https://zed.dev/blog/zed-for-windows-is-here, 2025-10-15].
- Text systems: CoreText (macOS, `gpui_apple/text_system.rs`), DirectWrite (Windows), cosmic-text 0.19 + swash +
  optional zed-font-kit (Linux, `gpui_wgpu/cosmic_text_system.rs`) [src].
- Other deps worth noting: `accesskit 0.24` (GPUI now has accessibility), `taffy =0.15.0` with flexbox/grid/block,
  `lyon 1.0`, `resvg/usvg`, `etagere` (atlas packing), optional `hot-reload` feature via Dioxus `subsecond 0.7.10`
  [src: `crates/gpui/Cargo.toml`, zed `Cargo.toml` lines 529–532, 922].

### 1.2 Element / view / entity model

- Elements implement three phases: `request_layout` → `prepaint` → `paint` [src: `crates/gpui/src/element.rs`
  lines 74–96, 301–305].
- Views are `Entity<V: Render>`; state lives in entities owned by `App`; `cx.notify()` marks a view dirty;
  `Window::mark_view_dirty` inserts the view and its ancestors into `dirty_views` [src: `window.rs` 2264–2276].
- **Every frame rebuilds the element tree from `render()`** of each view, except views embedded with
  `Entity::cached(style)` / `AnyView::cached(style)`: "The rendered subtree is reused until the entity is notified
  (or the cached bounds / text style change). Caching requires a definite size" [src: `view.rs` 250–259]. A cached,
  non-dirty view replays its previous ranges with `window.reuse_prepaint(range)` and `window.reuse_paint(range)` [src:
  `view.rs` 550–554, 617–630].
- **Taffy tree is cleared every frame** (`self.layout_engine.as_mut().unwrap().clear()` in `Window::draw`) [src:
  `window.rs` line 3524]. gpui-fast confirms: "Upstream clears the whole Taffy tree at the end of every frame" [doc:
  https://raw.githubusercontent.com/longbridge/gpui-fast/main/docs/retained-mode.md].
- Ownership/data-flow design write-up: https://zed.dev/blog/gpui-ownership (2024-01-25) [doc].

### 1.3 Scene and renderer

- `Scene` holds typed primitive vectors: `Shadow`, `Quad`, `Path`, `Underline`, `MonochromeSprite` (glyphs),
  `SubpixelSprite` (subpixel-AA glyphs), `PolychromeSprite` (emoji/images), `Surface` (`PaintSurface`) [src:
  `crates/gpui/src/scene.rs` 222–231].
- Draw order: each primitive gets an `order` from a `BoundsTree` of primitive bounds (layers push/pop bounds), so
  non-overlapping primitives share an order and batch together; `finish()` sorts each vector by `order` (sprites by
  `(order, texture_id)`), and `batches()` merges the sorted streams into `PrimitiveBatch`es [src: `scene.rs` 41–200].
- `Scene::replay(range, prev_scene)` copies paint operations from the previous frame's scene — this is how cached
  views avoid repainting [src: `scene.rs` 141–149].
- **Instanced rendering:** D3D11 renderer issues `DrawInstanced(4, instance_count, ...)` per batch with a per-batch
  constant buffer; max instance buffer 256 MiB; paths drawn into a 4× MSAA target (`PATH_MULTISAMPLE_COUNT = 4`)
  [src: `gpui_windows/src/directx_renderer.rs` 29–30, 1168–1263]. The blog says paths are drawn "to one MSAA texture
  the size of the render target, then copied" [doc: windows-progress-report].
- `PaintSurface` (external video/GPU content) exists only on macOS/iOS as a `CVPixelBuffer` [src: `scene.rs`
  766–774]. **There is no first-class "embed my own GPU texture" primitive on Windows/Linux** [inf from src]. A Zed
  PR (#62379) adds native window composition (native content with GPUI overlays above), which gpui-fast adopted [doc:
  https://raw.githubusercontent.com/longbridge/gpui-fast/main/README.md].
- Shaders exist in MSL, HLSL and WGSL (plus WGSL storage/subpixel/WebGL variants) [src: file listings; doc:
  windows-progress-report].

### 1.4 How a frame is produced; frame pacing and latency

- The platform window calls an `on_request_frame` callback; the callback (in `window.rs` ~1790–1950) [src]:
  1. throttles inactive windows (unless input is high-rate) and caps to ~60 fps under thermal pressure
     (`ThermalState::Serious|Critical` → 16.667 ms);
  2. runs `on_next_frame` callbacks (animation);
  3. if the invalidator is dirty (or forced) → `window.draw(cx)` then `window.present()`; else, if `needs_present`
     → `present()` only (re-present the previous frame);
  4. re-arms the platform frame source if still dirty.
- **`InputRateTracker`**: if ≥ 60 input events/s arrive within a 100 ms window, keep presenting for 1 s
  ("Used for selective VRR (Variable Refresh Rate) optimization", "to prevent display underclocking during active
  input") [src: `window.rs` 1322–1366, 1889–1894].
- **macOS (blog, 2024-02-07, https://zed.dev/blog/120fps)** [doc]: on ProMotion displays, frames not being presented
  let the display downclock, causing hitches even at < 4 ms frame times; fix = keep presenting for 1 s after the last
  input. They removed `presentsWithTransaction` and `wait_until_scheduled` from the steady-state path, call
  `present_drawable` before `commit`, use a pool of instance buffers (triple buffering), and moved from
  `CADisplayLink` (60 fps cap) to `CVDisplayLink` dispatched to the main queue. Startup/resize still use the
  synchronised (transactional) path to avoid wobble. Result: stable 120 fps on M1/M2 (Metal HUD).
  Current source: `CAMetalLayer.maximumDrawableCount = 3`, `set_presents_with_transaction` toggle, instance-buffer
  pool, `wait_until_scheduled` + `present_drawable` in the transactional path [src:
  `gpui_apple/src/metal_renderer.rs` 43–131, 213–214, 457–460, 555–558].
- **Windows** [src: `gpui_windows/src/directx_renderer.rs`, `vsync.rs`, `platform.rs`]:
  - Swap chain is `DXGI_SWAP_EFFECT_FLIP_SEQUENTIAL`, `BufferCount = 3`, BGRA8, created with
    `CreateSwapChainForComposition` (DirectComposition, premultiplied alpha) by default, or
    `CreateSwapChainForHwnd` when `GPUI_DISABLE_DIRECT_COMPOSITION` is set (lines 26–30, 1295–1350, 1695).
  - `Present(0, 0)`: **sync interval 0**; GPUI does its own pacing (line 251).
  - A dedicated `VSyncProvider` thread waits on `DwmFlush()` each vblank, falls back to `Sleep(interval)` if it
    returns in < 1 ms (monitor asleep / occluded), and signals the UI thread to draw (`vsync.rs`; `platform.rs`
    433–483). Interval from `DwmGetCompositionTimingInfo`.
  - History [doc, commit messages mirrored at git.secluded.site]: #11731 moved Windows 10 vsync from a ~15 ms timer
    (60 fps cap) to `DwmFlush`; #35969 (2025-08) tried `DCompositionWaitForCompositorClock` on Windows 11 and removed
    `MsgWaitForMultipleObjects`; #39913 (2025-10) backed it out ("still experiencing bugs with the
    DCompositionWaitForCompositorClock API") and uses `DwmFlush` unconditionally.
    https://git.secluded.site/zed/commit/2d34a4aec90823017566d74c6557409580ce6688,
    https://git.secluded.site/zed/commit/b62f9595286d322e0a78daa73f58921c23a51d03,
    https://git.secluded.site/zed/commit/a1e5f6bb7cc68f55fe3a0405a83bcdd63f2d8f02
  - A comment in `window.rs` notes "only Windows pumps platform events (and thus requests frames) mid-draw", so
    re-entrant draws are deferred [src: `window.rs` ~1800].
- **Linux wgpu**: `preferred_present_mode: Option<PresentMode>`, default `Fifo`; Mailbox suggested for mobile;
  power preference HighPerformance; Vulkan|GL on Linux [src: `gpui_wgpu/src/wgpu_renderer.rs` 118–123,
  `wgpu_context.rs` 374, 476]. An earlier X11 "reduce input latency and ensure rerender priority" change (#13355)
  was reverted because input lagged under load [doc: https://git.secluded.site/zed/commit/82435075a5c727b8231b94f901027d94f37a74ef].
- Latency numbers: Zed publishes no keystroke-to-photon measurements on its blog that I could find; third-party
  sites quote ~2 ms "keystroke latency" without methodology [rep, not trustworthy].

### 1.5 Event dispatch and hit testing

- Mouse events are hit-tested against the **previous rendered frame's** hitboxes (`window.rendered_frame.hit_test(position)`),
  hitboxes are inserted during prepaint (`insert_hitbox(bounds, behavior)`), listeners run in `Capture` then
  `Bubble` phase; pointer capture per hitbox [src: `window.rs` 94–119, 877, 1173, 3383, 5313, 5474].
- Keyboard goes through a `DispatchTree` built each frame (focus path, key contexts, keymap → actions, multi-stroke
  pending input with 1 s timeout) [src: `window.rs` 1258–1264, 1372–1395].
- Threading: UI, layout, paint and present on the main thread; a background executor for async work; Windows has a
  vsync thread; macOS display link callbacks dispatched to the main queue [src + doc 120fps; inf for the general
  statement].

### 1.6 Standalone use

- crates.io `gpui` 0.2.2 (2025-10-22) is far behind main [crates.io]. Longbridge publishes `gpui-pre` snapshots
  (0.3.9 = zed@93d7456, 2026-10-12) and builds on them [crates.io; gpui-component `Cargo.toml` lines 62–81].
- **gpui-component / GPUI Kit 0.8.0** (Longbridge, used in Longbridge Pro): 75+ components, dock layout
  (serialisable splits/tabs), code editor "Stable performance at 200K lines with Tree-sitter highlighting and LSP",
  virtual tables/lists, AccessKit, headless UI tests, wasm, and **`gpui-shell`: a JavaScript extension runtime built
  on QuickJS** (a Longbridge `quickjs-jit` fork of rquickjs) [doc:
  https://raw.githubusercontent.com/longbridge/gpui-component/main/README.md; src: `crates/shell/Cargo.toml`].
- **gpui-fast** (Longbridge, experimental, Apache-2.0, tracks Zed via `UPSTREAM`): makes every view a retained
  subtree with dependency tracking (entities/globals read, list/scroll versions, hover, bounds), keeps Taffy nodes
  across frames, splices unchanged stretches from the previous frame, and replays the AccessKit build log.
  Measured Linux release CPU per frame: unchanged window 3.31 → 0.19 ms (−94 %), sidebar scrolling 5.95 → 0.79 ms,
  streaming quotes 5.06 → 1.35 ms; scroll layers reuse cached GPU tiles [doc: gpui-fast README and
  `docs/retained-mode.md`].
  **Lesson: upstream GPUI's per-frame rebuild costs ~3 ms CPU on an unchanged window; retained + dependency
  tracking removes ~90 %.**

---

## 2. Linebender: Masonry / Xilem, Vello, Parley, Kurbo/Peniko, AnyRender, imaging

### 2.1 Masonry / Xilem

- Repo `linebender/xilem` main: version 0.4.0, MSRV 1.96, crates `masonry_core`, `masonry`, `masonry_imaging`,
  `masonry_testing`, `masonry_winit`, `xilem_core`, `xilem_masonry`, `xilem_web`, `tree_arena`; deps pinned to
  `vello 0.8`, `wgpu 28`, `parley 0.8`, `winit 0.30.13`, `accesskit 0.24`, `ui-events 0.3` and Linebender's new
  `imaging 0.0.1` render abstraction (backends `imaging_vello`, `imaging_vello_hybrid`, `imaging_vello_cpu`,
  `imaging_skia`, `imaging_wgpu`) [src: https://raw.githubusercontent.com/linebender/xilem/main/Cargo.toml].
  Note the lag behind the latest vello (0.11) / parley (0.12) [inf].
- Activity: latest commit 2026-09-14 ("xilem: Expose imaging backends as features #1847"); a few commits a month
  [doc: https://github.com/linebender/xilem/commits/main].
- Q1 2026 status (https://linebender.org/blog/tmil-25/, 2026-04-19) [doc]: Masonry moved onto `imaging` (can run on
  Vello CPU without a GPU), new layout system, new widgets; input via `ui-events` (incl. IME) to reduce reliance on
  winit; core maintainers Raph Levien and Daniel McNab started full-time jobs at Canva; office hours paused until
  2027-01.
- **Pass system** [doc: `masonry_core/src/doc/pass_system.md`, main]:
  - Event passes: `on_pointer_event`, `on_text_event`, `on_access_event`, bubbling from target to root; animation
    pass `update_anim`.
  - Rewrite passes after every event, rerun until flags clear (bounded): mutate, update_widget_tree,
    update_disabled, update_stashed, update_focusable, update_focus, **layout** (measure/layout, only invalidated
    widgets), update_scrolls, **compose** (transforms only: "Transform-only presentation changes (e.g. scrolling)
    should request compose instead of requesting layout"), update_pointer, update_props.
  - Render passes just before a frame: **paint** ("gets a ... Scene description from each widget ... stitched
    together in pre-order") and **accessibility** (AccessKit nodes).
  - External mutation: `RenderRoot::edit_root_widget()` is how Xilem applies its reactive diff; it runs the
    rewrite passes before returning.
- **Paint caching** [src: `masonry_core/src/passes/paint.rs`]: per-widget `scene_cache: HashMap<WidgetId, (pre,
  main, post Scene)>`; only widgets with `request_paint` re-record; all cached scenes are then appended
  (with transforms) into layer scenes each frame (`VisualLayerKind::Scene`), so the whole scene is re-encoded and
  re-rasterised every frame even if one widget changed [src + inf].

### 2.2 Vello family (big change in 2026)

- README (main) [doc: https://raw.githubusercontent.com/linebender/vello/main/README.md]:
  - **Vello CPU** (`vello_cpu`): "the most mature choice"; SIMD + multithreading.
  - **Vello GPU** (`vello_gpu`, formerly `vello_hybrid`): CPU preprocessing, GPU raster/composite via vertex +
    fragment shaders, wgpu and native WebGL2 backends, no compute shaders; "intended to become the primary renderer
    for production GPU use cases as it matures; Vello CPU is currently overall more mature".
  - **Original compute renderer** (`vello`): moved to `research/`, "remains an experimental implementation for
    compute-capable GPUs".
- Sparse strips: paths flattened, binned into tiles, represented as sparse horizontal strips; coverage kept only
  around boundaries [doc: `ARCHITECTURE.md`]. Text via `glifo` (glyph outlines, colour emoji, future atlas
  caching) [doc].
- Q1 2026: Vello Hybrid "roughly beta quality", rough edges, needs performance work; first glyph-caching pass; a
  planned unified "Vello API" proved harder than expected [doc: tmil-25].
- Slint labels its Vello renderer "Experimental, and never selected automatically" (compute) [doc: Slint docs, §5].
- **For cadatomic [inf]:** Vello re-encodes and re-rasterises the whole scene each frame and has no retained GPU
  layers in its API; not production-grade on GPU yet. Fine for an SVG/icon/vector path renderer or the CPU fallback,
  not the core UI or the PCB canvas hot path.

### 2.3 Parley / Fontique / Glifo

- Parley = rich text layout; stack = Fontique (enumeration, fallback, mmap), HarfRust (HarfBuzz port, shaping),
  Skrifa (font reading, scaled/hinted outlines), ICU4X (bidi, segmentation); selection/editing utilities;
  WPT-tested through Blitz [doc: https://raw.githubusercontent.com/linebender/parley/main/README.md].
- Recent: CoreText system enumeration, CSS `text-indent`, all AccessKit text properties; adopters Bevy, CuTTY,
  Gosub, Takumi [doc: tmil-25]. egui 0.37 uses fontique for system fonts (`egui_system_font_provider`) [doc: egui
  0.37.0 release notes].

### 2.4 AnyRender and imaging

- AnyRender (DioxusLabs): `PaintScene` trait + `WindowRenderer`/`ImageRenderer`; backends `anyrender_vello_hybrid`
  (vello_gpu), `anyrender_vello_cpu`, `anyrender_vello`, `anyrender_skia` [doc:
  https://raw.githubusercontent.com/DioxusLabs/anyrender/main/README.md].
- `imaging` (forest-rs) is Masonry's abstraction: "Focuses on performance and a broader set of operations", backends
  Vello CPU, Vello Hybrid, Skia [doc: tmil-25].
- forest-rs also has **`subduction`** ("Retained layer tree and presenter contract for Subduction compositors";
  backends apple/windows/wayland/x11/web/android/wgpu) and **`frameclock`** (display-frame scheduling with
  adapters `frameclock_apple` display-link timing, `frameclock_windows`, `frameclock_wayland` presentation timing,
  `frameclock_x11` Present, `frameclock_web`; example `winit_paced_redraw`) and `invalidation` (dirty tracking /
  topological invalidation, no_std) [doc: https://github.com/forest-rs/frameclock,
  https://raw.githubusercontent.com/forest-rs/subduction/main/subduction_core/README.md,
  https://github.com/forest-rs]. Early stage (few stars, no releases) [doc].

---

## 3. Makepad

- Repo `makepad/makepad`, branch `dev`, version 2.0.0, MIT OR Apache-2.0 [src: `Cargo.toml`]. crates.io still
  `makepad-widgets 1.0.0` (2025-05-13).
- Own platform layer, no winit: `platform/src/os/{apple,windows,linux,web,gpusim}` [src: directory listing]. Apple =
  Metal, Windows = D3D11 (repo description "osx/metal, windows/dx11 linux/opengl"); **Linux now defaults to Vulkan**
  via vendored `ash` + `naga` (WGSL → SPIR-V), falling back to OpenGL ES ("Vulkan when a hardware device answers,
  OpenGL ES otherwise"; `MAKEPAD_GPU=auto|gl|vulkan`) [src: `platform/Cargo.toml` lines 12–27].
- Rendering model: widgets draw with **per-widget shaders** (instanced draw calls; each `Draw*` struct is a shader
  with instance fields). Draw lists are cached: `DrawList2d` + `will_redraw()` — a view whose area was not marked
  for redraw keeps its GPU instance buffers and is resubmitted without re-recording; uploads only when
  `instance_dirty`/`uniforms_dirty` [src: `platform/src/draw_list.rs` 702–727, 1130–1174, 1493–1643;
  `draw/src/cx_2d.rs` 335–360].
- Live design: the 1.x `live_design!` compile-time DSL is being replaced in 2.0 by **Splash**, a runtime
  "live-editable UI DSL and runtime script integration" with hot reload and streaming evaluation [doc: README `dev`;
  rep: https://agent-skills.md/skills/zhanghandong/makepad-skills/makepad-2.0-splash (Splash released 2026-02-12)].
- Performance claims: the site gives no numbers ("Generated UI code should still ship as fast native software") [doc:
  https://www.makepad.nl/].
- **Lesson [inf]:** retained GPU draw lists per container + "redraw only the dirty area's list" gives near-zero
  CPU when idle and cheap partial updates; but every widget being a custom shader language makes CSS-driven content
  (our case) awkward.

---

## 4. Slint

- crates.io 1.18.1 (2026-09-21), master 1.19.0; licence GPL-3.0-only OR royalty-free 2.0 OR Slint Software 3.0 [src:
  `internal/core/properties.rs` header].
- Backends: `qt`, `winit` (default), `linuxkms`; renderers: software (partial rendering, line-by-line), FemtoVG
  (OpenGL, or wgpu variant for Metal/Vulkan/D3D), Skia (OpenGL/Metal/Vulkan/D3D), Vello (experimental, compute) [doc:
  https://docs.slint.dev/latest/docs/slint/guide/backends-and-renderers/backends_and_renderers/].
- **Partial rendering also on Skia**: `PartialRenderingState`, `DirtyRegion`, dirty-region history indexed by buffer
  age (up to 3 back buffers), enabled by `SLINT_SKIA_PARTIAL_RENDERING` or when the surface supports it [src:
  `internal/renderers/skia/lib.rs` 26, 331–431].
- Property binding engine: dependency-tracked properties with pinned intrusive lists (lazy re-evaluation on
  dirty) [src: `internal/core/properties.rs` header]. Slint master depends on wgpu 29 and 30 for its wgpu interop
  feature (render your own wgpu content under/over the UI) [src: Slint `Cargo.toml` 157–165].
- **Lesson [inf]:** compile-time property graph + item tree + dirty regions = minimal repaint; buffer-age-aware
  damage is the technique to copy for the CPU/compositor-limited case. Latency of its winit backend inherits winit's
  redraw scheduling (no Slint-specific measurements found).

---

## 5. Iced 0.14

- 0.14.0 released 2025-12-07; master `0.15.0-dev`; wgpu 30, a forked winit (`iced-rs/winit` rev 48116469), forked
  cosmic-text, `cryoglyph` (glyphon fork), lyon, tiny-skia 0.12 + softbuffer [src: iced `Cargo.toml` 170–249;
  doc: CHANGELOG].
- **Reactive rendering** (PR #2662, merged 2024-11-13, shipped 0.14): redraw only after an `update` or when a widget
  requests one; widgets track last-drawn status; `Widget::on_event` renamed `update`, `Shell::capture_event`;
  `unconditional-rendering` feature restores always-redraw [doc: https://github.com/iced-rs/iced/pull/2662].
  Previously mouse moves produced "hundreds of identical frames". No measurements published.
- Elm architecture: `view()` rebuilds the widget tree after each update and layout reruns [inf from the
  architecture; iced has `canvas::Cache` geometry caching and primitive culling in `column`/`row` (#2611) and
  "basic layer merging" (#3033)] [doc: CHANGELOG].
- Compositor: `desired_maximum_frame_latency: 1`, present mode `AutoVsync` or `AutoNoVsync`, overridable by env
  [src: `wgpu/src/window/compositor.rs` 291–434].
- Also: time-travel debugging, `comet` devtools with presentation metrics, headless E2E tests, hot reloading (#3000),
  IME (#2777) [doc: CHANGELOG].

## 6. Floem (Lapce)

- crates.io 0.2.0 (2024-11-14); repo main active. Deps: forked winit (`lapce/winit` → `floem-winit`), wgpu 27,
  taffy 0.9.2, parley 0.7, peniko 0.6; renderers vger (default GPU), vello, AnyRender Skia, tiny-skia fallback
  [src: https://raw.githubusercontent.com/lapce/floem/main/Cargo.toml; doc: README].
- **Fine-grained signals + retained view tree**: "The view tree is constructed only once"; signals inspired by
  leptos_reactive; effects send `UpdateMessage`s to specific views (`id.update_state`), which mark style/layout dirty
  [doc: README; src: `src/window/handle.rs` 1102–1123].
- Frame: `process_update_messages` (budgeted, up to 32 iterations) → `style()` (only `style_dirty` views) →
  `layout()` (taffy) → `commit_box_tree()` (understory box tree for hit testing) → `paint()`; **paint walks the
  whole tree every painted frame** (background fill + `paint_with_traversal(root)`), no per-view paint cache
  [src: `src/window/handle.rs` 593–945].
- **Lesson [inf]:** Floem is the closest Rust analogue to Svelte's model (signals → targeted view updates, retained
  tree, incremental style/layout). Its weak spot is full repaint; pair the model with GPUI/Makepad-style paint
  replay.

## 7. Blitz / dioxus-native

- Blitz: "A modular HTML/CSS rendering engine", **beta**; renders many no-JS sites; `blitz-dom` uses Stylo (CSS),
  Taffy (layout), Parley (text); `blitz-paint` → AnyRender; `blitz-shell` = winit + AccessKit + muda [doc:
  https://raw.githubusercontent.com/DioxusLabs/blitz/main/README.md].
- **DOM mutation API** (`DocumentMutator`): `create_element`, `create_text_node`, `set_node_text`, `set_attribute`,
  `clear_attribute`, `set_style_property`, `append_children`, `insert_nodes_before/after`, `replace_node_with`,
  `remove_node`, `set_custom_widget`, `set_sub_document`, templates (`deep_clone_node`, `template_contents`),
  `node_at_path` [src: `packages/blitz-dom/src/mutator.rs`].
- Paint: whole document repainted into an AnyRender scene per frame; no paint caching found in `blitz-paint/src/lib.rs`
  [src grep: no cache/damage symbols; inf].
- **Lesson [inf]:** the mutator API is the right shape for a Svelte runtime → Rust seam (svelters already has its
  own); Stylo is a heavyweight but complete CSS alternative to svelters' Rust CSS engine.

## 8. Freya, Vizia

- Freya 0.5 (beta.12/rc.10, 2026-10): own component model ("Any type that implements the `Component` trait"),
  `freya-skia-safe 0.104` (Skia) on `glutin` GL, Torin layout, winit 0.30.13, AccessKit 0.25.1; `freya-core` has
  `render_pipeline`, `layers`, `text_cache`, `rendering_ticker` modules [src: Freya `Cargo.toml`,
  `crates/freya-core/src/lib.rs`; doc: README].
- Vizia 0.4.0: `vizia_core` on skia-safe + morphorm layout + `vizia_reactive`, backends winit or baseview (audio
  plugins), AccessKit [src: vizia `Cargo.toml`, `crates/vizia_core/Cargo.toml`].
- Neither publishes latency measurements [inf: none found].

---

## 9. egui / eframe: why it is slow for cadatomic

- egui README (0.37.0) [doc: https://raw.githubusercontent.com/emilk/egui/0.37.0/README.md]: immediate mode "does a
  full layout each frame"; "For most cases you can expect egui to take up 1-2 ms per frame"; sizing paradox → store
  size for next frame (one-frame delay/flicker) or a second pass ("request_discard", `max_passes`), done "only ... in
  very rare circumstances"; idle when nothing changes.
- Tessellation: shapes are tessellated to meshes on the CPU each frame (epaint) [inf from egui architecture;
  0.37 adds post-tessellation transforms #8587].
- **Measured latency** (issue #7761, Windows, 120 Hz, vsync): ~2 frames (~17 ms) on glow vs **~3 frames (~25 ms)
  on wgpu**; wgpu also used 215–265 MB VRAM vs 31 MB glow; startup 1–7 s vs 100–300 ms [doc/rep:
  https://github.com/emilk/egui/issues/7761, opened 2025-12-09, still open].
- Fix attempts: PR #8103 set `desired_maximum_frame_latency = Some(1)` by default (merged 2026-04-14), reverted
  2026-05-18 (#8167, choppy macOS resize), re-landed 2026-05-26 (#8203, disabling `present_with_transaction` when
  latency==1) [doc: https://github.com/emilk/egui/pull/8103]. Wumpf warned latency 1 "likely stops CPU/GPU
  parallelism" [doc].
- **In the 0.37.0 tag, `WgpuConfiguration::default()` uses `SurfaceConfig::HIGH_THROUGHPUT`
  (`AutoVsync`, `desired_maximum_frame_latency: Some(2)`); `SurfaceConfig::LOW_LATENCY` (latency 1, None on iOS)
  exists but is opt-in** [src: https://raw.githubusercontent.com/emilk/egui/0.37.0/crates/egui-wgpu/src/lib.rs
  lines 68–100, 382–386]. *Actionable now for cadatomic: set `wgpu_options.surface = SurfaceConfig::LOW_LATENCY`.*
- eframe run loop (0.37.0) [src: `crates/eframe/src/native/run.rs`]: `ControlFlow::Wait`/`WaitUntil(next_repaint)`,
  repaint = `window.request_redraw()` → work done in `RedrawRequested`; on Wayland `RedrawRequested` waits for the
  compositor frame callback; a timeout runs logic if `RedrawRequested` is overdue; invisible windows on Windows get
  no `RedrawRequested`.
- egui 0.37 deps: wgpu 30, winit 0.30.13, accesskit 0.25, fontique 0.11, skrifa 0.44 [src: egui 0.37.0
  `Cargo.toml`]. 0.37 also fixed "a burst of extra repaints before request_repaint_after deadlines" (#8595) and added
  `Context::set_repaint_observer` [doc: 0.37.0 release notes].
- **Where cadatomic's 2–3+ frames come from [inf]:** (1) egui-wgpu default frame latency 2 + FIFO (≈ 2 queued
  frames); (2) input → `request_repaint` → winit `RedrawRequested` on the next loop iteration/vsync; (3) svelters JS
  handlers run on the *next* egui frame after the event, then the scene mutation shows one frame later still; (4)
  full rebuild + tessellation each frame at scale.

## 10. winit

- Stable 0.30.13 (2026-03-02); 0.31.0-beta.4 (2026-10-24): trait-based `ActiveEventLoop`/`Window`,
  `can_create_surfaces`, unified `Pointer*` events (touch/pen folded in), IME `request_ime_update`/`ImeHints`,
  native popups with positioner, clipboard API, X11 smooth resize via sync extension, MSRV 1.85, split into
  `winit-win32`, `winit-appkit`, `winit-wayland`, `winit-x11`, … [doc: https://github.com/rust-windowing/winit/releases;
  src: repo tree].
- **No frame clock / vsync API.** macOS: `request_redraw` pushes the window into `pending_redraw`, drained at the end
  of the run-loop iteration — no `CVDisplayLink`/`CADisplayLink` [src: `winit-appkit/src/app_state.rs` 50, 288–290,
  371]. Wayland is the only backend with a compositor frame callback; Windows uses `WM_PAINT`/`RedrawWindow`
  [src: `winit-win32/src/event_loop.rs` 1397–1414]. Apps pace frames themselves (WaitUntil deadlines), as winit
  maintainers suggest [doc: https://github.com/rust-windowing/winit/discussions/3377; issue
  https://github.com/rust-windowing/winit/issues/2412 "frame pacing hard to generalise"].
- **Windows modal loops:** title-bar click enters the modal loop (≈ 500 ms pause, mitigated by posting a dummy
  `WM_MOUSEMOVE`); right-click system menu still freezes the event loop; size/move is flagged via
  `WM_ENTERSIZEMOVE`/`WM_EXITSIZEMOVE` [src: `winit-win32/src/event_loop.rs` 1319–1365]. During modal resize the app
  only gets `WM_PAINT`-driven redraws [inf].
- macOS: continuously-rendering windows lag 200–500 ms when resized by window managers (issue #3644) [rep:
  https://github.com/rust-windowing/winit/issues/3644]; FIFO `nextDrawable` blocking on the main thread is a known
  pitfall (field reports) [rep: https://github.com/tecs-dev/tecs/pull/10].
- Windows supports `with_no_redirection_bitmap` (`WS_EX_NOREDIRECTIONBITMAP`), so a winit HWND can host a
  DirectComposition tree [src: `winit-win32/src/lib.rs` 549–551; winit 0.30.13 `platform/windows.rs` 489–561].
- No pointer-event coalescing API (`getCoalescedEvents`-style) found [inf: not in 0.31 release notes].
- Several frameworks fork winit (iced, Floem) [src].
- **Verdict [inf]:** a custom platform layer is justified on **Windows** (frame clock thread on `DwmFlush`,
  DComp + flip-model swapchain, avoiding modal-loop stalls, DirectManipulation scrolling) and **macOS**
  (display-link-driven frames, ProMotion handling, transactional present during resize). On Linux, winit or GPUI's
  own Wayland/X11 code are both viable. A middle path: keep winit for windows/input/IME and own the frame clock and
  presentation (frameclock crate, or a port of GPUI's `vsync.rs` / display-link code).

## 11. wgpu (30.0.1)

- Latency knobs: `PresentMode` (`Fifo`, `FifoRelaxed`, `Mailbox`, `Immediate`, `AutoVsync`, `AutoNoVsync`) and
  `SurfaceConfiguration::desired_maximum_frame_latency` (Vulkan: swapchain images = latency + 1; GL ignores it;
  DX12 + Mailbox limits framerate) [doc: egui PR #8103 thread].
- DX12 swapchains are always created with `DXGI_SWAP_CHAIN_FLAG_FRAME_LATENCY_WAITABLE_OBJECT` and
  `ALLOW_TEARING` when supported [src: `wgpu-hal/src/dx12/mod.rs` 1379–1387].
- Surface targets: `SurfaceTargetUnsafe::CoreAnimationLayer(*mut c_void)` (CAMetalLayer),
  `CompositionVisual` (IDCompositionVisual, DX12), `SurfaceHandle` (DComp surface handle), `SwapChainPanel`,
  `Drm`, `RawHandle` [src: `wgpu/src/api/surface.rs` 376–456].
- Open PR #8388 "Implement DXGI Swapchain on Vulkan" (opt-in; ~100 µs overhead; "more control over frame pacing";
  changes requested 2026-10-07) [doc: https://github.com/gfx-rs/wgpu/pull/8388]. Related #9559 (acquire latency on
  AMD Immediate), #10548 (`DXGI_PRESENT_RESTART`), #10641 (wait on acquire fence only for FIFO on Windows) [doc].
- Overhead vs native: Zed says native renderers "probably have better performance and wider compatibility"
  [doc]; egui #7761 measured large VRAM/RAM overhead on DX12/Vulkan vs GL (215–265 MB vs 31 MB) [rep]; an
  empty-window comparison 100 MB vs 10 MB was disputed as not under load [doc: PR #46758]. Zed nevertheless chose
  wgpu for Linux [doc].
- **Lesson [inf]:** for cadatomic, wgpu everywhere has a decisive advantage: UI and 3D viewports share one device
  and queue, so viewport textures are sampled directly by the UI pass (no cross-API sharing). Use latency 1,
  Mailbox where available, own pacing, and DComp/CAMetalLayer surfaces for compositor control.

## 12. Other building blocks

| Block | Version | Use | Notes |
|---|---|---|---|
| taffy | 0.15.0 (2026-10-12) | layout | Used by GPUI (cleared per frame), Floem, Blitz, svelters. Keep the tree across frames so its cache survives (gpui-fast). [crates.io; doc gpui-fast] |
| parley + fontique | 0.12.0 | text layout, fallback | HarfRust + Skrifa + ICU4X; AccessKit text props. [doc] |
| cosmic-text | 0.19.0 | text layout alt. | Zed's Linux text system; iced forks it. [src] |
| swash | 0.2.10 | glyph rasterisation | Zed Linux uses it with cosmic-text. [src] |
| skrifa / read-fonts / harfrust | 0.49 / 0.46 / 0.15 | font reading / shaping | fontations stack, very active. [crates.io] |
| glyphon | 0.12.0 | wgpu text atlas | iced uses a fork (cryoglyph). [src] |
| accesskit | 0.25.1 | a11y | GPUI 0.24, egui/Freya 0.25. [src] |
| wgpu | 30.0.1 | GPU | see §11 |
| lyon | 1.0.19 | path tessellation | GPUI depends on lyon. [src] |
| femtovg | 0.27.0 | canvas-API GL/wgpu 2D | Slint renderer; "quality sometimes sub-optimal" (Slint docs). [doc] |
| kurbo / peniko | 0.13.1 / 0.6.1 | curves / brushes | shared vocabulary of Linebender/AnyRender/Floem. [crates.io] |
| vello_cpu / vello_gpu | 0.3.0 | 2D vector raster | CPU mature; GPU beta. [doc] |
| etagere | — | atlas packing | used by GPUI. [src] |
| frameclock / subduction | unreleased | frame pacing / compositor layers | early. [doc] |
| ui-events | 0.3.0 | input event types | Masonry, Floem. [src] |

---

## 13. Comparison table

| Framework | Scene/element model | Per-frame work | Renderer | Platform layer | Hit test / events | Latency work |
|---|---|---|---|---|---|---|
| GPUI (Zed) | entities + views; element tree from `render()` each frame; `cached` views replay | rebuild elements, Taffy cleared, prepaint/paint; cached views replay paint ranges | instanced typed primitives (quad/shadow/path-MSAA/underline/mono/subpixel/poly sprites/surface), atlas text | own: AppKit+Metal, Win32+D3D11+DComp, Wayland/X11+wgpu, web | prev-frame hitboxes, capture/bubble, keymap dispatch tree | display link / DwmFlush thread, present-sync 0, 1 s keep-present on high-rate input, thermal/inactive throttling |
| gpui-fast | GPUI + dependency-tracked retained views | reuse unchanged views, layout nodes, text; GPU tile scroll layers | GPUI | GPUI + native composition | GPUI | −73…−94 % CPU/frame measured |
| Masonry/Xilem | retained widget tree, Xilem diffs into it | invalidation-flag passes; per-widget scene cache; full scene re-raster | imaging → Vello CPU/GPU/Skia | winit 0.30 + ui-events | pointer target + bubbling | none published |
| Makepad | widgets as shaders, draw lists | only dirty draw lists re-recorded; resubmit all | own instanced shaders; Metal/D3D11/Vulkan(GL ES fallback)/WebGL | own | area-based | none published |
| Slint | compiled item tree + property graph | dirty properties → dirty regions → partial repaint | software / FemtoVG / Skia / Vello(exp.) | winit / Qt / KMS | item tree | partial rendering, buffer-age |
| Iced 0.14 | Elm: `view()` rebuilds widget tree | redraw only on update or widget request | wgpu (SDF quads, cryoglyph text) / tiny-skia | forked winit | widget tree | frame latency 1; reactive rendering |
| Floem | signals + retained views, built once | dirty style/layout; full repaint | vger / vello / skia / tiny-skia | forked winit | understory box tree | budgeted updates |
| Blitz | DOM (Stylo+Taffy+Parley), mutator API | restyle/relayout dirty; full repaint | AnyRender (vello_gpu/cpu/skia) | winit | DOM events | none |
| Freya / Vizia | components / reactive views | incremental (Torin) / reactive | Skia | winit (+baseview) | tree | none |
| egui 0.37 | immediate | full UI + layout + tessellation per frame | epaint meshes → wgpu/glow | winit 0.30 | same-frame rects | default frame latency 2; ~3 frames measured |

---

## 14. What this means for cadatomic (inferences)

1. **Biggest latency wins are scheduling, not rasterisation:** (a) handle input → run the Svelte handler →
   apply scene mutations → layout → paint → present *in the same frame* (not the next); (b) one queued frame
   (`desired_maximum_frame_latency = 1`) plus own frame clock (GPUI `vsync.rs` / display link / frameclock);
   (c) keep presenting ~1 s after high-rate input (GPUI `InputRateTracker`) so VRR/ProMotion displays don't
   downclock; (d) on Windows, flip-model swapchain through DirectComposition, pace with `DwmFlush` on a thread.
2. **Idle CPU ≈ 0 needs retained everything:** retained scene (svelters already has it), persistent Taffy tree,
   cached text layouts, per-subtree recorded primitive ranges replayed when clean (GPUI `Scene::replay`, Makepad
   draw lists, Masonry scene cache), and damage/layers for big scrolling surfaces (gpui-fast scroll layers, Slint
   partial rendering).
3. **Renderer:** a GPUI-style instanced primitive renderer (SDF rounded quads with borders, shadows, MSAA paths via
   lyon, glyph/emoji atlases, image sprites, plus a "viewport texture" primitive) on wgpu is small (thousands of
   lines, not tens of thousands) and proven at 120 Hz. Vello is not production-ready on GPU and re-rasterises
   everything each frame.
4. **Forking GPUI vs writing:** GPUI's element/entity model is a second UI model next to svelters' scene; its
   platform crates are bound to `gpui`'s `PlatformWindow` traits; its Windows/macOS renderers are D3D11/Metal,
   which cannot sample cadatomic's wgpu viewport textures without cross-API sharing; crates.io releases are stale
   and API churn is high (Longbridge maintains snapshots). Copying (Apache-2.0, with attribution) the platform
   frame-pacing code, DComp swapchain setup and instanced shaders into our own crates gets most of the value
   without the model mismatch. gpui-kit (dock, 200K-line editor, QuickJS extension shell) is the strongest
   "adopt instead" alternative and should be evaluated once as a benchmark.
