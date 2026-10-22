# WG5b: tree-view (done; in CI)

Spec: `plans/research/QT-SURVEY.md` (P0, QW1; lane row 4 "WG5b tree-view"), `plans/research/EGUI.md` §5
(Tier 1: "we build ... `tree-view`"). Base: LISTS (LS1 + SG5), DX2 (`MimeData`, `DragPayload`).
Branch `ci/wg5b`.

## What shipped

| Step | Where |
| --- | --- |
| Shared keyboard and pointer selection helper (`KeySelect`) | `crates/svelters-egui/src/views/selection.rs`, 4 unit tests; the tree uses it, WG5a may adopt it (API unchanged) |
| `tree-view` kind, virtualised rows, lazy expand | `kinds/treeview.rs` (kind, `Tree` state, keys, row paint, drop target), `kinds/virtuallist.rs` (tree branches) |
| Keyboard: arrows, Page, Home/End, Right/Left, Space, Enter, type-ahead, Ctrl+C | `treeview::tree_keys` → `Tree::key` → `KeySelect` |
| Selection by id: none, single, multi (SV46) | `KeySelect` on the tree; `<TreeView>` keeps ids per key for the view's life (`ListController` `keepIds`) |
| DnD of rows through `MimeData`/`DragPayload` | `tree_row` (source), `tree_drop_target` (target, WG4b's pointer claims) |
| A11y: Tree/TreeItem, level, expanded, selected, set size | `virtuallist::paint`, `tree_row` |
| `<TreeView>` (JS), `flattenTree` | `packages/native-runtime/src/lists.js`, exported and typed in `@svelters/lists` |
| Gallery page | `fixtures/gallery-egui/src/pages/Tree.svelte` (also the embed export `Tree`); `src/TreeWindow.svelte` wraps it in a window for the kittest |

### The kind

`tree-view` is the same `W::VirtualList(Box<VList>)` as `virtual-list`, with `VList::tree: Some(Tree)`
(`VList::tree()`, created in `scene/tree.rs`). The M5 protocol, row slots, `RowHeights`, binding in
`realize_lists` (which now walks both tags) and `ops.rs`' `vlist_call` dispatch are shared.

- Props: `selection` (`none`/`single`/`multi`, default `single`), `row-height` (24), `indent` (16 points
  per level; the disclosure triangle sits in the last one), `row-drag-type`, `row-drop-types`.
  New `P`s: `TreeIndent`, `RowDragType`, `RowDropTypes`. `virtuallist::set` has a tree branch.
- Methods: virtual-list's plus `setNodes(ids, levels, flags, labels, data)` (flags 1 expandable,
  2 expanded; level < 0 forgets the id), `setSelection(ids)`, `selection()`.
- Signals: `setup`/`bind`/`unbind`/`teardown`, `activate(pos)`, `expand(id)`, `collapse(id)`,
  `selection-changed()` (JS reads `selection()`), `copy()`, `drop-row(id, where, type, data)`.
- Layout: a row is laid out at `width - inset` and placed at `x = inset` (`VList::row_inset`,
  `(level + 1) * indent`); its full-width rect (selection fill, cursor outline, click and drag
  sense) is the list's x range.
- Keys: only while the tree has focus (a row click requests it; the list node is focusable).
  **EV3 integration (done, 2026-10-22 re-merge onto master f1f5810b)**: the tree records its list
  node with `focus::record_with(.., Role::Tree, tab_stop = true)` (one tab stop per tree, recorded
  before its rows). `Role::Tree` is a small additive hook in `focus.rs` (one variant, one `claims`
  arm) delegating to `kinds::treeview::claims`: the `TreeKey`s (arrows including Left/Right with
  any of Shift/Ctrl, PageUp/PageDown, Home/End, Space, Enter, Ctrl+C) and the keys that type a
  printable ASCII character (type-ahead); Alt chords are not claimed. EV3 takes the claimed key
  presses out of the pass's input and reports them through `consumed_keys`/`shortcut_override`,
  so a host keymap leaves them. `tree_input` reads the key presses just before that, and still
  takes the type-ahead `Text` events and egui's `Copy` event (not key events, so EV3 doesn't
  route them). The `InputState` key filtering is gone. The focus-lock filter on the arrows stays:
  it isn't key routing but egui's guard against its own directional focus moves, which run in
  `begin_pass` before EV3 and which EV3 does not cover.
