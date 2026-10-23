# OW1a4: `use:` actions of the interact units, collapsing-header accordion, actions and icon

Lane `ci/ow1a4` (fresh agent, 2026-10-22). Handoff: OW1a3 §5 items 1 and 2 (item 3, the table.rs
egui cells, is OW5b's; toast.rs was OW7's; item 4 stays open).

## 1. What landed

### 1.1 `use:` actions (`crates/svelters-egui/src/use_units.rs`)

One EV2b behaviour (`#[svelters::behavior(namespace = "sv")]`) per unit, exported by
`svelters:behaviors/sv` (the dump writes `behaviors/sv.js` and `behaviors.d.ts`; the event payload
records go to `svelters:app/sv`). Each wraps `UseUnit<U>` (OW0's adapter) and runs it in `after_ui`
through the new `UseUnit::run_on(resp, host_bits)`: the unit's `OWNS` bits are written to the host,
its focus requests applied, its AccessKit properties added to the host's node, and its arrow/Escape
claims become egui's focus-lock filter on the focused host (EV3 R11). User-visible intents go through
`BehaviorCx::dispatch` (EV1 filters can prevent them); value units are uncontrolled (they apply the
change unless prevented). `detached` clears the bits.

| Export | Params | Events | Bits |
|---|---|---|---|
| `hoverable` | — | `hover(bool)` | `:hover` |
| `pressable` | `activateOn`, `autoRepeat` | `press({count, button})`, `down`, `up(inside)`, `cancel`, `repeat` | `:active` |
| `focusable` | `tabStop`, `focusOnClick` | `focus(reason)`, `blur` | `:focus`, `:focus-visible` |
| `toggleable` | `checked`, `indeterminate`, `tristate`, `radio` | `toggle({checked, indeterminate})` | `:checked`, `:indeterminate` |
| `expandable` | `expanded`, `byClick` | `expand(bool)` | `:expanded` |
| `roving` | `on` | `rove(index)`; moves focus among siblings with `use:roving` | — |
| `rovingList` | `enabled`, `orientation`, `wrap`, `current` | `move(index)` | — |
| `typeahead` | `timeout` | `search(text)` | — |
| `anchored` | `anchor`, `placement`, `gap`, `flip`, `shift`, `matchWidth` | `placed({side, x, y, width, height})` | — |
| `dismissable` | `escape`, `outside` | `dismiss({reason, x, y})` | — |
| `tooltipable` | `delay`, `duration` | `tip(bool)` | — |
| `draggable` | `axis`, `threshold` | `drag({phase, dx, dy})` | `:dragging` |
| `dragToValue` | `value`, `min`, `max`, `step`, `speed` | `scrub(v)`, `commit(v)` | `:active`, `:dragging` |
| `steppable` | `value`, `min`, `max`, `step`, `page`, `wrap`, `wheel` | `change(v)` | — |
| `resizable` | `size`, `axis`, `invert`, `min`, `max`, `step`, `collapsible` | `resize(v)`, `commit(v)` | `:hover`, `:active`, `:dragging`, `:focus-visible` |
| `scrollable` | `content`, `offset`, `axes`, `keys` | `scroll({x, y})` | — |

**DragSource/DropTarget: no `use:` export (decided in-lane).** WG4b's common `drag-type`,
`drag-data` and `drop-types` props already run these two units on every element (`kinds/dnd.rs`)
with `:dragging` and `:drop-target`; an action would handle the same drag twice.

Scene seams added for it: `BehaviorCx::{host_bits, scene, focus_node}`; `behavior::around` applies a
pending `WANT_FOCUS` to any behaviour host (boxes take focus moves).

### 1.2 collapsing-header (`widgets/collapsing.rs`)

- `accordion="<group>"`: a user opening a header closes the open siblings of the same group through
  `WidgetCx::with_peer` (each announces `changed(false)`); headers without the prop, or in another
  group, are untouched.
- `slot="end"` header actions: `end` is the kind's second named slot (`kinds::extra_slots`; the
  backend resolves it to slot id 2, the spec lists it so the compiler accepts `slot="end"`, and the
  child carries the `kinds::SLOT` ext entry). The actions are real child nodes, taken out of the body's
  flow (`LayoutCx::out_of_flow`), placed right-aligned and centred in the header row, open or closed,
  and drawn while the pointer is over the header or it holds focus (`:focus-within`); the caption
  elides before them. The header itself stays a paint part: the actions did not need it to be a
  node part.
