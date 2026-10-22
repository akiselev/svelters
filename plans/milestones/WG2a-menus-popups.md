# WG2a: menus, popups, modal and inner windows on egui (`crates/svelters-egui`)

Design: `plans/research/EGUI.md` §5 (the element catalog the user approved as the built-in widget set on
2026-10-22, fast-tracked; §8), the X4 vocabulary (`plans/milestones/X4-menus.md`, `fixtures/menus`) and N1
naming. Lane WG2a (WIDGETS), branch `ci/wg2a`, from master `950678d4`; merged WG1 (`origin/ci/wg1` up to
`66fcd9df`: `Common::invalid`, `leaf::container`/`walk::paint_children`, `a11y::name` fallbacks, `Ty::StrList`)
and master up to `9e8b242b` (EV2b, CS0, Q3, Q1, LD1, TK1, CS5e, the cargo-slot and artifact rules; the
WG1/CS5e overlaps in `walk.rs` and `layout.rs` resolved here: CS5e's container opacity around WG1's chrome, WG1's
grid and collapsing header before CS5e's computed `display: grid`, one `LayoutGridContainer`). Its diff against master
therefore also carries WG1 until WG1 merges.

## Elements

Names are X4's (GTK and Qt already use them) where the kind exists there; `menu-button` and `popup` are
EGUI.md §5's (GTK's `menu-button` takes the same name; GTK's `popover` is `popup` here).

| Element | egui | Base | Children | Props (own) | Events | `bind:` |
|---|---|---|---|---|---|---|
| `menu-bar` | `egui::MenuBar` | Widget | `menu`, `action`, `menu-separator` | — | — | — |
| `menu` | `MenuButton` in a bar, `SubMenuButton` in a menu | MenuEntry | `menu`, `action`, `menu-separator` | `label` (text content) | — | — |
| `action` | `Button` (`shortcut_text`), `Checkbox`, `RadioButton` | MenuEntry | — | `label` (text content), `shortcut`, `checkable`, `checked`, `group` | `toggled(checked)`, `activate(checked)` | `checked` |
| `menu-separator` | `Ui::separator` | MenuEntry | — | — | — | — |
| `context-menu` | `egui::Popup` (menu kind) | MenuEntry, attaches to Widget | as `menu` | — (methods `popup`, `popdown`) | — | — |
| `menu-button` | `containers::menu::MenuButton` | Widget | as `menu` (a `menu` child's items inline) | `label` (text content), `active` (methods `popup`, `popdown`) | `changed(active)` | `active` |
| `popup` | `egui::Popup` | Box, attaches to Widget | Widget | `open` (false), `position` (bottom/top/left/right), `autohide` (true) (methods `popup`, `popdown`) | `changed(open)`, `closed` | `open` |
| `modal` | `egui::Modal` | Box | Widget | `open` (true), `title`, `dismissable` (true) | `changed(open)`, `closed` | `open` |
| `inner-window` | `egui::Window` | Box | Widget | `open` (true), `title`, `default-width` (320), `default-height` (240), `default-x`/`default-y` (null: egui places it), `closable`, `collapsible`, `resizable` (all true) | `changed(open)`, `closed` | `open` |

`MenuEntry` (abstract) has `enabled`, `visible`, `tooltip`, `accessible-name`, `anchor`: menu entries are not
widgets (a `box` does not accept one, and they have no layout props). The overlays derive from `Box`, so their
content is a flex box (`flex-direction`, `gap`, `padding`, `align-items`, ...) and their own `width`/`height` size it.

Behaviour:
- **Actions.** A click (or Enter, or an AccessKit click) picks an action. A check item toggles `checked`; a radio
  item (`group` non-empty) sets it and unchecks the other actions of the same parent and group, each announced
  (`prop_changed` when observed, `toggled`); then `activate(checked)`. `bind:checked` lowers onto exact
  observation with `toggled` as the fallback signal (the spec generator now binds through the first signal with
  an argument named like the prop when a kind has no `changed`).
- **Shortcuts.** `Ctrl+O`, `Ctrl+Shift+S`, `F5`, `Ctrl++`: modifiers `Ctrl`, `Shift`, `Alt`, `Cmd` (macOS
  command), `Primary` (Cmd on macOS, Ctrl elsewhere); keys are egui `Key` names. Validated on write (an unknown
  key or modifier is an error). Shown right-aligned (`ctx.format_shortcut`) and set as AccessKit
  `keyboard_shortcut`; the accessible name stays the label. Active while the menu's owner (`menu-bar`,
  `menu-button`, a `context-menu`'s owner) is drawn and enabled, menu open or not (`consume_shortcut`).
- **Context menus** open at the pointer on a secondary click on the owner's rect in the owner's layer
  (`Ui::rect_contains_pointer`, so a popup or window above it keeps the click). Nested owners: the innermost
  (smallest rect) wins within one pass. `popup()` opens it below the owner. The owner is the parent: a leaf it is
  attached to (the compiler's `attaches_to`; `scene/tree.rs` lets `context-menu` and `popup` under a slotless
  node), or the box it is placed in.
- **menu-button** opens on click like egui's; `active` follows egui's popup memory both ways (a host write or
  `popup`/`popdown` opens or closes it at the next pass; a user open or close is announced).
- **popup** is anchored to its owner (`RectAlign` from `position`, egui flips it when it does not fit), closes on
  Escape and, with `autohide`, on a click outside (`CloseOnClickOutside`). **modal** draws egui's backdrop and
  blocks the layers below; Escape or a backdrop click closes it when `dismissable`. **inner-window** is a movable
  `egui::Window` (title bar, close button when `closable`, collapse, resize). A user close (or `popdown`) writes
  `open = false`, announces it and fires `changed(false)` and `closed`.

## How it is built

| File | Content |
|---|---|
| `kinds/menus.rs`, `kinds/popups.rs` | The tables (one file per family, ARCH-EMBED §9), appended to `KINDS` after WG1's; `slot_accepts` and `attaches_to` (spec). |
| `menus.rs` | `MenuW` (the family's payload), set/get prop, `parse_shortcut`, measure, `announce`, activation and radio groups, shortcuts, painting of `menu-bar`/`menu-button` (`leaf.rs` dispatches) and `context-menu`. |
| `overlays.rs` | `OverlayW`, set/get prop, `out_of_flow`, the walk's out-of-flow dispatch (`paint_node`, `paint_attached`), `popup`/`modal`/`inner-window` painting. |
| `layout.rs` (additions) | `style_of`: out-of-flow kinds are `display: none` in their parent's flow; `intrinsic` arms; `layout_overlay`: an overlay's subtree laid out on its own. |
| Dispatch lines | `scene/node.rs` (`W::Menu`, `W::Overlay`, `is_container`, `text_of`), `scene/props.rs` (two routing arms each way), `scene/tree.rs` (payloads; attaching kinds under any node), `walk.rs` (out-of-flow dispatch; `paint_attached` after a leaf), `leaf.rs` (menu dispatch), `ops.rs` (`popup`/`popdown`), `spec.rs` (accepts, `attaches_to`, binding signal), `kinds/mod.rs` (`P` keys, registration). |

**Menus are immediate.** `menu-bar` and `menu-button` are taffy leaves; their entries are scene children taffy
never lays out (`display: none`), drawn each frame with egui's menus, so hover-to-switch in a bar, submenus and
arrow-key navigation are egui's own. A `menu-bar` takes no width of its own (one interact row high): it fills a
column; in a row it needs `flex-grow`.

**Overlays are laid out on their own.** In the parent's flow an overlay is `display: none` (no size, no gap).
When it is shown, `layout::layout_overlay` sets the top's display to flex for one run, computes its subtree as a
root (max-content, or the window's size for a resizable `inner-window`), rounds it and places it root-relative at
the area's cursor, so the walk paints its children with the usual origin. A root run may give the subtree the
hidden layout beneath a cache the overlay run would reuse (a boundary inside the overlay), so the overlay keeps
the layout generation it saw (`Counters::nodes_placed`, which every root run moves) and clears its subtree's
caches when it changed. Root runs leave the subtree's rects at zero until the next paint (`bounds()` of a node
inside a closed overlay is 0×0).

## Degradations and notes

- **No OS-modal or transient windows** (EGUI.md §5): `modal` is the egui form of a modal dialog; X7's
  `show="modal"` on a toplevel stays unsupported on egui (no renderer-side mapping to `<modal>` here).
- **Focus restore** after a menu or popup closes is EVENTS EV3's; until then egui's own handling applies (menus
  take focus for arrow keys and do not hand it back to the opener).
- **No icons** on `menu`/`action` yet (X4's `icon` prop is not in the table; an unknown prop is a compile
  error rather than a silent no-op). Add it with WG1's `icon` machinery (`fonts::icon_text`).
- **No native menus** (muda) — a Tier 2 pack.
- Only one egui memory popup is open per viewport: opening a context menu or menu-button menu closes another
  (egui's rule). `popup` keeps its own `open` and can stay open alongside a menu.
- egui's `Popup`/`Window` run a sizing pass on the first frame they are shown (invisible for one frame).

## Tests

- `tests/menus.rs` (kittest, real vtable): `menu_bar_menus_actions_and_check_items`,
  `radio_group_and_shortcuts`, `menu_button_opens_its_menu_and_reports_active`,
  `context_menu_on_a_leaf_and_on_a_box`, `popup_attached_to_a_button`, `modal_dialog`,
  `inner_window_with_content_and_close_button`, `kinds_resolve_and_entries_stay_out_of_boxes`. Each kind is
  rendered, found by role and label and driven by pointer, keyboard or AccessKit.
- Gallery page `fixtures/gallery-egui-menus` (every kind of the family; `<!-- kinds: -->` header like the other
  galleries), run end to end on the QuickJS host by `tests/menus_app.rs` (`gallery_page_menus_and_popups`) and by
  the TestHost (`expected.oplog`, `script.json`). `tests/oplog_replay.rs` skips it (its generic replay cannot produce menu
  and popup input natively; `menus_app.rs` does). It is its own fixture because there is no egui gallery app yet; it becomes a
  page of one when one exists.
- `crates/svelters-compiler/tests/wg2a_egui_menus.rs`: the page compiles, attaching and nesting rules, unknown
  props.
- `metadata/semantics/egui.json` and `packages/native-runtime/types/egui.d.ts` regenerated
  (`SVELTERS_UPDATE=1 cargo nextest run -p svelters-egui --test spec`); `packages/style/src/props.generated.js`
  and `crates/svelters-style/src/tables.generated.rs` regenerated (`tools/metadata/style/gen_props.py`: the
  overlays take the box layout setters, `menu-bar`/`menu-button` the widget ones).
- WG1's `tests/widgets.rs` `every_kind_has_a_tooltip_and_inputs_are_named` skips the menu entries (not widgets)
  and the overlays (out of flow) and sets a kind's own text-content prop; `tests/menus.rs` covers them.

## Verification

Local, after the last code change: `cargo check -p svelters-egui --all-targets` (clean);
nextest `-p svelters-egui --test menus --test menus_app --test spec --test widgets --test oplog_replay` (23/23) and
`-p svelters-compiler --test wg2a_egui_menus --test g1_egui_spec` (7/7); the gallery's
TestHost run (`expected.oplog` written with it). Not run locally: the full workspace, conformance, the other
backends, the JS suites (CI).
