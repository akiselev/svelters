# A1 qt-followups notes

- QDockWidget `widget`: G15 clearing remove (setWidget(nullptr) + setParent(nullptr) on child) verified; replace leaves old child `unparented`.
- QMdiArea `subwindows`: append-only + nested; removeSubWindow(w) + setParent(nullptr) passes the ownership test (the empty wrapper stays in the area, as noted before).
- QToolBox/QTabWidget currentIndex: the first-signal trigger must emit even with the two setup pages present: use `addX(new QWidget); setCurrentIndex(count-1)` (as QStackedWidget), not `addTab` alone.
- QGroupBox `checked`: `setup: setCheckable(true)` works; bind:checked verified.
- Dropped: QInputDialog.doubleValue (ingest types it i32 and props cannot override type), QWizard pages (removePage(int id) needs the id; QWizardPage has no id accessor).
