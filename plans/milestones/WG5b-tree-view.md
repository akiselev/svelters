# WG5b: tree-view (in progress, handoff)

Spec: `plans/research/QT-SURVEY.md` (P0, QW1; lane row 4 "WG5b tree-view"), `plans/research/EGUI.md` §5
(Tier 1: "we build ... `tree-view`"). Base: LISTS (LS1 + SG5, master `a52d437c`), DX2 (`MimeData`,
`DragPayload`). Branch `ci/wg5b`.

## Status

The first agent (the DX2 lane) stopped at a clean point near its context limit: only the shared
selection helper is built. The rest is a design with a code map, below.

| Step | State |
| --- | --- |
| Shared keyboard and pointer selection helper | **done**: `crates/svelters-egui/src/views/selection.rs`, 4 unit tests |
| `tree-view` kind, virtualised rows, lazy expand | next |
| Keyboard (arrows, expand and collapse, type-ahead) wired to the helper | todo |
| Selection by id, single and multi (SV46) | the helper does it; wiring is todo |
| DnD of rows through `MimeData`/`DragPayload` | todo |
| A11y: Tree/TreeItem, expanded, level | todo |
| Gallery page `fixtures/gallery-egui/src/pages` | todo |

## Done: `views/selection.rs` (shared with WG5a)

`KeySelect { mode: Mode::{None, Single, Multi}, cursor, anchor, selected: BTreeSet<u64> }` over a
flat ordered `&[u64]` of visible ids (a tree passes its flattened visible rows):

- `nav(ids, Nav::{Up, Down, PageUp, PageDown, Home, End}, Mods { shift, ctrl }, page)`: plain moves
  select the cursor; Shift extends from the anchor (multi); Ctrl moves without selecting (multi).
- `toggle_cursor()` (Space), `click(ids, id, mods)` (Ctrl toggles, Shift ranges), `go_to(ids, id, mods)`
  (a tree's parent or first child), `set(ids)` (host `setSelection`; single keeps one).
- `type_ahead(ids, text, now_secs, label)`: prefix within `TYPEAHEAD_RESET` (1 s), case-insensitive,
  wraps from the cursor; one repeated character cycles through rows starting with it.
- Selection is by id: ids filtered out stay selected (SV46).
- It returns `Change { cursor, selection }`: scroll the cursor into view, raise `selection-changed`.

Whoever lands second between WG5a and WG5b adopts this file. Nothing calls it yet; it is `pub mod
selection` in `views/mod.rs`.

## Design for the rest

**Kind.** `tree-view` reuses the `virtual-list` machinery rather than a new widget: the same
`W::VirtualList(Box<VList>)`, the M5 protocol (`splice`, `setRow`, `setup`/`bind`/`unbind`/`teardown`),
recycled slots and `RowHeights`. Add `tree: Option<Box<Tree>>` to `VList`, set when the kind is
created, holding
`nodes: HashMap<u64 /* item id */, TreeNode { level, expandable, expanded, label }>`, a `KeySelect`,
and `indent`. JS keeps the tree and **flattens the visible rows** into the list (splices). Expanding
splices children in after the parent; collapsing splices them out. Lazy loading is JS's: on `expand(id)`,
`TreeView` calls the user's `children(node)` (sync or a Promise) and splices.

- `kinds/treeview.rs`: `DEF` (`id: "TreeView"`, `tag: Some("tree-view")`, base `Widget`). Props:
  `virtual-list`'s, plus `selection` with `Enum(["none", "single", "multi"])` on `P::ListSelection`.
  `virtuallist::set` parses against `SELECTION` (none, single) and needs a tree-aware branch. Add
  `indent` (new `P::TreeIndent`). Methods: virtual-list's, plus `setNode(id, level, flags, label)`
  (flags: 1 expandable, 2 expanded), `setSelection(ids[])`, `selection()` returning a `Value::List` of
  I64. Signals: virtual-list's, plus `expand(id)`, `collapse(id)` and `selection-changed()` (no
  args; JS calls `selection()`, because `Ty` has no int list), plus `activate(pos)`.
- Registration: `kinds/builtin.rs:325` lists `super::virtuallist::DEF`; add the tree's next to it,
  and `pub mod treeview` in `kinds/mod.rs` (~line 52). Creation is in `scene/tree.rs:138`:
  `"VirtualList" => W::VirtualList(Box::default())`; add `"TreeView" => W::VirtualList(Box::new(VList::tree()))`.
