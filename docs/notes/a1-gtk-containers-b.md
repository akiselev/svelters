# A1 gtk-containers-b notes

- Generator gap (ListBox.children): `GtkListBox::remove` only accepts the GtkListBoxRow, not the wrapped widget
  (GTK warns "Tried to remove non-child" and does nothing). The slot op `remove` has no arg that yields the
  wrapper row (direct child of the parent containing `child`), so the ListBox children slot was dropped.
  FlowBox::remove does accept the inner widget, so FlowBox children works (wrapped:FlowBoxChild).
- Notebook pages are not direct children of the notebook (internal stack): `get: {contains: ancestor}`.
- gtk4-rs FlowBox has no `append`; use `insert(w, -1)` in triggers.
- Notebook: GTK emits one notify::page by itself after the notebook is shown; the gallery page skips the
  first event, otherwise the "echo page" label shows 1.
- Trigger `rust:` snippets can be self-contained (the bind:page binding test has no `setup`).
