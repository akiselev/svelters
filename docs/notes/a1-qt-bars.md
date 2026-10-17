# A1 qt-bars notes

- QStatusBar `widgets`/`permanent` many-slots NOT annotated (new gap): QStatusBar has no indexOf, count or
  itemAt, and the generator's many-slot contract requires `index_of` and `list` ops. removeWidget + setParent(nullptr)
  would be right for remove, but the slot cannot be declared without those ops.
- QMenuBar: `nativeMenuBar` fails (setNativeMenuBar(true) reads back false on the xcb test platform): dropped.
  Menus/actions: G7 still blocks (addMenu(QString) returns an unreachable QMenu*, addMenu(QMenu*)/insertMenu use QAction*).
- QToolBar: plain props/signals only; no window toolbar slot (G1) and actions need G7.
- QDialogButtonBox: no button slot (no insertion index/count/indexOf); standardButtons flags verified.
- QTabBar: tabs only via methods (addTab/insertTab/removeTab/moveTab); `bind:currentIndex` verified.
- The `window` element has only one statusbar/menubar visible in the gallery (the app window), so QMainWindow props are verified by tests only.
