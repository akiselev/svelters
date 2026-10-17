# X7: toplevels, non-widget object kinds, toolbars and docks

Lane X7, started 2026-10-17 on master 2aa2127. Gaps: `metadata/A1-GAPS.md` (G9, G12/G11, G1 remainder, G5 remainder, G8 on GTK). Working notes: `docs/notes/x7-toplevels-kinds.md`.

Order: items 1 to 3 are the commitment; 4 and 5 are left to a follow-up lane when time or context runs short (the report says so).

## 1. G9: toplevel kinds other than `window`

Design (declarative, no blocking API):

- A toplevel kind is any element whose lifetime class is `toplevel` (GTK `GtkWindow` subclasses, Qt `QMainWindow`/`QDialog` subclasses). The generator emits `TOPLEVELS` (tag -> `{open, modeless, modalProp, dialog}`) into `bindings.generated.js`, so the renderer knows without a native call.
- A toplevel element may appear anywhere in a component, not only at the root. The renderer never inserts it into its JS parent's native widget. It realizes it, makes it **transient for the nearest enclosing toplevel element** (`host.insert(window, dialog)`: ABI convention, below), and shows it. `{#if open}<messagebox ...>{/if}` inside a window is a dialog that lives as long as the block.
- Show policy is the JS-only attribute `show`: `modal` (default for dialog kinds: Qt `open()`, GTK `modal=true` + `present()`), `modeless` (Qt `present` = `show()`, GTK `present()`), `none` (created hidden; the app calls `bind:this` `.call('open')`). Plain windows default to `modeless`.
- Dialogs inside a window are shown after the window is presented (deferred list on the enclosing toplevel).
- ABI (additive, no new vtable member): `insert(parent, child, before=0, slot=0)` where `child` is a TOPLEVEL handle means "child is transient for the window containing `parent`"; `remove(parent, child)` undoes it. GTK: `gtk_window_set_transient_for`. Qt: `setParent(parent->window(), windowFlags())`. The cascade already reports children of a closed window as `parent`.
- Lifetime: TOPLEVEL class. A dialog closed natively reports `SVL_GONE_CLOSED`; the node stays in the tree with `dead = 'closed'`. A Qt `QDialog` finished by `done()`/`accept()`/`reject()` is hidden, not closed; apps listen to `onfinished`/`onresponse` and drop the `{#if}`.
- Galleries: GTK `08-dialogs` and Qt `09-dialogs` get a live dialog section.

## 2. G12/G11: non-widget object kinds

Generate kinds for objects reached as borrowed handles, annotate their props/methods/signals with a `verify_subject` that reaches the object through a getter, and add the blocked facts (TextView buffer editing, Range/SpinButton adjustment, Qt equivalents). Details are written once the generators have been read (see notes).

## 3. QMainWindow toolbars and docks

`toolbar` and `dockwidget` elements and `parent:area` / `parent:breakBefore` attributes through the X3 mechanism; membership by `toolBarArea`/`dockWidgetArea` because QMainWindow has no `indexOf`/`count`. `tabIcon` only if an icon value form is cheap.

## 4. G5 remainder, 5. G8 on GTK

Follow-up lane if time runs out.
