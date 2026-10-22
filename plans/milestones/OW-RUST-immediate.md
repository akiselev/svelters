# OW-RUST: svelters' own widgets in immediate mode, for Rust chrome

Lane OW-RUST (thinkpad, `ci/ow-rust`), 2026-10-22. Branched from `origin/master` a6f72d82 (OW0,
EV1, OW-CSS stage 1 and OW1a merged). Design: ARCH-WIDGETS Q5 (decided by the user: an
immediate-mode Rust layer after OW1a), §1.1–§1.3, OW0's "Write a widget" step 10, WIDGET-PARITY
§2.23 and the OW-RUST row of §5, cadatomic's decision register (00-decisions.md, "how berth and the
shell move onto OW-RUST").

**Status: done for OW1a's kinds.** Rust code in egui draws a button, check box, toggle button,
switch, label, separator or icon that paints the same shapes as the Svelte widget of the same
kind. It has the same AccessKit node and the same units, state bits, CSS and EV3 focus, and it
needs no scene node. A paint-only seam covers chrome that keeps its own interaction (berth's tabs).

## 1. What landed

`crates/svelters-egui/src/imm/` (new, `svelters_egui::imm`):

| Piece | What it does |
|---|---|
| `imm::Sv` (`EguiBackend::imm()`, `Sv::ua_only()`) | The style source. A **virtual element** (`imm::El`: tag, `style_as`, id, classes, attributes) in a chain of virtual ancestors (`sv.within(El::new("div").class("toolbar"))`, root `chrome`, which `:root` matches) is matched and computed by the **scene's own `Stylist`**: theme tokens, `sv.base`, `sv.affordance`, author sheets and media. Results are cached per chain and state, and the cache is dropped when the cascade restyles everything (new `SceneStyle::generation`, bumped where `all` is taken). With no cascade, or with `ua_only`, widgets paint from their UA tables, as scene nodes do then. Inside a scene pass (the scene is busy) the last cached style is used. |
| `imm::Button`, `Checkbox`, `ToggleButton`, `Switch`, `Label`, `Separator`, `Icon` | egui-style builders: options, then `show(ui, &sv)` (measures, allocates) or `show_at(ui, rect, &sv)` (the caller lays out). Common options: `class`, `element` (the whole `El`), `id`, `enabled`, `tooltip`, `accessible_name`. They return `imm::Response` (derefs to `egui::Response`): `clicked()` (pointer, Space, Enter, AccessKit `Click`, `repeat` steps), `double_clicked`, `secondary_clicked`, `changed()` (toggles write the caller's `&mut bool`), `state` (the bits). |
| Units without a node (`imm::run`) | The kind's units in declaration order (Hoverable, Focusable, Pressable, Toggleable) through `interact::run`, with the bits kept between frames in egui memory by widget id. The prop mirror (`selected`) is set before the units run, and `DISABLED` follows the enabled state. A disabled widget runs in a disabled child `Ui`, keeps its shape at `opacity` .4 (R9) and shows its tooltip while disabled. `next_frame` drives repaints. |
| Same paint | Redone on OW1a2's structure (master ba5f7239). Each kind's paint and measure are free functions, which the kind and `imm` both call: `button::{paint, measure, Look}` (taking the parts' views, so the kind passes its transition overlays); `checkbox::{paint, paint_indicator, measure_indicator}` (OW1a2's `paint_indicator_kind`/`measure_with_indicator` are now thin `WidgetCx`/`MeasureCx` wrappers of these, and the radio button keeps using them); `switch::paint`; `toggle_button::{paint, measure}`; `label::{layout, paint, measure, Look}` (`layout` returns the galley and the text rect, the hit area); `separator::{paint, measure}`. Kinds without `PAINTS_BOX` (label, separator) also get the CSS host box the walk paints behind them (`Ran::host_box`, as `style::before_leaf`). |
| EV3 without a node | `focus::record_foreign`. An `imm` widget is in egui's Tab order in paint order with the scene's widgets, outside every focus scope (`Entry.node` is now `Option<NodeId>`). While it has focus, the keys its role claims (Space, Enter) leave the pass's input (R11), and `EguiBackend::shortcut_override` answers for it. |
| AccessKit | `widget_info` as the kind, then the kind's role and the units' `a11y` (toggled, Click, Focus), `accessible_name` (EM1). |
| `imm::chrome` (paint-only seam, berth BD7) | `chrome::state(ui, &resp, props)` runs Hoverable, Pressable and (when the sense is focusable) Focusable as observers (`interact::observe`, new: nothing is applied to egui) on **the caller's own response**. `props` carry the caller's bits, `DISABLED` included (a disabled control on an enabled `Ui`). `sv.chrome(ui, &resp, &el, ua, props)` returns the `Styled` in that state. `chrome::surface` / `ring` / `icon` / `label` / `galley` paint through the primitives, traced under the caller's id, and `chrome::text_color` / `font` serve the caller's own text layout. `chrome::BUTTON_UA` is the push button's table. |

Shared code changed (pure moves, no behaviour change; the ow1a, ow1a2, own_widgets, ow_css and
focus_keys tests pass unchanged): `widgets/{button,checkbox,switch,toggle_button,label,separator}.rs`
(paint and measure functions), `focus.rs` (`record_with` → `push_entry`, `record_foreign`,
`foreign_focused`; `Entry.node` optional), `interact/mod.rs` (`run` = `observe` + applying focus),
`style.rs` (`generation`, `compute_virtual`, `part_virtual` = `compute_pseudo` when a sheet has rules
for the part, as the scene), `paint` (`text_font` in-crate), `tests/common/mod.rs`
(`Rig::with_extra`).

```rust
let sv = backend.imm();                                   // keep one; clones share the cache
let bar = sv.within(El::new("div").class("toolbar"));     // `.toolbar button { … }` reaches these
if imm::Button::new("Save").icon("codicons:save").show(ui, &bar).clicked() { save(); }
imm::Checkbox::new(&mut wrap, "Wrap").show(ui, &bar);
imm::Separator::vertical().show(ui, &bar);
```

## 2. Acceptance

### 2.1 DESIGN R-rules

An `imm` widget satisfies exactly the rules its kind satisfies. It runs the kind's UA table, CSS
and paint, which the kittests below check state by state.

| Rule | Status for `imm` |
|---|---|
| R1 raised (button) | as the kind: `button-bg`, `button-border-color`, radius 4, `elev-1` lip. Tested (rest, both with and without a theme) |
| R2 pressed sinks / R3 hover in the same frame | as the kind; the bits are this pass's, so no lag. Tested (pressed and hovered paint equal the scene's for button, checkbox, switch, toggle-button, label) |
| R8 focus visible, keyboard only | as the kind: Tab gives `:focus-visible` and the ring, a click focuses without one (FX1). Tested (`owrust_focus_tab_order_focus_visible_and_key_claims`, ring paint equal to the scene button's) |
| R9 disabled with a reason | `opacity` .4, `:disabled`, no intents, tooltip shown while disabled. Tested (`owrust_disabled_keeps_its_shape_and_its_tooltip`) |
| R10 cursors | button: `pointer` / `not-allowed` from CSS/UA, set while hovered (the other kinds have no cursor in their UA tables yet, as in the scene) |
| R17 toggles by fill and glyph | as the kinds (check mark + `toggle-on-*`, knob + accent). Tested (checked and unchecked paint) |
| R13 tabs, R11 sashes | not widgets here: the `imm::chrome` seam lets cadatomic paint berth's tabs and sashes from CSS with the unit-driven bits. Their UA tables come with OW5 (tabs) and OW2 (sashes) |

### 2.2 WIDGET-PARITY

| Row | Status |
|---|---|
| §5 OW-RUST: "the façade exposes parts, `style_as`, painters and Policy" | **parts**: `Sv::part_computed` (the hook stays `None` until OW-CSS stage 2 computes `::part()`, as `SceneStyle::part_computed`; one TODO at `part_virtual`); **`style_as`**: `El::style_as` (tested); **painters**: the paint context carries the host tag, so CSS-selected part painters run as in the scene; **Policy**: `interact::run` reads the context's `native::Policy` |
| §5 OW-RUST: "Rust button, toggle, menu-button and tab strip pass the same VIS1 checks as the Svelte kinds" | button and toggles: same PaintTrace as the Svelte kinds in every pointer state, focus-visible, checked and disabled (kittests). menu-button: OW3's kind first. Tab strip: OW5's `tab` kind first; meanwhile the chrome seam. VIS1: see §2.3 |
| §2.23 QDockWidget → berth | the seam of §3 |
| The kinds' own rows (§2.1–§2.3, §2.9, §2.10) | inherited: whatever OW1a/OW1a2 add to a kind's paint function reaches `imm` with no change here |

### 2.3 VIS1

The VIS1 gate is still not on master (`ci/vis1` unmerged, VIS1c not landed), so no VIS1 check
ran. What VIS1 measures, the PaintTrace, is what the kittests compare. Each `imm` widget traces its
shapes and parts under its egui id (`Id::value()`), the same `TraceKind`/part/rect/fill/stroke/font
as the scene node. VIS1c can therefore point its rule-based checks at an `imm` widget unchanged.

### 2.4 Pins

Pin 7 (focus-click / focus-tab): an `imm` button behaves as the scene's under FX1/GTK. A click
focuses with no ring, Tab shows the ring, and Space activates. This is kittested, not checked
against the goldens. Pins 1–6 (GTK/Qt menus, text and scroll goldens) are not touched: no kind
here has them.

## 3. Tests (all added in this lane)

`crates/svelters-egui/tests/imm.rs`. Each test draws a scene node and an `imm` widget in the same
frame, the `imm` one at the node's rect moved down by a whole number of points, and compares the
two PaintTraces relative to their rects:
- `owrust_button_paints_as_the_scene_button_in_every_pointer_state`: rest, hover, press, with no
  theme and with the default theme
- `owrust_button_props_icon_selected_frame_small`
- `owrust_toggles_paint_and_toggle_as_the_scene_ones`: checkbox, switch, toggle-button, checked
  and unchecked, every pointer state, a click flips both, `changed` once
- `owrust_label_and_separator_paint_as_the_scene_ones`
- `owrust_icon_paints_as_the_button_icon_part`: the same tint and size as the button's
  `::part(icon)`. There is no own `icon` kind yet; when no icon font is loaded, both paint nothing.
- `owrust_author_css_reaches_virtual_elements_and_restyles`: a `button` rule, `.toolbar button`
  through `Sv::within`, restyle after a sheet change, a class on the element, `style_as`
- `owrust_accesskit_matches_the_scene_widget`: role, name, toggled, disabled, value, Click/Focus
  actions
- `owrust_disabled_keeps_its_shape_and_its_tooltip`
- `owrust_focus_tab_order_focus_visible_and_key_claims`: Tab goes from the scene button to the
  `imm` one, the ring paint is equal, Space activates and is claimed (`shortcut_override`), a click
  focuses without a ring
- `owrust_chrome_hover_press_and_focus_from_the_callers_response`,
  `owrust_chrome_disabled_on_an_enabled_ui`, `owrust_chrome_galley_in_the_style_colour_and_font`:
  berth's BD7 cases (§4), on a berth-like painter that makes its own responses

Run locally (filtered): `--test imm --test ow1a --test own_widgets --test focus_keys`, 49 tests,
all passing on the final code.

## 4. The cadatomic seam (berth BD7, approved by main-cadatomic)

berth stays free of svelters (cadatomic decision O9). The seam therefore has two halves: berth's
egui-only `ChromePainter` trait (berth BD7, `docs/BD7-chrome-painter.md`, approved in cadatomic
register 5f17ecd) and cadatomic's implementation of it on `svelters_egui::imm`. This lane's first
proposal (a `button` method that owned its interaction, `tab() -> Rect`, `metrics(&Ui)`, pinned tabs,
seven methods) was superseded by BD7's shape. BD7 §5 lists every difference.

**berth's trait (BD7).** It is paint only. berth makes every `Response` (ids, senses, input,
cursors, focus, the whole AccessKit tree, DockText labels), including those of its own buttons: tab
close, scroll arrows, group close and float, region pin and collapse, float dock and close, window
maximize and close. Methods take `&Ui` or `&Painter`, return nothing and are all defaulted (an empty
impl is `EguiChrome`). Containers are called twice (`Stage::Under`, `Stage::Over`):

```rust
fn metrics(&self, ctx: &Context, style: &DockStyle) -> ChromeMetrics;   // no Ui; cached per frame
fn frame(&mut self, ui: &Ui, rect: Rect, f: &FrameInfo);
fn group(&mut self, ui: &Ui, rect: Rect, g: &GroupInfo);
fn strip(&mut self, ui: &Ui, resp: &Response, rect: Rect, s: &StripInfo);
fn tab(&mut self, ui: &Ui, resp: &Response, t: &TabInfo);            // label: berth's Galley in tab_font, label_pos, visible, icon_rect, closable, dirty
fn button(&mut self, ui: &Ui, resp: &Response, b: &ButtonInfo);      // ButtonKind, place, enabled
fn title_band(&mut self, ui: &Ui, resp: Option<&Response>, rect: Rect, t: &TitleBandInfo);
fn stripe(&mut self, ui: &Ui, rect: Rect, s: &StripeInfo);
fn stripe_button(&mut self, ui: &Ui, resp: &Response, b: &StripeButtonInfo);
fn sash(&mut self, ui: &Ui, resp: &Response, rect: Rect, s: &SashInfo);
fn drop_preview(&mut self, painter: &Painter, rect: Rect, d: &DropPreviewInfo);
fn drag_ghost(&mut self, painter: &Painter, pointer: Pos2, g: &GhostInfo);
fn carry_badge(&mut self, ui: &Ui, resp: &Response, b: &BadgeInfo);
```

There are no pinned tabs.

**svelters' half (`imm::chrome`, this lane).** Each of BD7's four cases has a kittest:

| BD7 need | svelters call | Test |
|---|---|---|
| 1. hover, pressed and focused with the R8 ring, read from berth's `Response` | `sv.chrome(ui, resp, &El::new("chrome-button").class("tab-close"), chrome::BUTTON_UA, props)`. The units only *observe* (`interact::observe`: no focus request, no Escape rule, nothing changed in egui). Then `chrome::surface` and `chrome::ring` | `owrust_chrome_hover_press_and_focus_from_the_callers_response`: `:selected` from props and `:hover` from the response on a draggable tab; hover and pressed fills on the close button; a click changes no focus and shows no ring; Tab (berth's ids) gives `:focus-visible` and the ring |
| 2. disabled (the non-closable close: hover-only sense, disabled, on an enabled `Ui`) | `props` holds `StateBits::DISABLED`: no press and no focus, still hoverable (R9), `opacity` from the UA table (the painter is faded) | `owrust_chrome_disabled_on_an_enabled_ui` |
| 3. a paint-only icon by key in a rect | `chrome::icon(painter, id, &st, key, rect)`: tinted by `color`, sized by `font-size`, traced as part `icon`. It goes through `imm::paint_icon_key`, the one function that ICR1's merge re-points at `icons::paint(painter, &icons::registry(ctx), key, rect, &IconStyle{…})`, with the glyph fallback on `None` | checked in `owrust_chrome_galley_in_the_style_colour_and_font` (on today's icon path). The registry test lands with the ICR1 re-point |
| 4. text in a rect with a part's text style; berth lays out with its `tab_font` | `chrome::galley(painter, id, &st, &galley, pos)` paints berth's galley in the style's `color`. `chrome::text_color(&st)` and `chrome::font(ctx, &st)` (CSS `font-*` as an egui `FontId`, for `ChromeMetrics::tab_font`). `chrome::label` lays out and paints a string | `owrust_chrome_galley_in_the_style_colour_and_font` |

