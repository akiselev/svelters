# PRESTO-render: presto's wgpu renderer (presto batch 1, lane RENDER)

Branch `ci/presto-render` from `ci/presto-base`. Brief: `plans/cloud/presto-b1/BRIEF.md`,
`plans/cloud/presto-b1/LANE-render.md`. Design: architecture §8, `notes/round2/gpui-port.md`.

## Landed

The brief's API, unchanged in signature and extended:

- `Renderer::new(device, queue, target_format, glyphs)`, `register_texture`, `unregister_texture`,
  `render(device, queue, encoder, target, scene) -> RenderStats`, `end_frame()`.
- Additions: `set_text_params(TextParams { gamma, grayscale_contrast })` (defaults 1.8 / 1.0, the
  platform lane should feed DirectWrite's values on Windows and 1.0 / 0 on macOS), `forget_image`,
  `forget_path` (for `ResourceDelta::removed_*`), `clear_caches`, `format()`,
  `damage_regions(damage, target)` (the scissor plan), `gamma_correction_ratios`, constants
  `FRAMES_IN_FLIGHT`, `MAX_ATLAS_PAGES`.
- `headless::{HeadlessTarget, FORMAT, render_to_image, ReadbackError}`: an offscreen `Rgba8Unorm`
  target that persists across renders (for damage tests) and its readback.
- `RenderStats { draws, instances, batches, upload_bytes, atlas_pages, path_passes, regions, skipped, cpu_us }`.

Deliverables:

1. **Pipelines** (`pipelines.rs`, created in `Renderer::new`), one WGSL module (`shaders.wgsl`) ported
   from GPUI `gpui_wgpu/src/shaders.wgsl` @ a5c4ca8: Quad (per-corner radii, per-edge borders, GPUI's
   dashed borders, dotted as 1:1 dashes, solid fill, linear gradients with up to 8 stops in CSS angle
   convention, interpolated premultiplied), Shadow (GPUI's erf blur; offset/spread applied on the CPU;
   CSS blur = 2σ; inset), Line (butt/round/square caps, dashes, wavy, any direction), Mask sprite (R8
   with the Windows Terminal gamma/contrast, uniforms; MIT notice kept), Image sprite (premultiplied
   RGBA: images with radii/src rect/nearest-or-linear, colour glyphs, colour icons, registered surfaces),
   Path (lyon fill/stroke meshes rasterised into a 4× MSAA intermediate sized to the batch bounds, then
   composited), and a scissored Clear. Every instance has an affine transform and a device clip (hard
   rect + one antialiased rounded rect). The shader is validated by naga in a unit test
   (`pipelines::tests::shaders_parse_and_validate`, the one test run locally: passes).
2. **Batching** (`flatten.rs`, `bounds_tree.rs` ported from GPUI): order = 1 + max order overlapped,
   sort by (order, pipeline, texture), runs merged. Instance records: one storage buffer per frame in
   flight (a ring of 3), one bind group per slot, kinds addressed by `first_instance`; uploads (records
   and globals) go through a `StagingBelt` into the caller's encoder, so several renders before one
   submit stay ordered. Non-sRGB targets, premultiplied output, blending in sRGB-encoded space.
3. **Atlases** (`atlas.rs`): etagere bucketed pages, 1024², 2048² when a tile needs it, own page for
   bigger tiles, R8 and RGBA8, generation per page (GPU texture recreated on change), page-level
   frame-generation LRU eviction when the budget (8 pages per kind) is spent, idle pages freed after
   600 frames; pages used in the current frame are never evicted. Glyphs: presto-base `GlyphKey`, 4×1
   subpixel x bins and whole-pixel baselines for axis-aligned text (GPUI's quantisation), rasterised
   through the `GlyphRasterizer` on a miss; empty glyphs are remembered.
4. **Damage** (`damage.rs`): damage clipped to the target, overlaps merged, one scissor per region or the
   union when it is ≤ 1.5× the area or there are > 8 regions; primitives outside the damage union are
   culled; damaged areas are cleared with the clear pipeline (`LoadOp::Clear` when the whole target is
   redrawn).
5. **Headless** path: `render_to_image` / `HeadlessTarget::read_rgba`.
6. **RenderStats** as above.

## Tests (written; not run locally except the shader validation)

- Unit (no GPU): bounds tree (GPUI's two tests, `rand` replaced by an LCG), gamma ratios against
  text-stack §3, atlas (padding, LRU eviction, no eviction of pages in use, oversized pages, idle
  freeing), path cache, damage plan, record sizes, and batching in `flatten::tests`: 10k disjoint quads
  → 1 batch, 10k stacked quads → 1 batch, buttons with labels → 2 batches (3 with an overlay), colour vs
  mask glyphs, subpixel bins, culling, push-clip, path batches and targets, gradient resampling,
  rotated geometry.
- GPU (`tests/gpu.rs`, gated on `PRESTO_GPU_TESTS=1`): one image per primitive group (rounded quad with
  borders, dashed/dotted, gradient, shadows, lines, glyphs, images/icons, paths, surfaces + rounded
  clips) with exact-pixel asserts plus a reference comparison; 10k quads in one draw; damage-only redraw
  leaves every other pixel untouched and clears empty damage; atlas churn over 50 frames.
- **References don't exist yet.** The first CI run records them and fails with "reference recorded,
  inspect it"; each PNG is also printed as `PRESTO_REFERENCE <name> <base64>` in the test log, so the
  consolidation can recover them from the CI log, inspect, and commit them to
  `crates/presto-render/tests/reference/` (or run with `PRESTO_BLESS=1` on a machine with an adapter).
  Expect the ten image tests to fail on every OS until then.

## Not run

Everything but `shaders_parse_and_validate` and `cargo check -p presto-render --all-targets`. No GPU in
this container (no Vulkan ICD), so pipeline creation against a real device is first exercised by CI.

## Stubbed / left out (none pretends to work)

- `Prim::Canvas` is not drawn: canvas pipelines (architecture §9.5) are later work; each one counts in
  `RenderStats::skipped`.
- LCD subpixel text (§9.4), the depth pre-pass (§8.2, not benchmarked), persistent per-layer instance
  buffers for clean layers (§8.2 "retained layers keep persistent buffers"; this renderer re-flattens
  the scene each frame), device-loss recovery (§8.5: the compositor recreates the `Renderer`;
  `clear_caches` exists).
- Approximations, by design for now: one rounded clip per instance (an outer rounded clip keeps only its
  rect when an inner one is pushed); a `PushClip` under rotation clips to its bounding box; dotted
  borders are square 1:1 dashes, not round dots; gradients with more than 8 stops are resampled to 8;
  CSS drop shadows are not cut out under their element; an instance buffer larger than
  `max_storage_buffer_binding_size` drops the frame's draws (counted as skipped).

## Cross-lane edits

- `THIRD_PARTY_NOTICES.md`: a new "presto-render (Apache-2.0 and MIT ports)" section (GPUI shaders and
  bounds tree, Windows Terminal gamma), and the Apache-2.0 text at
  `crates/presto-render/LICENSES/Apache-2.0-GPUI.txt`.
- `Cargo.lock` (wgpu, etagere, lyon, naga, png, pollster, presto-text as a dev-dependency).
- presto-base: no changes.
- `.github/workflows/presto.yml` (base's file), one line: the "Which presto crates exist" step ran
  `ls -d crates/presto-* crates/svelters-presto` under `pipefail`; `crates/svelters-presto` doesn't exist,
  so `ls` exits 2 and every presto job failed before building (run 40221188131). Now
  `(ls … 2>/dev/null || true)`. The same fix belongs on `ci/presto-base`.

## Questions for the coordinator

1. **Reference images from CI.** `presto.yml` (base) uploads no artifacts. Recommendation: add an
   `actions/upload-artifact` step for `crates/presto-render/tests/reference/` on failure; until then the
   base64 lines in the log are the source.
2. **Uncoloured coverage icons.** `display::Icon { color: None }` with an A8 image: drawn black here.
   Recommendation: widgets always pass the resolved `color` for monochrome icons, and `None` only for
   colour icons; then this case never arises.
3. **Retained layers.** The compositor re-sends every `DrawItem` each frame, and the renderer re-flattens
   and re-uploads them (10k quads ≈ 2.5 MB). Recommendation: a later lane adds per-layer cached instance
   ranges keyed by the compositor's layer ids, once COMPOSITOR's layer API settles.
