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
| Same paint | Each OW1a kind's paint (and measure) moved, unchanged, into a function the kind and `imm` both call: `widgets::{button,checkbox,switch,toggle_button,label,separator}::paint` (+ `measure`, `Look`). Kinds without `PAINTS_BOX` (checkbox, switch, label, separator) also get the CSS host box the walk paints behind them (`Ran::host_box`, as `style::before_leaf`). |
| EV3 without a node | `focus::record_foreign`. An `imm` widget is in egui's Tab order in paint order with the scene's widgets, outside every focus scope (`Entry.node` is now `Option<NodeId>`). While it has focus, the keys its role claims (Space, Enter) leave the pass's input (R11), and `EguiBackend::shortcut_override` answers for it. |
| AccessKit | `widget_info` as the kind, then the kind's role and the units' `a11y` (toggled, Click, Focus), `accessible_name` (EM1). |
| `imm::chrome` (paint-only seam) | `chrome::state(ui, &resp, props)` runs Hoverable, Pressable and (when the sense is focusable) Focusable on **the caller's own response** (any sense, such as berth's draggable tab). `sv.chrome(ui, &resp, &el, ua, props)` returns the `Styled` in that state. `chrome::surface` / `label` / `ring` paint box, text and focus ring through the primitives, traced under the caller's id. |

Shared code changed (pure moves, no behaviour change; the OW1a, own_widgets and focus_keys tests
pass unchanged): `widgets/{button,checkbox,switch,toggle_button,label,separator}.rs` (paint and
measure functions), `focus.rs` (`record_with` → `push_entry`, `record_foreign`, `foreign_focused`),
`style.rs` (`generation`, `compute_virtual`, `part_virtual`), `tests/common/mod.rs`
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
- `owrust_chrome_paints_a_foreign_element_on_the_callers_response`: a berth-like draggable tab on
  its own response, `tab:selected` / `tab:selected:hover` from author CSS

Run locally (filtered): `--test imm --test ow1a --test own_widgets --test focus_keys`, 49 tests,
all passing on the final code.

## 4. The cadatomic seam (proposal; the coordinator settles it with main-cadatomic)

berth stays free of svelters (cadatomic decision O9). The seam therefore has a berth half (an
egui-only trait, berth's package, API reviewed by main-cadatomic) and a cadatomic half (an
implementation of that trait on `svelters_egui::imm`).

**svelters side (this lane, on master after merge).** cadatomic needs nothing more than §1:
- plain controls inside the chrome (tab close, strip scroll arrows, pin, float, maximize, overflow):
  `imm::Button::new("").icon("codicons:close").class("tab-close").accessible_name("Close")
  .frame(false).id(id).show_at(ui, rect, &sv)`. These are full widgets: units, focus, AccessKit, R-rules;
- surfaces berth interacts with itself (tabs, which drag; strips; title bands; sashes; drop
  previews): `let st = sv.chrome(ui, &resp, &El::new("tab").class("dirty"), Ua::NONE,
  StateBits::SELECTED)`, then `imm::chrome::surface` / `label` / `ring`. The look comes from
  cadatomic's theme CSS (`tab`, `tab:selected`, `tab-strip`, `title-band`, …) until OW5's `tab` kind
  brings UA tables (R13). Then the same calls take `widgets::tab::UA`.

