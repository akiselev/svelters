# OW5a: lists, tree and DnD as own widgets (first slice)

Lane `ci/ow5a` (thinkpad), from `origin/master` (d0542a3a, then 2cf3f4db) with `origin/ci/ow1a2` merged
(Expandable, `parts::paint_chevron`); OW1a2 then merged to master as ba5f7239, which is merged here. Design: `plans/research/ARCH-WIDGETS.md` §6.1 (`virtual-list`, DnD
rows) and §7.2 (OW5); look from DESIGN.md R6, R8, R10, R14, R15 and §4 tokens; acceptance rows from
WIDGET-PARITY §2.18 (OW5a). Coordinator decision (2026-10-22): pooled rows are `list::part(row)`, styled
per row through `Stylist::compute_pseudo_at(…, Some((i, n)), …)`.

**Decided (coordinator, 2026-10-22):** the split is accepted. OW5a1 is this lane. OW5a2 (grid layout,
rubber band, sticky section headers, multi-column tree, rename in place, expand animation) runs **after
OW5b**, because both use the column header. The per-row forced host bits for `::part(row)` (below) are
approved; VIS1c and DOC2 cite them.

**Split** (the OW5a parity rows alone are about 7 d). This record covers
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

  Results are cached per (node, part, state, position). Only drawn rows are computed (paint calls
  `part_at` per drawn row). The whole cache is dropped at any restyle and is capped at 4096 entries. It
  is one map in `SceneStyle`, not per node, so SG6's per-node budget does not change (`scene_memory`
  passes). With no `::part(row)` rule (no theme, or a theme without row rules), `part_at` returns `None`
  without computing, and rows paint from the UA table `ROW`.

  **What authors write.** Row states are the row's own, not the list's (a deliberate exception to P1's
  "pseudo-classes after `::part()` read the host"). `:focus-within` stays the list's:

  | State | Selector |
  |---|---|
  | rest | `virtual-list::part(row)` |
  | hover | `virtual-list::part(row):hover` |
  | pressed | `virtual-list::part(row):active` |
  | active selection | `virtual-list:focus-within::part(row):selected` |
  | inactive selection | `virtual-list:not(:focus-within)::part(row):selected` |
  | cursor row (`currentIndex`) | `virtual-list::part(row):current` |
  | keyboard focus ring (R8) | `virtual-list::part(row):focus-visible` |
  | drop into the row | `virtual-list::part(row):drop-target` |
  | stripes (P1, 1-based logical position) | `virtual-list::part(row):nth-child(even)` |
  | tree: expandable / expanded | `tree-view::part(row):has-children`, `tree-view::part(row):expanded` |
  | tree: the chevron | `tree-view::part(twistie)`, `tree-view::part(twistie):expanded` |
  | row contents (row nodes mirror the bits) | `virtual-list:focus-within > :selected { color: … }` |
  | bar | `virtual-list::part(thumb-y):hover`, `::part(scrollbar-y)` |
  | other parts | `::part(placeholder)`, `::part(drop-line)` |

  `tree-view` takes the same selectors as `virtual-list`.
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
- `layout.rs`: no change after the merge. RA3's `MeasureCx::id` in OW1a2's `with_measure` is master's own fix
  (ba5f7239, `Id::new(("svelters-egui.measure", node))`), taken as is.

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

## Handoff

- **OW5a1 follow-ups** (below): proposed for a fresh agent.
- **OW5a2** comes after OW5b (see the decision above).

## OW5a1b handoff (for a fresh agent; coordinator-approved 2026-10-22)

Start from `ci/ow5a` (or master once it merges) and read this record first. **Keep the `Scene::part_at` row design exactly as
built**; DOC2 and VIS1c cite it. Items, in a sensible order:

1. **DragSource / DropTarget units.** Move `kinds/dnd.rs`'s per-node source and target halves into `interact/dnd.rs` `Unit`s
   (intents: `DragBegin`, `DragEnd { dropped, action }`, `Enter(type)`, `Leave`, `Drop { type, data, at, action }`).
   - Keep the signal shapes, including `drop`'s 5th arg and `drag-end`'s 2nd.
   - The `ghost` and the file-drop highlight should paint from UA tables (`::part(ghost)`, `::part(drop-line)`) instead of
     egui `Visuals`.
2. **List row DnD.** `row-drag-type`/`row-drop-types` on `virtual-list`.
   - Generalise `tree_drop_target`/the tree's drag start in `virtuallist.rs`; the props already exist in `P`.
   - Add `internal-move` (reorder within the list, a `move(from, to)` signal).
   - Add DnD auto-expand on trees: hovering an `into` target expands it after `Policy::double_click_ms`-like delay
     (Qt `autoExpandDelay`).
