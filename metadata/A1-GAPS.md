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
| G2 | GTK method calls with arguments or return values: **closed by X1 (master 865482d)** except out/in-out params, arrays and callbacks (S2) | out params, arrays | - |
| G3 | Object-returning getters (borrowed handles): **closed by X1** (GTK: any GI getter; Qt: ~110 generated getters) | Qt methods with required args on unannotated classes, list returns | - |
| G4 | Boxed/value types: **closed by S1 + S2** (typed values, out/in-out params, exact GVariant, GValue props, VALUE handles); open: GTK time-only/date-only records, model data | few | - |
| G5 | Sync signal handlers: **bool results closed by X2 (master ba5bb65)**; open: typed `value` results, listen phases, Qt `key_release`/generic events | GTK: `handler_result: value` signals; Qt event families | gtk-ranges (SpinButton input/output), qt-text |
| G6 | Model-backed views (GListModel, QAbstractItemModel): **closed by X5 (branch worktree-agent-a8dce564007788244, 28a8e17 GTK, abd7024 Qt)** through the portable `virtuallist` kind (`packages/lists`, `plans/milestones/M5-lists.md`). Open: ColumnView/TableView/TreeView columns, `GtkDropDown`/`GtkComboBox`/`QComboBox` models, `GtkTreeView`/`QTreeView` hierarchies | the 12 GTK / 11 Qt view kinds map to `virtuallist` (list, grid); the rest stay as listed | gtk-lists-b, qt-lists-b (re-run) |
| G7 | Non-widget child objects (QAction, GMenuModel); S2 added `QMenu.yaml` (tag `menu`) with callbacks as a proof: extend it | QMenu, QMenuBar, QToolBar actions; GtkPopoverMenu(Bar) | qt-menus, gtk-menus |
| G8 | Native-to-JS callbacks: **closed on Qt by S2** (`callbacks:`, async methods); **sort/filter for model-backed views closed by X5 by design** (JS sorts and filters, keyed diff pushes minimal splices; see M5-lists.md). Open on GTK: DrawingArea draw func, `GtkListBox`/`GtkFlowBox` sort/filter/header funcs, `scale_format_value_func` | DrawingArea, ListBox/FlowBox sort/filter/header funcs | gtk-display (DrawingArea), gtk-lists-a (sort/filter) |
| G9 | Toplevel kinds other than `window` cannot be mounted (renderer) | every Qt dialog element's gallery presence; GTK AboutDialog, Popover-like toplevels | qt-dialogs (gallery), gtk-dialogs |
| G10 | GTK slots whose native ops take a widget subclass: **closed by the A1 lead** (generator downcasts to `accepts`, upcasts getters; MenuButton `popover` verified) | MenuButton `popover`, Popover-typed slots, Notebook menu labels | gtk-buttons, gtk-dialogs |
| G11 | Enums outside the Gtk namespace: **enums closed by the A1 lead** (Pango/Gdk nicks kept on the property; Label `ellipsize`/`wrap-mode` verified); non-Gtk object kinds still open | Label `ellipsize`/`wrap-mode`, Inscription `wrap-mode`, GdkCursor/Paintable props | gtk-text |
| G12 | Non-widget objects reached as borrowed handles have no generated kind (GtkTextBuffer, GtkAdjustment) | TextView text editing, Range/SpinButton value and range | gtk-text, gtk-ranges |
| G13 | Qt many-slots without an index/list API: **closed by the A1 lead** (`order: append_only` slots, membership by isAncestorOf; QStatusBar `widgets`/`permanent` verified) | QStatusBar `widgets`/`permanent`, QDialogButtonBox buttons | qt-bars |

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

**Status 2026-10-17: closed by lane X1** (`docs/notes/x1-borrowed-gi.md`): dynamic GIRepository
calls with scalars, strings, enums/flags, objects, GStrv and boxed values. A1 annotates useful
methods with `dispatch: dynamic` (generated test through `Harness::call_args`). Still open (S2):
out/in-out parameters, arrays other than GStrv, callbacks, static functions. The original text follows.

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

**Status 2026-10-17: closed by lane X1**: GTK getters mint borrowed handles (weak-ref tracked,
`owner`/`parent` death reasons); Qt has about 110 generated getters (`gen_getter`). Still open: Qt
methods with required arguments on classes without an override, list returns. The original text follows.

ABI v2 defines the BORROWED lifetime class but no `get_prop`/`call` mints a borrowed handle yet
(backend-abi.md "As implemented": "Only owned and toplevel are minted today").

