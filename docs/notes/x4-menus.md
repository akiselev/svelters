# X4: menus and actions (working notes)

Lane X4, 2026-10-17, master 2aa2127. Plan: `plans/milestones/X4-menus.md`.

## Survey (before coding)

- Qt generator is widget-centred: slots cast children with `qobject_cast<T*>` but the generated tests assume `QWidget` parents (`isAncestorOf`). QAction is QtGui, so there is no `metadata/qt/QAction.json`: the ingest filter is `module == qtwidgets`.
- GTK extras (`gio-file`, `virtuallist`) are hand-wired in `backend.rs` (create/destroy/set_prop/get_prop/call/listen/resolve/describe). The menu module follows the `virtuallist` pattern.
- The GTK test harness has no key-event synthesis; shortcut tests need a way (see below).