- `icon`: a key or `family:name`, `::part(icon)` (16px, `icon` colour, `disabled-fg` when disabled),
  painted through ICR1's registry with `paint::paint_icon`; `content: none` hides it.
- `open` is now a `measure` prop: a host write (`bind:open`, JS) lays the node out again. Before, only
  a user toggle did, so a JS-opened header kept its body at the hidden layout.

## 2. Acceptance

### R-rules (DESIGN §2)

- **R3** hover: the `use:` units set `:hover`; note the bits a behaviour writes show from the next
  frame (behaviours run after the host draws). Own widgets keep same-frame hover.
- **R8** focus keyboard-only: `use:focusable` sets `:focus-visible` on Tab, never on a pointer press
  (test `ow1a4_use_focusable_is_a_tab_stop_with_focus_visible`).
- **R9** disabled stays hoverable: `use:hoverable`/`use:tooltipable` run on disabled hosts (the unit
  rules).
- **R10** cursors: `use:resizable` sets the resize cursor on hover and drag.
- **R11** sashes: `use:resizable` runs the sash unit (hover delay, keyboard, collapse).
- **R14** chevrons: the collapsing-header twistie stays a chevron that follows `:expanded`
  (test `ow1a4_twistie_is_an_r14_chevron_that_follows_expanded`).

### WIDGET-PARITY §2.16 rows

- QToolBox one-page-open (`accordion`): **done**.
- QToolBox `setItemIcon` (`icon`): **done**.
- collapsing-header header actions (`slot="end"`): **done**.
- `QToolBox::tab` styling: `::part(header)` and `:expanded` done; `:first`/`:last`/`:only-one`
  **deferred** to OW-CSS's structural pseudo-classes.

### VIS1

`svelters-testkit --test vis1`: 3/3 pass locally (`own_widgets_match_vscode` includes the pane
header probe on `collapsing-header::part(header)`), with the gallery fixture rebuilt.

### Pins

No native-behaviour golden (pins 1-7) covers collapsing-header or the `use:` actions; none changed.

### Tests (`crates/svelters-egui/tests/ow1a4.rs`, 22, all pass locally)

- the module and typings export all 16 actions;
- one test per export (16), plus `ow1a4_use_unit_bits_leave_with_the_behaviour`;
- `ow1a4_accordion_opening_one_closes_the_others`, `ow1a4_header_actions_in_slot_end_sit_in_the_header_and_show_on_hover`,
  `ow1a4_header_icon_paints_through_the_icon_primitive`, `ow1a4_twistie_is_an_r14_chevron_that_follows_expanded`.

Also run locally, green: `ow1a2`, `ow1a3`, `spec`, `scene_memory`, `scene` (with
`mutations_allocate_nothing_without_observers`), `own_widgets`, egui-kinds `kinds`
(`checked_in_kinds_json_and_typings_are_current`), `wg4b` `gallery_export_stubs_are_current`.

Generated, in order: `SVELTERS_UPDATE=1 … --test spec` (egui.json, egui.d.ts), `gen_props.py` (no
change), `gen_reference.py` (collapsing-header page); the app dumps of `examples/egui-kinds` and
`fixtures/gallery-egui` (`SVELTERS_UPDATE=1`, they now carry `behaviors/sv.js`, `sv.js` and the `sv.*`
behaviour kinds).

## 3. Open (handoff)

1. **Behaviour hooks for AccessKit and EV3:** the `use:` units add their AccessKit properties from
   `after_ui` through `accesskit_node_builder`, and claim keys through egui's focus-lock filter only.
   A `Behavior::claims` that feeds `focus::push_entry` (R11 key take-out for host keymaps) is not
   there yet.
2. **Same-frame bits for `use:` units (R3):** run the unit in `before_ui` on last frame's response
   (`ctx.read_response`) so bits apply before the host paints.
3. **Double handling diagnostic:** a `use:` unit on an own widget that composes the same unit is not
   reported yet (ARCH-WIDGETS §1.2).
4. **Security policy (ARCH-SECURITY):** the `sv` behaviours are reachable by any code that can import
   `svelters:behaviors/sv`; they only act on the element they are attached to, but the module needs
   an entry in the extension policy's module allow-list when that lands.
5. Still from OW1a3 §5: `label::selection` colours and `:visited` (OW-P2).
