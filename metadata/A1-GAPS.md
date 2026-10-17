# A1 framework gaps

Framework or ABI features whose absence blocks annotations. This is separate from facts that were
tried and failed a generated test (those are listed in each batch's status line in
`metadata/A1-TODO.md`). Framework lanes pick a gap from here and implement it; the A1 lead then
re-runs the batches marked `blocked: gap:<id>`.

Counts are of facts on widget kinds (GTK: `GtkWidget` subclasses and widget interfaces; Qt:
`QWidget`/`QLayout` subclasses), from the ingested metadata on 2026-10-17. They are upper bounds:
some blocked facts are not worth exposing anyway.

| Id | Gap | Blocked facts (approx.) | Batches |
| --- | --- | --- | --- |
| G1 | Child attributes (`set_child_attr`) | ~40 slot/attr facts | gtk-containers-b, qt-containers-b, qt-mainwindow |
| G2 | GTK method calls with arguments or return values | GTK: 669 methods with args, 396 zero-arg getters | every GTK batch (methods column) |
| G3 | Object-returning getters (borrowed handles) | GTK: 154 methods; Qt: ~60 | gtk-text-b, qt-text-b, qt-selection |
| G4 | Boxed/value types: **mostly closed by S1 (ABI v3, master 8071bb2)**; open: out params, GTK time-only/date-only records, model data | re-check per kind | gtk-display, qt-display |
| G5 | Sync signal handlers: **bool results closed by X2 (master ba5bb65)**; open: typed `value` results, listen phases, Qt `key_release`/generic events | GTK: `handler_result: value` signals; Qt event families | gtk-ranges (SpinButton input/output), qt-text |
| G6 | Model-backed views (GListModel, QAbstractItemModel): **closed by X5 (branch worktree-agent-a8dce564007788244, 28a8e17 GTK, abd7024 Qt)** through the portable `virtuallist` kind (`packages/lists`, `plans/milestones/M5-lists.md`). Open: ColumnView/TableView/TreeView columns, `GtkDropDown`/`GtkComboBox`/`QComboBox` models, `GtkTreeView`/`QTreeView` hierarchies | the 12 GTK / 11 Qt view kinds map to `virtuallist` (list, grid); the rest stay as listed | gtk-lists-b, qt-lists-b (re-run) |
| G7 | Non-widget child objects (QAction, GMenuModel) | QMenu, QMenuBar, QToolBar actions; GtkPopoverMenu(Bar) | qt-menus, gtk-menus |
| G8 | Native-to-JS callbacks (draw functions, sort/filter funcs): **sort/filter for model-backed views closed by X5 by design** (JS sorts and filters, the keyed diff pushes minimal splices; no `GtkSortListModel`/`QSortFilterProxyModel` with JS funcs, see M5-lists.md). Open: DrawingArea draw func, `GtkListBox`/`GtkFlowBox` sort/filter/header funcs (they have widget children, not a model; use the same JS-side approach or a CALLBACK-based func), `scale_format_value_func` | DrawingArea, ListBox/FlowBox sort/filter/header funcs | gtk-display (DrawingArea), gtk-lists-a (sort/filter) |

## G1: child attributes

Per-child facts that are neither a property of the child nor of the parent: grid cell and span,
notebook/tab label text, stack page name and title, paned resize/shrink, form-layout label,
QMainWindow toolbar and dock areas, splitter stretch factors. ABI v2 has no `set_child_attr`
(listed as "Later ABI additions" in `plans/architecture/backend-abi.md`).

- Blocked: `Gtk.Grid` children (`attach(child, col, row, w, h)`), `Gtk.Notebook` tab labels as text,
  `Gtk.Stack` `add_titled` (name/title), `Gtk.Paned` resize/shrink, `Gtk.Fixed` `put(x, y)`,
  `Gtk.Overlay` measure/clip, `QGridLayout`, `QFormLayout`, `QTabWidget` tab text and icon,
  `QToolBox` item text, `QMainWindow` toolbars/docks (`addToolBar(area, w)`), `QSplitter` stretch.
- Repro: `<grid><label data-col="1" data-row="2">x</label></grid>` must call
  `gtk_grid_attach(grid, label, 1, 2, 1, 1)`; changing `data-row` later must move it
  (`gtk_grid_remove` + `attach`, or the `GtkGridLayoutChild` row property).
- Workaround used in A1 where possible: insert-only defaults (Notebook pages with no tab label,
  QTabWidget tabs with an empty title). Those slots are annotated and tested; the attribute facts are not.

## G2: GTK method calls with arguments or return values

The GTK backend dispatches only `dispatch: zero_arg` methods returning void, through generated
gtk4-rs calls (`crates/svelters-gtk/src/generated/slots.rs::call_method`). PLAN M3 lists "GTK
dynamic GI method invocation"; it is not implemented.

- Blocked: e.g. `gtk_calendar_select_day(GDateTime)`, `gtk_level_bar_add_offset_value(name, value)`,
  `gtk_editable_select_region(start, end)`, `gtk_spin_button_spin(direction, increment)`,
  `gtk_text_buffer_insert_at_cursor`, `gtk_label_select_region`, every getter (`gtk_entry_get_text_length`).
- Repro: `entry.call('select_region', 0, 3)` on `<entry>` must call `gtk_editable_select_region(e, 0, 3)`;
  `entry.call('get_text_length')` must return a number. Today both fail with "method takes no arguments" / unknown method.
- Qt is not affected: `gen_cpp.py` generates typed thunks for annotated methods with i32/bool/f64/string params and returns.

## G3: object-returning getters (borrowed handles)

ABI v2 defines the BORROWED lifetime class but no `get_prop`/`call` mints a borrowed handle yet
(backend-abi.md "As implemented": "Only owned and toplevel are minted today").

