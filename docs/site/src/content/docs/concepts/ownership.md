---
title: Ownership and lifetimes
description: Who owns each native object, how handles die, and which parts of the lifetime model are implemented versus approved for ABI v2.
---

JavaScript and the toolkits keep separate heaps. Both earlier prototypes (the 2021 svelters Qt code and the clippo chat client) left ownership to Qt's parent tree and failed in the same ways: leaks, double frees and use-after-free where two heaps referenced each other without an explicit death protocol. The v2 model makes lifetime explicit.

:::note[Implemented vs approved]
The model below is the research result. ABI v2 (approved 2026-10-17, lands at the start of M2) adds the pieces marked **ABI v2**. Sections end with what exists today; the full split is in [ABI v2 and implementation state](#abi-v2-and-implementation-state).
:::

## The model

1. **Lifetime is explicit and tree-driven.** The renderer's `destroy` (children first, at the end of a microtask sweep) is the only thing that kills native objects JS created. Garbage collection never decides when a widget dies.
2. **Handles are never reused.** A handle is a `u64` below 2^53 from a monotonic per-backend counter, so JS numbers are exact. A late release cannot hit a different object, and no generation bits are needed. Using a stale handle is an error, never a crash.
3. **Native code never holds a JS value.** Events go back as `(handle, signal, args)`, so the two heaps cannot form a cycle. Natives implemented in JS (models, factories) hold an integer ID into a host table.
4. **GC only cleans up after explicit destroy.** It releases borrowed and value handles, and every GC release is idempotent. Releases go through a host queue drained at checkpoints, never from a QuickJS finalizer directly (a class `Drop` runs synchronously in the middle of JS execution).
5. **Each handle has one of four lifetime classes.**
6. **A native death cascades.** When a toplevel dies natively, or anything else dies under us, every handle under it dies, and JS is told through a queued notification.

## Lifetime classes

The backend decides the class when it mints a handle, driven by metadata.

| Class | Minted by | Strong ref held by | `destroy(h)` does | Native death reports |
| --- | --- | --- | --- | --- |
| `owned` | `create` | the handle table (always); the native parent also holds one while the object is inserted | GTK: disconnect, unparent if needed, drop the table ref. Qt: disconnect, `deleteLater`. WinRT: remove from parent, `Release`. Apple: `removeFromSuperview`, drop the strong ref | `on_destroyed(h, reason)` |
| `toplevel` | `create` of a window kind | the toolkit's toplevel list (GTK), or the table (Qt, WinRT, Apple); GTK's table holds it weakly | native window destroy (`gtk_window_destroy`, `deleteLater`, `Close`, `close` plus release) | `on_destroyed(h, CLOSED)` plus cascade |
| `borrowed` | `get_prop` or `call` returning an object we did not create (`QComboBox::lineEdit`, a ScrolledWindow viewport, template parts) | its native owner only; the table holds a weak ref | forget only | `on_destroyed(h, OWNER)` |
| `value` | `get_prop` or `call` returning a refcounted value (QIcon, GdkTexture, GBytes, GVariant, GdkPixbuf) | the table (a copy or a ref) | unref, drop the copy | never |

The API pages show a kind's lifetime class where the generator knows it. The full `lifetime` block, per-parameter `ownership` and `hazards` fields are planned metadata (below).

Kinds that are not widgets:

- **Controllers, actions, menus, layouts, columns** are tree elements with `owned` handles. Native parent ownership varies (GTK `add_controller` takes a ref; Qt `QAction(parent)`; `addAction` does not own), so it is recorded per operation in `effects`.
- **Models and delegates** are JS-created `owned` handles passed as reference props (`model={m}`). Reference props never transfer ownership. GTK views ref their models; Qt views do not own them.
- **Value objects** are marshalled by value in `svl_value` when small. Large or immutable refcounted ones become `value` handles.
- **Recycled views** (table cells, GTK ListView rows, WinUI `ItemsRepeater`) must never be handed out as borrowed handles, because identity changes under recycling.

## GTK floating references

- `create` uses `g_object_new`. For `InitiallyUnowned` (all widgets) the table must `ref_sink`; `gtk4-rs` does this automatically.
- `gtk_box_append`, `set_child` and similar take their own ref.
- Methods with `transfer full` on an input (rare in GTK4, for example `gtk_widget_add_controller`) need the generated thunk to `ref` before the call, so the table keeps its own ref.
- `g_signal_emit` holds a ref on the instance, so dropping the last table ref inside a handler is safe; finalization happens after the emission.

**Implemented:** the GTK backend holds non-toplevels strongly (so `remove` followed by re-`insert` works) and toplevels weakly, because `gtk_window_destroy` only drops GTK's own reference.

## Qt parent ownership

- An owned, inserted widget is deleted by its parent. That fires `destroyed`, then `on_destroyed`.
- A removed widget must be unparented. `QBoxLayout::removeWidget` leaves it parented and visible, so the `remove` descriptor adds a `setParent(nullptr)` step. After that the handle table is its only owner and `destroy` deletes it.
- `QObject::destroyed` for a `QWidget` fires before its children die, with the `QPointer` still set and the object still castable. The backend erases its table entry before calling the host. `on_destroyed` may therefore arrive for a parent before its children, and the host must only record it.

**Implemented:** each Qt table entry holds a `QPointer<QObject>` plus a `destroyed` connection; `native_destroyed` calls `on_destroyed`.

## `deleteLater` versus `delete`

- Never delete synchronously inside signal emission. A synchronous `delete` in a slot silently skips the remaining slots; `deleteLater` runs them.
- `destroy` erases the entry from the table, so the handle is stale immediately, whatever the toolkit's deletion timing. Deferred deletes posted from an outer loop are not processed inside a nested `exec()`, which does not matter because our semantics do not depend on when the delete actually happens.
- GTK equivalent: an idle-deferred unref. A clicked, mapped `GtkButton` keeps two extra refs for about 250 ms from an internal timeout, so tests drain for about 600 ms.

**Implemented:** Qt `destroy` disconnects, erases, then calls `deleteLater()`.

## Toplevels and native close

The approved policy follows React Native's `onRequestClose`:

- **Controlled window** (the element has a `close-request` listener): the backend vetoes the native close (GTK `close-request` returns `Stop`; Qt `closeEvent` calls `ignore()`) and posts the event. JS updates state, Svelte unmounts the window, and the renderer calls `destroy`.
- **Uncontrolled window** (no listener): the native default destroys it, the backend reports `on_destroyed(h, CLOSED)`, and the death cascades to every handle inserted below it.
- **Quit policy is the host's.** Qt uses `setQuitOnLastWindowClosed(false)` because Qt's default quits while hidden windows are still alive. The host quits when the last toplevel handle dies, unless the app opted out.

**Implemented today (P5):** GTK native destroy of a window invalidates every handle whose widget is a descendant of the window, children first, drops the table's strong reference and reports each through `on_destroyed`. Qt native close only hides the window; `quitOnLastWindowClosed` ends `run()` with all handles still live, the host is dropped right after, and no stale-handle errors occur. The veto policy and host-decided quit are **ABI v2**.

GTK zombie trap: `gtk_window_destroy` on a window anyone still holds strongly does not dispose it and emits no `destroy` signal; `present()` then warns about an inconsistent state. Hence the weak toplevel table.

## Natively created children (borrowed)

JS can reach children the toolkit made (`combo.lineEdit`, `scrolled.viewport`, the `GtkText` inside an entry) only through getters whose metadata says `returns.ownership: borrowed`. The handle holds a weak reference. A strong reference would be wrong: a strongly held auto-viewport survives the ScrolledWindow dropping it, orphaned and useless, and Qt cannot hold a strong reference at all. When the owner deletes the object, the handle becomes stale with reason `OWNER`; a later getter call returns a new handle for the new object. A borrowed handle cannot be inserted or removed through the renderer unless metadata marks the operation as adopting it. The JS-visible child of a wrapping slot (ScrolledWindow's viewport) stays the user's widget.

## Toolkit traps

Facts the generated tests keep pinned:

| Trap | Fact |
| --- | --- |
| `QLayout::removeWidget` | leaves the widget parented and visible; remove must `setParent(nullptr)` |
| `QMainWindow::setCentralWidget` | deletes the old widget; `takeCentralWidget` unparents; `setCentralWidget(nullptr)` deletes |
| `setMenuBar(nullptr)` / `setStatusBar(nullptr)` | delete the bar; the getters lazily create a new one |
| `QFormLayout::removeRow` | deletes both label and field; `takeRow` keeps them |
| `QTabWidget::setTabBar(nullptr)` | segfaults (no null check) |
| `QComboBox::setEditable(false)` | deletes the line edit; `true` makes a new one |
| `QScrollArea::setWidget` | deletes the old widget, reparents the new one into the viewport; `setViewport` deletes the old viewport |
| `delete` versus `deleteLater` | a synchronous delete in a slot skips later slots; `deleteLater` does not |
| `QObject::destroyed` | for a QWidget: fires before children die, `QPointer` still set; for a plain QObject the `QPointer` is null |
| `quitOnLastWindowClosed` | defaults to true; a hidden last window quits the loop |
| GTK dispose versus finalize | `destroy` signal is dispose; a widget is disposed only on its last unref, except toplevels via `gtk_window_destroy` if nobody else holds them |
| `gtk_window_destroy` with a held ref | no dispose and no signal; zombie window |
| GTK container removal | drops the parent's ref; finalizes at once if the table held none |
| GTK window close | the default handler destroys; held descendants survive orphaned |
| ScrolledWindow auto-viewport | `set_child(NULL)` unparents the user child and drops the viewport |
| Transient GTK refs | about 250 ms after a click |

## Ownership facts as metadata

Planned additive fields on `svelters-element-kind/v0`, each with `origin` and `provenance`, each with a generated test:

```yaml
lifetime:                       # per kind
  class: owned | toplevel | value | borrowed_only
  native_refcount: gobject_floating | gobject | qobject_tree | com | objc_arc | none
  destroy: { native: "QObject::deleteLater()" | gtk_window_destroy | "unref" | "Window::Close" }
  close: { request_signal: close-request, veto: return_stop | event_ignore | cancel_arg, default: destroy }
  internal_children:
    - name: lineEdit
      get: "QComboBox::lineEdit() const"
      exists_when: "editable == true"
      invalidated_by: ["QComboBox::setEditable(bool)", "QComboBox::setLineEdit(QLineEdit*)"]
methods / ctors / slot ops:
  params:  [{ name, ownership: borrowed | transfer_full | transfer_floating | parent_to_self | adopt | reference_kept | reference_weak }]
  returns: { ownership: none | full | floating | borrowed | caller_owns_unparented }
  effects: { child: ..., old: ..., default: ..., self: deleted | deleted_later, args: { <param>: deleted | parented | referenced } }
  hazards: [ null_arg_crash, deletes_synchronously, blocking_nested_loop, invalidates:<getter>, emits_during_dispose ]
```

Sources for the facts: GTK from GIR `transfer-ownership` plus the `InitiallyUnowned` base; Qt from the PySide typesystem (`<parent>`, `<define-ownership>`, `replace_child`) and probes; manual annotation for the rest. The `effects` vocabulary for slot operations already exists in the annotation files today. See [Metadata and annotations](/guides/annotation-workflow/).

## ABI v2 and implementation state

ABI v2 is approved and lands at the start of M2 (`SVL_ABI_VERSION` becomes 2). Current header: version 1, `on_destroyed(self, h)` with no reason, no `handle_info`. Details: [`on_destroyed`](/concepts/abi/#on_destroyed), [`destroy`](/concepts/abi/#destroy), [ABI v2](/concepts/abi/#abi-v2-planned).

| Piece | State |
| --- | --- |
| Handle table with stale-handle errors; handles numbered from 1 in creation order | implemented |
| Qt `QPointer` plus `destroyed` connection; `destroy` as `deleteLater` | implemented |
| GTK strong table for widgets, weak for toplevels; cascade on native window destroy | implemented (P5 fix) |
| Host `on_destroyed` forgets the handle (`Core::forget_handle`) and never enters JS | implemented; JS is not told |
| Renderer microtask sweep: removed nodes `destroyTree`d, children first, unless re-attached | implemented |
| Qt native close reports `on_destroyed` for the window subtree | not implemented (widgets hide only) |
| `on_destroyed(h, reason)` with `svl_gone` reasons | **ABI v2** |
| `handle_info` and the four `svl_lifetime` classes in the ABI | **ABI v2** |
| `destroy` idempotent and defined per lifetime class | **ABI v2** |
| Cascading native death to all descendants on both toolkits | **ABI v2** (GTK done for windows; Qt cascades by itself) |
| Host forwards deaths to JS (`__svelters_destroyed(h, reason)`) | **ABI v2** |
| Renderer `dead` state and `StaleHandleError(reason)`; re-inserting a dead node re-realizes it | **ABI v2** |
| Borrowed and value handles with `FinalizationRegistry` release; explicit `.dispose()` | **ABI v2** |
| Window close veto (`onRequestClose`), host-decided quit, Qt `quitOnLastWindowClosed(false)` | **ABI v2** |
| Handle identity stored on the object (GObject qdata, Qt property or hash erased on `destroyed`) instead of an address map | **ABI v2** follow-up |
| Metadata `lifetime`, `ownership`, `hazards` and their generated tests | planned (M3 and later) |

Not probed in the research: Qt deferred delete inside nested `exec()` loops, WinRT and Apple behaviour. For those platforms the design carries over with different refcount kinds: COM has no destroy notification (staleness is detected lazily when `resolve` fails), and Apple uses an associated-object sentinel whose `dealloc` calls `on_destroyed`.

Sources: `docs/notes/ownership-and-lifetimes.md`, `plans/architecture/backend-abi.md`, `docs/notes/p5-integration.md`, `docs/notes/p3-gtk-lane.md`, `docs/notes/p4-qt-lane.md`.