- Blocked: `GtkTextView:buffer` (and all GtkTextBuffer editing), `GtkNotebook.get_nth_page`,
  `GtkEntry:buffer`, `QComboBox::lineEdit()`, `QTabWidget::tabBar()`, `QAbstractScrollArea::verticalScrollBar()`,
  `QTextEdit::document()`, `QMainWindow::menuBar()`.
- Repro: `const buf = textview.getProp('buffer')` must return a handle (borrowed) on which
  `buf.setProp('text', 'x')` works, and which goes stale (`owner`) when the text view dies.

## G4: boxed/value types

**Update (S2, master e5a208d):** out and in-out parameters (`out:`/`inout:` on Qt methods, GTK
through GI), exact GVariant typing, GValue-typed GTK properties and VALUE-class handles for boxed
values are done. Remaining: GTK time-only/date-only records and model data (G6).

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

**Update (S2):** closed on Qt: `callbacks:` (functor/std::function params become JS functions) and
`async:` (Promise-returning methods settled by a signal, e.g. `QDialog::open` + `finished`). GTK
callback parameters (`gtk_drawing_area_set_draw_func`, list sort/filter funcs) remain open.

Callback-typed parameters (`unsupported_reason: callback_scope` in the GTK metadata) need a JS
function table in the host (native code never holds a JS value) and usually a sync call.

- Blocked: `gtk_drawing_area_set_draw_func` (needs a drawing API too), `gtk_list_box_set_sort_func` /
  `set_filter_func` / `set_header_func`, `gtk_flow_box_set_sort_func`, `gtk_scale_set_format_value_func`.
- Repro: `<listbox sort={(a, b) => ...}>` must reorder rows natively by the JS comparator.

## G9: toplevel kinds other than `window`

`packages/renderer/src/index.js` `attachNative` throws "only <window> may be mounted at top level"
and always calls `present`. Dialog elements (Qt `dialog`, `messagebox`, `inputdialog`, ...; GTK
`aboutdialog`) are toplevel kinds (`gen_is_toplevel`, lifetime `toplevel`) but cannot be mounted,
so apps cannot `open()` them through `bind:this`.

- Repro: `{#if show}<messagebox bind:this={mb} text="Hi" />{/if}` at the top level of an App must
  create the dialog hidden (or shown modelessly, per a declared policy) and let `mb.call('open')` show it.
- Needs: the renderer to know a kind is toplevel without a native call (a `toplevel` flag in the
  generated schema/descriptor, or `handle_info` exposed to JS) and a per-kind "show on mount"
  method (`present` for windows, none for dialogs).

## G10: typed slot ops on GTK

`gen_backend.py` emits slot ops on `&gtk::Widget`; gtk4-rs `MenuButton::set_popover` takes
`Option<&impl IsA<Popover>>` and `popover()` returns `Option<Popover>`, so the generated code does
not compile. Fix in the generator: downcast the child to the slot's `accepts` type before the call
and upcast the getter's result. Blocks MenuButton `popover`, Popover-accepting slots.

## G11: non-Gtk enums and kinds in the GTK metadata

`tools/metadata/gtk/ingest.py` writes kinds for the Gtk namespace only; properties typed with
`Pango.EllipsizeMode`, `Pango.WrapMode` or Gdk types have `enum<...Pango...>` refs with no enum kind,
so the generator cannot pick a test sample. Fix: ingest the enums/flags referenced by Gtk
properties from their own namespaces (Pango, Gdk, Gsk, Graphene) into `metadata/gtk/`.

## G12: borrowed objects without a generated kind

X1 mints borrowed handles for any GObject a getter returns, but kinds that are not in
`metadata/gtk` (GtkTextBuffer is a GObject in Gtk but not a widget; GtkAdjustment likewise) have
generated kinds only for their own props, and the A1 annotation format has no way to verify facts
on a borrowed object reached through another element. Repro: `textview.getProp('buffer').setProp('text', 'x')`
must change the text view's content; `scale.getProp('adjustment').setProp('upper', 100)` the scale's range.
Needs: a `verify_subject` form "borrowed from <tag>.<getter>" in the GTK override format and tests that
reach the object through the getter.

## G13: Qt many-slots without index/list ops

`gen_cpp.py` requires `insert` (with a `before != null` variant), `remove`, `index_of` and `list`
for every many-slot. `QStatusBar::addWidget/insertWidget/removeWidget` and
`QDialogButtonBox::addButton/removeButton` have no `indexOf`/`count`/`itemAt`. Fix in the generator:
allow `order: append_only` many-slots with only `insert` + `remove`, membership through
`isAncestorOf`, and generated tests for insert/remove/ownership without order checks (and an
`index_of` computed from the layout/children list where one exists). Owner: A1 lead (generator).
