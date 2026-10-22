# WG5a: item-view interaction (keyboard, keyed selection, cell editing) on egui

Scope: `plans/research/QT-SURVEY.md` §3 row WG5a (P0 gaps QW2 keyboard and type-ahead, QW5/SV46 keyed
selection, QW3 cell editors; QW17 sticky sections if cheap). Lane WG5a, branch `ci/wg5a`, from
`origin/master` a52d437c (INT1, LISTS, SG4, FX1, DX2 merged).

## What landed

| Piece | Where |
|---|---|
| **Shared selection helper** (WG5b's, copied unchanged from `ci/wg5b` 3d3bfdcc so both lanes carry the identical file): `KeySelect` keyed by id, none/single/multi, Up/Down/PageUp/PageDown/Home/End, Shift ranges, Ctrl moves and toggles, Space, clicks, type-ahead | `crates/svelters-egui/src/views/selection.rs` (+ the same `pub mod selection;` line in `views/mod.rs`) |
| **egui input for it** (additive, WG5a): `read_keys` (consumes the focused view's keys from egui's input), `lock_arrows` (egui focus navigation leaves the arrows alone), `apply` (inputs → `KeySelect`, plus Ctrl+A and Enter), `RowKeys` (ids + type-ahead labels, splice), `ids_of`/`ids_value`/`labels_of` | `views/item_keys.rs` |
| `Ty::IdList` (`number[]`, ABI list of i64) and `Ty::Snippet` (a function the runtime keeps; the backend sees `true`) | `kinds/mod.rs`, `spec.rs`, `scene/props.rs` |
| `virtual-list`: focus, keys, type-ahead, keyed multi-selection | `kinds/virtuallist.rs` |
| `table`: row ids, keys, type-ahead, keyed selection, cursor cell, cell editing, a11y | `views/table.rs`, `kinds/views.rs`, `views/mod.rs` (dispatch, editor layout) |
| `table-column` editor snippets in the JS runtime (`set()` keeps a function-valued prop as a snippet; `cellEditor`) | `packages/native-runtime/src/index.js` |
| `leaf::props_changed` (the stamp/journal/observe half of `widget_changed`, without `changed`) | `leaf.rs` |
| `RowSource::label(row)` (type-ahead text of Rust rows; defaulted) | `scene/virt.rs` |
| Logic step `EguiBackend::realize_edits` after `realize_lists` | `frame.rs` |
| Gallery page `ItemViews` (also an embed export): a 1000-part table, filter, multi-selection, a Qty `editor` snippet with validation, a Note column with the built-in editor | `fixtures/gallery-egui/src/pages/ItemViews.svelte` |
| Spec and typings regenerated (`SVELTERS_UPDATE=1 … --test spec`) | `metadata/semantics/egui.json`, `packages/native-runtime/types/egui.d.ts` |

## API

### `virtual-list` (additive; M5 unchanged)

| | |
|---|---|
| `selection` | `none` \| `single` \| `multi` (new). |
| `selected-ids` (bind) | the selected item ids (`number[]`). Kept by id across splices, sorts and filters: a filtered-out id stays selected and is selected again when it comes back. |
| `selected` | M5's position: now the cursor's position when the cursor row is selected (`select(pos)` as before). A host write selects that item and puts the cursor on it. |
| `splice(pos, removed, ids, labels?)` | optional fourth argument: one type-ahead label per id. Rust rows: `RowSource::label`. |
| `onselectionchange(ids)` | async; the user changed the selection. |
| keys | a row click focuses the list. Focused: arrows, PageUp/PageDown, Home/End (cursor; scrolled into view), Shift (range), Ctrl (move alone, Ctrl+Space toggles), Space, Ctrl+A, Enter (`activate(pos)`), typing (type-ahead, 1 s prefix). |
| a11y | `ListBox` with `multiselectable` in multi mode and `active_descendant` = the cursor row while focused; rows `ListBoxOption` with `selected`; the cursor row has a focus outline. |

### `table`

| | |
|---|---|
| `set-ids(ids, labels?)` | the id of every row by position (and type-ahead labels); the row count is then `ids.length`; null: ids are positions again. Re-sends `range`. |
| `selected` (bind) | **changed from WG4a's index to row ids** (`number[]`). Without `set-ids` a row's id is its position, so WG4a's usage reads `[3]` instead of `3` (the `TableView` gallery page initialises `[]`; `"Selected row: {selected}"` prints the same). `changed(selected)` carries the ids. |
| `selection` | `none` \| `single` \| `multi`. |
| `current` (bind), `current-column` (bind) | the cursor row's id (-1 none) and column. |
| `onselectionchange(ids)` | async. |
| `activated(row)` | double click on a non-editable cell, or Enter (position, as before). |
| `oncommit(e, rowId, column, value)` | **sync**. Accept: return nothing. Reject: `e.result = 'reason'` (shown under the cell for 5 s) or `e.preventDefault()` (a generic reason). A rejected edit reverts. An accepted one updates the cell in the data window at once (the host's next `set-rows` wins). |
| `edit(rowId, column)`, `end-edit(commit)` | start an edit / end the open one. |
| keys | a click focuses the table. Focused: Up/Down/PageUp/PageDown/Home/End, Shift/Ctrl, Space, Ctrl+A, Enter (activate), Left/Right (cursor column), F2 (edit), typing (edit the cursor cell if its column is `editable`, else type-ahead on the labels, else on the first column's window text). Editing: Enter commits, Escape reverts, Tab/Shift+Tab commit and edit the next/previous editable cell (row-major), focus leaving the editor commits, a click on another row commits. |
| a11y | the table node (`Table`, `multiselectable`, row and column counts); a focus node (`Grid`, named "<name> keyboard") whose `active_descendant` is the cursor cell; rows `Row` with `row_index` and `selected`; cells `Cell` with `column_index`, `selected`, `read_only` unless editable, the cell's label as their child; the built-in editor is a text input named by its column title. |

### `table-column`

| | |
|---|---|
| `editable` | its cells can be edited. |
| `editor={snippet}` | the cell editor: a snippet taking the edit `{ row, column, value (bindable), typed, commit(), cancel() }`, e.g. `{#snippet quantity(edit)}<text-edit bind:text={edit.value} />{/snippet}`. Without one, a built-in text field edits the cell text. |
| runtime protocol | sync signals `editor-bind(row, value, typed)`, `editor-value()` (returns the value through `e.result`), `editor-unbind()`; methods `set-editor(outlet)`, `end-edit(commit)`. The runtime (`cellEditor` in `index.js`) mounts the snippet **once** per column into a row outlet (`rowOutlet()`, as `ListController` does) and rebinds it per edit through `$state` (row, value, typed); the table adopts the outlet as a child, lays it out at the edited cell's size and draws it over the cell; focus goes to its first focusable widget. This is ARCH-SCENE's realised-row pool shape with one realised editor per column (cadatomic confirmed nothing more is needed). |

## How it works

- **Key handling is local and additive** (EV3+ is in flight): each view reads keys only while its own
  focus id (a `focusable_noninteractive` interact under the rows) has egui focus, consumes them from
  egui's input and sets a focus-lock filter for the arrows. Tab and Escape keep egui's meaning except
  inside an edit. Nothing in `focus.rs` or the key routing changed.
- **EV3+ (merged from master before the push).** EV3 records `virtual-list` rows as the list's focus
  stops (`focus::record_with`, `Role::List`: the rows claim the navigation keys, Space and Enter).
  WG5a keeps that model: focus is on the **cursor row**; the stop is the cursor row when drawn, else
  the first drawn row (EV3 used the selected position). The list's own focusable interact only holds
  focus while the cursor row is not drawn (a jump such as End scrolls to it next frame) and hands
  it to the row once drawn; Tab onto the list lands on the stop row. The table is not in EV3's
  record (its focus id is unknown to EV3, so EV3 treats it as a host widget: no default/cancel
  button on Enter/Escape). **Known gap:** a snippet editor's single-line `text-edit` inside a
  dialog scope with a `default` or `cancel` button: EV3 may route Enter/Escape to that button
  before the table sees them (a follow-up: the table should record its editor as claiming Enter,
  Escape and Tab).
- **No JS in the pass.** The table records edit starts, end requests and the cell's size in the pass;
  `realize_edits` (logic, after `realize_lists`) runs `editor-bind`, then for an end `editor-value`,
  the table's `commit` and `editor-unbind` through `call_sync`, then closes the edit, records a
  rejection's reason, and queues Tab's next cell (`pending_edit`, started at the next paint).
- **Rows have ids, not positions.** The table's `set-ids` and the list's splice ids feed `KeySelect`;
  page size is the rows the view showed last pass. Type-ahead builds an id→position index once per
  typed text (100k rows: one scan per keystroke).
- **egui_table detail:** `row_ui` is called once per column region (sticky and scrolled), so the row
  interaction id is per region (`ui.scope_id()`), and an empty region is skipped.
- **Runtime detail:** a `$state` array is a Proxy, which the host's list conversion does not read as
  an array; `set()` now writes `Array.from(value)` for arrays (any list prop).

## Gates

| Gate (QT-SURVEY WG5a) | Test | Result |
|---|---|---|
| a 100k-row table is fully usable from the keyboard | `tests/wg5a.rs` `a_100k_row_table_is_usable_from_the_keyboard_and_keeps_selection_by_id` (click, Down, End to row 100k shown, Home, PageDown, Shift+Down, type-ahead `part5000`, Right) | passed locally (debug) |
| F2 edits a cell; commit/revert; Tab to the next cell; reject with the reason shown | `wg5a.rs` `table_cells_edit_with_f2_commit_reject_revert_and_tab` (built-in editor), `a_column_editor_is_realised_once_and_rebound_per_edit` (the column-editor protocol from a Rust host: one realisation, two binds); `tests/gallery_views.rs` `item_views_page_edits_with_a_snippet_editor_and_validates_commits` (the real Svelte snippet through the runtime, `oncommit` rejecting and accepting, Tab into the built-in editor) | passed locally (debug) |
| selection survives a filter (and a re-sort) | `wg5a.rs` table test (`set-ids` reversed, then filtered), `virtual_list_selection_survives_a_resort_and_a_filter` | passed locally (debug) |
| `virtual-list` keyboard, type-ahead, multi-selection, a11y | `wg5a.rs` `virtual_list_keyboard_type_ahead_and_keyed_multi_selection` | passed locally (debug) |
| unit | `views::item_keys` (2), `views::table` (2), `views::selection` (WG5b's 4) | passed locally |

The "`QuantityField`-shaped test kind" of the survey's gate is covered by the snippet editor (a
`text-edit` in the column's `editor`); a provider kind as the editor works the same way (the snippet
renders it) but no provider kind is in this crate's tests.

## Not done (follow-ups)

- **QW17 sticky group headers: not done (not cheap).** JS rows are realised per position, and the
  header of the group above the viewport is usually not bound; a sticky header needs group
  boundaries in the protocol (a `groups` splice argument or `RowSource::group`) and an extra
  realised row pinned at the top. Proposed for LS2 (Rust row model), where the model can answer
  group starts.
- `<ListView>` (`lists.js`) does not expose the keyed selection yet: its ids are the controller's
  native ids, reissued when a key leaves and comes back, so a `bind:selectedKeys` must keep keys in JS
  and push ids (`selected-ids`) after each update (WG5b's TreeView design does exactly this; share it).
  It also does not pass labels to `splice` (type-ahead) or wire `onselectionchange` (its rest props
  drop `on*` functions).
- `selection="multi"` on `virtual-list` via `ListView`'s `selection` prop works natively, but `bind:selected` there
  is still M5's position.
- Editors from Rust row sources (a provider-kind editor bound without JS) and persistent editors
  (an editor per row, always shown) are not here; the PropertyGrid uses the snippet editor.
- Column reordering, bindable column widths (WG4a's list) unchanged.

## For WG5b (tree-view)

- `views/selection.rs` is identical on both branches; keep it so, and put additions next to it
  (`views/item_keys.rs` is WG5a's: `read_keys(ui, text)`, `lock_arrows`, `apply`, `RowKeys`).
- `virtual-list` now has `sel: KeySelect` in `VList`, `SELECTION = none|single|multi`,
  `selected-ids` (`P::SelectedIds`, `Ty::IdList`: the int list the tree's design missed),
  `selectionchange(ids)`, focus and key handling in `virtuallist::paint` (`keys_input`, `click_row`,
  `selection_changed`) and the a11y (`multiselectable`, `active_descendant`). A tree built on `VList`
  gets all of it; it only needs Left/Right (collapse/expand/parent/child) before `item_keys::apply`
  and `TreeItem` roles. `RowSource::label` gives Rust rows type-ahead text.

## Verification (local, this lane)

After merging `origin/master` (1786957f: EV3+, DS1, Q2, H1b/H3c): `tools/dev/cargo-slot cargo check
-p svelters-egui --all-targets` (clean); `SVELTERS_UPDATE=1 … --test spec` (generated files unchanged
by it); `cargo build -p svelters-compiler --bin svelters-compile` and `pnpm --filter
@svelters-fixtures/gallery-egui build`; `cargo nextest run -p svelters-egui` filtered to `binary(wg5a)
| binary(gallery_views) | binary(vlist) | binary(views) | binary(observe) | binary(widgets) |
binary(focus_keys) | test(/item_keys|views::selection|views::table/)`, without
`list_view_runs_on_egui`: 54 passed (the list suites rerun after the EV3 focus fix: 19 passed).
Before the merge: `python3 -m pytest -q tools/metadata` (155 passed), `pnpm --filter
@svelters/native-runtime test` (124 passed) and its `check` (svelte-check, 0 errors); the runtime
files did not change in the merge from my side.

Not run locally: the rest of svelters-egui's tests, `list_view_runs_on_egui`, the workspace,
conformance, the other backends, the other packages' tests. CI runs them.
