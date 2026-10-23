# ZM1: UI zoom (VS Code-style Ctrl+= / Ctrl+- / Ctrl+0) for the egui backend

Lane ZM1, WSL PC, started 2026-10-22 on master 797835a5. For cadatomic.

Decided (coordinator, 2026-10-22), see the ZM1 row in `STATUS.md`:
- the host API takes a scale factor (egui's `zoom_factor`, multiplied with the OS scale);
  cadatomic maps its levels as 1.2^n, like VS Code, and calls the API;
- in Embed mode egui's `zoom_with_keyboard` is off: cadatomic owns the shortcuts;
- the standalone runner (`svelters-egui-run`) binds Ctrl+=/-/0 itself, in the same 1.2^n levels.

## API (final names)

| Name | What |
|---|---|
| `Embed::set_zoom(f32)` / `Embed::zoom() -> f32` | the host API cadatomic calls |
| `EguiBackend::set_zoom(f32)` / `EguiBackend::zoom() -> f32` | the same on the backend (a host that owns the backend itself) |
| `EmbedConfig::zoom_keys: svelters_egui::zoom::ZoomKeys` | who handles the keys: `Host` (default), `Embed` (`EmbedConfig::app`), `Egui` |
| `svelters_egui::zoom::{ZOOM_STEP, level_factor, nearest_level, step, MIN_LEVEL, MAX_LEVEL, MIN_ZOOM, MAX_ZOOM}` | the 1.2^n levels (−8..=8) and egui's factor range (0.2..=5.0) |

- `set_zoom(z)` sets egui's `zoom_factor` (`pixels_per_point = z × native_pixels_per_point`).
  The factor is clamped to 0.2..=5.0, and a non-finite or non-positive factor is ignored. egui
  applies it at the start of the next pass and repaints every viewport. A backend not yet bound to
  a context (eframe's context comes later) applies it at its first logic. The zoom belongs to the
  context, so it zooms the host's own egui UI as well (as VS Code zooms the whole window).
- `zoom()` returns the factor `set_zoom` asked for until egui applies it, then the context's
  factor. If the host sets `ctx.set_zoom_factor` directly, svelters follows it and does not fight
  it.
- `ZoomKeys::Host`, the `EmbedConfig` default: svelters handles no zoom keys, and `Embed::new` and
  `attach` turn egui's `zoom_with_keyboard` off. Ctrl+= reaches the host's UI unconsumed.
- `ZoomKeys::Embed`, set by `EmbedConfig::app` (used by `svelters-egui-run`, `run_app` and
  `TestApp`): `Embed::logic` consumes Ctrl+= / Ctrl+Plus (in), Ctrl+- (out) and Ctrl+0 (reset;
  Cmd on macOS) and steps one 1.2^n level per press. A factor between two levels steps to the
  neighbouring level first. egui's handler is off.
- `ZoomKeys::Egui`: svelters leaves egui's built-in handler (steps of 0.1) as it is.

## What holds at any zoom, and why

- **CSS px stay CSS px.** CSS px are egui points, and the scene, the style engine, layout and hit
  testing all work in points, so a zoom changes no style or layout input. The viewport shrinks in
  CSS px (media `width` queries follow), as in a browser.
- **Text stays crisp.** egui re-rasterises the font atlas per `pixels_per_point`; no texture is
  scaled. The test checks every glyph of a label: it has 1.5× the texels at 1.5, one texel per
  physical pixel, and about the same size in points.
- **Layout recomputes.** At the first logic after the root viewport's `pixels_per_point` changes
  (a zoom, or a move to another monitor), `follow_zoom` (`zoom.rs`, called from `before_pass`)
  remeasures every measured leaf. Text advances snap to physical pixels, so a size in points can
  move by a fraction, and only leaves whose size changed relayout. Roots resize with the window's
  size in points. Slots already relayout on a scale change (`note_drawn`, `root().scale`). SG6's
  paint cache and the provider widget caches already key on `pixels_per_point`.
- **SVG and icons render at device resolution.**
  - `icon source=`: egui's `Image` sizes its hint in physical pixels, and the default texture
    loader keys `.svg` uris on that hint.
  - `image`: it was drawn with `SizeHint::default()`, the SVG's own size in pixels, which was
    blurry at any zoom and on HiDPI. Now it loads its texture at the physical size it is drawn at.
    It is measured from the image loader (`try_load_image`, `source_size`), so measuring makes no
    second texture.
  - `data:image/svg+xml` uris: egui's texture loader keys only `.svg` uris on size, so it would
    keep the first raster and scale it. A new `SvgTextureLoader` (`assets.rs`, installed by
    `assets::install`) keys them on the hint. `elements::images::uri_of` now keeps a `data:`
    source as a uri; before, only sources containing `://` were kept, so a base64 data uri became
    `file://data:...`.
  - Eviction across zoom levels: when one uri has rasters at several sizes, the sizes not used in
    the last pass are dropped at the end of a pass. egui's texture loader already does this; the
    new `SvgTextureLoader` and `SvgLoader`'s CPU raster cache now do too. `assets::texture()` (Rust
    panels) drops a uri's textures made at another `pixels_per_point`.
  - ICR1's icon registry (`origin/ci/icr1`, not merged when this lane started) rasterises at
    `round(pt × ppp)` physical px (rule Z2) and keys its cache on that, so it follows the zoom
    without ZM1 code. Follow-up for ICR1 after it merges: its cache keeps the rasters of earlier
    zoom levels (it evicts only older generations and versions). This is bounded by 17 levels ×
    icons × colour sets. Eviction by last use, as above, would bound it to what is drawn.
- **AccessKit bounds stay correct.** egui sends node bounds in points under a root transform of
  `pixels_per_point`; the test checks that the bounds of the button equal its drawn rect × 1.5.
- **Hit testing matches.** It works in points. The test clicks the drawn centre at zoom 1.5, and
  the button gets the click.

## Tests

`crates/svelters-egui/tests/zoom.rs` (kittest, headless), plus `zoom::tests::levels_step_by_1_2`:

- `zoom_scales_physical_pixels_and_keeps_the_layout_in_css_px`: at 1.5 the button keeps its size
  in points, is 1.5× larger in physical px, and its AccessKit bounds equal the drawn rect × 1.5. The
  leaves are remeasured once, a click at the drawn centre hits the button, and out-of-range and NaN
  zooms are handled.
- `text_is_rasterised_again_at_the_zoom`: the font atlas holds the glyphs at the new scale.
- `svg_icons_and_images_render_again_at_device_resolution`: an `asset://` SVG icon (20 → 30 px), an
  `image` (16 → 24 px) and a `data:` SVG icon (20 → 30 px) render again at the new size, the old
  rasters are dropped, and `assets::texture` re-rasterises.
- `the_runner_steps_zoom_by_1_2_on_ctrl_plus_minus_zero`: `ZoomKeys::Embed` (`EmbedConfig::app`)
  steps 1.0 → 1.2 → 1.44 → 1.2 → 1.0 → 1/1.2, then resets with Ctrl+0. The keys are consumed, and
  egui's handler is off.
- `an_embedder_owns_the_zoom_keys`: under the default `ZoomKeys::Host`, Ctrl+= reaches the host's
  UI unconsumed and the host calls `set_zoom(level_factor(1))`. Neither svelters nor egui zooms on
  Ctrl+-.

Not done (from the STATUS row): no snapshot comparison at 80–150% against the VIS1 sheets. The
kittest runner here has no renderer, and the tests above check the properties directly.

## Outcome

Filled in when the lane is reported: sha, CI run ids, what was not run locally.