3. **Selection.** `selection="toggle"` (a click toggles, no modifier) and `"range"` (Shift only) in
   `views/selection.rs` `Mode`; WG5a and the table share it, so keep their tests green.
   - Also gate the tree's click selection (`selection-changed`) through `list_rows::allowed`.
4. **`bind:current`** on list and tree. A new prop holds the cursor's id and is announced through `leaf::widget_changed`;
   `:current` is already on the rows.
5. **Smaller items:**
   - the `scrollTo(pos, hint)` hint (`nearest`/`top`/`bottom`/`center`; `VScroll::reveal` takes the alignment);
   - Ctrl+C `copy` on `virtual-list` (`item_keys::Input::Copy`);
   - `uniform-rows` (skip measuring: `RowHeights::uniform`);
   - `slot="empty"` (a child shown when `count() == 0`; needs RA3's container slots);
   - `row-hovered`;
   - the "selection follows the context click" Policy row;
   - expand helpers in `svelters:views/tree` (`expandAll`, `collapseAll`, `expandToDepth`, JS side, `@svelters/lists`).
6. **Acceptance.**
   - Keep green: this record's tests, `vlist.rs`, `wg5a.rs`, `treeview.rs`, `wg4b.rs`, `scene_memory`, the sweeps.
   - Add one test per item above.
   - Then flip the WIDGET-PARITY §2.18 rows marked "not done" here.

OW5a2 (grid layout, rubber band, sticky section headers, multi-column tree, rename in place, expand animation) is a
separate later lane, after OW5b.

## OW5a1b (lane `ci/ow5a1b`, thinkpad, from `ci/ow5a` 560dac4b)

Done in the handoff's order. The `Scene::part_at` row design is unchanged.

### What landed

1. **DragSource / DropTarget units** (`interact/dnd.rs`, new; ARCH-WIDGETS §1.2):
   - `DragSource` owns `:dragging`. Its intents are `Drag::Begin` and `Drag::End { dropped, action }`, the second reported
     the pass after the release. `DragSource::payload` links the payload to the unit. The unit also picks the cursor
     (grab, grabbing, copy or alias by the action, no-drop).
   - `DropTarget` owns `:drop-target`. Its intents are `Enter(type)`, `Leave`, and `Drop { ty, data, at, action }`,
     which comes after `Leave`. It is fed an `Offer` while it holds the claim.
   - `run_bare` runs a unit on a zone that has no `egui::Response` of its own.
   - `kinds/dnd.rs::node` is now only the context side: the claims, the payload, OS files and the signals. The unit
     state lives in egui temp data by node id, because `Common` is interned (SG6). The WG4b/WG6 signal shapes are
     unchanged: `drop`'s 5th arg `action`, `drag-end`'s 2nd, `text/uri-list` file drops. Auto-scroll stays on
     Scrollable.
   - Any node with `drop-types` now gets `:drop-target` while a drag it accepts is over it, and its source gets
     `:dragging`.
2. **Drag visuals from CSS**:
   - The ghost is the source's `::part(ghost)`: UA `GHOST` is `menu-bg`, `menu-border-color`, `menu-radius`, `elev-3`,
     `fg`, opacity 0.9 and padding, painted on a tooltip layer and traced.
   - A node target's highlight is its `::part(drop-line)` outline (UA `DROP_LINE`, 2 pt `focus`).
   - Both read their CSS through `Scene::part_at`, so `virtual-list::part(ghost) { … }` works.
   - No egui `Visuals` are left in DnD.
3. **Row DnD on plain lists** (`virtuallist.rs` `row_drag`, `row_drop`; trees use the same code):
   - `row-drag-type` and `row-drop-types` moved from `Tree` to the list's `RowDnd`. The list's payload is the dragged
     ids (the selection when the dragged row is selected), a line each, plus `text/plain` labels.
   - `drop-row(id, where, type, data)` on `virtual-list`.
   - `internal-move` (list) drags one row with `application/x-svelters-row-move` and accepts only its own drag. A drop
     between rows raises `move(from, to)`, where `to` is the gap. A drop into the row's own gap raises nothing.
   - **DnD auto-expand** on trees: a drag held `into` a collapsed row for `Policy::drag_expand_ms` raises `expand`
     once (EV1-gated).
   - The list is `:drop-target` while a drag it takes is over it.
4. **Selection**:
   - `views/selection.rs` `Mode::Toggle` (Qt MultiSelection): a click toggles with no modifier, moves only move, and
     Space toggles.
   - `Mode::Range` (Qt ContiguousSelection): one run; Shift extends it and Ctrl acts as a plain input.
   - `Mode::is_multi` sets AccessKit multiselectable and enables Ctrl+A.
   - `selection="toggle"|"range"` works on list and tree. The table keeps its own enum (OW5b), and its tests are
     untouched.
   - The tree's click selection now goes through EV1 (`list_rows::allowed(…, "selection-changed")`): when prevented,
     the cursor still moves.
5. **`bind:current`** (`current`, `P::CurrentItem`) on list and tree:
   - It holds the cursor's id. User moves are announced through `leaf::widget_changed`.
   - The host's value moves the cursor without selecting, and is not echoed back.
6. **Smaller items**:
   - `scrollTo(pos, hint)` takes `nearest`, `top`, `bottom` or `center` (`VScroll::reveal_at`).
   - Ctrl+C on a focused list raises `copy(ids)` (`item_keys::take_copy`). The table is unchanged.
   - `uniform-rows` skips measuring, with rows at `row-height`.
   - `row-hovered(pos, id)` on list and tree.
   - The context click selects first (`Policy::context_click_selects`: true everywhere except AppKit, which outlines
     the row and keeps the selection).
   - Tree expand helpers in JS: `expandAll`, `collapseAll`, `expandToDepth`, `expandRecursively`. They are in
     `@svelters/native-runtime/lists`, re-exported by `@svelters/lists`, and mapped as `svelters:views/tree`. Each
     returns the keys for `bind:expanded`.
   - **Deferred: `slot="empty"`.** It needs named container slots, which RA3 deferred.

Shared changes:
- `native/mod.rs`: `Policy::context_click_selects` and `drag_expand_ms` (500 ms on every row; the exact platform
  values are **verify**).
- `kinds/mod.rs`: `P::CurrentItem`, `InternalMove`, `UniformRows`.
- `scene/props.rs`: the matching arms.
- `kinds/docs.rs`: the list and tree css notes.
- Regenerated, in this order: `egui.json` and `egui.d.ts`, then `gen_props.py`, then the reference.

### R-rules, parity rows, pins

- **R6/R8/R10/R14/R15**: as OW5a. Row states are unchanged. The new `:drop-target` on the list and node targets uses
  the `list-drop`/`focus` tokens.
- **R12** (lift while dragging): the ghost is `elev-3` at 0.9 opacity, from CSS.
- **WIDGET-PARITY §2.18**:
  - done: `toggle`, `range`, list/tree `bind:current`, list `row-drag-type`/`row-drop-types` and `internal-move`, the
    `scrollTo` hint, `row-hovered`, the selection following the context click, `copy` on the list, `uniform-rows`, DnD
    auto-expand, the tree helpers, the ghost, the drop indicator;
  - deferred: `slot="empty"`.
- **Pins**: none apply, as in OW5a. VIS1 is not on master, so it was not run.

### Tests (all run locally, pass)

- New in `tests/vlist.rs`:
  - `ow5a1b_toggle_and_range_selection`
  - `ow5a1b_bind_current_is_the_cursor`
  - `ow5a1b_scroll_to_takes_a_hint`
  - `ow5a1b_copy_uniform_rows_hover_and_context_selection`
  - `ow5a1b_list_rows_drag_and_drop` (the ghost's fill comes from `::part(ghost)` CSS; the list is `:drop-target`)
  - `ow5a1b_internal_move_reorders`
  - `ow5a1b_tree_auto_expands_on_drag_hover_and_gates_selection`
- New lib units:
  - `interact::dnd` (`ow5a1b_drop_target_enters_leaves_and_drops_once`,
    `ow5a1b_drag_source_reports_the_targets_action_the_pass_after`)
  - `views::selection::ow5a1b_toggle_and_range_modes`
- New JS test: `packages/native-runtime/test/lists.test.mjs` (the expand helpers).
- The acceptance set, green:
  - `vlist` (all, including `list_view_runs_on_egui`), `wg5a` (including the 100k test), `treeview`
  - `wg4b` (including the DnD gallery page), `scene_memory` (SG6), `spec`
  - the `observe` and `widgets` every-kind sweeps
  - the lib units `kinds::dnd`, `kinds::treeview`, `views::selection`, `item_keys` and `scene::virt`
- The fixtures were built with `ci/ow5a`'s compiler binary. The compiler is unchanged on this lane.
- Clippy: no new warnings.

**Not run locally:** the full workspace nextest, the other CI workflows, full conformance, and VIS1.

## Not done (OW5a1 follow-ups)

OW5a1b did all of these (see its section above) except `slot="empty"`, which waits for named container slots
(RA3 deferred them).
