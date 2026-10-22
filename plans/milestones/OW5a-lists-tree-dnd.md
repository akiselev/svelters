# OW5a: lists, tree and DnD as own widgets (first slice)

Lane `ci/ow5a` (thinkpad), from `origin/master` (d0542a3a, then 2cf3f4db) with `origin/ci/ow1a2` merged
(Expandable, `parts::paint_chevron`). Design: `plans/research/ARCH-WIDGETS.md` §6.1 (`virtual-list`, DnD
rows) and §7.2 (OW5); look from DESIGN.md R6, R8, R10, R14, R15 and §4 tokens; acceptance rows from
WIDGET-PARITY §2.18 (OW5a). Coordinator decision (2026-10-22): pooled rows are `list::part(row)`, styled
per row through `Stylist::compute_pseudo_at(…, Some((i, n)), …)`.

**Split proposed to the coordinator** (the OW5a parity rows alone are about 7 d). This record covers
**OW5a1**. Two lists are left for later:
- **OW5a2** gets the large rows: grid layout, rubber band, sticky sections, the multi-column tree (after
  OW5b's header), rename in place, the expand animation.
- **OW5a1 follow-ups**: the rows listed under "Not done".

## What landed

### Rows: `crates/svelters-egui/src/widgets/list_rows.rs` (new)

Lists stay `W::VirtualList` payloads: the virtualiser, the row pool and M5 stay in `kinds/virtuallist.rs`.
What a list paints and how it scrolls are now svelters' own, from CSS and UA tables. No egui `Visuals` and
no `collapsing_header::paint_default_icon` are left in the list or tree paint.

- **`::part(row)` per pooled row.** `Scene::part_at(list, "row", state, (pos, len))` (`style.rs`, new)
  matches the list's `::part(row)` rules with the host's state bits replaced by the row's own state, at
  the row's logical position (P1). It uses the existing `Forced` element. So these all work on recycled
  rows:
  - `virtual-list::part(row):hover`
  - `virtual-list:focus-within::part(row):selected` (active) and `:not(:focus-within)` (inactive)
  - `::part(row):focus-visible`
  - `::part(row):nth-child(even)`
  - `tree-view::part(row):has-children:expanded`

  Results are cached per (node, part, state, position) and dropped at any restyle; the cache is capped
  at 4096 entries. With no `::part(row)` rule, rows paint from the UA table `ROW`.
- **`RowState::bits`**:
  - `:hover`, `:active`
  - `:selected`, `:current` (the cursor)
  - `:focus-within` (the list has focus: active vs inactive selection)
  - `:focus`/`:focus-visible` (the cursor row; visible for keyboard focus only)
  - `:drop-target`
  - `:has-children` (bit 23, P5), `:expanded`
  - `:disabled`
- **Row nodes mirror** `:selected`, `:current`, `:drop-target`, `:has-children` and `:expanded`
  (`list_rows::mirror`). Their contents restyle from base.css:
  `virtual-list:focus-within > :selected { color: var(--sv-list-sel-fg) }`, and the same for `tree-view`.
- **Paint parts:**
  - `row` (box and outline)
  - `twistie` (R14 chevron, `parts::paint_chevron`, `twistie-w`, `icon` colour)
  - `placeholder` (an unbound position, `list-hover`)
  - `drop-line` (2 pt `focus` line before or after a row; around the list for the end)
  - `scrollbar-y`, `thumb-y`

  All of them are traced (`PaintTrace`, tag `virtual-list`/`tree-view`).
- **The bar is OW2's Scrollable** (`ListBar`). The list keeps its f64 `VScroll` offset, decided before
  rows bind (R22). Each frame the unit gets that offset; whatever the unit moves goes back as a pending
  delta: thumb drag, track press, the wheel remainder the list did not take, kinetic scroll, AccessKit
  scroll actions. It shows overlay bars with the policy fade, `:state(sv.overflow-*)` on the list (R15
  shadow in base.css, painted over the rows by `paint_shadows`), and an AccessKit `ScrollBar`. The list's
  own 8 pt bar and its wheel code are gone.
- **R8 keyboard modality**: `VList::kbd` is set by Tab-in or keys and cleared by a click. Only then does
  the cursor row show its ring.
- **Rows whose contents are clipped away** (a row partly scrolled out) are still named: by their
  type-ahead label, else "Row N" (`label_row_by_contents` now returns whether it named the row). Without
  this, kittest's accessibility check failed after a thumb drag.

### Tree rows (`kinds/treeview.rs`)

`tree_row` paints through `list_rows`. An `Expandable` unit runs on the twistie's response
(`on_click: true`) and on the row's: AccessKit Expand/Collapse always; a click with `expand-on="click"`;
a double click with `"double-click"`. Left/Right stay `tree_keys`' (the tree convention of going to the
parent or first child).

