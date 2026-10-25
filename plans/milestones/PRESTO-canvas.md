# PRESTO-canvas: viewports on OS layers and native canvas layers (batch presto-b3, lane CANVAS)

Branch `ci/presto-canvas` from `ci/presto-base` (6f66dcdb). Brief: `plans/cloud/presto-b3/LANE-canvas.md`,
`plans/cloud/presto-b1/BRIEF.md`. Plan rows W-V1, W-V2 (presto side).

## Landed

### W-V2: `crates/presto-canvas` (new crate)

- **`world`:** `f64` world space (`DVec2`, `WorldRect`), `Camera { center, zoom, y_axis }` (zoom about a
  point, pan by a drag), and `Projection`. A projection maps a chunk's `f32` offsets to device pixels with
  one affine. Its translation `(origin − center)·zoom` is computed in `f64`, so the GPU never sees a
  large coordinate. Tested exact at 0.5 m from the origin at 1 px/nm.
- **`chunk`:** typed instance chunks (`ChunkBuilder`):
  - analytic records (`ShapeInstance`, 40 bytes): rects (fill, stroke, rounded), segments (butt, round
    and square caps), circles and rings, arcs;
  - filled polygons, tessellated by lyon at build time;
  - text labels.
  Each kind is sorted by extent, so LOD is a prefix (binary search, no record touched). Labels split
  by on-screen em size into Slug glyphs (≥ 6 px), greeked bars (≥ 1.5 px) or nothing. A minimum
  stroke width in device pixels keeps hairlines visible.
- **`canvas`:** `Canvas` (shared `Arc`) with a **static** and an **overlay** plane. `Canvas::edit` builds
  and publishes a whole `Snapshot` atomically (chunk lists of `Arc`s). The renderer takes one snapshot
  per frame.
- **`cull`:** `plan()` does per-chunk culling against the world rect the device clip sees, plus LOD and
  the label split.
- **`slug` + `text`:** Slug GPU text.
  - CPU side: skrifa outlines become quadratics in ems (cubics split in 4, lines as degenerate
    quadratics), with up to 16 horizontal and vertical bands per glyph, sorted for the early exit.
  - `coverage()` is a Rust mirror of the shader. It is tested on squares, holes, a cubic circle and
    real Noto Sans glyphs (`l` stem, `o` counter).
  - `CanvasFonts` holds registered fonts, a one-line layout (charmap and advances, no shaping) and a
    shared glyph cache.
  - The GPU `GlyphTable` (records, curves, bands) is append-only.
