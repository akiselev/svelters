# A1 qt-containers-b notes

- QMdiArea has no count/indexOf/itemAt, so the `subwindows` many slot is blocked (gap G13). Probe: `addSubWindow(w)` wraps w in a
  QMdiSubWindow (`w->parent()` is the wrapper); `removeSubWindow(w)` reparents w to null but leaves the empty wrapper in
  `subWindowList()`; only `removeSubWindow(wrapper)` drops it. A future slot needs `nested: true` and a list op over subWindowList.
- QDockWidget::setWidget replaces the old widget by hiding it; it stays a child of the dock (also after setWidget(nullptr)).
  The one-slot ops cannot express that (`old: unparented` would fail; `take` cannot use `then` with `receiver: child`).
  New gap: one-slot replace/remove where the native setter leaves the old child parented. `widget` slot left out.
- Gallery test requires every class in the `gallery_page!` list to be in the widget tree, so unexposed layouts are only in the
  `kinds:` comment. Enum JS names are kebab-case of the enumerator (`sub-window-view`, `creation-order`).