cadatomic's painter maps berth's infos to virtual elements and bits. Each of `tab`, `strip`,
`title_band`, `stripe`, `sash` and `drop_preview` becomes a tag (`El::new("tab")`, …). The flags
become bits or classes: `active` → `SELECTED`, `group_focused` → `.focused` on the strip, `dirty` →
`.dirty`, dragging → `DRAGGING`, `!enabled` → `DISABLED`. Painting goes through
`chrome::surface`/`galley`/`icon`/`ring`. `metrics` reads sizes from `Sv::computed` (`height`,
`padding`) and the tab font from `chrome::font`, so berth's layout and the paint agree. Until OW5's
`tab` kind brings UA tables (R13), the look of `tab`/`strip`/`title-band` is cadatomic's theme CSS
(`Ua::NONE`). Chrome buttons use `chrome::BUTTON_UA`, the push button's table.

## 5. Deviations, findings and notes

- **Name.** ARCH-WIDGETS Q5 wrote `svelters_egui::ui::button`. The module is `imm`, as the lane
  brief suggested, because `ui` collides with `egui::Ui` and `Widget::ui` in reading. The builders
  follow egui's own style (`Button::new(..).show(ui, &sv)`), not free functions: options grow
  per kind, as WIDGET-PARITY adds props.
- **Measure state.** `show` measures at rest (state `NONE`). The scene measures with the node's
  current computed style. The two differ only when CSS changes a size by state, which no sheet
  does today.
