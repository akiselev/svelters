# CS5g: the egui style deferrals that do not need SG4

Design: `plans/research/CSS.md` §2.7 ("egui: in-process") and §5 (row CS5); CS5e's "Deferred" list
(`CS5e-egui-cascade.md`); Q3/Q3w for the compiler's CSS. Built on CS0 (`crates/svelters-style`), CS5e and CX1.
Lane: CS5g (STYLE writer), branch `ci/cs5g`.

**Decided (coordinator, 2026-10-22):** CS5g takes three of CS5e's deferred items: `box-shadow`, compile-time
checking of egui `<style>` values, and CSS-only sizes as relayout boundaries. Absolute positioning waits for SG4,
which has since merged (see "SG4" below). The CS0 lead approved the engine change, the compiler design (existing
codes only, no `compile.rs`/`diag.rs` edits) and the `is_boundary` change.

## What landed

| File | Content |
|---|---|
| `svelters-style` `values.rs`, `table.rs`, `lib.rs` (own commit) | `ValueType::Shadow` and `ComputedValue::Shadows(Box<[Shadow]>)`. `Shadow { x, y, blur, spread, color: Option<Color>, inset }`. The grammar is `none`, or a comma-separated list where each shadow is `inset`, 2 to 4 lengths and a colour; `inset` and the colour can come before or after the lengths. Lengths take `px`, `em`, `rem` and `calc()`, and a unitless number counts as px (the setter grammar). `%` and a negative blur are errors. `currentcolor`, written or left out, resolves from the element's computed `color` (the cascade computes `color` first); `None` only when the element has no `color` (egui then uses its visuals' text colour). `Display` prints the computed list. |
| `tools/metadata/style/gen_props.py` | `RS_TYPE["shadow"] = "Shadow"` (was `Raw`). |
| `svelters-egui` `style/shadow.rs` (new, STYLE) | Shadow paint. **Outer** shadows go below the box: an epaint `RectShape` moved by (x, y), grown by the spread (its corner radius grows by the spread too), with `blur_width = 2 × blur`. A CSS blur radius `b` makes a penumbra that reaches `b` on each side of the edge, and epaint's `blur_width` is the whole penumbra. The list paints back to front, so the first shadow ends up on top. **Inset** shadows go above the background as four blurred bands around the "hole" (the box moved by (x, y) and shrunk by the spread), clipped to the box. `paint_bounds` returns the node's rect grown by its outer shadows. |
| `style.rs` (STYLE, small) | `box-shadow` added to `ENGINE_PROPS` (`via: css`, `decl_type: shadow`). `paint_box` paints outer shadows, then `paint_fill` (the old body), then inset shadows. Stock widgets (`before_leaf`) get the outer shadows of the current state's style (`states[0/1/2]` from this pass's hover and press) behind the widget. |
| `walk.rs` (EMBED, three lines) | Culling uses `style::paint_bounds(arena, i, r)` instead of `r`, so a node whose shadow is on screen still paints when the node itself is off screen. |
| `scene/dirty.rs` (SCENE, `is_boundary` only) | It reads the node's taffy style (`layout[i].style`: props overlaid by the computed style, the same thing `layout_style` produces) instead of `common(i)`. A definite px width and height, `flex-shrink: 0` or definite px min sizes, and no baseline alignment (self or parent `align-items`) now count when they come from CSS alone. |
| `metadata/semantics/egui.json`, `tables.generated.rs`, `props.generated.js` | Regenerated (spec test with `SVELTERS_UPDATE=1`, then `gen_props.py`). |
| `svelters-compiler` `lower/engine_css.rs` (new), `lower/mod.rs` (4 lines), `Cargo.toml` | For a backend with `style_engine_in_backend()`, `lower()` runs `Stylesheet::parse` and `Stylist::new(Table::for_toolkit(backend)).add_sheet(...)` on `root.css` and maps each diagnostic's line and column (1-based, column in UTF-16 units) to a byte span in the `.svelte` file. The span runs from the declaration to the end of its value. Svelte's syntax is blanked in place first: `:global(x)` keeps `x`, a bare `:global` goes, and a top-level `:global { … }` becomes `* { … }`. `:state(ns.name)` accepts any name, because provider states register at run time. An engine parse diagnostic on a line the compiler's own `<style>` parser already reported on is skipped. `compile.rs` and `diag.rs` are untouched. |

