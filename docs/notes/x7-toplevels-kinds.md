# X7 working notes

Lane X7, branch reset to master 2aa2127 at start. Plan: `plans/milestones/X7-toplevels-kinds.md`.

## Findings before coding

- `attachNative` threw for non-`window` top-level elements and always called `present`.
- GTK has a precedent for implicit tree semantics by child type: event controllers are "inserted" with `add_controller` (reserved `CONTROLLER_SLOT`). A toplevel child inserted into a parent follows the same idea (transient-for), no new slot id.
- Qt `QWidget::setParent(parent)` drops the window type flags; a transient dialog needs `setParent(parent, flags)`, so it lives in the template's `b_insert`, not in a slot.

## Item 1 (G9): what was built

- Generator: `tools/metadata/common/gen_bindings.py` also emits `TOPLEVELS` into `bindings.generated.js` (tag -> `open`/`modeless` candidate method names, `modalProp`, `dialog`) and `ToplevelAttrs`/`ToplevelTag` into `bindings.d.ts`. GTK reads the lifetime class `toplevel` of kinds with an element tag (only `window` and `aboutdialog` are exposed GTK toplevels: the deprecated GtkDialog family is `unexposed`). Qt reads `construct.class` metadata lifetime. Where both toolkits define a tag the candidate lists are merged (`open` before `present`), the renderer takes the first one the host resolves (`pickMethod`).
- Renderer (`packages/renderer/src/index.js`): `topSpec`, `mountTop`, `showTop`, `topAncestor`. A toplevel element anywhere in the tree is realized, `host.insert(nearestEnclosingToplevel, el)` (ABI "Toplevels in the tree") and shown by `show="modal|modeless|none"` after the enclosing toplevel was shown (`pending` list). `flat`/`firstFlat` skip toplevels so they are never layout children nor `before` anchors. Removal: `host.remove(enclosing, el)`, then the usual sweep destroys it. At the root any toplevel kind is accepted, other tags still throw.
- GTK backend (`insert_transient`, `TRANSIENT_SLOT`): `set_transient_for`; the parent window's destruction destroys its transient windows from an idle (GTK would leave them orphaned) and reports them `parent`. Qt template (`b_insert`/`b_remove`): `setParent(parent->window(), flags)`.
- Qt `QDialog.yaml` got `present` (= `show()`, modeless). `open` (window-modal, non-blocking) was already there.
- Test dump: `Host::toplevel_handles()` (transient dialogs are not roots any more); the counter driver appends every live toplevel to the final dump; GTK/Qt dumps carry `modal`, `transient`, `visible` on windows.
- Gotchas: `QMessageBox` is modal by default even with `show()`; `<window>` on GTK is a `GtkApplicationWindow`; handle numbers in galleries depend on the page mounted first (the Qt "open all dialogs" button is #24).
- Tests: renderer unit tests (3), fixture `fixtures/dialogs` (oplog), `crates/svelters-gtk/tests/toplevels.rs` (4), `examples/counter-qt/tests/toplevels.rs` (4), gallery `gtk_08_dialogs_toplevels` and `qt_09_dialogs_all_kinds` (every Qt dialog kind modal+transient+visible).

## Item 2 (G12 / G11): objects reached as borrowed handles

What was already true: every GObject of the Gtk namespace has a generated kind (`GtkTextBuffer`, `GtkAdjustment`), creatable by name, with reflective props and signals, and a borrowed handle gets the nearest ancestor kind. What was missing was a way to annotate and verify facts on a kind that is no element, and the forwarding that makes the common case usable as an element property.

GTK:

- `verify_subject: {borrowed: {tag, getter}}` (override schema, `Model.make` in `gen_backend.py`): the generated tests of a non-element kind get their subject from `Harness::borrow_from(tag, getter)` (a BORROWED handle, as an app would get it). Method tests use `h.obj(o)` for non-widget subjects. New annotation files: `TextBuffer.yaml` (text, enableUndo; changed, begin/end-user-action, modified-changed; set_text, insert_at_cursor, get_char_count, get_line_count, undo) and `Adjustment.yaml` (6 props, value-changed/changed, configure, clamp_page, get_value, get_minimum_increment), through `textview.get_buffer` and `scale.get_adjustment`.
- Forwarded properties: `virtual: {op: via_object, via: <object prop>, property: <name on that object>}` (`PropInfo.via/target`, flag `F_VIA`). `set_prop`/`get_prop` act on the object `via` returns, and echo suppression blocks the element's own listeners. Used for `GtkRange` (`value`, `lower`, `upper`, `stepIncrement`, `pageIncrement`, `pageSize`: all through `adjustment`), `GtkSpinButton` (`lower`, `upper`, `stepIncrement`, `pageIncrement`), `GtkTextView:text` (through `buffer`).
- Forwarded signal: `via: buffer` on a signal record (`SignalInfo.via`): GtkTextView `changed` is connected on the buffer (the handler id is remembered with its target, `State.via_targets`, so listen(off), destroy and echo blocking use the right object) and carries the new text like GtkEditable `changed`. Replacing the buffer after listening is not followed.
- Bindings: `bind:text` on `textview`, `bind:value` on `scale`/`scrollbar` (through the adjustment). `fixtures/buffer-bind` + `crates/svelters-gtk/tests/host_buffer.rs` run it with the real host: native typing and slider drags reach the app, app writes reach buffer and adjustment.
- Not done: GtkTextIter-like boxed types. Iterators need out parameters of a boxed struct and mutable iterator semantics; the text APIs that need them (`get_text(start, end)`, `insert(iter, text)`) have simple counterparts that are annotated instead (`text`, `set_text`, `insert_at_cursor`). Pango/Gdk object kinds (GdkCursor, GdkPaintable/Texture) were not annotated: the kinds exist (every Gtk-namespace object does; Gdk/Pango namespaces are not ingested as kinds, only their enums, G11), so an app only sees them as opaque BOXED values. Ingesting Gdk object kinds is a follow-up.

Qt:

- `tools/metadata/qt/ingest.py` has an allowlist `EXTRA_CLASSES` of non-QtWidgets classes (module recorded in `native.module`, which `gen_cpp.py inc()` now uses for the include). Ingested: `QTextDocument`. `QItemSelectionModel` needs a `QAbstractItemModel` to be useful (M5 territory) and is left out.
- `QTextDocument.yaml`: element `textdocument` (a QObject, not a widget; the generator needed no change beyond the include): 9 props, 9 methods, 3 signals, all with generated tests. `QTextEdit::document()` was already annotated, so `edit.call('document')` returns a borrowed handle of kind `textdocument`: test `borrowed.rs document_kind`, gallery page `03-text` (`document blocks: 2`).

## Item 3: QMainWindow toolbars and docks

- `QMainWindow.yaml` slots `toolbars` and `docks` (append-only; membership by isAncestorOf), with child attributes `area` (top/left/right/bottom) on both and `breakBefore` on toolbars: `<toolbar parent:area="left">`, `<dockwidget parent:area="right">`. Remove also unparents (removeToolBar/removeDockWidget only hide).
- Generator: append-only slots may carry attrs; attr `set.expr` (`$h`, `$c`, `$v`) and `get.expr` with `$c`; a `child` argument is cast to the native parameter type; attr `setup:` (a toolbar break needs a toolbar before it); `route_by_kind: true` slots now take unnamed children by class (a `<toolbar>` in a `<window>` goes to `toolbars`, the label stays central). Test: `toplevels.rs areas_route_by_kind`, gallery `06-layout` (second window with toolbars and a dock).
- `tabIcon` dropped: an icon attribute as a theme name cannot be read back without an icon theme (`QIcon::name()` is empty headless), so no honest generated test; needs a real icon value form.

## Merge of master (X6 styling)

Resolved in `index.js` (STYLE_ATTRS and TOPLEVELS exports), `index.d.ts`, ABI doc, `gallery.rs` (`run_page` takes colours; `run_page_clicks` too). The counter report has a new field `dump_all` (root tree plus the other live toplevels); `dump` stays root-only. `kinds.generated.js` regenerated with `gen_kinds.py`. The Qt "open all dialogs" handle in the gallery is now #26 (two more sidebar buttons).

## Not done (follow-up lane)

Items 4 (typed sync results, Qt key-release/mouse/wheel/focus event families) and 5 (GTK DrawingArea draw func, ListBox/FlowBox sort/filter/header funcs) were not started. Notes for them: for the draw func prefer a recording canvas (ops recorded in JS and replayed natively at snapshot time) so no JS runs during snapshot; sort/filter/header can follow X5's push-the-result approach.
