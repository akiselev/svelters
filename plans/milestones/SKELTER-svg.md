# SKELTER-svg: vector icons and live colour slots (cadatomic batch 12, lane skelter-svg)

Branch `ci/skelter-svg`, from master fc7903fe. Brief: the coordinator's message (user, 2026-10-25:
"Do 1 and 2"). Docs: `docs/skelter/icons.md`.

## What landed

**1. Vector icons and SVG images on the GPU**

- `skelter-widgets` `paint/icon/vector.rs`: `VectorArt::parse` parses an SVG once per content
  (usvg, cached for the process) into `VectorLayer`s in paint order. Each layer has:
  - a path, uploaded as a `PathId` through the new `Resources::path`;
  - a transform;
  - its kind (`Fill` or `Stroke`);
  - an opacity;
  - a `PaintSource`: a literal `Color`, `CurrentColor`, or `Slot(IconSlot)`.

  Strokes are outlined on the CPU by tiny-skia's stroker (caps, joins, miter limit, dashes, as
  resvg draws them), so the renderer only fills and `PathPaint` needed no change. Slots are found
  by parsing with sentinel `color`s on the root and the slot classes, after `slot_svg`'s rewrite.
  The public types are `#[non_exhaustive]`.
- `registry::paint` above `VECTOR_THRESHOLD_PX` (24): the chosen master's layers as `Prim::Path`s
  through the lyon/MSAA pipeline. At or below it: resvg's pixel-snapped raster. `choose_master` is
  unchanged on both paths. `Painted.vector` says which path drew the icon.
- `<image src="….svg">` and the `icon` kind's `source` SVGs (`Loaded::record_vector`): vectors above
  the threshold, clipped to the box and its rounded corners. `image-rendering: pixelated` keeps the
  raster.
- skelter-core `PrepareCx::share_path` / `unshare_path`: a path shared by key across the document,
  so every node that draws an icon draws the same `PathId` (one mesh per scale step).
  skelter-widgets' `Hosted` uses it.
- skelter-render: `PathCache` counts hits and tessellations (`PathCacheStats`, test-only accessor).

**2. Live colour slots**

- skelter-base `display::{Palette, PaletteSlot, PALETTE_SLOTS}`:
  - `Prim::Icon` and `PathPrim` gain `slot: Option<PaletteSlot>`. Slot 0 is `currentColor`, slots
    1–3 are `--icon-subject`, `--icon-accent` and `--icon-badge`.
  - New compositor property `AnimProperty::Palette(u8)`.
  - `Palette::apply` recolours a list's slotted primitives.
- Vector layers reference their slot. Raster icons:
  - without slots: one `A8` mask tinted from slot 0;
  - with slots: one `A8` mask per slot (`vector::slot_masks`; a later layer knocks out the earlier
    masks), whose keys don't depend on the colours. Literal colours go into one RGBA raster.
- skelter-compositor: `PropTrees::anim_palette` on effect nodes, resolved down the effect chain
  (`PropTrees::palette`). `push_item` substitutes the colours, and the list diff damages the item.
- skelter-core: a node whose recorded list draws only slotted primitives (`palette_only`) sends its
  CSS `color` transition to the compositor as `Palette(0)`, on its effect node, made in the same
  commit. Easing is skelter-anim's keyframe sampling, as for opacity. `transitionend` fires on the
  compositor's `Finished`. Other nodes keep the UI-thread transition. A theme switch re-records the
  icon lists with new colours: no raster and no upload.

**3. Multicolour icons (coordinator request for cadatomic lane theme, 2026-10-26)**

- `IconColors::{Tint, Own}` (`#[non_exhaustive]`) is an explicit per-icon flag on `Icon::Svg`. Set
  it with `Icon::multicolour(size, svg)` or `.colors(IconColors::Own)`. `Tint` is the default and
  today's behaviour.
- With `Own`, literal colours are drawn as they are:
  - at 24 px or below: an RGBA raster of the literals plus live per-slot masks;
  - above 24 px: vectors in their literal colours.

  `currentColor` and the slots stay live either way.
- Mask keys now encode the colour mode and which slots are set. Without that, two slot sets could
  share a key and get different masks.
- Test: `a_multicolour_icon_keeps_its_colours_and_current_color_stays_live`. A red plus
  `currentColor` icon keeps red at 16 and 96 px with no slot set; its `currentColor` part follows
  `color` with no new raster; the same SVG registered `Tint` stays monochrome.

## The threshold

Raster when P ≤ 24 physical pixels, vectors above that. I rendered side-by-side screenshots on
lavapipe (raster from `registry::rasterize` and vector from `VectorArt::record`, through
`skelter_render`), for:

- a stroked Lucide-style gear with a classed accent;
- a 16-unit file icon with filled subject bars;

at 16, 20, 24 and 32 pt × 1, 1.25, 1.5 and 2. The screenshots went to the user and are not
committed.

At 32 px and above, vector and raster can't be told apart. From 16 to 25 px, 4× MSAA draws thin
1-unit strokes slightly lighter and softer than resvg's analytic coverage of the pixel-snapped
master. So the raster keeps the sizes the masters are designed for (≤ 24), and 25–31 px goes to
vectors: there the masters are scaled or padded anyway (`Fit::Scaled` or `Fit::Padded`).

## Fallbacks

These draw from the raster path at every size, and `IconError::Fallback { key, why }` reports them
once per key and reason:

