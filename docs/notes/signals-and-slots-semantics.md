# Signals with return values, sync handlers, and child slots (GTK4 / QtWidgets)

Status: research notes, 2026-10-16. Builds on `widget-metadata-ingestion.md` §1.3, §3, §4
(child policy, `handler_result`, `sync_required`) and `gtk-v1-archaeology.md` (named slots
`<child:titlebar>`, `<controller:motion>`). Those are not repeated here.

## Part 1. Synchronous handlers and return values

### 1.1 GTK: non-void signals (scripted counts)

`scripts/gir_signal_returns.py` (GIR, static) and `scripts/gtk_signal_accumulator_probe.py`
(PyGObject under Xvfb, runtime) over GTK 4.22 / Adw 1.x:

| Namespace | signals | non-void | of which deprecated owner | of which keybinding `action` | signals with out-params |
|---|---|---|---|---|---|
| Gtk-4.0 | 348 | 67 | 18 (TreeView, IconView, ComboBox, EntryCompletion, ShortcutsSection) | 25 | 3 (2 non-void: `Overlay::get-child-position` out `allocation`, `SpinButton::input` out `new_value`) |
| Adw-1 | 55 | 15 | 1 | 0 | 1 (`SpinRow::input`) |
| Gdk-4.0 | 35 | 2 | 0 | 0 | 0 |

Every non-void signal is `when=last` (class handler runs after `connect()` handlers).
Return types: Gtk `gboolean` 57, `GdkDragAction` 4, one each of `utf8`, `GdkContentProvider`,
`GdkGLContext`, `Notebook`, `GObject`, `gint`.

GIR does **not** record the accumulator, so the probe connects two handlers (h1 returns
TRUE/non-default, h2 records) and emits with synthesized arguments, one subprocess per signal:

| Behaviour | Count (Gtk+Adw) | Meaning | Examples |
|---|---|---|---|
| `stop_on_true` | 33 (26 Gtk + 7 Adw) | `g_signal_accumulator_true_handled`-style: TRUE stops later handlers and the class handler | `Window::close-request`, `EventControllerKey::key-pressed`, `Widget::query-tooltip`/`mnemonic-activate`/`keynav-failed`, `DropTarget::accept`/`drop`, `DragSource::drag-cancel`, `Label::activate-link`, `Switch::state-set`, `Range::change-value`, `SpinButton::output`, `GLArea::render`, `IMContext::*`, `Adw.TabView::close-page`, `Adw.*::extra-drag-drop` |
| `stop_after_first` | 10 (6 + 4) | first connected handler's value wins, later handlers never run (`g_signal_accumulator_first_wins` or stop-on-non-zero) | `DropTarget::enter`/`motion` -> `GdkDragAction`, `DragSource::prepare` -> `GdkContentProvider`, `ComboBox::format-entry-text` |
| `last_wins` | 32 (28 + 4) | no accumulator; last handler (or class closure) value is the result | the keybinding `action` signals (`Paned::move-handle`, `Notebook::reorder-tab`, ...), `EventControllerKey::modifiers`, **`EventControllerScroll::scroll`**, `SpinButton::input`/`Adw.SpinRow::input`, `GLArea::create-context`, `Notebook::create-window`, `Adw.TabView::create-window`, `Adw.TabOverview::create-tab`, `Adw.NavigationView::get-next-page` |
| not probed | 9 | crashed on fake args (`FlowBox::move-cursor`, `Paned::cycle-handle-focus`, `ShortcutsSection::change-current-page` hit `g_assert`), `GdkEvent`/`TreeModel` args, abstract `Gdk.Surface` | |

Consequences:

- Accumulator kind must be a curated fact (`origin: runtime-probe`), not inferred from the
  return type: `scroll` returns gboolean but does *not* stop on TRUE.
- With `stop_after_first`, the *first connected* handler decides. Svelters connects one native
  closure per (object, signal) and fans out in JS (v1 lesson 3), so JS-side fan-out must
  reproduce the accumulator itself (stop at first TRUE / take first value).
- The class handler runs last; returning TRUE from a JS handler suppresses default behaviour
  (`close-request` -> window stays; probe: `close-request -> True: visible=True`).
- Signals that matter for apps (non-deprecated, non-keybinding, widget-facing): about 30 in
  Gtk, ~12 in Adw. Keybinding `action` signals are emitted by shortcuts; apps rarely connect
  them and they need no sync path in v1 (expose as queued notifications or not at all).
- 323 Gtk vfuncs (class + interface) exist in GIR; they are only reachable by GType
  subclassing (`snapshot`, `measure`, `size_allocate`, `GtkBuildable`, `GListModel` impls).
  These are the GTK analogue of Qt virtual overrides; see 1.3.

### 1.2 Qt: where return values actually live

Qt signals return void in practice (moc allows non-void signals; only the last directly
connected slot's value is returned; none of QtWidgets' public signals use it). Results are
expressed through **virtuals** and **event acceptance**. `scripts/qt_virtual_categories.py`
over the qdoc indexes (123 classes: QtWidgets QObject subclasses + the item-model/validator
bases), folding overrides into the introducing class, gives **376 distinct virtuals**:

| Category | Count | Reachable without subclassing? | Sync result? |
|---|---|---|---|
| `event_handler` (`*Event(QXxxEvent*)`) | 73 (25 on QWidget, rest QGraphics*) | **yes**: `installEventFilter` sees the event before the virtual | accept/ignore + filter bool |
| `event_handler_veto` (`closeEvent`) | 2 | yes (event filter) | yes |
| `event_dispatch_bool` (`event`, `eventFilter`, `viewportEvent`, `nativeEvent`, `sceneEvent`, `editorEvent`) | 6 | `event` yes via filter; `eventFilter` *is* the mechanism | bool |
| `paint` (`paintEvent`, delegate `paint`, `drawRow`, `initStyleOption`) | 27 | `paintEvent` via filter (QPainter during the Paint event is allowed); delegates no | side effect, must be sync |
| `geometry_query` (`sizeHint`, `minimumSizeHint`, `heightForWidth`, `sizeHintForRow`, ...) | 12 | **no** | value |
| `model` (`QAbstractItemModel` + proxy: `data`, `rowCount`, `index`, `parent`, `flags`, `setData`, `filterAcceptsRow`, `lessThan`, ...) | 45 | no | value, very hot |
| `validator_formatter` (`QValidator::validate/fixup`, `QSpinBox::textFromValue/valueFromText`, `QAbstractSpinBox::stepEnabled`) | 9 | no | value + in/out args (`validate(QString&, int&)`) |
| `delegate` (`createEditor`, `setEditorData`, `setModelData`, ...) | 10 | no | value/side effect |
| other value-returning (`QAbstractItemView::indexAt/visualRect`, `QLayout::itemAt`, `QGraphicsItem::boundingRect/shape`, `QDialog::exec`, `QMainWindow::createPopupMenu`, ...) | 100 | no | value |
| other void (`setModel`, `setGeometry`, `timerEvent`-like hooks, ...) | 92 | mostly irrelevant to apps | - |

"Virtuals that matter" for a Svelte app, i.e. that must be on the generated-subclass list:
the 25 QWidget event handlers (all coverable by **one** event filter per widget, no
subclass), `closeEvent`, `sizeHint`/`minimumSizeHint`/`heightForWidth` (3), `paintEvent`
(filter is enough), the model surface (~15 of the 45: `rowCount`, `columnCount`, `data`,
`setData`, `headerData`, `flags`, `index`, `parent`, `hasChildren`, `canFetchMore`/`fetchMore`,
`mimeData`/`dropMimeData`, `roleNames`, plus proxy `filterAcceptsRow`/`lessThan`), validators
(4), delegates (~6). **About 30 virtuals need generated C++ subclasses**; everything input-
related goes through event filters. Models/delegates/validators are not widgets: they are
JS-implemented native objects (`svelters-qt-build` emits `SvlJsModel : QAbstractItemModel`,
`SvlJsValidator : QValidator`, `SvlJsDelegate : QStyledItemDelegate`, `SvlWidget<T> : T` for
size hints), reusing the generated-subclass mechanism in `backend-abi.md`.

Probed semantics (`scripts/qt_sync_probe.py`, offscreen):

```text
close: filter ignore=False return=False -> close() returned True, visible=False
close: filter ignore=False return=True  -> close() returned True, visible=False   <- return True alone does NOT veto
close: filter ignore=True  return=False -> close() returned True, visible=False   <- closeEvent() re-accepts
close: filter ignore=True  return=True  -> close() returned False, visible=True   <- both needed
key filter: keyPressEvent ran 1x for A(filtered)+B ; text='b'
```

