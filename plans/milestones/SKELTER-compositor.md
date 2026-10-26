# SKELTER-compositor: skelter-compositor (batch presto-b1, lane COMPOSITOR)

Branch `ci/presto-compositor`. Brief: `plans/cloud/presto-b1/BRIEF.md`, `LANE-compositor.md`. Owns
`crates/skelter-compositor/**` only.

## Landed

- **API** (brief signatures kept): `Compositor::spawn(cfg, glyphs, to_ui, wake_ui) -> CompositorHandle`;
  `CompositorHandle` (`Send + Clone`): `instance`, `add_window`, `add_os_layer`, `remove_window`, `commit`,
  `fast`, `resize` (→ `ToUi::ResizeDone`), `set_visible`, `register_texture`, `shutdown`.
  Additive: `CompositorConfig::new(clock, refresh)`, `gpu() -> Option<GpuContext>`
  (device/queue for egui islands and offscreen viewports), `resize_os_layer`, `remove_os_layer`,
  `unregister_texture`, `set_refresh`, `needs_ui_frame(window, bool)`, `ui_frame_skipped`, `ui_cost`,
  `ui_input_cost`, `high_rate_input`, `fling`, `smooth_scroll`, `set_frame_source` (a
  `skelter_platform::FrameSource` per window, waited on by a `skelter-vsync-<id>` thread that posts ticks,
  occlusion and feedback), `set_os_layer_committer` (DComp `Commit` after every configure),
  `os_compositing()` (true only with DX12, for `WindowDesc::os_compositing`), `frame_args(window)`
  (the frame clock's `FrameArgs` snapshot, republished after every clock update; APP-TEST #6), `set_caret_blink`.
- **Thread** (`lib.rs::run`): one per process; one blocking wait (`recv_timeout` to the engine's next
  deadline), then every queued message, then what is due. The `Instance` is created on the calling
  thread so the main thread creates surfaces at once; the device is created with the first surface.
- **GPU** (`gpu.rs`): backends DX12 only on Windows, Metal on Apple, Vulkan then GL elsewhere; adapters
  ranked by power preference, software (lavapipe/WARP/llvmpipe) last, or chosen by
  `CompositorConfig::adapter` (cadatomic's `adapter::select`) with `CompositorConfig::limits` for
  `required_limits`, the same after device loss; non-sRGB format; Fifo,
  `desired_maximum_frame_latency = 1`; a persistent target per surface, encode first and **acquire late**,
  copy the whole target into the acquired image (one blit; damage limits rendering, not the copy, so
  no buffer-age assumption); surfaces
  without `COPY_DST` redraw whole; `Outdated` reconfigures, `Suboptimal` reconfigures after present;
  GPU completion through `on_submitted_work_done` fired by a waiter thread blocking on each submission
  (no polling in the loop); device loss: `set_device_lost_callback` → rebuild device/queue/renderer,
  reconfigure every surface, full damage, `ToUi::DeviceReset`; uncaptured wgpu errors are logged, not
  panics. Offscreen targets (`attach_offscreen`) and `read_pixels` for headless runs and screenshots.
- **Committed tree** (`tree.rs`): layers, the four property trees, lists, runs, images, paths; commits
  applied in order (folding); world transforms with scroll translation; clip chains (rounded kept when
  axis-aligned and innermost); effect opacity products; flattening per surface (a layer's items, then
  children by `(z, insertion)`; OS-layer subtrees to their own surface in layer space; texture layers as
  a cached `Prim::Surface` list). Revision checks (architecture §4.5): a fast commit not newer than the
  applied commit is rejected; an older commit keeps the fast-owned field (scroll offset, local transform,
  opacity) until a commit at or past the fast revision releases it; offsets a compositor fling owns are
  kept too.
- **Damage** (`damage.rs`): diff of flattened items against the previous frame per surface (list,
  resolved transform, clip, opacity, paint predecessor), plus `LayerDelta::Damage` and the caret; device
  rects with a 1 px margin, merged, capped at 8; empty damage = no render, no present.
- **Engine** (`core.rs`): `skelter_frame::FrameClock` per window behind the `FramePacer` trait, every
  action handled (`ApplyCommit` gates which pending commits a draw shows, `ForceFullRedraw`, timers,
  `BeginUiFrame`); signals sent back: `Submitted`, `Discarded` (no damage), `DrawFailed` (acquire
  skipped), `GpuDone`, `Presented` (estimated from GPU completion on the timer grid when no platform
  feedback), `Visible`, `AnimationsActive`, `DeviceLost`. Timer vsync at the refresh interval until a
  platform `FrameSource` posts ticks. Compositor animations (opacity, transform, clip, scroll offset)
  sampled with `skelter_anim::sample` at the frame target, `Started`/`Finished`/`Aborted` events, fill
  forwards holds, pause/resume, retarget (springs through `skelter_anim::retarget`). Flings own their
  scroll node and report `ToUi::ScrollMoved` per frame; a scroll fast commit stops them. Caret from
  `FastCommit::Caret`, blinking per window by `CaretBlink { cycle, timeout }` (on for the first half of
  each cycle from the last caret change, steady after the timeout; defaults GTK 1200 ms/10 s, Qt 1000 ms,
  Windows and macOS 1060 ms; `set_caret_blink`; TEXT-INPUT R3, decided as Q2). `ToUi::Presented` carries the input seqs applied in
  that frame, followed by `ToUi::FrameTimings` (draw, submit, GPU done, present; APP-TEST #7).
  APP-TEST #8 holds: a `BeginFrame` is not sent while a commit is unapplied and goes out after the draw
  that applies it if the UI is still dirty (test); #10: the interval pacer is gone, `ui_frame_skipped`
  → `Signal::UiFrameSkipped` frees the clock (test). Resize handshake: reconfigure, draw every arrived commit, present, `ResizeDone`.

## Seams (stubbed until other lanes land)

- `seams::SceneRenderer`: implemented by `skelter_render::Renderer`, the default (`CompositorConfig::new(clock,
  refresh)`); the trait stays so tests can record draws. RENDER's contract is kept: non-sRGB
  targets, `end_frame` once per submitted frame after `submit` (also when acquire fails: the encoder is
  submitted, not dropped), `ResourceDelta` removals forwarded to `forget_image`/`forget_path`.
- Scroll motions are skelter-anim's (`scroll::Fling` relative to the fling's start, `SmoothScroll`
  absolute, velocity-keeping retarget); no seam left. The rubber band (`RubberBandReturn`, elastic
  overscroll past an edge) is not driven by the compositor yet: flings clamp at the edges.

## Not implemented (listed, not faked)

- Present with damage (wgpu 30 has none; patch P1): presents are full; damage limits rendering and copies.
- DXGI waitable (`DontWait` + our own multi-object wait): the default DX12 `Wait` mode is used.
- macOS live resize with `presentsWithTransaction` and present on the main thread (needs the platform
  lane's resize path); DComp `Commit` after configuring a visual (the platform owns the DComp device).
- `Action::Represent` (VRR keep-alive): wgpu cannot present without acquiring a new image; skipped.
- `FastCommit::CanvasView` is stored per canvas but `FrameScene` has no field for canvas views, so the
  renderer cannot use it yet.
- A surface reported `Lost` must be recreated by the UI thread; there is no `ToUi` message to ask for it.
- Tiles for scroll layers (architecture §7.5: only after a measurement).

## Fixed: black window with a caret (CADATOMIC, 2026-10-25)

The copy into the swapchain image used to be damage ∪ the damage of the last `swap_images − 1`
frames, full only for the first 4 presents. A WSI that cycles through more images than that (or hands
them out in another order) presented images that had missed frames, so caret-only frames left
everything but the caret black. Now the whole persistent target is copied every frame.
`tests/headless.rs::caret_only_frames_keep_the_content_on_every_swapchain_image` (an emulated
six-image swapchain via `GpuBackend::attach_offscreen_chain`) reads `[0,0,0,0]` in the content area
under the old policy and passes with the fix; run locally on lavapipe with the rest of the crate's
tests (all pass). Separately, skelter-core should not send `FastCommit::Caret` for an editor that is not
rendered (`display: none`): the compositor draws the caret rect it is given.

## CI

skelter run 40228927630 on 9a58f1c4 (Linux, Windows, macOS): every skelter-compositor target passes,
including the GPU-gated headless tests; the run is red only for `skelter-core --lib` (lane CORE). The
five svelters workflows skip skelter-only pushes since base cc4f184d.

## Not run

Run locally once, filtered, to diagnose CI's failures on a9503acb (and passing after the fix):
`cargo test -p skelter-compositor --test engine --test tree_damage --test caret`. Fixed: damage no
longer counts a removal as reordering its successor (only survivors that change relative order are
damaged); the fling test counted presents, but frames whose content scrolled out of view rightly
present nothing, so it counts sampled frames.


Per the batch rule, nothing was run locally beyond `cargo check -p skelter-compositor --all-targets`.
Tests written, run only by CI: `tests/engine.rs` (virtual clock + recording backend: first frame,
commit folding, smooth scroll with retarget, fast-commit revision rejection, fling frames without commits then idle, fling stopped
by a scroll, idle does not wake, BeginFrame back-pressure, opacity animation with fill forwards,
UI-thread animations aborted here, resize handshake, hidden windows, occlusion from a platform
frame source), `tests/tree_damage.rs`
(`tests/caret.rs`: the blink rule; paint order, folding, scroll and clip resolution, OS-layer surfaces, texture layers, damage diff,
device rects, buffer-age history), `tests/headless.rs` (GPU-gated: a one-quad scene through skelter-render into an offscreen target, GPU
completion, readback; skips without an adapter).

## Cross-lane edits

- `skelter-base/src/commit.rs`: additive `ToUi::FrameTimings { window, frame, draw, submit, gpu_done,
  present, estimated }` (coordinator decision for APP-TEST #7).
- `skelter-base/src/commit.rs`: additive `ToUi::Occluded { window, occluded }` (decided by the coordinator
  with PLATFORM, 2026-10-25), sent when the platform frame source reports occlusion; the compositor stops
  drawing an occluded window and redraws it whole when it comes back.

## Questions for the coordinator (each with a recommendation)

1. **`FastCommit::Caret` has no colour and no space.** Recommendation: add `color: Color` and a
   `transform: PropNodeId` (the editing node's space) to it in skelter-base; until then the caret is
   drawn in window points with `CompositorConfig::caret_color`.
2. **Transform animations compose after the committed local transform** (`world = parent · local ·
   anim`). Recommendation: keep it, and have skelter-core commit the node's local transform without its
   CSS `transform` while a compositor transform animation drives it (CSS replaces, does not compose);
   the transform origin then goes into the keyframes as translate pairs.
3. **UI-placed animations (paint/layout) in a Commit** are answered with `AnimationEvent::Aborted`
   here, since the compositor cannot run them. Recommendation: skelter-core never sends them; if the UI
   wants the compositor to pick their start time, add an `AnimationCmd::ReportStart` later.
4. **`Commit::a11y`, `cursor` and `ime_area` are ignored by the compositor.** Recommendation: they are
   UI-thread concerns (the platform's AccessKit adapter, cursor and IME live there); skelter-core should
   hand them to the platform directly and could drop them from `Commit` in a later base revision.
5. **Every `BeginFrame` must be answered** by a commit or `CompositorHandle::ui_frame_skipped`, or the
   clock never sends another (back-pressure). Recommendation: skelter-core's `UiFrameState` does this.
7. **`CompositorConfig::ppi` defaults to 96** (the CSS reference) for fling physics. Recommendation:
   skelter-platform passes the monitor's value; flings then match the OS's distance.
6. **Default refresh**: `CompositorConfig::new` takes the interval; the platform should call
   `set_refresh` per window from its monitor's rate. Recommendation: skelter-platform does it on window
   creation and on monitor change.
