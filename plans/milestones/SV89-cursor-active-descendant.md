# SV89: a focused widget names its keyboard cursor to assistive technology

Branch `ci/sv-b5`, base `master` `830d7b5`. Lane brief: the coordinator's message to the session (no batch file).
Status: built and tested; CI below.

## The problem

A focused `table` moved its cursor and selection on Up, Down, Home and End, and its rows went out as virtual
children with their `selected` state, but nothing named the cursor row: no `active_descendant` on the table and no
focus sub-target. AccessKit's focus stayed on the table node, so a screen reader announced nothing as the cursor
moved.

## What landed

Rule: while the cursor's item is in the accessibility tree, the widget's node names it as `active_descendant` and is
sent again whenever the cursor moves; when it isn't (scrolled out, not realised, a closed select), none. An
`aria-activedescendant` prop still wins (core applies it after the kind). Page: `docs/skelter/a11y-cursor.md`.

Kinds checked, and what changed:

| Kind | Items | Change |
| --- | --- | --- |
| `table` | virtual children | `TableKind::a11y` sets `active_descendant` to the cursor row's virtual id. Cursor moves already re-sent the node (`deliver` → `request_a11y`). |
| `virtual-list` | realised row nodes (not virtual children) | new `VirtualChildren::cursor_row` (skelter-core, default `None`); `Document::a11y_node` names the node bound to that row. `current` is a `Paint`-class prop, so a cursor move re-sends the list. |
| `tree-view` | realised row nodes | same, through the tree's `VirtualChildren::cursor_row` (the list's). |
| `select` | options as virtual children | the same gap while open: the highlighted option was never named, and highlight moves only repainted. Shim `A11yCx::active_child` (index into `children`; the host maps it to the virtual id) and shim `EventCx::a11y` (the host calls `request_a11y`); `Select` sets both. |
| `menu` | real nodes with anchors | already names its active item (SV60 anchors); unchanged. |

No kind called `set_focus_sub`; `active_descendant` was chosen instead because it needs no focus bookkeeping
(no re-targeting when the cursor row leaves the realised range, and the owner keeps the platform focus).

## Tests (run, all pass)

`cargo test -p skelter-widgets --lib -- the_cursor_row the_highlighted_option`:

- `table::kinds::tests::the_cursor_row_is_the_tables_active_descendant`: click, Down, Down, Home, End (row 99,
  scrolled into view) each name the cursor row; scrolling the cursor row out of view names none.
- `views::virtual_list::tests::the_cursor_rows_node_is_the_lists_active_descendant`: realised rows, a press then
  Down, Down, Home name the bound row node.
- `views::tree::tests::the_cursor_row_follows_the_cursor`: `cursor_row` across expand and collapse.
- `views::select::tests::the_highlighted_option_is_the_active_descendant`: F4, Down, End, Escape.

Not run locally: the full nextest, other crates. `cargo clippy --no-deps -p skelter-core -p skelter-widgets` with
the local toolchain (1.97) reports lints in skelter-core and skelter-anim lines this lane did not touch; none in the
changed lines.

## CI

`0aa8f3f` (master `830d7b5` merged: already up to date), all green: js 40681594590, linux-gtk 40681594773,
linux-qt 40681594775, macos-appkit 40681594673, skelter 40681594537, windows-winui 40681594668.
