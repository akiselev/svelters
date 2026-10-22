# CS5e: the egui cascade (`crates/svelters-egui/src/style.rs`)

Design: `plans/research/CSS.md` §2.2, §2.3, §2.5, §2.7 ("egui: in-process"), §2.8 and §5 (row CS5);
`plans/research/ARCH-SCENE.md` §4.3, §6, §7.1–7.2 (step 7, same-frame state); `plans/research/TRACKS.md` §2.1 STYLE
row CS5e; CADATOMIC SV4, SV5, SV11. Built on CS0 (`crates/svelters-style`, branch `ci/cs0`) and SG1–SG3, RA1, EV2b.
Lane: CS5e (STYLE writer), branch `ci/cs5e`.

**Decided (coordinator, 2026-10-22):** D9 on egui first, ahead of CS1. The compiler no longer matches or lowers
`<style>` rules for a backend whose spec says `style.engine: "backend"` (egui). The runtime sends those rules as CSS
to the in-process engine. Inline `style=` setter lowering is unchanged. The compiler change is kept minimal and
flag-gated in `lower/mod.rs` because CX1 is editing `diag.rs`, `compile.rs` and the bin's `--check` at the same time.

## What landed

| File | Content |
|---|---|
| `style.rs` (new, STYLE) | `impl svelters_style::Element for NodeRef<'_>`: key `NodeId::to_raw()`, light-tree navigation, tag, `#id`, classes, attribute selectors read props by name, state = `StateBits.0` passed through. `ProviderStates: StateNames` resolves `:state(ns.name)` through RA1's registry. `SceneStyle` (the stylist, the sheets, a per-slot column of classes / id / inline block / computed style / per-state styles, the dirty list, an internal journal observer). The restyle: top-down by depth, children follow only when inherited values or custom properties changed. The walk hooks `before_container` / `before_leaf` (same-frame state, then restyle, then paint). `layout_style` (computed style over the node's taffy `Style`). Paint (`RectShape` for boxes and labels, per-state `WidgetVisuals` for stock widgets, `opacity`). The `style` host op (`style_op`). The Rust API on `EguiBackend`: `add_sheet`, `remove_sheet`, `set_tokens(scheme, map)` (SV5), `set_color_scheme`, `set_classes`, `set_inline`, `computed_style`. `pub mod style`, plus `svelters_egui::svelters_style` re-exported. |
| `scene/mod.rs`, `scene/access.rs` (SCENE, two lines each) | `Scene::style: SceneStyle`. `Scene::restyle` builds the taffy style through `style::layout_style`, so a JS prop write keeps the CSS overlay. |
| `walk.rs` (EMBED, small) | `before_container` before a container's children, with a child `Ui` for its `opacity`. `before_leaf` after the leaf's child `Ui` is made and before the widget or provider paints. |
| `frame.rs` (EMBED, small) | Logic step 7: `restyle_pending` after the checkpoint and rAF, before remeasure. `after_pass` asks for a repaint when a walk-time restyle shows only in the next frame. |
| `layout.rs` (SCENE/STYLE, small) | `display: grid` → `compute_grid_layout`, `impl LayoutGridContainer`, and `resolve_calc_value` (calc handles from `style.rs`). taffy features `grid` and `calc` are on. |
| `ops.rs` (shared, one fn) | `Backend::style` → `style_op`. TR1's visual channels go through it unchanged. |
| `spec.rs` → `metadata/semantics/egui.json` → `gen_props.py` → `tables.generated.rs`, `props.generated.js` | The egui style table gets `engine: "backend"` and the engine-only properties (`style::ENGINE_PROPS`, `via: css` with `decl_type`, computed because the egui table's dialect is `none`, CS0 1d68a9d3): `background-color`, `color`, `border-color`, `border-width`, `border-radius`, `opacity`, `font-size`, `display` (flex/grid/none), `grid-template-columns/rows`, `grid-column/row`, `margin-*`, `padding-*`. |
| `svelters-compiler` `semantics/{spec,mod,backend,overlay}.rs`, `lower/mod.rs` (flag-gated) | `StyleSpec::engine`, `PlatformSemantics::style_engine_in_backend` (default false). `lower()` drops the sheet when the flag is set. |
| `packages/native-runtime/src/style.js` | `SVL_STYLE_TARGET` 4 = `engine`: `SHEET` carries the sheets as Svelte scoped them (`sheets.css`); `@tokens` is a `:root` sheet (as on GTK, refreshed on `setTokens`/scheme); `CLASSES` carries the user classes (no `sv-k-` markers); `NAME`; `INLINE` carries the raw `style=` text. No setter rules or opacity op run in JS. |
| `packages/native-runtime/vite.js` | `nativeSheets(…, 'egui')` returns `{ css: <Svelte's scoped CSS> }` and skips the X6 translator. |
| `packages/testhost/src/index.js` | `runBundle` answers target 4 for `toolkit: egui` fixtures. |
| `fixtures/{counter,motion}-egui/expected.oplog` | Regenerated: the sheet goes as CSS, the scope class appears on every element, no compile-time `set gap/flex-direction/align-items`. |

### Behaviour

- **Activation.** Nothing runs until the first style input (a sheet, classes, an id, inline style or tokens). The
  first one restyles every node and starts the journal observer. An app without styles pays nothing, and its state
  bits stay as they are today (see "Deferred").
- **Same-frame state** (CSS.md §2.3, ARCH-SCENE §7.2 d). Before a leaf paints, the walk reads this pass's interaction
  with `ctx.read_response(id)`. For a TextEdit or a provider node the id is the node's egui id; for the other
  built-in leaves it is the child `Ui`'s first auto id, which egui hands to the widget. From the response the walk
  sets `hovered`, `pressed` and `focused`. For containers, `hovered` and `pressed` come from the pointer over the
  rect (`ui.rect_contains_pointer`). From props it sets `disabled` (`enabled`), `checked` (checkbox), `selected`
  (button) and `exiting` (TR1 inert channel). `focus-within` moves along the focus chain. The engine's
  `state_changed` reach is restyled at once: the node itself, its descendants and its later siblings show the change
  in this frame, ancestors (`:has`) in the next. These bits are the walk's (`style::WALK_BITS`), and the observer
  ignores them.
- **Stock widgets** (button, checkbox, slider, text-edit) get three computed styles when the sheets test `:hover` or
  `:active`: the node's state without hover/press, plus hover, plus hover and press. These go into
  `widgets.{noninteractive, inactive}`, `hovered` and `active`, so egui switches them in the same frame as it always
  does. A widget with no hover rule keeps egui's own hover feedback.
- **Logic restyle** (step 7). The observer records `Created`, `Inserted`/`Removed` (the engine's
  `structure_changed` reach on the parent; an inserted subtree restyles), `Prop` (only props some attribute
  selector names) and `State` (the non-walk bits the sheets test: provider bits, `open`, `expanded`, `invalid`,
  …). Classes, id and inline mark what they reach when written. A media change (viewport size, scheme from
  `SVL_STYLE_SCHEME`/`set_color_scheme` or egui's `dark_mode`) or a change of the body font size restyles
  everything.
- **Layout** (`layout_style`): `width`/`height`/`min-*`/`max-*`/`flex-basis` take px, `%` and `calc()` (taffy calc
  handles: an interned `(px, %)` table indexed past taffy's tag bits, no `unsafe`). Also `margin` and `margin-*`
  (with `auto`), and for containers `padding`/`padding-*`, `gap`/`row-gap`/`column-gap`, flex direction, wrap and
  alignment, and `display: grid` with track lists (`px`, `%`, `fr`, `auto`, `min-content`, `max-content`,
  `repeat(n, …)`) and `grid-column`/`grid-row` (`n`, `span n`, `a / b`). `visible: false` and `display: none` hide.
  A prop the runtime set explicitly wins (`NodeCore::explicit`; an explicit `margin`/`padding` also beats the
  per-side properties). TR1's clip channel still caps the size.
- **Tokens** (SV5): `set_tokens(None | Some(scheme), &[("--cad-panel", "#202020"), …])` registers a theme-origin
  sheet `:root { … }`, under `@media (prefers-color-scheme: …)` when it is for one scheme, with any namespace. The
  runtime's `--sv-*` tokens arrive the same way as `@tokens`.

## Acceptance

- **Cascade over a real scene:** `tests/style.rs::cascade_over_a_real_scene`. Checks the engine target answer; type,
  class, `#id`, child combinator and `:first-child`; inline over rules; inherited `color`; `var()` from Rust tokens,
  then dark-scheme tokens with `set_color_scheme`; `calc(50% - 20px)` width; `padding-left`; `margin-top`; a class
  change; an explicit `width` prop beating every rule; a provider state written from Rust matching
  `:state(cadt.pending)` through the journal and the logic restyle.
- **Hover restyle in the same frame:** `tests/style.rs::hover_restyles_in_the_same_frame`. One `harness.step()`
  after the pointer moves over a button: the button is `HOVERED`, its column box too, `button:hover` and
  `button:hover + label` are computed (the sibling restyled before it painted), and no checkpoint runs. The layout
  change of `box:hover > label { min-height }` shows at the next frame. Unhover restores in one frame.
- **Prop states and grid:** `tests/style.rs::prop_states_and_grid`. `:checked` and `:disabled` from props;
  `display: grid` with `100px 1fr`, `grid-column: 1 / span 2`, and the second row. A `checked` write flips state
  and style.
- **End to end through JS:** `tests/counter.rs::counter_runs_under_kittest` now also checks that `.row { flex-direction:
  row }` from the counter's `<style>` lays out the text edit and label side by side. The rule reaches the engine as
  scoped CSS, and the scoped classes come through `CLASSES`.
- Unit: `style::tests::{state_bit_tables_are_pinned_together, grid_templates_and_placements_parse,
  calc_handles_resolve, token_sheets}`. Compiler: `g1_egui_spec::{style_rules_are_left_to_the_engine,
  inline_setter_values_are_checked}`. Runtime: `test/style-engine.test.mjs`.

## Run locally, and not

Run (filtered, after the last code change): `svelters-egui` `binary(style)`, `binary(spec)`, `binary(counter)`
(`timers_fire_from_schedule_wake` failed only because the `timers-egui` fixture was not built here),
`binary(oplog_replay)`, the `style::tests` unit tests; `egui-kinds-example` tests; `svelters-compiler`
`binary(g1_egui_spec)`; `node --test test/style-engine.test.mjs test/style.test.mjs`; the counter-egui and
motion-egui fixture op logs (regenerated with `UPDATE=1`); `gen_props.py --check`; clippy on `svelters-egui` and
`svelters-compiler`.
**Not run locally:** the full workspace nextest, the rest of the compiler and runtime suites, conformance, the
GTK/Qt/AppKit/WinUI workflows. GitHub CI is the gate.

## Deviations, and why

- **The scene keeps per-node style data in its own column (`Scene::style`, by slot index, validated by `NodeId`),
  not in an `ExtKey` column.** Restyling on hover would otherwise journal an `Ext` record for every computed style.
  A reused slot is detected by its id, so freeing needs no hook.
- **Built-in state bits are set only once styling is active.** Setting them for every app would add `State` records
  for observers and stamp ticks in apps and tests that never asked for them. Turning them on always is a one-line
  change once that is wanted (MCP/queries without styles).
- **`:focus` maps onto egui's model.** egui draws a focused widget with `active` visuals. A `:focus` rule therefore
  applies to whichever slot is current, and `active` = hover + press for the per-state styles.
- **`is_boundary` still reads the props**, so a CSS-only definite size is not a relayout boundary yet. This is
  correct, just less incremental.
- **`margin`/`padding` stay single lengths in the table.** They are kind props the compiler still lowers for inline
  `style=`. The per-side longhands carry insets, and the shorthands `margin: 4px 8px` follow once inline lowering
  goes to the engine too.

## Deferred

- **Typography paint and measure:** `font-size` is computed and inherited (for `em`), but the walk and
  `Metrics::intrinsic` do not use it yet (measure has to know the node's font). Same for `font-weight`,
  `font-family`, leaf `padding` (egui's button padding) and `box-shadow`.
- **Leaf box paint for stock widgets beyond `WidgetVisuals`.** Their `background-color` and `border` go to egui's
  visuals, not a `RectShape`.
- **Absolute positioning and insets (`top`/`left`/…)**: no hoisted children in the adapter yet (SG4).
- **Compile-time checking of `<style>` values on egui.** With D9 the engine checks them at run time (diagnostics on
  stderr). The compiler can link `svelters-style` for this (Q3).
- **Inline `style=` through the engine.** The compiler still lowers static setter declarations of `style=` to props
  (explicit, so they also win over `!important` rules). Moving them to `INLINE` is the next step of D9.
- **Kittest snapshot gallery** (CSS.md §5 CS5 acceptance): needs the `wgpu` kittest feature (lavapipe). The tests
  read computed styles, state bits and rects instead of pixels.
- `:open`, `:expanded`, `:invalid`, `:busy`, `:read-only`, `:dragging` have no built-in kind that sets them. They
  work as soon as a provider or Rust sets the bits (observer path, as `:state()`).

## Notes for the next lanes

- **CS2 / MCP:** `EguiBackend::computed_style(h)` and `svelters_style::{matches, select}` over `NodeRef` give
  `computed_style` and selector queries on the live scene.
- **H2 (CSS-only hot reload):** re-sending `SHEET` replaces the sheet and restyles. No remount is needed.
- **TR2:** `transition:` declarations can read the before/after `ComputedStyle` in `restyle_node`.
- **WIDGETS:** `leaf.rs` is untouched. If a widget stops taking the child `Ui`'s first auto id (for example it
  allocates a label first), give it an explicit id and add it to `before_leaf`'s id match.
