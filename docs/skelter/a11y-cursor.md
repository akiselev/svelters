# The keyboard cursor and assistive technology

A widget that keeps a keyboard cursor inside itself (a table's current row, a list's current item, an open
select's highlighted option) holds focus on its own node: the cursor is a row or item inside it, not a node
with focus of its own. For a screen reader to announce the item the arrow keys land on, the widget names it as its
AccessKit **active descendant**. Platforms report the active descendant as the focused object while its owner has
focus, and announce it again each time it changes.

## The rule (SV89)

While the cursor's item is in the accessibility tree, the widget's node names it as `active_descendant`, and the
node goes out again whenever the cursor moves. When the item is not in the tree (scrolled out of a table's drawn
rows, a list row not realised, a select that is closed), the widget names none. An `aria-activedescendant` prop on
the node wins over the widget's own choice.

| Kind | Items | Active descendant |
| --- | --- | --- |
| `table` | rows and cells as virtual children | the cursor row's virtual child (`a11y::virtual_id(table, index)`) |
| `virtual-list` | realised row nodes | the node bound to the cursor row |
| `tree-view` | realised row nodes | the node bound to the cursor row |
| `select` | options as virtual children | the highlighted option while the list is open |

## Writing a kind

- **Items as virtual children** (`A11yCx::add_virtual_child`): set `out.set_active_descendant(virtual_id(node,
  index))` in `Kind::a11y`, with the index `add_virtual_child` returned. A widget on the shim sets
  `A11yCx::active_child` to the index into its `children`; the host turns it into the virtual id.
- **Items as realised rows** (`VirtualChildren`): return the cursor's row from `VirtualChildren::cursor_row`; the
  document names the node bound to it.
- **Re-send on every cursor move.** A `Paint`-class prop change (`current`) re-sends the node; a cursor that
  moves without one asks for it (`EventCx::request_a11y`, or `a11y = true` on the shim's `EventCx`).
