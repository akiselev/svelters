# WG1: the SC1 widget subset on egui

Spec: `plans/research/TRACKS.md` §2.1 WIDGETS, row WG1; `plans/research/CADATOMIC.md` SC1 row and SVP6;
`plans/research/EGUI.md` §5 (Tier 0) and §1.5 (kittest fails unnamed inputs). Builds on EG0, RA1, SG1/SG2,
EM1, SG3 and EV2b (merged in).

## What landed

Seven new built-in kinds, appended to `KINDS` (every earlier kind id and member id is unchanged), plus
three pieces of shared machinery that every kind gets: `tooltip` on containers too, an `invalid` flag, and
accessible-name forwarding.

| Kind | Base | Props (own) | Events | Bindings |
|---|---|---|---|---|
| `toggle-button` | Widget | `text`, `checked` (widget), `radio`, `frame`, `font-weight` | `changed(checked)`, `clicked` | `bind:checked` |
| `combo-box` | Widget | `options` (`string[]`), `selected` (index, widget), `hint-text` | `changed(selected)` | `bind:selected` |
| `separator` | Widget | `orientation` (`horizontal` \| `vertical`) | — | — |
| `icon` | Widget | `name`, `glyph`, `family` (default `icons`), `size` (null = body), `font-weight`, `frame`, `selected` | `clicked`, `double-clicked`, `secondary-clicked` | — |
| `grid` | Widget | `grid-template-columns` (default `auto 1fr`), `gap`, `row-gap`, `column-gap`, `padding`, `align-items`, `justify-items` | — | — |
| `collapsing-header` | Box | `text`, `open` (widget) | `changed(open)` | `bind:open` |
| `scroll-area` | Box | `direction` (`vertical` \| `horizontal` \| `both`), `scroll-x`, `scroll-y` (widget) | `changed(scroll-x, scroll-y)` | `bind:scroll-x`, `bind:scroll-y` (natively normalizing: clamped to the content) |

On every kind (Widget base): `invalid` (bool). On `label` and `button`: `font-weight`.