**berth side (proposed shape for berth's `ChromePainter`, for main-cadatomic to review).** Every
method gets the `Ui`, the rect berth laid out and the response berth made. The painter only paints,
except `button`, which owns its interaction:

```rust
pub trait ChromePainter {
    /// Sizes berth lays out with (tab height, paddings, strip and title-band heights, sash hit width).
    fn metrics(&self, ui: &egui::Ui) -> ChromeMetrics;
    fn strip(&mut self, ui: &mut egui::Ui, rect: egui::Rect, s: &StripInfo);           // focused group, overflow
    fn tab(&mut self, ui: &mut egui::Ui, resp: &egui::Response, rect: egui::Rect, t: &TabInfo) -> egui::Rect; // returns the content rect (title)
    fn button(&mut self, ui: &mut egui::Ui, id: egui::Id, rect: egui::Rect, b: ChromeButton, enabled: bool) -> egui::Response; // Close, ScrollLeft/Right, Pin/Unpin, Float/Dock, Maximize/Restore, Collapse
    fn title_band(&mut self, ui: &mut egui::Ui, resp: &egui::Response, rect: egui::Rect, title: &str, active: bool);
    fn sash(&mut self, ui: &mut egui::Ui, resp: &egui::Response, rect: egui::Rect, vertical: bool);
    fn drop_preview(&mut self, painter: &egui::Painter, rect: egui::Rect, kind: DropKind);
}
// TabInfo { title, active, focused_group, dirty, pinned, closable, dragging }
```

berth's default painter keeps today's egui-only drawing (`draw.rs`: `colors()`, `icon()`).
cadatomic's painter maps `TabInfo` to classes and bits: `active` → `SELECTED`, `focused_group` →
`.focused` on the strip, `dirty`/`pinned` → classes, `dragging` → `DRAGGING`. It calls `imm` as
above. `ChromeMetrics` comes from the same CSS (`Sv::computed` → `padding`, `height`), so berth's
layout and the paint agree.

Proposed for STATUS.md's "Requests from cadatomic" (svelters' side): **OW-RUST seam: svelters
ready (`svelters_egui::imm`, `imm::chrome`); berth's `ChromePainter` shape proposed in
OW-RUST-immediate.md §4 for main-cadatomic's review.**

## 5. Deviations, findings and notes

- **Name.** ARCH-WIDGETS Q5 wrote `svelters_egui::ui::button`. The module is `imm`, as the lane
  brief suggested, because `ui` collides with `egui::Ui` and `Widget::ui` in reading. The builders
  follow egui's own style (`Button::new(..).show(ui, &sv)`), not free functions: options grow
  per kind, as WIDGET-PARITY adds props.
- **Measure state.** `show` measures at rest (state `NONE`). The scene measures with the node's
  current computed style. The two differ only when CSS changes a size by state, which no sheet
  does today.
- **Host-box state for kinds without `PAINTS_BOX`.** The walk paints that box with last pass's
  response bits (`before_leaf` reads `read_response`, and it sets `focus-visible` on any focus).
  `imm` uses this pass's unit bits. Both are equal once the frame settles, which is what the tests
  compare. Finding for OW1a2: the scene's bits for those kinds lag one frame, and a pointer-focused
  checkbox's host gets `:focus-visible`.
- **Finding for OW1a2.** The theme's `checkbox` base rule sets background and border on the
  *host*. The walk paints that host box across the whole row (448 px wide in the kittest) behind the
  indicator, where `parts.rs` intends the rule for the indicator only. `imm` reproduces it, for
  parity. Fix it in the kind or the sheet, and `imm` follows with no change here.
- **Finding.** An own `separator` with no `accessible-name` fails kittest's unnamed-input check
  (Splitter is an input role) in this harness. The test names both separators.
- **Icons.** `imm::Icon` and the button's `icon` resolve through today's icon path
  (`paint::paint_icon`, `IconKey`). ICR1's registry re-points it with no change here.
- **State storage.** Unit state lives in egui temp data under `id.with("svelters-imm")`. It stays
  until the context drops (egui temp data is not collected), a few bytes per widget id ever drawn.
  This is fine for chrome. A long list of `imm` rows with changing ids should use scene widgets.
- **Not done.** No `use:` adapters (OW1a3). No menu-button or tab kinds (OW3, OW5). No
  `ChromeMetrics` helper (berth's trait decides the fields first). OW-CSS stage 2 part styles reach
  `imm` through `SceneStyle::part_virtual` (one TODO).