- filters, masks, clip paths and blend modes;
- patterns, linear gradients and radial gradients;
- embedded images, text, and a group opacity over several shapes.

Gradients are not in `PathPaint`. A gradient fill needs per-pixel colour in the path pipeline: a
gradient uniform per path instance, local positions from the vertex stage, and a stop ramp, plus a
change to how a batch's paths share one intermediate. That is more than a small shader change, so
they fall back.

- A fallback icon **with slots** keeps the old baked RGBA raster, so its slots are not live (a new
  colour set is a new raster).
- Literal-coloured layers draw as themselves when slots are set, and as slot-0 coverage when none
  are, as the `A8` raster did.
- I don't have the cadatomic icon kit's sources in this container (svelters sessions don't edit or
  check out cadatomic). **No kit icon has been checked against the vector path yet.** The theme lane
  can list the kit's fallbacks from `take_diagnostics()` (`IconError::Fallback`) on its first run.
  The kit's spec (stroked, `currentColor`, slot classes) is all in the supported set.

## Counts (each counts events, not time)

| Test | What | Result |
| ---- | ---- | ------ |
| `registry::tests::vector_above_the_threshold_raster_below` | 16, 24, 48, 96, 200 px and fractional scales (20, 24, 25, 32, 48, 125 px) | vector iff px > 24; every vector size draws the same 2 paths |
| `registry::tests::zoom_and_scale_changes_never_rasterise_a_vector_icon` | 20 zoom frames re-recorded, then scale 1.25, 1.5, 2 | 0 rasters, 2 paths |
| `flatten::tests::a_zoom_and_a_scale_change_draw_vector_icons_from_the_mesh_cache` | compositor zoom over 20 frames (1×→2×), then scale changes | 0 atlas uploads; 18 tessellations (9 steps × 2 paths), 22 hits, then 8 more hits and 0 new tessellations after 4 scale changes |
| `registry::tests::a_theme_switch_over_100_slotted_icons_rasterises_nothing` | 100 slotted icons at 1× (masks) and 2× (vectors), dark → light | 200 masks and 4 paths before, the same after: 0 new rasters or uploads; every colour changed |
| `commit::an_icon_colour_transition_runs_on_the_compositor` (skelter-core) | hover colour transition | one `Palette(0)` start on the icon's effect node; 0 icon lists committed over the next 19 frames (the box next to it re-records each frame) |
| `engine::a_palette_animation_recolours_slotted_icons_without_commits` (compositor) | the same transition, drawn | colour eases red→blue, slot alpha kept, unslotted icon untouched, 0 `BeginFrame`, override cleared at the end |
| `refusals_missing_and_broken_icons_are_reported` | an icon with a filter at 48 px | raster `Prim::Icon`, one `Fallback { why: "filter" }` over two paints |
| `flatten::tests::a_toolbar_of_200_vector_icons_is_one_path_batch_per_layer_run` | 200 two-layer 32 px icons over a background quad | 400 path draws, **1 path batch, 1 MSAA intermediate** (935×274 px, 4×), **2 meshes cached** |
| `gpu::vector_icons_stay_sharp_at_200_px` (`SKELTER_GPU_TESTS=1`) | a 24-unit disc at 200 px, 1× and 1.25× | ≤ 2 partly covered pixels across the left, top and diagonal edges |

Budget note: a path batch's intermediate is the union of its paths' bounds. A toolbar spread over a
window makes one large 4× MSAA target (the 935×274 above is about 1 M samples × 4). Batching is
fine; the memory is the cost to watch if toolbars span 4K windows.

## Not done / open

- Hosted nodes never release images on removal (as before). Shared paths are counted the same way,
  so a removed node's paths stay until the document goes.
- An icon whose literal-colour RGBA raster sits beside slot masks is not `palette_only`, so its
  colour transition stays on the UI thread.
- An icon's `--icon-*` custom-property changes re-record (no `@property` transitions); `color`
  transitions run on the compositor.

## Run, and not run

- Run (filtered, after the last code edit):
  - skelter-widgets `paint::icon`, `controls::image`, `controls::icon`;
  - skelter-core `tests::commit`;
  - skelter-compositor `--test engine palette`;
  - skelter-render `--lib paths flatten::tests::a_`;
  - skelter-render `--test gpu vector_icons_stay_sharp paths_fill` on lavapipe (installed in the
    container for this; CI has it);
  - `cargo check --all-targets` of every skelter crate and svelters-skelter.
- Not run locally: the full workspace tests, the other GPU tests, the svelters backends and
  conformance. GitHub CI is the gate.
- Screenshots: a scratch test with a temporary skelter-widgets dev-dependency of skelter-render,
  removed before committing (Cargo.lock unchanged).

## CI and merge

- 895cc841: five of six workflows green. skelter failed on all three OSes in skelter-core's
  spec-hash test, which builds the test-kind registry in another order and lacked the new `icon`
  test kind. Fixed in 17a3bd19.
- 9baddbdf (with `IconColors`): all six green.
- Merged master e98d2a64 (7b81bfef), then 4bf37234 (1b6447c1). Conflicts were in skelter-widgets
  `host.rs` (prepare's emitted signals and the overlay recording, now also given the shared paths)
  and skelter-compositor `tests/engine.rs` (`Drawn` keeps both fields).
- 1b6447c1: all six green (js 40259199533, linux-gtk 40259199528, linux-qt 40259199516,
  macos-appkit 40259199564, skelter 40259199531, windows-winui 40259199567). The coordinator merged
  it as master 992fffb2.