| Piece | Where |
|---|---|
| Kind tables, one file per family | `kinds/controls.rs` (toggle-button, combo-box), `kinds/decor.rs` (separator, icon), `kinds/layouts.rs` (grid, collapsing-header, scroll-area); `KINDS` (`kinds/builtin.rs`) only lists them; `invalid` and `font-weight` appended in `builtin.rs` |
| `Ty::StrList` (spec `"type": "list", "of": "string"`, TS `string[]`, ABI `SVL_LIST`; a static attribute is whitespace-separated, the compiler's `strlist` form) | `kinds/mod.rs`, `scene/props.rs`, `spec.rs` |
| `W` variants, `Common::invalid`, `is_container` for the three containers, `text_of` | `scene/node.rs` (SCENE file, small edit) |
| Payload defaults | `scene/tree.rs::builtin_payload` (SCENE file, small edit) |
| Prop get/set (a bad `grid-template-columns` is refused; `open` and `direction` restyle; `font-weight`, `options`, icon props remeasure; offsets are paint-only) | `scene/props.rs` |
| Measure (toggle, combo, separator, icon, collapsing header row; `font-weight` on label/button), taffy grid (`grid` feature, `LayoutGridContainer`), `parse_grid_template`, collapsing-header layout, `overflow: scroll` | `layout.rs` |
| Leaf painting; `container` (container tooltip, collapsing header row, scroll viewport); `invalid_outline`; `widget_changed` | `leaf.rs` |
| Container branch → `leaf::container`; `paint_children`; `invalid` outline of provider kinds | `walk.rs` (small edit) |
| Name fallbacks and AccessKit `invalid` | `a11y.rs` (EMBED file, small edit) |
| SVP6: `register_weight`, `register_icons`, `icon`, `clear` | `fonts.rs` (`svelters_egui::fonts`) |
| Binding generation: the bound value is the `changed` argument named like the prop (else 0); `natively_normalizes`/`read_back` for `scroll-x`/`scroll-y`; string layout props are `"string"` in the style table | `spec.rs` |
| Generated | `metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` (`SVELTERS_UPDATE=1 … --test spec`) |
| Tests | `tests/widgets.rs` (one kittest per kind, fonts, invalid, tooltip on every kind), `tests/widgets_panel.rs` + `fixtures/embed-egui/src/TaskPanel.svelte` (the task-panel-shaped form, compiled from Svelte, mounted in a slot) |

### Behaviour notes

- **grid**: auto-placement row by row; rows are implicit `auto`. `grid-template-columns` accepts `auto`,
  `min-content`, `max-content`, `<n>fr`, `<n>px` or a bare number (points), `<n>%`, `minmax(a, b)`,
  `fit-content(<len>)`, `repeat(<n>, tracks)`. No per-child placement (`grid-column`/span) yet.
- **collapsing-header**: a flex box (its `Box` props lay out the body) whose top padding is the measured
  header row and left padding the indent (plus its own `padding`). Closed, it is a leaf of the header's size
  and its children get taffy's hidden layout (not drawn, not in the AccessKit tree). The header is the node's
  own egui id (anchor-stable), role `DisclosureTriangle`, `expanded` state; click, Enter or Space toggle it.
  No open/close animation. A toggle from the widget is a style change (`invalidate_style`): the body is drawn
  from the next layout (one frame later), never from a stale hidden layout.
- **scroll-area**: taffy `overflow: scroll` on the scrolling axes (`scrollbar_width` 0: egui's bars float),
  so children are laid out at their full size and overflow; the walk draws them inside
  `egui::ScrollArea::show_viewport`, culled by its clip rect. A host write of an offset is applied once
  (the node keeps what egui reported last frame); egui's clamped offset is written back and announced when
  it differs (a user scroll, or a clamped write).
- **toggle-button**: `egui::Button::selected`; `radio` makes a click only check (a segmented group: the app
  unchecks the others, e.g. `checked={dir === d} onchanged={() => (dir = d)}`) and the role `RadioButton`;
  otherwise role `Button` with `toggled`.
- **combo-box**: `egui::ComboBox` with `selectable_label` rows; `selected` out of range shows `hint-text`.
  It has no caption: name it with `accessible-name` or `tooltip`.
- **icon** (SVP6): text = the family's name table entry for `name`, else `glyph`, else `name`. The family is
  `FontFamily::Name(family)` when the context has it, else Proportional. Frameless: role `Image` (or
  `Button` when `clicked` is listened); `frame`: an `egui::Button` (icon button). Name: `accessible-name`,
  else `tooltip`, else `name`. egui 0.37 **panics on a glyph no installed font has**
  (`MissingGlyphPolicy`); cadatomic's Phosphor family covers its table.
- **font-weight** (SVP6): `fonts::register_weight(base, weight, family)`; a node picks the registered family
  nearest its weight (ties: heavier from 400 up), with the base standing for 400; a family the `Context`
  does not have is skipped. Measured and drawn with the same font.
- **tooltip on every kind**: leaves and provider kinds already had it; containers now get a hover-only
  response at their rect (it takes no clicks from children), the collapsing header on its header row.
- **invalid**: `Common::invalid`, drawn as a 1 pt outline in `Visuals::error_fg_color` around what the node
  drew (leaves, provider kinds, container rects, the header row), and AccessKit `invalid` (`a11y::decorate`,
  so provider kinds get it too). The app sets it; nothing sets it natively.
- **Accessible names** (`a11y::name`): `accessible-name`; else a text field's hint, a slider's caption, an
  icon's tooltip or name; else, for a node without text of its own (combo box, provider kind, slider without
  caption), its tooltip. Checkbox, button, toggle-button and collapsing header name themselves by their text.

## Deviations

- **`grid-template-columns`, not `columns`**: layout props use CSS names (D1), so the style table gets the
  setter for free. Its default (`auto 1fr`, a form) is not CSS's (`none`).
- **The new containers' chrome is painted from `leaf.rs`** (`leaf::container`), called by the walk's container
  branch; the walk keeps the child iteration (`walk::paint_children`). `Box`/`Window` go through the same
  function and only gain the tooltip and `invalid` outline.
- **`collapsing-header` and `scroll-area` extend `Box`** (chain `…, Box, Widget`), so their bodies take
  `flex-direction`, `gap`, `padding`, `align-items`, `justify-content`.
- **`invalid` does not set `StateBits::INVALID`.** The prop lives in `Common`; the state bit would need the
  scene's write path (`access.rs`, SCENE). CS5e: either read `Common::invalid` for `:invalid`, or mirror it in
  `Scene::set_value`.
- **No collapsing animation, no striped grid, no per-child grid placement.**
- **One `changed` event carries both offsets** on `scroll-area`; the spec's binding reads the argument named
  like the prop.

## Notes for WG2 (and WG2a/b, WG4a/b) and CS5e

- New kinds: a family file under `kinds/`, its `KindDef` consts appended to `KINDS`, a `W` variant and
  `builtin_payload` arm, `set_prop`/`get_prop` arms, a `Metrics::intrinsic` arm, a `leaf` arm (or a
  `leaf::container` arm for a container that paints). Use `leaf::widget_changed` for widget-side writes
  (journal, `prop_changed`, `changed` event) and call `a11y::decorate` on the response.
- A container must declare its layout behaviour: what changes its own size (return `Dirty::LAYOUT` from
  `set_prop`, or `invalidate_style` from the walk) and what is paint-only.
- Popups (combo box) draw in egui `Area`s outside the node rect and outside the hit list (SG3); menus in WG2
  will hit the same thing.
- **CS5e** styles these kinds: per-kind paint is in `leaf.rs` (`weighted`, `invalid_outline`, the collapsing
  header's `interact_selectable` visuals, the toggle's `Button::selected`). `font-weight` is a prop today; the
  computed style should feed the same `fonts::family_for`. The grid parser (`layout::parse_grid_template`) is
  the place to accept `grid-template-columns` from CSS.

## Verification (local)

- `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets` and `-p egui-kinds-example --all-targets`:
  clean (after the last merge of origin/master).
- `SVELTERS_UPDATE=1 tools/dev/cargo-slot cargo nextest run -p svelters-egui --test spec` regenerated the two
  generated files.
- `tools/dev/cargo-slot cargo nextest run -p svelters-egui --test widgets --test widgets_panel`: 11/11 after
  the last code change (bundle from `pnpm --filter ./fixtures/embed-egui build`); `--test spec --test observe`
  passed on the code before the last test-only fixes.
- Not run locally (coordinator's call): the full svelters-egui and egui-kinds-example suites, the workspace,
  conformance, the other backends, the JS tests. CI runs them.