- **Transitions.** `imm` widgets have no CSS transitions yet: the scene samples them per node
  (`SceneStyle::overlay`), `imm` paints the computed end value at once. Settled frames are equal
  (what the kittests compare). **Follow-up (coordinator, 2026-10-22):** per-id CSS transitions for `imm` widgets and `imm::chrome` (a `Transitions<egui::Id>` sampled like `SceneStyle::overlay`).
- **Host-box state for label and separator** (no `PAINTS_BOX`): the walk paints that box with the
  node's bits as of the start of the pass, `imm` with this pass's unit bits; equal once settled.
- **Resolved by OW1a2** (findings of the first push): the checkbox's full-row host box and the
  lagging host bits of the indicator kinds (they paint their own box now); the separator has no
  AccessKit role any more (decoration), so neither has `imm::Separator`.
- **Icons.** `imm::Icon` and the button's `icon` resolve through today's icon path
  (`paint::paint_icon`, `IconKey`). ICR1's registry re-points it with no change here.
- **State storage.** Unit state lives in egui temp data under `id.with("svelters-imm")`. It stays
  until the context drops (egui temp data is not collected), a few bytes per widget id ever drawn.
  This is fine for chrome. A long list of `imm` rows with changing ids should use scene widgets.
- **Stable cores.** `checkbox::paint_indicator`/`measure_indicator`, `button::paint`/`measure`, `label::layout`/`paint`/`measure`, `switch::paint`, `toggle_button::paint`/`measure`, `separator::paint`/`measure` keep their signatures; OW1a3 changes the `WidgetCx` call sites and the insides of `label::paint`/`layout`.
- **Not done.** No `use:` adapters (OW1a3). No menu-button or tab kinds (OW3, OW5). No
  `ChromeMetrics` helper (berth's trait decides the fields first). OW-CSS stage 2 part styles reach
  `imm` through `SceneStyle::part_virtual` (one TODO).