So the Qt mapping of a JS veto on `close` is **`event->ignore(); return true;`** in the
filter. For input events, `return true` from the filter = "the widget never sees it" and
`accept()/ignore()` (set by the widget's handler) = "do not / do propagate to the parent".

### 1.3 Categories

| # | Category | GTK examples | Qt examples | Rate | Delivery |
|---|---|---|---|---|---|
| A | Notification (void) | `clicked`, `notify::*`, `changed` | all signals | any | **queued** to checkpoint |
| B | Veto / handled bool | `close-request`, `key-pressed`, `activate-link`, `state-set`, `mnemonic-activate`, `keynav-failed`, `change-value`, `Adw.TabView::close-page` | `closeEvent` (ignore), any event via filter (return true / accept) | human-rate | **sync** |
| C | Query with side-channel object | `query-tooltip` (fill `GtkTooltip`, return bool), `Overlay::get-child-position` (out rect), `SpinButton::input` (out double) / `output` | `QWidget::event(QEvent::ToolTip)` + `QToolTip::showText`, `QValidator::validate(QString&, int&)` | pointer-rate | **sync**, borrowed args |
| D | Value-returning negotiation | `DropTarget::enter`/`motion` -> `DragAction`, `DragSource::prepare` -> `ContentProvider`, `create-window` -> `Notebook` | `dragEnterEvent` + `acceptProposedAction`, `createPopupMenu`, `createEditor` | pointer-rate | **sync** |
| E | Pull model / data provider | `GListModel::get_item`/`get_n_items` (JS-backed, v1 `GJsListModel`), `SignalListItemFactory::bind` | `QAbstractItemModel::data/rowCount/index`, `filterAcceptsRow`, `lessThan` | **very hot** (per visible cell per paint) | sync, but served from a **native-side cache/snapshot**, not per-call JS |
| F | Layout/paint override | vfuncs `measure`, `snapshot` | `sizeHint`, `heightForWidth`, `paintEvent` | per layout/frame | sync; v1: unsupported or cached value (`sizeHint` as a settable prop on the generated subclass) |

Category E is where per-call sync JS is wrong. SVR16 (model-driven views) should keep the
model's data in a native snapshot that JS patches at checkpoints (`rowsInserted` /
`items-changed` emitted from the patch), so `data()` never enters JS. Category F is best
modelled as *props on the generated subclass* (`sizeHint={[w,h]}`) rather than callbacks.

### 1.4 Prior art

| Binding | Signals with return values | Virtual overrides | Async |
|---|---|---|---|
| **gjs** | One `GClosure` per `connect`; marshaller calls into SpiderMonkey synchronously inside emission and converts the JS return with the signal's return GType (any truthy JS value -> TRUE for gboolean). Calling JS during GC sweep is refused ("Attempting to call back into JSAPI during the sweeping phase of GC"), and closures of finalized objects are invalidated. | `GObject.registerClass` + `vfunc_name` methods, installed into the GType class struct; synchronous. | No special casing: an `async` handler returns a Promise, which is truthy, so `close-request` silently becomes TRUE (window never closes). |
| **PyGObject** | Same closure model; `None` -> FALSE; return converted by GType. | `do_name` methods on a Python GObject subclass. | Same pitfall for coroutines (returns a coroutine object). |
| **PySide6/Shiboken** | Signals are void; slots can return values only via `invokeMethod`. | Shiboken generates a wrapper subclass per class (`QWidgetWrapper` in `qwidget_wrapper.cpp`) that overrides **every** virtual; each override takes the GIL, looks up (and caches) a Python override, else calls the base. Only objects constructed from Python get the wrapper. Event filters via `QObject.installEventFilter` with a Python `eventFilter`. | none; Python runs inside the virtual. |
| **NodeGui** | Signals and `QEvent`s share one API: `widget.addEventListener('clicked', cb)` (signal) and `addEventListener(WidgetEventTypes.Close, cb)` (event). Each wrapped class is an `N`-prefixed C++ subclass (`NWidget`, `NPushButton`) with an `EventWidget` mixin whose `event()` override calls JS synchronously on the Qt thread (qode merges Node's libuv loop into Qt's) and passes a wrapped native event, so JS can call `accept()`/`ignore()` in the callback. | Generated subclasses, limited set. | Same: an async callback has already returned when it awaits. |
| **QML** | `onClosing: (close) => close.accepted = false`: the result is a field on an event object, not a return value. | n/a | Same. |
| **DOM / Electron** | `preventDefault()` / `stopPropagation()` on the event object; return value ignored (except legacy `return false` in `on*` attributes). Electron `BrowserWindow 'close'` + `e.preventDefault()` is sync for the same reason. | n/a | `async` handler can still call `preventDefault()` before its first `await`. |

The QML/DOM pattern (result = flags on an event object) is the one that survives
async handlers and Svelte's delegation; gjs/PyGObject's "return value is the result"
pattern is the one that breaks.

### 1.5 Design: sync re-entry into JS

**What Svelte gives us** (checked in `svelte@5.57.2/src/internal/client/dom/elements/events.js`):

- Non-delegated events (`$.event(name, el, handler)`, e.g. `onclose-request={...}`, which
  compiles fine on a custom element) call `addEventListener` with a wrapper that **returns the
  handler's return value**, run inside `without_reactive_context` (active reaction/effect cleared).
- **Delegated** events (`click`, `input`, `change`, `keydown`, `keyup`, `pointer*`, `mouse*`,
  `focusin/out`, `contextmenu`, `beforeinput`, `touch*`; `DELEGATED_EVENTS` in
  `src/utils.js`) are dispatched by `handle_event_propagation`, which **discards the return
  value** and honours only `event.cancelBubble`.

So a `return true` convention cannot work for `onkeydown`, and it is fragile in general.
**The result must be read from the event object after dispatch.**

**Recommended model**

1. Metadata marks each signal/event `delivery: queued | sync`, and gives the result recipe
   (`result:` in 2.5.9; extends §4 of the ingestion note). Only signals with a JS listener are
   connected (`listen(h, sig, true)`); a sync signal with no listener costs nothing.