New props:
- `expand-on` (`twistie` default, `click`, `double-click`)
- `root-decorated` (default true; level-0 rows get no twistie when false)

New signals:
- `row-clicked(pos, id)`
- `row-context(pos, id)`

`virtual-list` has the same two signals. TreeItem a11y adds the Expand/Collapse actions.

### Selection, keyboard, EV1

Rows keep the shared `views/selection.rs` `KeySelect` and the EV3 claims (`item_keys::read_keys` and
`lock_arrows` for lists; `tree_input`/`focus::record_with` with `Role::Tree` for trees).
`list_rows::allowed(s, i, name)` runs EV1's `dispatch_signal` (Rust filters and behaviours) before each
default action:
- the list's `selectionchange` (click and keys): when prevented, the cursor moves but the selection stays;
- `activate` (double click, Enter);
- the tree's `expand`/`collapse` (twistie, row, Left/Right);
- the tree's `activate`.

### DnD (WG6 QA9 taken from `origin/ci/wg6`)

`kinds/dnd.rs`:
- Drop actions: `drop` gets a 5th arg `action`, and `drag-end` a 2nd (`move`; `copy` with Ctrl or Option
  on macOS; `link` with Ctrl+Shift or Cmd+Option; `none` when not dropped). The cursors show the action
  (Copy/Alias). Rust targets use `DragPayload::accept_as`/`action`.
- OS file drops arrive as `text/uri-list` (`file://` URIs joined by `\r\n`) on any target that accepts that
  type.
- Auto-scroll is now on the Scrollable unit (`dnd::auto_scroll(ctx, view) -> Option<Vec2>`, 24 pt edges,
  at most 12 pt a frame). It is used by the own `scroll-area` and by the list/tree bar; egui's
  `scroll_with_delta` is no longer called.

Both signal signatures are kept as WG6 specified (the kind table, typings and reference are regenerated);
the `wg4b.rs` assertions are updated.

### Shared changes

- `style.rs`: `Scene::part_at`, `SceneStyle::part_at` cache.
- `kinds/mod.rs`: `P::ExpandOn`, `P::RootDecorated`.
- `scene/props.rs`: their arms.
- `theme/base.css`: the OW5a rules above.
- **Merge fix for the coordinator.** RA3 (master) added `MeasureCx::id`, and OW1a2's
  `LayoutCx::with_measure` builds a `MeasureCx` without it. The fix in `layout.rs` is
  `id: egui::Id::unique(("svelters-egui.measure", self.node.to_raw()))`. Master needs the same line when
  OW1a2 merges.

## DESIGN rules satisfied

- **R6**: rows are flat with four distinct states: `list-hover`, `list-sel` + `list-sel-fg`,
  `list-sel-inactive`, and the 1px focus outline. `list-drop` covers drops into a row.
- **R8**: the ring is 1px `focus` at offset −1 on the cursor row, keyboard only (a click shows none).
- **R10**: `cursor: pointer` on rows (computed `cursor`, set while hovered).
- **R14**: twistie chevrons on expandable tree rows (right; down when expanded).
- **R15**: a list scrolled away from its top shades that edge. Overlay bars show on hover or scroll and
  fade.
- Tokens: `list-hover`, `list-sel`, `list-sel-fg`, `list-sel-inactive`, `list-drop`, `focus`,
  `twistie-w`, `icon`, `scrollbar*`, `scroll-shadow`.

## WIDGET-PARITY rows (§2.18)