- Blocked: `GtkTextView:buffer` (and all GtkTextBuffer editing), `GtkNotebook.get_nth_page`,
  `GtkEntry:buffer`, `QComboBox::lineEdit()`, `QTabWidget::tabBar()`, `QAbstractScrollArea::verticalScrollBar()`,
  `QTextEdit::document()`, `QMainWindow::menuBar()`.
- Repro: `const buf = textview.getProp('buffer')` must return a handle (borrowed) on which
  `buf.setProp('text', 'x')` works, and which goes stale (`owner`) when the text view dies.

## G4: boxed/value types

**Status 2026-10-17: mostly closed by lane S1** (ABI v3 value model, `docs/notes/s1-handoff.md`):
typed values, enums as names, flags, records (Rect/Size/Point/Insets/Color), boxed with dispose,
BYTES, dates (`QDate/QTime/QDateTime`, `GDateTime`) and string forms (QKeySequence, QUrl, QIcon,
QFont, QColor, GIcon, PangoFontDescription, GdkRGBA, GFile). Batches annotate these facts now.
Still open (lane S2): out/in-out parameters, GTK time-only/date-only records, list models.
The original text follows for reference.

The value model (`plans/architecture/backend-abi.md#planned-abi-v3-value-model-m3`) is not
implemented: props and signal args of boxed/value types are not converted.

- Blocked GTK: `GtkColorDialogButton:rgba` / `GtkColorChooser:rgba` (GdkRGBA), `GtkFontDialogButton:font-desc`
  (PangoFontDescription), `GtkCalendar` date (GDateTime), `GtkLabel:attributes` (PangoAttrList),
  `GtkImage:paintable`/`gicon` (GdkPaintable/GIcon; `icon-name` and `file` work), string lists (`GStrv`).
- Blocked Qt: `QColor`, `QFont`, `QIcon`, `QPixmap`, `QDate`/`QTime`/`QDateTime` (QDateEdit, QCalendarWidget),
  `QKeySequence`, `QSize`/`QPoint` props, `QStringList` (QComboBox items), and signals carrying them
  (`QCalendarWidget::clicked(QDate)`, `QDateEdit::dateChanged(QDate)`).
- Repro: `<colordialogbutton rgba="#ff0000">` must set a GdkRGBA; `bind:date` on `<qdateedit>` needs
  `QDate` <-> ISO string both ways.

## G5: sync signal handlers with return values

**Status 2026-10-17: bool results closed by lane X2** (`docs/notes/x2-sync-frame.md`, "What A1
should annotate"): GTK signals with `handler_result: bool_stop` (and `sync_required: true` for
`close-request`, `key-pressed`, `drop`, `accept`, `change-value`, `activate-link`, `state-set`) go
through `call_sync`; Qt has synthetic `close_request` and `key_press`. X2 recorded its closures as
X2-G1..G5 (sync results, per-event close veto, key handled, event controllers, rAF).
Still open: typed `value` results (`SpinButton::input/output`, drag actions), listen phases
(after/capture), Qt `key_release`/generic event families. The original text follows.

The backends return a fixed default to the toolkit (`false` for `bool_stop`, nothing for `value`)
and post the event asynchronously; only `close-request` is special-cased. `listen` flags for sync
handlers are listed under "Later ABI additions".

- Blocked: `GtkSwitch::state-set` (delay the state), `GtkSpinButton::input`/`output` (custom
  formatting), `GtkScale` format function, `GtkWidget::query-tooltip`, `GtkLinkButton::activate-link`
  (veto), `GtkNotebook::create-window`, event-controller handlers, Qt validators (`QValidator`),
  `QWizard::validateCurrentPage`.
- Repro: `<switch onstateset={() => true}>`: the switch must not change its `state` until the app sets it.

## G6: model-backed views

`GtkListView`, `GtkGridView`, `GtkColumnView`, `GtkDropDown` (model), `GtkTreeView`, `GtkIconView`,
`GtkComboBox` (model), `GtkCellView`, `GtkTreeExpander`; `QListView`, `QTreeView`, `QTableView`,
`QColumnView`, `QListWidget`, `QTreeWidget`, `QTableWidget`, `QUndoView`, `QHeaderView`,
`QComboBox`/`QFontComboBox` models. Planned for M5.

- Repro: `{#each items as it}<listitem>...</listitem>{/each}` inside `<listview>` must create a
  JS-backed GListModel with Svelte-rendered rows.
- Not blocked: `GtkListBox` and `GtkFlowBox` (widget children), `GtkDropDown` from a string list
  only after G4, `GtkComboBoxText`/`QComboBox::addItem(QString)` as methods.

## G7: non-widget child objects

Menus and toolbars hold actions (`QAction`, `GMenuItem`), which are not widgets and have no element kind.

- Blocked: `QMenu`, `QMenuBar` items, `QToolBar` actions, `QToolButton` menus, `GtkPopoverMenu`,
  `GtkPopoverMenuBar`, `GtkMenuButton:menu-model`.
- Repro: `<menubar><menu title="File"><action text="Quit" ontriggered={quit} /></menu></menubar>` must
  create a `QMenu` with a `QAction` and deliver `triggered`.

## G8: native-to-JS callbacks

Callback-typed parameters (`unsupported_reason: callback_scope` in the GTK metadata) need a JS
function table in the host (native code never holds a JS value) and usually a sync call.

- Blocked: `gtk_drawing_area_set_draw_func` (needs a drawing API too), `gtk_list_box_set_sort_func` /
  `set_filter_func` / `set_header_func`, `gtk_flow_box_set_sort_func`, `gtk_scale_set_format_value_func`.
- Repro: `<listbox sort={(a, b) => ...}>` must reorder rows natively by the JS comparator.