2. The backend's native closure/filter calls `host->call_sync(h, sig, args, n, out)`.
   The host builds a `NativeEvent` and dispatches it through the shim's `dispatchEvent`
   (so Svelte's delegation and bubbling via `composedPath()` work), then computes `out` from:
   - `event.defaultPrevented` (`preventDefault()`),
   - `event.cancelBubble` (`stopPropagation()`),
   - `event.returnValue` (explicit value for categories C/D: `DragAction`, tooltip bool),
   - writable out-fields declared by the metadata (`event.newValue` for `SpinButton::input`,
     `event.input`/`event.pos` for `QValidator::validate`),
   - the handler's return value, **only if** it is not a thenable and the signal declares
     `accept_return: true` (convenience for non-delegated names; lint-warned otherwise).
3. Mapping to native results (per signal, generated from metadata):

| Native | `preventDefault()` | `stopPropagation()` | value |
|---|---|---|---|
| GTK `stop_on_true` veto (`close-request`, `activate-link`, `state-set`, `TabView::close-page`) | TRUE | TRUE (same; one bool) | - |
| GTK input (`key-pressed`, `mnemonic-activate`, `keynav-failed`) | TRUE (= `GDK_EVENT_STOP`) | TRUE | - |
| GTK `query-tooltip` | - | - | `returnValue`/`tooltip.setText(...)` -> TRUE if tooltip set |
| GTK `DropTarget::enter/motion` | `0` (reject) | - | `returnValue: DragAction` (default: the target's preferred action) |
| Qt `Close` (filter) | `ev->ignore(); return true` | - | - |
| Qt input event (filter) | `return true` (widget's own handler skipped) | `ev->accept(); return true` (no propagation to parent) | - |
| Qt `DragEnter`/`DragMove` | `ev->ignore()` | - | `ev->setDropAction(returnValue); ev->accept()` |

   On GTK, input propagation (capture/bubble through controllers on ancestors) is already done
   by GTK; returning TRUE stops it, so both flags collapse to one bool. On Qt they map to two
   different mechanisms (filter result vs `accept()`), which is why the event object keeps
   them separate.
4. **Re-entrancy rules** inside `call_sync` (a "sync scope" flag in the host):
   - The JS handler may read anything (props via `getProp`, `$state`, `$derived`); Svelte
     already runs handlers outside any reactive context.
   - Writes to `$state` are allowed. Svelte schedules effects through a microtask; the host
     does **not** drain QuickJS jobs inside `call_sync`, so effects and the resulting graph
     patches stay queued until the next checkpoint. `flushSync()` inside a sync handler is a
     dev error.
   - Host tree/prop ops (`insert`, `remove`, `setProp`, `call`) issued from a sync scope are
     **appended to the checkpoint op queue**, not applied. Exception: methods on *borrowed
     argument objects* (`GtkTooltip`, `QEvent`, `GdkDrop`, `QMimeData`) and methods annotated
     `sync_safe` (pure getters, `acceptProposedAction`) run immediately. Borrowed handles are
     invalidated when `call_sync` returns; later use throws.
   - This makes "destroy the window from its own `close-request`" safe (deferred) and keeps
     GTK's emission/Qt's `QCoreApplication::notify` stack free of tree mutations. Probes show
     both toolkits tolerate it anyway (`box.remove(self)` inside `clicked`; `deleteLater`
     inside an event filter), but we do not rely on that.
   - **Ordering:** before running a sync handler, the host delivers already-queued
     notifications (checkpoint step 1 only, no effects/flush), so JS sees native order
     (e.g. the last `notify::text` before `close-request`).
   - **Nesting:** a sync emission can arrive while JS is on the stack (JS applied an op ->
     toolkit emitted synchronously, e.g. `window.close()` -> `close-request`). QuickJS allows
     the nested `JS_Call`; Svelte is safe because the dispatch runs `without_reactive_context`.
     Because ops issued from JS in a checkpoint are applied directly in P (no batching),
     nested sync is possible; cap depth (8), and above it return the default with a diagnostic.
5. **Promise returned** (async handler): use whatever flags were set **before the first
   `await`** (they are already on the event object), ignore the Promise for the result, and
   in dev warn only if the signal's result is value-typed (C/D) and no flag/field was set.
   The idiomatic veto-then-confirm pattern then just works:

   ```svelte
   <Window onclose-request={async (e) => {
     if (!dirty) return;
     e.preventDefault();                 // window stays open (TRUE / ignore())
     if (await confirmDiscard()) win.destroy();   // runs at a later checkpoint
   }} />
   ```

   This is deliberately the opposite of gjs (Promise is truthy -> TRUE).
6. **Exceptions**: report through the source-mapped diagnostic path and return the
   metadata's `default_result` (the value that equals "no JS handler": FALSE / don't filter /
   base implementation for virtuals).
7. **Timeouts**: the toolkit is blocked for the whole call; a hang freezes the UI. Use the
   QuickJS interrupt handler with a per-call budget: dev warns at 16 ms (one frame) and
   interrupts at 1 s; release interrupts at a configurable budget (default 250 ms). On
   interrupt, the exception unwinds JS, the host returns `default_result` and reports it.
   There is no meaningful "continue later" for a sync result.
8. **Category E/F are not call_sync users.** Models are native snapshots patched at
   checkpoints; size hints are props on generated subclasses. `call_sync` is for
   human/pointer-rate B/C/D only, which keeps its per-call overhead (event object
   allocation + dispatch) irrelevant.

### 1.6 Two-way binding and echo suppression

**Svelte constraint (checked against the 5.57.2 compiler):** `bind:value` on any element other
than `input/textarea/select` is a compile error (`bind_invalid_target`), and unknown names
like `bind:text` on a custom element are `bind_invalid_name`. **Native two-way binding must go
through generated wrapper components** with `$bindable()` props:

```svelte
<!-- generated Entry.svelte -->
<script>
  let { text = $bindable(''), ...rest } = $props();
</script>
<gtk-entry {...rest} text={text} onnotify:text={(e) => (text = e.currentTarget.text)} />
```

(`onnotify:text` compiles to `$.event('notify:text', ...)`; the colon form warns
`attribute_illegal_colon` but works; a generated name such as `onnotify-text` avoids the warning.)

Native change signals per property come from metadata: GTK `notify::<prop>` (`ParamSpec`
`EXPLICIT_NOTIFY`), Qt `NOTIFY` of `Q_PROPERTY` (`textChanged`, `valueChanged`,
`currentIndexChanged`), with a curated preference for *user-edit* signals where they exist
(Qt `textEdited` vs `textChanged`, `QAbstractSlider::sliderMoved`/`actionTriggered`).

Probed echo behaviour (`gtk_sync_probe.py`, `qt_sync_probe.py`):

```text
GTK Entry  set_text x, x, (blocked) y, y  -> notify::text ['x', 'x', 'y']   <- re-emits on identical set!
GTK Label  set_label x, x                 -> 1 notify (EXPLICIT_NOTIFY + equality check)
GTK SpinButton set_value 5,5,500 (max 99) -> value-changed 5, 99 (clamped value is emitted)
GTK Switch set_active(True)               -> state-set True, then notify::active   <- sync signal emitted by a programmatic set
Qt QLineEdit setText x, (blocked) y, y    -> textChanged ['x']   (unchanged set does not emit; textEdited never)
Qt QSpinBox  setValue 5,5,500 (max 99)    -> valueChanged [5, 99]
own writable props on Gtk widget classes: 581, EXPLICIT_NOTIFY: 498
```

Rules:

1. When the renderer sets a bound property, it brackets the native set with
   `g_signal_handler_block(obj, our_notify_handler_id)` / `QSignalBlocker`-equivalent. Block
   **our** handler id, never `blockSignals(true)` on Qt (that would silence the app's other
   connections and accessibility/semantic observers). On Qt, use a per-handle
   "suppress (prop)" flag checked by our connector instead of `QSignalBlocker`, for the same reason.
2. After the set, **read back** the property. If it differs from what JS wrote (clamping,
   normalisation, validator rejection: `SpinButton` 500 -> 99), post one synthetic change
   event so `$bindable` converges to the native value. Do not rely on the echo for this.
3. Compare before setting (`value !== current`) in the shim; GtkEntry/GtkText re-notify on
   identical text and would also move the cursor.
4. Programmatic sets can emit **sync** signals (`Switch::state-set`). Our closures must not
   call into JS for emissions caused by our own set: block the sync handler too while applying
   a host op (the "applying ops" flag doubles as the echo guard).
5. Prefer the user-intent signal where metadata has one (`textEdited`); then no blocking is
   needed for that property at all.
6. GTK `freeze_notify`/`thaw_notify` is useful when a checkpoint sets several props on one
   object, but not for suppression: notifications are delivered at thaw.

## Part 2. Child slots

### 2.1 Qt catalogue

Probed with `scripts/qt_slot_ownership_probe.py` (PySide6 6.11, offscreen). "Replace" = set A,
then set B, then flush `DeferredDelete`. "Clear" = setter with `nullptr`.

**Single slots (setter/getter pairs)**

| Owner.slot | Content | Default | Replace: old child | Clear (`nullptr`) | Notes |
|---|---|---|---|---|---|
| QMainWindow.central | widget | none | **deleted** | ok, old deleted | `takeCentralWidget()` removes without deleting |
| QMainWindow.menuBar | QMenuBar | **lazy**: `menuBar()` creates one | deleted (incl. lazy default) | getter recreates a default | no take; `setMenuWidget` (any widget) shares the slot, same semantics |
| QMainWindow.statusBar | QStatusBar | lazy, like menuBar | deleted | getter recreates | |
| QDockWidget.widget | widget | none | **kept** as child of the dock | ok | |
| QDockWidget.titleBarWidget | widget | none (native title bar) | kept as child | ok, native title restored | |
| QScrollArea.widget | widget | none | **deleted** | **ignored** (no-op) | remove via `takeWidget()`; child is parented to the viewport |
| QAbstractScrollArea.viewport | widget | builtin `QWidget` | deleted (incl. default) | creates a fresh default viewport | |
| QAbstractScrollArea.cornerWidget | widget | none | kept as child | ok | |
| QAbstractScrollArea.h/vScrollBar | QScrollBar | builtin | deleted | ignored | `addScrollBarWidget(w, alignment)` is a separate many-slot (keyed by alignment) |
| QComboBox.lineEdit | QLineEdit | **builtin when `editable`** | deleted (incl. default) | ignored (warning) | precondition `editable=true`; `setEditable(false)` deletes it, `setEditable(true)` recreates default |
| QComboBox.view | QAbstractItemView | builtin QListView | deleted | ignored | no API restores the default: create a new QListView |
| QComboBox.model | model | builtin QStandardItemModel (child) | deleted only if parent == combo (default yes, JS model no) | rejected | non-widget |
| QComboBox / QAbstractItemView .itemDelegate | delegate | builtin | not deleted, not owned | combo rejects, view ok | non-widget, reference |
| QTableView.horizontalHeader / verticalHeader, QTreeView.header | QHeaderView | builtin | deleted | ignored | |
| QAbstractItemView.model | model | none | not deleted | ok | non-widget, reference |
| QAbstractItemView.indexWidget(index) | widget | none | **deleted** | - | keyed by `QModelIndex` (row/col); belongs to model-driven views (SVR16) |
| QTabWidget.tabBar | QTabBar | builtin | deleted | **segfaults** (no null check) | protected in Qt 5, public in 6 |
| QTabWidget.cornerWidget(corner) | widget | none | kept as child | ok | keyed by `Qt::Corner` (2 slots) |
| QToolButton.menu / QPushButton.menu | QMenu | none | not owned | ok | non-widget-ish (top-level popup); reference |
| QLabel.buddy | widget | none | reference | ok | **reference, not a slot** |
| QWidget.layout | QLayout | none (QGroupBox/QFrame too) | **refused** if a layout exists (warning, old kept) | - | non-widget container: widgets go *into* the layout |

**Multi slots (insert/remove families)**

| Owner.slot | Cardinality / order | Insert | Remove: child afterwards | Per-child attrs |
|---|---|---|---|---|
| QMainWindow.toolbars | many, ordered **per area** | `addToolBar(area, w)`, `insertToolBar(before, w)`, `addToolBarBreak(area)`/`insertToolBarBreak(before)` | `removeToolBar`: kept as child, hidden | `area` (read back `toolBarArea(w)`), `break_before` |
| QMainWindow.docks | many, positional | `addDockWidget(area, w[, orientation])`, `splitDockWidget(a, b, o)`, `tabifyDockWidget(a, b)` | `removeDockWidget`: kept as child | `area` (read back `dockWidgetArea`), orientation, tab group |
| QSplitter | many, native index | `insertWidget(i, w)`, `replaceWidget(i, w)` (returns old, unparented) | no remove op: `setParent(nullptr)` / `hide` | stretch factor (`setStretchFactor(i, s)`), collapsible(i), sizes (parent-level) |
| QStackedWidget | many, native index | `insertWidget(i, w)` | `removeWidget`: kept as child | - (`currentIndex` is parent state) |
| QTabWidget.pages | many, native index | `insertTab(i, w, icon, label)` | `removeTab(i)`: kept as child of the internal stack | `title`, `icon`, `toolTip`, `whatsThis`, `enabled`, `visible` (all `setTabX(i, v)`) |
| QToolBox.items | many, native index | `insertItem(i, w, icon, text)` | `removeItem(i)`: kept as child | `text`, `icon`, `toolTip`, `enabled` |
| QWizard.pages | many, **keyed by int id**, order = id | `setPage(id, page)` / `addPage` | `removePage(id)`: kept as child | id; `QWizardPage` props are on the page |
| QMdiArea | many, unordered | `addSubWindow(w)` returns QMdiSubWindow wrapper | `removeSubWindow` | wrapper geometry |
| QLineEdit.actions | many, keyed by position | `addAction(action, LeadingPosition/TrailingPosition)` | `removeAction` | position; action **not owned** |
| QWidget.actions (context menus, QMenu, QToolBar) | many, ordered | `insertAction(before, a)` | `removeAction` | - (non-widget QAction; QToolBar also `insertWidget(before, w)` wraps a widget in an action) |
| QBoxLayout | many, native index, **mixed items** | `insertWidget(i, w, stretch, alignment)`, `insertLayout`, `insertStretch(i, s)`, `insertSpacing(i, px)` | `removeWidget`: still child of the parent widget and **still visible** | `stretch`, `alignment` (`setStretch(i, s)`, `setAlignment(w, a)`) |
| QGridLayout | positional grid | `addWidget(w, row, col, rowSpan, colSpan, align)`, `addLayout` | `removeWidget` (as above) | row/col/spans (read back `getItemPosition`, verified `(1,2,2,3)`); no move op -> remove + re-add; parent-level `rowStretch`, `columnStretch`, `rowMinimumHeight` |
| QFormLayout | rows of (label, field) pairs | `insertRow(r, label, field)` (label may be a string), `setWidget(r, role, w)` | `removeRow(r)` **deletes both widgets**; `takeRow(r)` keeps them | row, role (`LabelRole/FieldRole/SpanningRole`) |
| QStackedLayout | as QStackedWidget | | | |

**Takeaways:** the same verb ("set") means *delete old* (QMainWindow, QScrollArea, viewport,
headers, tab bar, combo line edit/view), *keep old as hidden child* (QDockWidget, corner
widgets), or *reference* (menus, delegates, models, buddy). Several clears are no-ops or crash.
Removal from layouts leaves the widget parented and visible. None of this is in the
QMetaObject; the PySide typesystem has part of it (`replace_child`, `<parent action=...>`),
so every op needs an annotated effect and a test (2.5).

### 2.2 GTK4 (+ Adw) catalogue

Probed with `scripts/gtk_slot_probe.py`. GTK has no delete-on-replace: the parent holds one
ref; replacing/removing **unparents** and drops it. The probe confirms the old child is
finalized when nobody else holds a ref (all 10 single slots tested). With svelters the host
handle holds a ref, so "old child survives until the JS node is dropped" uniformly; the
shim/host decides lifetime, not GTK.

| Owner.slot | Cardinality / order | Ops | Default / implicit content | Per-child attrs |
|---|---|---|---|---|
| `child` single-child widgets (Window, Button, Frame, Expander, Popover, ScrolledWindow, Viewport, Revealer, AspectFrame, ListBoxRow, FlowBoxChild, MenuButton, Adw.Bin/Clamp/StatusPage, ...) | one | prop `child` | Button/MenuButton: `label`/`icon-name` props build internal content that `child` replaces | - |
| ScrolledWindow.child | one | prop | **auto-wraps** non-`GtkScrollable` children in a `GtkViewport` (`get_child()` returns the Viewport) | - |
| Window.titlebar | one | prop `titlebar` | none (CSD draws an internal title bar) | - |
| Paned.start/end | one each | props `start-child`/`end-child` | - | `resize-start-child`, `shrink-start-child` (parent props keyed by slot) |
| CenterBox / ActionBar start/center/end | one each (CenterBox), many (ActionBar start/end) | props / `pack_start`, `pack_end`, `center-widget` | - | - |
| HeaderBar / Adw.HeaderBar | start, end: many, **append-only** (no insert-at, no reorder); title: one | `pack_start`, `pack_end`, `remove`, prop `title-widget` | title: none -> shows window title | - |
| Notebook.pages | many, native index | `insert_page_menu(child, tab, menu, pos)`, `reorder_child`, `remove_page(page_num(child))` | **tab label**: none -> GTK draws "Page N"; menu label: none -> derived from tab | `NotebookPage` (live props): `tab-expand`, `tab-fill`, `reorderable`, `detachable`, plus sub-slots `tab-label` / `menu-label` (widgets) |
| Notebook.action-start/end | one each | `set_action_widget(w, pack_type)` | - | - |
| Stack / Adw.ViewStack | many, insertion order, keyed by name | `add_named`, `add_titled`, `add_child`, `remove` (no insert-at, no reorder) | - | `StackPage`: `name`, `title`, `icon-name`, `needs-attention`, `visible`, `use-underline` |
| Overlay | `child` one + `overlays` many (z-order = add order) | prop / `add_overlay`, `remove_overlay` | - | `measure`, `clip` via **methods** `set_measure_overlay(w, b)`; `get-child-position` signal for placement |
| Grid | positional grid, unordered | `attach(w, col, row, w, h)`, `attach_next_to`, `remove` | - | `GridLayoutChild`: `column`, `row`, `column-span`, `row-span` (live: moving a child = set prop, no re-add) |
| Box | many, native order | `append`, `prepend`, `insert_child_after(w, sibling)`, `reorder_child_after`, `remove` | - | none (packing via child's `hexpand`/`halign`) |
| ListBox / FlowBox | many, native index | `append`, `insert(w, pos)`, `remove` | **auto-wraps** in `ListBoxRow`/`FlowBoxChild` | row props on the wrapper |
| Fixed | many, positional | `put(w, x, y)`, `move`, `remove` | - | `x`, `y` (or transform) |
| Expander / Frame label-widget | one | prop `label-widget` | **convenience prop clobbers the slot**: `set_label("y")` replaces a custom label widget with a new GtkLabel (probed) | - |
| MenuButton.popover / Adw.SplitButton | one | prop `popover` | `menu-model` (GMenuModel, non-widget) builds a `PopoverMenu` into the same slot; setting `popover` clears `menu-model` (probed) | - |
| Popover (attached popups) | one per anchor | `set_parent(anchor)` / `unparent` | - | `pointing-to`, `position` |
| TextView anchors | many, positional in text | `buffer.create_child_anchor(iter)` + `add_child_at_anchor(w, anchor)`; `add_overlay(w, x, y)`/`move_overlay` | - | text offset; anchor is a non-widget object |
| Widget.controllers | many, unordered | `add_controller`, `remove_controller` | widgets ship internal controllers (`observe_controllers` on a Label reported 2 after adding 1) | `propagation-phase`, `propagation-limit` are props of the controller itself |
| Widget.layout-manager | one, non-widget | prop | per-class default | - |
| ListView / GridView factory, DropDown factory/list-factory/header-factory | one, non-widget (`GtkListItemFactory`) | prop | - | the factory is a **template**, not a child: maps to a snippet with item params (v1 `listViewProxy` portal) |
| ColumnView.columns | many, native index, non-widget (`ColumnViewColumn`) | `append_column`, `insert_column(pos, c)`, `remove_column` | - | `title`, `expand`, `resizable`, `fixed-width`, `visible`, `sorter` + each column's own `factory` (snippet) |
| Adw.ApplicationWindow / Adw.Window | one | prop `content` (not `child`!) | - | - |
| Adw.ToolbarView | `top-bars` many, `bottom-bars` many, `content` one | `add_top_bar`, `add_bottom_bar`, `remove`, prop | - | parent props `top-bar-style`, `reveal-top-bars` |
| Adw.NavigationSplitView / OverlaySplitView | sidebar, content: one each | props (`NavigationPage` required for NavigationSplitView) | - | - |
| Adw.TabView | many, native index | `append`, `insert(child, pos)`, `reorder_page`, `close_page` | - | `TabPage`: `title`, `icon`, `indicator-icon`, `loading`, `needs-attention`, `tooltip` |
| Adw.ActionRow / EntryRow / ExpanderRow | prefixes, suffixes: many append-only; ExpanderRow also `rows` | `add_prefix`, `add_suffix`, `add_row`, `remove` | - | - |
| Adw.PreferencesDialog/Page/Group | pages -> groups -> rows, append-only | `add`, `remove` | - | page `title`, `icon-name` |
| Adw.NavigationView | stack, imperative | `push`, `pop`, `add` | - | not a declarative slot; expose as methods |

### 2.3 What Svelte 5 allows (checked with `svelte@5.57.2`)

Compiled in `$SCRATCH/svtest` (not kept):

| Form | Result |
|---|---|
| `<gtk-window><child:titlebar>...</child:titlebar></gtk-window>` (v1 namespaced tag) | compiles; with `fragments: 'tree'` it becomes `document.createElement('child:titlebar')` |
| `<gtk-header-bar><gtk-button slot="start"/></gtk-header-bar>` | compiles to `$.set_custom_element_data(btn, 'slot', 'start')` **before** `$.append` (static and `template_effect` alike), so the shim knows the slot when the child is attached |
| `slot="..."` under a non-hyphenated parent (`<headerbar>`) | **error** `slot_attribute_invalid_placement` (needs a component or custom-element parent) |
| `{#snippet start()}...{/snippet}` inside a native element | compiles, but it is only a local declaration: **nothing is passed to the element** and nothing renders |
| `bind:value` / `bind:text` on a custom element | **errors** (`bind_invalid_target`, `bind_invalid_name`) |
| `grid:row={0}` attribute | compiles (`set_custom_element_data(el, 'grid:row', 0)`), warns `attribute_illegal_colon` |
| `onclose-request={...}`, `onnotify:text={...}` | compile to non-delegated `$.event('close-request', ...)` |

So: snippets reach the backend only through **components**; `slot=""` works only with
hyphenated native tag names; whatever the surface, the runtime sees `createElement` +
attributes + `insertBefore`/`remove` on anchors (comments). Slot routing must be done by the
shim from (a) a slot-proxy element kind, (b) the child's `slot` attribute, (c) the child's
kind, (d) the parent's default slot, in that order.

### 2.4 Svelte-facing surface: options and recommendation

| Option | Pros | Cons |
|---|---|---|
| Snippet props on generated wrapper components (`<HeaderBar>{#snippet title()}...{/snippet}</HeaderBar>`) | idiomatic Svelte 5; typed (`title?: Snippet`) in generated `.d.ts`; absence/presence maps cleanly to "keep default"/"replace" | wrapper component per kind (generated, cheap); many-slots inside `{#each}` read less naturally |
| `slot="name"` on native children | DOM-like; works in `{#each}`; zero wrappers | needs hyphenated parent tags; untyped string; Svelte 4 connotation |
| Namespaced tags (`<child:titlebar>`, v1) | proven in v1; explicit; a real proxy node to hang attrs on | `attribute_illegal_colon`-style oddness, untyped, unfamiliar |
| Wrapper item components (`<Tab title="x">`, `<GridCell row col>`, `<Dock area="left">`) | the natural home for **per-child attributes** and per-item sub-slots (Notebook tab label) | one more nesting level |

**Recommendation: one runtime primitive, two authoring surfaces.**

1. **Runtime primitive = slot proxy nodes.** The shim defines non-native element kinds
   `sv-slot` (attrs: `name`) and `sv-item` (attrs: `slot`, plus attached props). They create no
   widget. Widget descendants of an `sv-slot` are routed to the parent's slot `name`; an
   `sv-item` contributes exactly one item (its first native child, plus its own nested
   `sv-slot`s for sub-slots such as `tab-label`) with its attributes as attached props. The
   `slot="x"` attribute on a native child and the v1 `<child:x>` tag are normalised to the same
   (slot, attrs) record. Routing order: proxy -> `slot` attr -> **kind routing** (a child whose
   kind is accepted by exactly one slot: controllers, QMenu, QAction, QLayout, QStatusBar,
   QMenuBar, QToolBar, QDockWidget, like uic does for QMainWindow) -> default slot.
2. **Authoring surface A (primary): generated wrapper components.**
   - Named single slots -> snippet props:
     `<Window>{#snippet titlebar()}<HeaderBar/>{/snippet}...</Window>`, rendered by the
     wrapper as `{#if titlebar}<sv-slot name="titlebar">{@render titlebar()}</sv-slot>{/if}`.
   - Slots with per-child attributes -> item components: `<Notebook><Page title="One">
     {#snippet tab()}<Label .../>{/snippet} ...content... </Page></Notebook>`,
     `<Grid><Cell row={0} col={1} colSpan={2}><Entry/></Cell></Grid>`,
     `<MainWindow><ToolBar area="left" breakBefore/><Dock area="right">...</Dock></MainWindow>`.
   - Template slots (factories, columns) -> snippets with parameters:
     `<ListView model={m}>{#snippet row(item)}...{/snippet}</ListView>`,
     `<ColumnView><Column title="Name">{#snippet cell(item)}...{/snippet}</Column></ColumnView>`.
3. **Authoring surface B (escape hatch, no wrappers):** hyphenated native tags with `slot=""`,
   e.g. `<gtk-header-bar><gtk-button slot="start"/></gtk-header-bar>`; and kind routing for
   non-widget children (`<gtk-event-controller-key onkey-pressed={...}/>` under any widget;
   v1's `<controller:motion>` becomes unnecessary).
4. **Replace-default vs fill-empty** is a per-slot metadata fact (`default.kind`): `none`
   (fill), `builtin` (exists at construction: QComboBox line edit/view, headers, viewport,
   tab bar), `lazy` (QMainWindow menu/status bar), `convenience` (built from props:
   Expander `label`, MenuButton `menu-model`, Notebook "Page N"). Absent snippet = do not
   touch. Present = replace. Snippet goes away = run the slot's `restore` op (recreate builtin;
   clear for `none`; for `convenience`, re-apply the prop). Conflicts between a convenience
   prop and the slot (Expander `label` vs `label-widget`) are declared so the wrapper drops the
   prop while the snippet is present.
5. **Non-widget contents.** Controllers, actions, menus, layouts and columns are elements
   (they have props, signals and identity); models and delegates are **props** holding JS
   objects/handles (`model={m}`), never children; factories are snippets. QLayouts are a
   transparent container element: `<q-widget><q-vbox-layout>...children...</q-vbox-layout></q-widget>`,
   and the generated `QGroupBox`/`QFrame` wrappers default to an implicit `QVBoxLayout` with a
   `layout="grid|form|hbox"` prop.
6. **Removal**: per-slot `on_remove`: `restore_default` (builtin slots), `empty` (fill slots),
   and for Qt ops that *delete* on remove (`QFormLayout::removeRow`, `setMenuBar(nullptr)`,
   `setCentralWidget(nullptr)`), the op is declared `destroys_child: true`; the shim then
   treats the JS node as dead (it is anyway: Svelte never reinserts a removed node into another
   parent except via the same component's anchors; slot changes are remove + recreate).
7. **ABI consequence** (answers the "Slot routing" open question in `backend-abi.md`):
   `insert(parent, child, before, slot)` is enough for placement if `before` is the next
   *native sibling in the same slot* (the shim computes it by skipping comments, text and
   other slots), but attached props need their own op:
   `set_child_attr(parent, child, slot, attr_id, value)` (GTK: `GridLayoutChild`/`NotebookPage`/
   `StackPage` props or methods; Qt: `setTabText(i)`, re-add for grid moves). Item proxies'
   initial attrs ride on `insert` as an attrs list to avoid insert-then-move.

### 2.5 Slot annotation format (agent-written, generator-verified)

> **Requested format.** Per `plans/architecture/widget-metadata.md` ("Annotation loop"), the
> native operations of each slot are **manual metadata written by agents** and verified by
> generated headless tests. This section is that annotation format. It replaces the
> `children.slots[].ops` sketch in `widget-metadata-ingestion.md` §4 and keeps its field
> names (`policy`, `default_slot`, `slots[].name/cardinality/accepts/attached_props`), so a
> v0 entry is still a valid subset. File: `metadata/overrides/<toolkit>/<Kind>.yaml`, key `children`.

#### 2.5.1 Shape

```yaml
children:
  policy: named_slots            # none | single_child | ordered | named_slots | layout_managed | pages | model_driven
  default_slot: central          # where unrouted children go (null = error)
  slots:
    - name: <slot>               # Svelte-facing name (snippet prop / slot attr / sv-slot name)
      content: widget            # widget | menu | action | layout | controller | column | page_object | factory
      accepts: [QWidget]         # native kinds; also drives kind routing (unique acceptor wins)
      route_by_kind: false       # true: a child of an accepted kind lands here without slot="..."
      cardinality: one           # one | many
      order: none                # one: none | many: native_index | before_sibling | append_only | per_key | unordered
      key: null                  # many+per_key: attr whose value partitions order (QMainWindow toolbar `area`)
      default:                   # what is in the slot before anyone fills it
        kind: none               # none | builtin | lazy | convenience
        restore: null            # op name used when the slot is emptied (see ops)
        conflicts_with_props: [] # convenience props that overwrite this slot (Expander `label`)
      on_remove: empty           # empty | restore_default | destroys_child
      attrs: []                  # per-child attributes (attached props), see 2.5.2
      requires: []               # preconditions, see 2.5.3
      ops: {}                    # op descriptors, see 2.5.2
      verify: {}                 # extra test steps beyond the derived ones, see 2.5.4
      provenance: [{origin: manual|uic|typesystem|qdoc|gir|probe, ref: "..."}]
```

#### 2.5.2 Op descriptors

Op names are fixed; the generator maps them to the backend vtable (`insert`, `remove`,
`set_child_attr`) and to tests. Every slot must define `get` plus enough to insert and remove.

| Op | When the shim uses it | Required for |
|---|---|---|
| `set` | insert into a `one` slot | `one` |
| `insert` | insert into a `many` slot | `many` |
| `remove` | Svelte removes the node | all |
| `replace` | `one` slot, new child while one is present (if absent: `remove` + `set`) | optional |
| `get` | read back the current child / list; used by tests and by `bind:this` on slots | all |
| `list` | enumerate children of a `many` slot in native order | `many` |
| `clear` | empty a `one` slot (if absent: `remove`) | optional |
| `restore` | put the builtin/lazy default back | `default.kind != none` |
| `move` | reorder within the slot (if absent: `remove` + `insert`) | optional |
| `attr_get` / `attr_set` | per attribute in `attrs[]`; if `attr_set` is absent, a change is `remove` + `insert` | per attr |

Descriptor:

```yaml
insert:
  native: "QMainWindow::addToolBar(Qt::ToolBarArea, QToolBar*)"   # qdoc/GIR symbol; validated against the API DB
  # alternatives; the first whose `when` holds is used
  variants:
    - when: "before != null && attr(before, area) == attr(child, area)"
      native: "QMainWindow::insertToolBar(QToolBar* before, QToolBar* toolbar)"
      args: [before, child]
  args: [attr.area, child]       # argument expressions, see below
  returns: ignore                # ignore | child | handle:<name> (e.g. Notebook page index, StackPage, TabPage)
  effects:                       # ownership/state effects, each becomes a test assertion
    child: parented              # parented | wrapped:<Kind> | referenced | unchanged
    old: null                    # for set/replace: deleted | kept_child | unparented | unref | referenced_only
    default: deleted             # for builtin/lazy defaults: deleted | kept_child | null
  emits: []                      # native signals emitted synchronously (echo-guard + tests), e.g. [currentChanged]
```

Argument expressions: `self`, `child`, `before` (next native sibling in the same slot, or null),
`index` (native index computed by the shim among same-slot siblings), `attr.<name>` (child's
attribute, already converted with the attr's type/enum map), `item.<name>` (a handle returned
by an earlier op, e.g. `item.page` = `NotebookPage`), `const:<literal>`, `null`. Expressions
are the only "code" in the file; the generator turns them into C++ (Qt) or GI calls (GTK).

Attribute record:

```yaml
attrs:
  - name: area                   # Svelte attr name on the child / sv-item (camelCase in .d.ts)
    type: enum<Qt.ToolBarArea>
    js_values: {top: TopToolBarArea, left: LeftToolBarArea, right: RightToolBarArea, bottom: BottomToolBarArea}
    default: top
    get: {native: "QMainWindow::toolBarArea(QToolBar*) const", args: [child]}
    set: null                    # null: change = remove + insert
```

#### 2.5.3 Preconditions

```yaml
requires:
  - check: "QWidget::layout(self) != null"      # boolean expression over self/child
    fix: {native: "new QGridLayout(self)"}        # optional auto-fix the backend applies once
    otherwise: error                              # error | warn | apply_fix
```

Examples: QWidget `layout`-managed slots need a layout; QComboBox `line_edit` needs
`editable=true` (`fix: setEditable(true)`); `QAbstractItemView.index_widget` needs a model;
GtkNotebook `tab-label` sub-slot needs the page to exist (`requires: item.page != null`);
Adw.NavigationSplitView `sidebar` accepts only `AdwNavigationPage`.

#### 2.5.4 Verification recipe

The generator **derives** a test per op from `effects` (the "Annotation loop" defaults: insert,
read back via `get`, remove, check default restored or empty; ownership via `QPointer` /
GObject weak ref after `DeferredDelete` / main-context drain). `verify` only adds steps:

```yaml
verify:
  fixture: "QMainWindow w; w.show();"        # optional; default = construct self, show
  steps:                                     # small step DSL, translated to C++ (Qt) or Python/GI (GTK)
    - new: A as QToolBar
    - op: insert {child: A, attr.area: left}
    - expect: "attr_get(A, area) == left"
    - op: insert {child: B, attr.area: left, before: A}
    - expect: "list(area=left) == [B, A]"
    - op: remove {child: A}
    - expect: "alive(A) && parent(A) == self && !visible(A)"
```

Predicates: `get()`, `list(...)`, `alive(x)` (weak ref / `QPointer`), `parent(x)`,
`visible(x)`, `attr_get(x, name)`, `emitted(signal)`. Tests run under Xvfb/offscreen; a
failure goes back to the agent with output (plan step 3). The probe scripts in this directory
are prototypes of these tests (`qt_slot_ownership_probe.py`, `gtk_slot_probe.py`).

#### 2.5.5 Worked example: QMainWindow

```yaml
kind: QMainWindow
children:
  policy: named_slots
  default_slot: central
  slots:
    - name: central
      content: widget
      accepts: [QWidget]
      cardinality: one
      default: {kind: none}
      on_remove: empty
      ops:
        get:    {native: "QMainWindow::centralWidget() const"}
        set:    {native: "QMainWindow::setCentralWidget(QWidget*)", args: [child],
                 effects: {child: parented, old: deleted}}         # probe: old DELETED
        remove: {native: "QMainWindow::takeCentralWidget()",          # NOT setCentralWidget(nullptr): that deletes
                 effects: {child: unparented}}
        # no `replace`: shim does remove (take) + set, so the old JS node survives if Svelte reuses it
      provenance: [{origin: uic, ref: "WriteInitialization::acceptWidget"}, {origin: typesystem, ref: replace_child},
                   {origin: probe, ref: qt_slot_ownership_probe.py}]
    - name: menubar
      content: widget
      accepts: [QMenuBar]
      route_by_kind: true
      cardinality: one
      default: {kind: lazy, restore: restore}
      on_remove: destroys_child
      ops:
        get:     {native: "QMainWindow::menuWidget() const"}       # menuBar() would create one
        set:     {native: "QMainWindow::setMenuBar(QMenuBar*)", args: [child], effects: {child: parented, old: deleted, default: deleted}}
        remove:  {native: "QMainWindow::setMenuBar(QMenuBar*)", args: [null], effects: {child: deleted}}
        restore: {native: "QMainWindow::setMenuWidget(QWidget*)", args: [null]}  # lazy default re-created on next menuBar()
    - name: statusbar        # same shape as menubar: setStatusBar / statusBar() lazy / setStatusBar(nullptr) deletes
      content: widget
      accepts: [QStatusBar]
      route_by_kind: true
      cardinality: one
      default: {kind: lazy}
      on_remove: destroys_child
      ops:
        get:    {native: "QMainWindow::findChild<QStatusBar*>(Qt::FindDirectChildrenOnly)"}
        set:    {native: "QMainWindow::setStatusBar(QStatusBar*)", args: [child], effects: {child: parented, old: deleted}}
        remove: {native: "QMainWindow::setStatusBar(QStatusBar*)", args: [null], effects: {child: deleted}}
    - name: toolbars
      content: widget
      accepts: [QToolBar]
      route_by_kind: true
      cardinality: many
      order: per_key
      key: area
      attrs:
        - {name: area, type: "enum<Qt.ToolBarArea>", default: top,
           js_values: {top: TopToolBarArea, bottom: BottomToolBarArea, left: LeftToolBarArea, right: RightToolBarArea},
           get: {native: "QMainWindow::toolBarArea(const QToolBar*) const", args: [child]}, set: null}
        - {name: breakBefore, type: bool, default: false,
           get: {native: "QMainWindow::toolBarBreak(QToolBar*) const", args: [child]},
           set: {native: "QMainWindow::insertToolBarBreak(QToolBar*)", args: [child], when: "value"}}   # clearing: removeToolBarBreak
      ops:
        list:   {native: "QMainWindow::findChildren<QToolBar*>(Qt::FindDirectChildrenOnly)", filter: "toolBarArea(x) == key"}
        insert:
          native: "QMainWindow::addToolBar(Qt::ToolBarArea, QToolBar*)"
          args: [attr.area, child]
          variants:
            - when: "before != null"
              native: "QMainWindow::insertToolBar(QToolBar*, QToolBar*)"
              args: [before, child]
          effects: {child: parented}
        remove: {native: "QMainWindow::removeToolBar(QToolBar*)", args: [child],
                 effects: {child: kept_child_hidden}}            # probe: still child of the window
      verify:
        steps:
          - new: A as QToolBar
          - op: insert {child: A, attr.area: left}
          - expect: "attr_get(A, area) == left"
          - op: remove {child: A}
          - expect: "alive(A) && parent(A) == self && !visible(A)"
    - name: docks
      content: widget
      accepts: [QDockWidget]
      route_by_kind: true
      cardinality: many
      order: unordered       # position = area + split/tabify, not an index
      attrs:
        - {name: area, type: "enum<Qt.DockWidgetArea>", default: left,
           get: {native: "QMainWindow::dockWidgetArea(QDockWidget*) const", args: [child]},
           set: {native: "QMainWindow::addDockWidget(Qt::DockWidgetArea, QDockWidget*)", args: [value, child]}}  # re-adding moves it
        - {name: tabifyWith, type: "ref<QDockWidget>", default: null,
           set: {native: "QMainWindow::tabifyDockWidget(QDockWidget*, QDockWidget*)", args: [value, child]}}
      ops:
        insert: {native: "QMainWindow::addDockWidget(Qt::DockWidgetArea, QDockWidget*)", args: [attr.area, child],
                 effects: {child: parented}}
        remove: {native: "QMainWindow::removeDockWidget(QDockWidget*)", args: [child], effects: {child: kept_child_hidden}}
        list:   {native: "QMainWindow::findChildren<QDockWidget*>(Qt::FindDirectChildrenOnly)"}
```

Svelte: `<MainWindow>{#snippet menubar()}<MenuBar>...</MenuBar>{/snippet}<ToolBar area="left"/>
<Dock area="right" title="Props">...</Dock><Editor/></MainWindow>` (toolbars/docks by kind,
`Editor` to `central` as default slot).

#### 2.5.6 Worked example: QComboBox (replaceable built-ins)

```yaml
kind: QComboBox
children:
  policy: named_slots
  default_slot: null           # items come from `model`/`items` props, not children
  slots:
    - name: lineEdit
      content: widget
      accepts: [QLineEdit]
      cardinality: one
      default: {kind: builtin, restore: restore}       # exists iff editable
      on_remove: restore_default
      requires:
        - {check: "QComboBox::isEditable(self)", fix: {native: "QComboBox::setEditable(bool)", args: [const:true]}, otherwise: apply_fix}
      ops:
        get:     {native: "QComboBox::lineEdit() const"}
        set:     {native: "QComboBox::setLineEdit(QLineEdit*)", args: [child],
                  effects: {child: parented, old: deleted, default: deleted}}   # probe: default and A both DELETED
        remove:  {native: "QComboBox::setLineEdit(QLineEdit*)", args: [null], effects: {}}  # ignored by Qt (warning) ...
        restore:                                       # ... so removal = drop and recreate the default:
          sequence:
            - {native: "QComboBox::setEditable(bool)", args: [const:false]}   # deletes the current line edit
            - {native: "QComboBox::setEditable(bool)", args: [attr_of_self.editable]}
          effects: {child: deleted}
      verify:
        steps:
          - op: set {child: A}            # A = new QLineEdit
          - expect: "get() == A && !alive(default)"
          - op: restore
          - expect: "!alive(A) && (get() != null) == self.editable"
    - name: view
      content: widget
      accepts: [QAbstractItemView]
      cardinality: one
      default: {kind: builtin, restore: restore}
      on_remove: restore_default
      ops:
        get:     {native: "QComboBox::view() const"}
        set:     {native: "QComboBox::setView(QAbstractItemView*)", args: [child],
                  effects: {child: "wrapped:QComboBoxPrivateContainer", old: deleted, default: deleted}}
        restore: {native: "QComboBox::setView(QAbstractItemView*)", args: ["new:QListView"], effects: {child: deleted}}
  # not slots: `model` and `itemDelegate` are props of type ref<QAbstractItemModel>/ref<QAbstractItemDelegate>.
props_overrides:
  model:        {role: reference, replace_effect: "old deleted iff parent(old) == self"}   # probe
  itemDelegate: {role: reference, replace_effect: "old kept, not owned"}
```

Svelte: `<ComboBox editable>{#snippet lineEdit()}<LineEdit placeholder="..."/>{/snippet}</ComboBox>`.
Snippet removed -> `restore` (default line edit back). Note `setView` re-parents the view into an
internal popup container, so `effects.child` is `wrapped:` and the test checks
`parent(parent(A))`-style ancestry rather than `parent(A) == self`.

#### 2.5.7 Worked example: grid cells (QGridLayout and GtkGrid)

```yaml
kind: QGridLayout              # a layout kind; reached via <q-widget><q-grid-layout>... or <Grid> wrapper
children:
  policy: layout_managed
  default_slot: cells
  slots:
    - name: cells
      content: widget          # QLayout also accepted via addLayout variant
      accepts: [QWidget, QLayout]
      cardinality: many
      order: unordered
      requires:
        - {check: "QLayout::parentWidget(self) != null", otherwise: error}   # layout must be installed on a widget
      attrs:
        - {name: row,     type: i32, default: 0, get: {native: "QGridLayout::getItemPosition(int,int*,int*,int*,int*)", args: [index_of_child, out.row, _, _, _]}, set: null}
        - {name: column,  type: i32, default: 0, get: {native: "...getItemPosition", out: column}, set: null}
        - {name: rowSpan, type: i32, default: 1, get: {native: "...getItemPosition", out: rowSpan}, set: null}
        - {name: colSpan, type: i32, default: 1, get: {native: "...getItemPosition", out: colSpan}, set: null}
        - {name: align,   type: "flags<Qt.Alignment>", default: 0,
           set: {native: "QLayout::setAlignment(QWidget*, Qt::Alignment)", args: [child, value]}}
      ops:
        insert:
          native: "QGridLayout::addWidget(QWidget*, int, int, int, int, Qt::Alignment)"
          args: [child, attr.row, attr.column, attr.rowSpan, attr.colSpan, attr.align]
          variants:
            - when: "is_layout(child)"
              native: "QGridLayout::addLayout(QLayout*, int, int, int, int, Qt::Alignment)"
              args: [child, attr.row, attr.column, attr.rowSpan, attr.colSpan, attr.align]
          effects: {child: "parented_to:parentWidget(self)"}
        remove:
          native: "QLayout::removeWidget(QWidget*)"
          args: [child]
          then: [{native: "QWidget::setParent(QWidget*)", args: [child, null]}]   # probe: otherwise stays visible in the parent
          effects: {child: unparented}
        list:  {native: "QLayout::itemAt(int)", iterate: count}
  parent_attrs:                # props on the layout itself keyed by row/col (Svelte: rowStretch={[0,1,0]})
    - {name: rowStretch,    type: "list<i32>", set: {native: "QGridLayout::setRowStretch(int,int)", each: [index, value]}}
    - {name: columnStretch, type: "list<i32>", set: {native: "QGridLayout::setColumnStretch(int,int)", each: [index, value]}}
---
kind: GtkGrid
children:
  policy: layout_managed
  default_slot: cells
  slots:
    - name: cells
      content: widget
      accepts: [GtkWidget]
      cardinality: many
      order: unordered
      attrs:                   # live layout-child props: a move is attr_set, not remove+insert
        - {name: column,     type: i32, default: 0, layout_child: GtkGridLayoutChild, prop: column}
        - {name: row,        type: i32, default: 0, layout_child: GtkGridLayoutChild, prop: row}
        - {name: columnSpan, type: i32, default: 1, layout_child: GtkGridLayoutChild, prop: column-span}
        - {name: rowSpan,    type: i32, default: 1, layout_child: GtkGridLayoutChild, prop: row-span}
      ops:
        insert: {native: gtk_grid_attach, args: [self, child, attr.column, attr.row, attr.columnSpan, attr.rowSpan],
                 effects: {child: parented}}
        remove: {native: gtk_grid_remove, args: [self, child], effects: {child: unparented}}
        get:    {native: gtk_grid_get_child_at}
      verify:
        steps:
          - op: insert {child: A, attr.column: 2, attr.row: 1, attr.columnSpan: 3, attr.rowSpan: 2}
          - expect: "query_child(A) == (2, 1, 3, 2)"      # probe: gtk_grid_query_child
          - op: attr_set {child: A, column: 0}
          - expect: "query_child(A).column == 0 && parent(A) == self"
```

`layout_child:` is shorthand the generator expands to
`gtk_layout_manager_get_layout_child(gtk_widget_get_layout_manager(self), child)` + `g_object_set`.
Svelte: `<Grid><Cell row={0} col={1} colSpan={2}><Entry/></Cell></Grid>` (the `Cell` wrapper
renders `<sv-item slot="cells" row=... column=...>`), or `<gtk-entry grid:row={0}>` without wrappers.

#### 2.5.8 Worked example: GtkNotebook pages with tab/menu label sub-slots (+ HeaderBar)

```yaml
kind: GtkNotebook
children:
  policy: pages
  default_slot: pages
  slots:
    - name: pages
      content: widget
      accepts: [GtkWidget]
      cardinality: many
      order: native_index
      item:                    # each item = one page child + item-scoped attrs and sub-slots (<Page> / sv-item)
        handle: {native: gtk_notebook_get_page, args: [self, child], returns: GtkNotebookPage}
        attrs:
          - {name: title,       type: string, default: null,
             set: {native: gtk_notebook_set_tab_label_text, args: [self, child, value]},
             conflicts_with_slot: tab}       # a tab snippet wins; title then only feeds accessibility/menu
          - {name: tabExpand,   type: bool, default: false, item_prop: tab-expand}
          - {name: reorderable, type: bool, default: false, item_prop: reorderable}
          - {name: detachable,  type: bool, default: false, item_prop: detachable}
        slots:
          - name: tab
            content: widget
            cardinality: one
            default: {kind: convenience, restore: restore}     # GTK draws "Page N" / title text
            on_remove: restore_default
            requires: [{check: "gtk_notebook_page_num(self, item.child) >= 0", otherwise: error}]
            ops:
              get:     {native: gtk_notebook_get_tab_label, args: [self, item.child]}
              set:     {native: gtk_notebook_set_tab_label, args: [self, item.child, child], effects: {old: unref}}
              restore: {native: gtk_notebook_set_tab_label_text, args: [self, item.child, item.attr.title]}  # null -> "Page N"
          - name: menu
            content: widget
            cardinality: one
            default: {kind: convenience}
            on_remove: restore_default
            ops:
              get: {native: gtk_notebook_get_menu_label, args: [self, item.child]}
              set: {native: gtk_notebook_set_menu_label, args: [self, item.child, child]}
              restore: {native: gtk_notebook_set_menu_label, args: [self, item.child, null]}
      ops:
        insert: {native: gtk_notebook_insert_page_menu, args: [self, child, null, null, index],
                 returns: ignore, effects: {child: parented}, emits: [page-added, switch-page]}
        move:   {native: gtk_notebook_reorder_child, args: [self, child, index], emits: [page-reordered]}
        remove: {native: gtk_notebook_remove_page, args: [self, "gtk_notebook_page_num(self, child)"],
                 effects: {child: unparented}, emits: [page-removed, switch-page]}
        list:   {native: gtk_notebook_get_pages}
    - {name: actionStart, content: widget, cardinality: one, default: {kind: none},
       ops: {set: {native: gtk_notebook_set_action_widget, args: [self, child, "const:GTK_PACK_START"]},
             get: {native: gtk_notebook_get_action_widget, args: [self, "const:GTK_PACK_START"]},
             remove: {native: gtk_notebook_set_action_widget, args: [self, null, "const:GTK_PACK_START"]}}}
---
kind: GtkHeaderBar
children:
  policy: named_slots
  default_slot: start
  slots:
    - {name: start, content: widget, cardinality: many, order: append_only,
       ops: {insert: {native: gtk_header_bar_pack_start, args: [self, child]},
             remove: {native: gtk_header_bar_remove, args: [self, child]}}}
    - {name: end,   content: widget, cardinality: many, order: append_only,
       ops: {insert: {native: gtk_header_bar_pack_end, args: [self, child]},
             remove: {native: gtk_header_bar_remove, args: [self, child]}}}
    - {name: title, content: widget, cardinality: one, default: {kind: convenience},   # window title text
       on_remove: restore_default,
       ops: {get: {prop: title-widget}, set: {prop: title-widget, args: [child]},
             restore: {prop: title-widget, args: [null]}}}
```

`order: append_only` tells the shim that an insert *before* an existing sibling in the same
slot needs a re-pack: remove the siblings after the insertion point and re-insert them in order
(the generated test checks the resulting order via `list`, which for HeaderBar is the start box's
children). The `emits` lists feed the echo guard (1.6) and the tests' `emitted(...)` checks.

Svelte: `<Notebook><Page title="Files">{#snippet tab()}<Box><Image icon="folder"/><Label label="Files"/></Box>{/snippet}<FileList/></Page></Notebook>`.

#### 2.5.9 Signals: matching annotation fields (Part 1)

For completeness, the per-signal fields Part 1 needs (extends `signals[]` in §4):

```yaml
signals:
  - name: close-request
    js_event: close-request
    delivery: sync                 # queued | sync
    accumulator: stop_on_true      # stop_on_true | stop_after_first | last_wins | n/a(qt); origin: probe
    result:
      kind: bool_stop              # none | bool_stop | value | out_params
      from: [preventDefault, stopPropagation]   # event flags that produce TRUE
      accept_return: false         # also honour `return true` (non-delegated names only)
      default: false               # no handler / exception / timeout
    borrowed_args: []              # e.g. [tooltip] for query-tooltip; handles invalid after return
    emitted_by_setters: []         # e.g. Switch::state-set <- set_active; drives the echo guard
    verify: {trigger: "gtk_window_close(self)", expect_with_prevent: "visible(self)", expect_without: "!visible(self)"}
  # Qt: events are pseudo-signals on the per-widget filter
  - name: close
    js_event: close
    native: {event_filter: QEvent::Close}
    delivery: sync
    result: {kind: bool_stop, from: [preventDefault], map: "ignore(); return true", default: "return false"}
```

## Open questions

1. **Deferred ops inside sync handlers vs P's direct application.** Rule 1.5.4 queues host ops
   issued from a sync scope. P applies ops directly at checkpoints. Is a second
   "deferred" queue acceptable before batching exists, or should P batch from the start?
2. **Delivering queued events before a sync handler** (1.5.4 "Ordering") can run arbitrary
   handlers in the middle of a native emission. The alternative, a sync handler seeing slightly
   stale state, is simpler. This needs a decision and a test with `notify::text` followed by
   `close-request`.
3. **Default for value-returning sync signals** (`DropTarget::enter`, `query-tooltip`) when no
   flag or field is set: is it "as if no handler", or the toolkit's class default? Is a
   per-signal `default` enough?
4. **Timeout budgets.** 16 ms warn and 1 s interrupt in dev are guesses. Interrupting JS
   mid-handler can leave app `$state` half-updated. Accept that, or make release builds only warn?
5. **Qt event filter cost.** One filter per widget with any sync event listener, or one
   application-wide filter (`qApp->installEventFilter`) that looks up handles? The app-wide
   filter sees every event of every object, but needs no per-widget install/uninstall.
6. **Keybinding `action` signals** (25 in Gtk): expose them as events, or only through the
   shortcut/action system (`GtkShortcutController`, `QAction`)?
7. **`destroys_child` removals** (`QFormLayout::removeRow`, `setMenuBar(nullptr)`,
   `setStatusBar(nullptr)`). Can Svelte's keyed `{#each}` ever move such a node out and back in?
   The shim assumes it can't (remove means dead). A fixture should confirm that, including
   transitions (`out:` keeps nodes alive after logical removal).
8. **QMainWindow menubar remove.** No Qt API takes the menu bar out without deleting it
   (`setParent(nullptr)` on a laid-out menu bar is not a supported path). Accept deletion, or
   hide it and keep it?
9. **Qt item-scoped attrs need indices** (`setTabText(i, ...)`). The backend has to map a child
   to its current index on every attr set (`indexOf`). Should backends keep a per-slot child
   list to avoid O(n), or is `indexOf` fine at the sizes we expect?
10. **Kind routing ambiguity.** A `QWidget` subclass accepted by several slots (e.g. a custom
    `QToolBar` subclass also accepted by `central`) needs a deterministic rule: the most-derived
    `accepts` match wins, and `default_slot` breaks ties. Does that match uic's choices for every
    QMainWindow child type?
11. **`GtkScrolledWindow` auto-viewport and `ListBox`/`FlowBox` row wrapping** add native nodes
    that the shim tree doesn't have. Effects like `wrapped:<Kind>` cover tests, but `bind:this`
    and semantics/accessibility need to know which handle is "the child". Recommendation: the
    JS-visible child is the user's widget, and the wrapper is internal.
12. **Wrapper component cost.** Generating one `.svelte` wrapper per element kind
    (~50 GTK + ~15 Adw + ~25 Qt with slots, more for `$bindable` props) affects bundle size.
    Generating wrappers only for kinds with slots or bindable props, and using bare native tags
    for the rest, is probably enough.

## Scripts

All in `docs/notes/scripts/`, stdlib + PyGObject/PySide6, run with `python3 -I`.

| Script | What it does |
|---|---|
| `gir_signal_returns.py [Gtk-4.0 Adw-1 Gdk-4.0] [--list] [--json]` | Non-void signals from GIR, grouped by return type, `when`, deprecation, `action` flag and out-params. Also counts vfuncs. |
| `gtk_signal_accumulator_probe.py [--json]` (under `xvfb-run -a`) | Runtime probe of each non-void signal's accumulator (`stop_on_true` / `stop_after_first` / `last_wins`). Uses one subprocess per signal and writes `gtk_signal_accumulator_probe.json`. |
| `gtk_sync_probe.py` (Xvfb) | Checks the `close-request` veto, notify echo on programmatic sets (Entry re-notifies when the value is identical), clamping, `Switch::state-set` firing on `set_active`, the EXPLICIT_NOTIFY census (498/581), and removing a widget from inside its own handler. |
| `qt_virtual_categories.py [--list]` | Puts the 376 distinct virtuals of the QtWidgets classes and the model/validator bases into categories (event handler, veto, model, geometry, ...), using the qdoc indexes. |
| `qt_sync_probe.py` (offscreen) | Checks the close veto through an event filter (needs `ignore()` **and** `return true`), key filtering, `textChanged` vs `textEdited` under QSignalBlocker, QSpinBox clamping, and `deleteLater` inside a filter. |
| `qt_slot_ownership_probe.py` (offscreen) | For 23 single-slot setters, records the default child, what happens to the old child on replace, and what a `nullptr` clear does. Also covers removal semantics of the multi-slot ops and QGridLayout position read-back. |
| `gtk_slot_probe.py` (Xvfb) | Records GTK/Adw replace semantics (old child finalized through weakref), convenience-prop defaults (Expander label, MenuButton menu-model), auto-wrapping (ScrolledWindow, ListBox, FlowBox), page objects, and GridLayoutChild live attrs. |