- **`gpu`:** `CanvasRenderer`.
  - `prepare` plans each placement, uploads chunk buffers once (keyed by chunk version and evicted after
    120 undrawn frames), syncs the glyph table and writes one dynamic-offset uniform per chunk draw.
    It also renders the static plane into a per-canvas cache texture when the cache key changes (plane
    revision, projection, LOD, rect).
  - `draw` (inside the host's pass) blits the cache, or draws the static plane directly once the key
    has changed two frames in a row (the view is moving), then draws the overlay on top.
  - Pipelines: shapes (SDF, 1 px AA), polygons, Slug glyphs (dilated quads) and the cache blit.
  - A pan or zoom changes only the uniforms.

### W-V2 in presto-render (cross-lane: RENDER's crate)

- `Kind::Canvas` batches: `Prim::Canvas` becomes a `CanvasDraw` (rect transform, clip without the frame's
  damage, so the cache survives damage changes).
- `Renderer::{register_canvas, unregister_canvas, set_canvas_view}`; `RenderStats::canvas` (`CanvasStats`).
- Canvases are prepared before the main pass and drawn per damage region under `clip ∩ region`. The
  renderer re-binds its own groups afterwards.

### W-V1 and W-V2 in presto-compositor (cross-lane: COMPOSITOR's crate)

- **`viewport` (new):** the UI-thread `Viewport`.
  - **Promotion:** `create_os_layer`, a wgpu surface from its `SurfaceTarget` (`viewport::create_surface`),
    then `add_os_layer`.
  - **Fallback:** when the platform has no layer or no surface can be made, a `LayerKind::Texture` layer
    drawn like `Prim::Surface`.
  - `layer_descs` builds the layers for the UI's commit. On an OS layer, the texture layer is drawn at
    the texture's own pixel size, top-left, so an old-size image is cut, never scaled.
  - `frame_ready` → `CompositorHandle::texture_updated`.
  - `set_bounds` runs the resize handshake in an order that never shows a stretched frame:
    - platform holds the layer (Wayland sync subsurface): set bounds → new-size frame → end;
    - otherwise: new-size frame first → move the layer.
  - `remove` drops the compositor's surface synchronously before the platform destroys the layer.
- **`CompositorHandle` (additive):**
  - viewports: `resize_os_layer_sync` (draws and presents the new size with every arrived commit, then
    answers), `remove_os_layer_sync`, `texture_updated`;
  - canvases: `register_canvas`/`unregister_canvas`, `set_canvas_camera(window, canvas, camera, lod,
    input_seq)` (the camera as a compositor property), `canvas_changed`.
- **Tree:** `LayerKind::Canvas(id)` layers draw a synthetic `Prim::Canvas` over their bounds. Dirty
  canvases (camera, `FastCommit::CanvasView`, `canvas_changed`) and dirty textures damage the flattened
  items that show them. `FastCommit::CanvasView` was a no-op on screen before: it set `canvas_views` but
  damaged nothing. It is now the canvas's post-camera view transform.
- **`Backend`/`SceneRenderer`:** `register_canvas`, `unregister_canvas`, `set_canvas_view`. `GpuBackend`
  keeps canvases across device loss. `Engine::insert_os_layer` registers a layer without a surface (tests).

### presto-platform (cross-lane: PLATFORM's crate, additive)

- **`PlatformCx::begin_os_layer_resize` / `end_os_layer_resize`:** default methods (`false` / no-op), so
  the backend lanes' crates keep compiling.
- **winit backend:**
  - Wayland: the subsurface goes to sync mode; the end returns it to desync and commits, which applies
    the cached viewport destination and the new buffer together.
  - Windows (DComp shows a swap chain at its own size) and generic: nothing to hold.
  - macOS: `create_layer` now sets `contentsGravity = topLeft` and `masksToBounds`, so a drawable whose
    size lags the frame is never scaled.
- **Headless:** records `PlatformCall::{BeginOsLayerResize, EndOsLayerResize}`.

### Tests (written; the ones I ran are noted)

- `presto-canvas`: unit tests (camera, chunk LOD, polygons, Slug coverage, snapshots, WGSL validation,
  record layouts); `tests/million.rs`; `tests/text.rs` (real font). **Run once locally: all pass.**
  - `tests/million.rs`: 1,000,448 primitives in 1024 chunks, board 0.25 m from the origin. A 60-frame
    pan at three zooms. CPU plan cost per frame, debug build: whole board 542 µs (1024 chunks
    visible, LOD keeps 690,579 of 1,000,448 records), 4 mm 19 µs (24 chunks), 20 µm 7 µs (4 chunks).
    The assert is < 4 ms per frame.
- `presto-compositor/tests/viewport.rs`, **run once locally: all pass.**
  - **The W-V1 counter:** 120 orbit frames on an OS layer draw only `SurfaceKey::Os`, with 0 commits
    and 0 `BeginFrame`.
  - The fallback redraws the window only inside the viewport's rect.
  - The handshake draws a full new-size frame before answering.
  - 20 camera pans draw the canvas rect only, with no commit; a `CanvasView` fast commit redraws too.
- `presto-render/tests/canvas.rs` (GPU, `PRESTO_GPU_TESTS=1`): shape pixels at the camera position, a
  cache hit when nothing changed, direct drawing while moving, the overlay over the cache, and a Slug `l`
  stem covered. **Not run (no GPU in the container): CI decides.**
- Checked: `cargo check --all-targets` for presto-canvas, presto-render, presto-compositor,
  presto-platform (Linux), presto, presto-test, presto-egui-island, svelters-presto, presto-widgets and
  presto-core.

## Not done, or partial

- **`[local]` gates (for the coordinator to queue in cadatomic's `docs/plan/REVISIT.md`; svelters lanes do
  not edit cadatomic):**
  1. PCB pan at display rate with 10⁶ primitives on real GPUs (iGPU, discrete).
  2. Resize without stretched frames per OS: Wayland sync-subsurface, macOS top-left gravity, DComp.
  3. The Wayland handshake with Mesa FIFO: whether the present inside sync mode can block on a frame
     callback.
- **The macOS and Windows shim edits compile only in CI** (cfg-gated).
- **Polygons are not antialiased** (triangles, no MSAA in the main pass). Copper pours show aliased
  edges unless the app strokes their outlines with min-width segments. Options: 4× MSAA intermediates
  like presto-render's paths, or analytic edge AA.
- **Labels are not shaped:** charmap and advances only, enough for designators, nets and values. Complex
  scripts would need presto-text shaping, feeding glyph ids.
- **Not done:**
  - atlas glyphs for 6–8 px labels: they become bars or Slug instead;
  - glyph-table eviction;
  - a spatial index over chunks (the full-board scan costs 542 µs for 1024 chunks in debug);
  - reusing the cache by offset during a pure translation.
- **Rounded clips and `PushClip`s inside a list** do not clip a canvas: only the item's clip and the
  damage scissor do.
- **The macOS live-resize model** (present on the main thread with `presentsWithTransaction`) is not
  needed for sublayers with top-left gravity; the window-level one stays COMPOSITOR/MACOS's.
- **wgpu fork patches** (out of scope; where they would help):
  - P1 `present_with_damage`: OS-layer and canvas presents are full presents today; with P1 a viewport
    orbit would present only its layer.
  - P3: CAMetalDisplayLink pacing of sublayers.
  - P4: off-main acquire on the sublayer surfaces.

## Cross-lane edits

- `crates/presto-render`: canvas batches, as above, needed to draw `Prim::Canvas` (brief: "drawn by
  presto-render as a layer kind").
- `crates/presto-compositor`: viewports and the camera property (brief).
- `crates/presto-platform`: two default `PlatformCx` methods plus the shims (brief: "a trait you define in
  presto-platform for them to implement"). The own-backend lanes (WIN32, MACOS, WAYLAND) inherit the
  defaults. WAYLAND should override them with sync/desync, as `os/wayland.rs` does.
- `THIRD_PARTY_NOTICES.md`: a presto-canvas section (Slug reference shaders, MIT; Quílez `sdArc`).
- `Cargo.toml`: the workspace member and dependency `presto-canvas`.
- No change to `presto-base`.

## Questions for the coordinator (decided by the coordinator, 2026-10-25: Q1 as recommended; Q2: CADATOMIC adds the gates to cadatomic's REVISIT.md; Q3: a follow-up)

1. **Camera precision.** The `f64` camera is a compositor property (`set_canvas_camera`), not a
   `FastCommit`. `FastCommit::CanvasView`'s `f32` affine cannot hold nm-precision pans far from the
   origin. Adding a variant to `presto-base` would break exhaustive matches (`presto/src/app.rs`).
   *Recommendation:* keep the handle method. Add `FastCommit::CanvasCamera` with `f64` fields at the next
   breaking `presto-base` window, so presto's dispatch can return it in `Dispatched.fast`.
2. **REVISIT entries.** Queue the three `[local]` gates above in cadatomic's REVISIT.md. *Recommendation:*
   yes, under M4.
3. **Polygon AA.** *Recommendation:* a follow-up with 4× MSAA intermediates per canvas cache, measured
   on lavapipe and an iGPU first.
