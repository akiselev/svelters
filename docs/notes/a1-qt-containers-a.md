# A1 qt-containers-a notes

- Tab/tool-box pages: the generated slot `order` test asserts child->parent() == the element, but QTabWidget/QToolBox reparent pages into an internal widget (QStackedWidget / scroll area contents). Slots `pages` on tabwidget/toolbox are therefore not verifiable (new gap: generated slot tests assume host == parent). Their currentIndex prop and binding were dropped with them (no pages can be added from JS, and the prop test cannot select an index on an empty widget).
- QStackedWidget currentIndex: prop test uses `sample: -1` (an empty widget cannot select 7); real behaviour is covered by the gallery page and the binding. Inserting the first page emits currentChanged(0), and a currentIndex write before pages exist is ignored: set bound state after mount (setTimeout) or the first-page event overwrites it.
- QGroupBox: `checked` is ignored unless the box is checkable (setChecked on a non-checkable box does nothing), and setCheckable(true) resets checked to true silently each call. `checked` prop dropped (generated test cannot verify it on a fresh box); toggled and clicked kept.
- toggled/clicked/splitterMoved/tabBarClicked/tabCloseRequested triggers emit the signal directly (a real click or drag needs a shown widget).
- QSplitter remove: `QWidget::setParent(nullptr)` on the child (receiver: child); passes the generated ownership test (unparented, not deleted).
- qdoc names QGroupBox::toggled(bool on); `on` must be quoted in YAML (bool).