### Diagnostics (existing codes only, all registered in CX1's `codes.rs` with these severities)

| Engine diagnostic | Code | Severity |
|---|---|---|
| Table: bad value, or a rule setting an inline-only property | `style_bad_value` (backend-tagged) | error |
| Table: property unknown to the backend's table (another backend may have it) | `style_unknown_declaration` (backend-tagged) | warning |
| Parse: a rule or declaration the engine drops (bad selector, unknown pseudo-class) | `style_syntax` | error |
| Parse: an at-rule the engine ignores (`@keyframes`, …) | `style_unsupported_at_rule` | warning |

These follow the engine's own severities. An unknown property is not an error because the engine keeps going
and ignores it, and the same `<style>` may target several backends.

### AppKit's table

AppKit's decl table already had `box-shadow` as `decl_type: shadow`, so it is typed now too, and
`Stylist::add_sheet` checks those values on AppKit. ST2's JS grammar (`packages/style/src/decl.js`: one shadow, 2–3
lengths, a colour, no `inset`) is a subset of the engine's, so no value that is valid on AppKit becomes an error.
The engine does not flag what only ST2 rejects (spread, `inset`, lists): ST2 still reports those at run time. No
production AppKit path runs the Rust `Stylist` yet (ST2's JS engine styles AppKit until CS1), so nothing changes at
run time there.

## Acceptance

- **Engine:** `values::tests::shadows` covers `none`, single shadows and lists, `inset` and colour on either side,
  unitless and `em` lengths, `calc()`, `currentcolor` (written, left out, unresolved), `Display`, a negative blur,
  `%`, too few or too many lengths, two colours, a trailing comma, and `none` inside a list.
  `cascade::shadows_compute_and_check` covers `em` from the element's font, inherited `color` as the omitted colour,
  `var()` inside the list, no inheritance, and a bad value as a table error at line 2, column 3 (AppKit table).
- **Shadow paint (kittest):** `tests/style_shadow.rs::shadows_paint_around_boxes_and_widgets` checks, on the frame's
  shapes, the outer shadow's rect (moved and spread), its `blur_width` (8 for CSS blur 4) and its radius (6 + 2).
  It checks the paint order (shadow, then background, then inset), that the inset makes four bands clipped to the
  box, and that a button's shadow follows its `:hover` style. `none` removes the shadows.
  `shadows_widen_the_culling_rect`: a box above the screen whose 400px spread reaches into view still paints its
  shadow (before CS5g the box was culled with it). Unit: `style::shadow::tests::{outer_shapes_and_margin,
  inset_bands_frame_the_hole}`.
- **CSS-only boundaries:** `tests/style_shadow.rs::css_only_sizes_are_layout_boundaries`. Rows sized only by
  `.row { width; height; flex-shrink: 0 }` are boundaries. A label change inside one row re-lays out that row alone
  (`stats()`: no whole-root layout, one boundary laid out). Once the CSS height is removed, the row is no longer a
  boundary.
- **Compiler:** `tests/cs5g_egui_css.rs`. `bad_values_are_errors_at_their_position` checks `box-shadow: 1px 2px -3px
  red` inside a rule after a `:global(...)` selector (error at 6:5, span = the declaration, backend `egui`) and
  `opacity: lots` as the second declaration on one line (9:32). The build fails. `unknown_properties_are_warnings`
  checks `colour: red` (warning at 7:5, no `style_syntax` for `:global`), and that a sheet with only warnings
  compiles. `engine_syntax_errors_and_states` checks that `:state(cad.pending)` is accepted and `button:nope` is a
  `style_syntax` error on line 2. Unit: `lower::engine_css::tests::{global_is_blanked_in_place,
  spans_follow_lines_and_utf16_columns}`.

## Run locally, and not

Run (filtered, after the last code change and the merges of origin/master with SG4 and CX1): `svelters-style`
`shadows`, `table_diagnostics`, `generated_tables_are_sorted_and_typed`; `svelters-egui` `binary(style_shadow)`,
`style::shadow::` unit tests, `binary(style)`, `binary(scene)` (before the SG4 merge), `binary(spec)`;
`svelters-compiler` `binary(cs5g_egui_css)`, `engine_css` unit tests, `binary(g1_egui_spec)`, `binary(cx1_codes)`,
`binary(cx1_check)`; `gen_props.py --check`; clippy on the touched files (`svelters-egui`, `svelters-style`,
`svelters-compiler`).
**Not run locally:** the full workspace nextest, the rest of the egui, compiler and runtime suites, the fixture op
logs (the compiler's output is unchanged and only diagnostics are added), conformance, and the GTK/Qt/AppKit/WinUI
workflows. GitHub CI is the gate.

## Deviations, and why

- **No cut-out under outer shadows.** CSS paints an outer shadow only outside the border box. Here it is one filled
  `RectShape` under the box, so it shows through a translucent background. egui's `Frame` shadows do the same, and
  cutting the shape out would need a custom mesh.
- **Inset corners are square.** The bands are clipped to the box's rect, not its rounded rect, and the hole has
  square corners. With a radius, the inset shadow's inner corners look sharp.
- **Stock widgets get outer shadows only.** An inset shadow would paint under the widget's own background, so it
  would not be visible. Painting it over the widget needs a hook after `leaf::leaf` (WIDGETS).
- **Engine parse diagnostics are deduplicated by line** against the compiler's own `<style>` parser, which still
  runs on egui for setter properties. A finer match would need the two parsers to agree on spans.
- **`currentcolor` in a compile-time check** resolves to transparent (the engine's `check_decl`), so it never errors.

## SG4 (merged during this lane)

SG4 (bd959510) added `position` and `left`/`top`/`right`/`bottom` on every kind, and `style.rs::layout_style`
maps their computed values. `%` and `calc()` insets go through `lpa` and the calc handles, and hoisting places the
absolute boxes. Nothing of CS5e's "absolute positioning and insets" item is left for this lane. What is still open:
- `position: fixed` (not mapped; SG4's note).
- Paint order of hoisted boxes against CSS z-order (positioned boxes after in-flow siblings): this lane did not
  check it, and `z-index` is not mapped.
- `is_boundary` and absolute boxes: SG4 falls back to a whole-root layout when a boundary's subtree has an absolute
  box whose containing block is above the boundary. A CSS-sized boundary (CS5g) goes through the same check.

## Notes

- **CX1 (merged during this lane):** `style_unknown_declaration` is registered as a Warning with a real description
  in `codes.rs` (the coordinator settled it in CX1). CS5g registers no new code.
- **CS5f** (`ci/cs5f`, not merged yet) also edits `style.rs` (fonts, leaf padding, `margin`/`padding` as insets)
  and `ENGINE_PROPS`. CS5g's changes there are a `mod shadow;` line, one `ENGINE_PROPS` row (`box-shadow`, between
  `border-width` and `color`, where CS5f has no rows), `paint_box` split into `paint_fill`, and `stock_shadows`
  after the stock visuals in `before_leaf`. Whichever lane merges second regenerates `egui.json`,
  `tables.generated.rs` and `props.generated.js`.

## Deferred

- `text-shadow` (labels: a blurred copy of the galley needs a texture or shape blur; epaint has none for text).
- Leaf box paint for stock widgets beyond `WidgetVisuals` (CS5e's item; CS5f takes the font and padding side).
- Inline `style=` through the engine (CS5e's item, D9's next step). Once it lands, the compile-time check should
  also run on static `style=` text for engine backends, through `DeclBlock::parse_inline` and
  `Stylist::check_inline`.
- The kittest snapshot gallery (needs the `wgpu` kittest feature). The shadow tests read shapes, not pixels.
