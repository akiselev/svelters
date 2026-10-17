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