- Ctrl+C raises `copy`; `<TreeView>` writes the selected rows to `svelters:clipboard` (the host's
  `Platform` clipboard, the same one `Host::platform().clipboard` is): `rowDragType` (each row's
  `rowData`, a line each) and `text/plain` (labels). The backend has no handle on the host's
  `Platform`, so the write is JS's.
- DnD: a row drag carries `DragPayload::from_mime(tree.mime(rows))` (the selection when the dragged
  row is selected) with `source` the tree; any WG4b target or Rust panel takes it
  (`payload.best(drop_types)`). As a target, the tree claims the pointer before its rows paint (so
  drop targets inside rows win), shows a line before/after or an outline into the row (pointer in
  the top quarter, bottom quarter, middle), and on release raises `drop-row` (id -1, `into`: below
  the last row). `dnd.rs` gained `claimed`, `over_target` and a `pub(crate)` `claim`/`ghost`.

### `<TreeView>` (JS)

Props `roots`, `children(node)` (array or Promise; lazy: called when a node first expands, cached
per key until `roots` changes), `hasChildren`, `key`, `label`, `rowData`, `row` snippet
`(node, level)` (default: a label), `filter` (a node shows when it or a loaded descendant passes),
`selection`, `bind:selected` (keys), `bind:expanded` (keys), `row-height`, `indent`, `rowDragType`,
`rowDropTypes`, `onactivate`, `onselect`, `onexpand`, `oncollapse`, `ondroprow(key, where, type,
data, node)`, `oncopy(nodes)`. It flattens the visible rows (`flattenTree`) into a `ListController`
(keyed diff, `keepIds`), and sends `setNodes` only for rows whose facts changed (and level -1 for
rows that left). The compiler treats it as an ordinary component (no `lower/lists.rs` checks).

## Tests (run locally, all passing)

Re-merge onto master f1f5810b (EV3+, DS1): `cargo check -p svelters-egui --all-targets` (clean);
nextest `--test treeview`, `--test focus_keys`, `--test spec` (`SVELTERS_UPDATE=1`: generated files
unchanged from the auto-merge), `--test vlist` (all but `list_view_runs_on_egui`, which needs the
built `fixtures/lists-egui/dist`: CI), `--lib kinds::treeview views::selection vlist`;
`gen_props.py --check`, `gen_kinds.py --check` (under `tools/dev/headless`) clean.

Original lane:

- `cargo nextest run -p svelters-egui --test treeview` (2): lazy Promise children on Right,
  Right/Left (expand, first child, parent, collapse), Shift+Down, Ctrl+C to the clipboard,
  type-ahead, Enter, disclosure click, Ctrl+click, selection kept across a filter and back (SV46),
  TreeItem level/expanded/selected; a row dragged into another (`drop-row into`); 100k children
  with ≤ 60 row slots and < 200 binds.
- `--lib kinds::treeview` (2), `--lib views::selection` (4), `--test spec` (regenerated
  `metadata/semantics/egui.json`, `types/egui.d.ts`; `gen_props.py` reran for the style tables),
  `--test observe`, `widgets every_kind_has_a_tooltip_and_inputs_are_named`.
- `node --test test/lists.test.mjs` (native-runtime, `flattenTree`), `@svelters/lists` tests,
  native-runtime `svelte-check`.

## Not done / follow-ups

- ~~EV3 `consume_key` adoption~~: done in the re-merge (above).
- Set sizes are over the whole flattened list, not per sibling group.
- No AccessKit Expand/Collapse actions yet (keyboard and the triangle only).
- `layout="grid"`-style options and per-row drag data from Rust row sources (a tree over a Rust
  `RowSource`) are not supported: a tree is JS-fed.