| Row | Status |
|---|---|
| AccessKit list/tree roles, set size, level, `active_descendant` | done (List/ListBox/ListItem/ListBoxOption; Tree/TreeItem with level, expanded, selected, Expand/Collapse actions) |
| `alternatingRowColors` for list and tree (P1 `::part(row):nth-child(even)`) | done (`part_at` with the logical position) |
| `activated`, `clicked`, `pressed`, `entered` | `row-clicked`, `row-context` done; `row-hovered` not done |
| context menu on rows | `row-context` done (the app opens a `context-menu`); "selection follows per Policy" not done |
| `rootIsDecorated` | done (`root-decorated`) |
| `itemsExpandable`, `expandsOnDoubleClick` | done (`expand-on`) |
| `autoScroll` during drags | done (Scrollable auto-scroll on lists, trees and scroll areas) |
| `drop-actions` (copy, move, link by modifier, QA9) | done (WG6's, on any node) |
| `selection="toggle"`, `"range"` | not done |
| list/tree `bind:current` | not done (`:current` is on the rows) |
| `scrollTo` hint | not done |
| `slot="empty"` | not done |
| `copy` on the list | not done |
| `uniform-rows` | not done |
| list `row-drag-type`/`row-drop-types`, `internal-move` | not done (the tree has them) |
| DnD auto-expand (`autoExpandDelay`) | not done |
| expand helpers in `svelters:views/tree` | not done |
| grid layout, rubber band, sticky sections, multi-column tree, rename in place, expand animation | OW5a2 (proposed) |

## VIS1 checks and pins

- The VIS1 gate (`crates/svelters-testkit/tests/vis1.rs`) is not on master yet (VIS1c), so the list
  checks were **not run**.
- **Gap 5** (list rows): tree rows now draw the twistie at their level, and list rows paint R6's states
  from `::part(row)`. The 38 px text inset of VS Code's explorer needs an icon in the row snippet; that is
  content, not the widget.
- **Gap 6** (tabs) is OW5c's.
- No native-behaviour pin covers lists (pins 1–7 are text and focus scripts; pins 8–10 are menus and
  scroll). None applies.

## Tests

New in `tests/vlist.rs` (OW5a section; `ListRig::with_kind` makes a `tree-view` too):
- `ow5a_rows_show_r6_states_and_a_keyboard_only_ring`: rest, hover, active and inactive selection fills
  all differ; no ring after a click, a ring after Down; the row node mirrors `:selected`/`:current`;
  `row-clicked`, `row-context`.
- `ow5a_part_row_rules_match_each_rows_position_and_state`: `:nth-child(even)` stripes by position, and
  `:selected` applies to the selected row only.
- `ow5a_the_bar_is_a_scrollable_thumb`: a thumb drag scrolls 2k rows with no unbound row; an AccessKit
  `ScrollBar` exists.
- `ow5a_filters_prevent_the_lists_default_actions`: EV1 Prevent on `selectionchange` and `activate`.
- `ow5a_tree_rows_expand_through_their_twistie_and_expand_on`: TreeItem level and expanded, two
  twisties, twistie → `expand`, `expand-on="click"` → `collapse`, `root-decorated={false}` hides
  level-0 twisties.

Acceptance run locally (all pass):
- `vlist.rs` (all 6 old + 5 new);
- `wg5a.rs` (5, including the 100k-row keyboard test);
- `treeview.rs` (2);
- `wg4b.rs` (7, including both DnD tests and the gallery page);
- lib units `scene::virt` (3), `kinds::dnd` (incl. `wg6_drop_actions_and_uri_list`), `kinds::treeview`,
  `views::selection`, `item_keys`;
- `scene_memory` (SG6 budget and culling);
- the sweeps `spec` (4), `observe`'s and `widgets`' every-kind tests.

Clippy on the lane's files is clean.

**Not run locally:** the full workspace nextest, the other CI workflows, full conformance, the VIS1
gate (not on master). CI is the gate.

## Not done (OW5a1 follow-ups)

- **Units.** DragSource/DropTarget are not separate `Unit`s yet: `kinds/dnd.rs` is still WG4b's per-node
  helper, with WG6's actions, files and auto-scroll.
- **Drag visuals.** The drag ghost and the file-drop highlight still read egui `Visuals` (`dnd::ghost`,
  `file_drop`).
- **Tree selection gate.** The tree's click selection (`selection-changed`) has no EV1 gate yet.
- **Parity rows.** Every row marked "not done" in the table above.