- Method dispatch: `ops.rs:195` checks `kinds::kind(k).tag == Some("virtual-list")` before
  `vlist_call`. Accept `"tree-view"` too and add the tree methods to `vlist_call`
  (`virtuallist.rs` ~line 417). Props are routed in `scene/props.rs:193` and `:353`
  (`W::VirtualList`); they need no change unless new `P`s are added. The `W::VirtualList` matches in
  `layout.rs:570-612`, `walk.rs:49,71,193`, `leaf.rs:373` and `virtuallist::paint` (line 784) cover
  the tree for free.

**Paint** (`virtuallist::paint`, per row ~line 830): when `v.tree` is set, inset the row rect by
`level * indent`, draw a disclosure triangle in the gutter for expandable nodes (`ui.interact` on
the gutter rect: a click raises `expand`/`collapse(id)`), and pick selection from `KeySelect`
(`is_selected(id)`, cursor outline) instead of `v.selected`. Row clicks go through
`KeySelect::click` with `ui.input(|i| i.modifiers)` (`command` as ctrl).

**Keyboard.** When the list has focus (make `list_id` focusable;
`ui.interact(r, list_id, Sense::click())` plus `memory.request_focus` on row click), read keys.
Use EV3's `Widget::consume_key` once EV3+ is on master (don't edit `focus.rs`); until then use
`ctx.input_mut(|i| i.consume_key(..))`.
- Up/Down/PageUp/PageDown/Home/End go to `KeySelect::nav`. `page` is `view.y / estimate`.
- Right: expandable and collapsed raises `expand(id)`; expanded goes to the first child with `go_to`
  (the next id, if its level is deeper).
- Left: expanded raises `collapse(id)`; otherwise go to the parent (scan up for a smaller level).
- Space goes to `toggle_cursor`, Enter to `activate`.
- `Event::Text` goes to `type_ahead` with `label` from `nodes`.
- A cursor change sets `v.reveal = Some(pos)`; a selection change emits `selection-changed`.

**A11y** (`scene::virt::a11y_row`, as virtual-list does): the list is `Role::Tree`, rows are
`Role::TreeItem` with `set_level(level + 1)`, `set_expanded(expanded)` for expandable rows,
`set_selected` and `position_in_set`/`size_of_set`. Siblings-only set sizes would need sibling
counts; the whole list is acceptable first.

**DnD.** WG4b's props (`drag-type`/`drag-data`/`drop-types`) work on any node, rows included, so
`TreeView` can put them on its row outlet. For a row drop with position (before/after/into), the
tree paints a drop indicator from the pointer's y within the row and emits
`drop-row(id, where, type, data)`. Use `DragAndDrop::payload::<DragPayload>(ctx)`,
`payload.best(drop_types)` (the first `drop-types` pattern the payload has; `MimeData` entries) and
`svelters_host::platform::type_matches`. A Rust panel drags rows in with
`DragPayload::from_mime(MimeData::new().with("cad/part", id).with("text/plain", name))`. Copying
rows to the clipboard is `Host::platform().clipboard.write(ClipboardKind::Clipboard, mime)`.

**JS** (`packages/native-runtime/src/lists.js`, `ListView` at ~line 380, `ListController` at ~90):
add `TreeView($$anchor, $$props)` reusing `ListController` with `element('tree-view')`. Props:
`roots`, `children(node)` (array or Promise), `hasChildren(node)`, `key(node)`, `label(node)`,
`row` snippet `(node, level)`, `selection`, `bind:selected` (keys), `bind:expanded` (a Set of keys),
`filter(node)`. It flattens the visible nodes (expanded ancestors, passing the filter) into the
`items` it hands the controller, and calls `setNode` for each bound id. Selection lives in JS as keys
(the source of truth), mapped to ids with `ctl.entries` (key to id) and pushed with `setSelection`.
That way it survives filtering even if an id is reissued. Also export from `@svelters/lists`, and
add typings.

**Tests.** kittest on a gallery-like fixture (`TestApp::from_source_stepped`): lazy expand
(children appear only after expand, Promise children), keyboard (Down/Right/Left/type-ahead),
multi-select with Shift, selection kept across a filter, a11y roles and levels
(`harness.query_by_label(..).accesskit_node()`), DnD between two rows (see
`tests/wg4b.rs::drag_and_drop_between_two_nodes` for the drag pattern), and a 100k-node tree that
binds only visible rows (`ListStats`).

## Not run

Only `cargo nextest run -p svelters-egui --lib views::selection` (4/4).
