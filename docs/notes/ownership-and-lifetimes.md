# Ownership and lifetimes

Status: research note, 2026-10-17. It proposes an ABI v2 change (§4) that has to go through `plans/architecture/backend-abi.md` before anyone implements it. Experiments are in `docs/notes/scripts/ownership/` and their outputs are checked in next to them.

## 0. Summary

- **Neither old prototype solved ownership. Both left it to Qt.**
  - In the 2021 svelters Qt code, nothing ever deleted a widget. JS wrappers held non-owning `QPointer`/`QPtr` pointers. Signal handlers were permanent rquickjs registry roots. `destroyed` was never connected.
  - clippo (cxx-qt + QML, 2023–25) leaves every lifetime to the Qt parent tree. Its bugs were a double free (Rust `UniquePtr` vs `QGuiApplication`), an inverted liveness check, and raw `Rc` pointers in `QModelIndex`, which it replaced with index arenas.
- **Recommended model:**
  1. Lifetime is explicit and driven by the tree. The renderer's `destroy` (children first, after a microtask sweep) is the only thing that kills native objects JS created.
  2. Handles are `u64` values that are never reused, so a late release can't hit a different object and no generation bits are needed. Using a stale handle is an error, never a crash.
  3. Native code never holds a JS value. Events go back as `(handle, signal)`, so the two heaps can't form a cycle.
  4. Garbage collection only cleans up after explicit destroy. It's used for borrowed handles and value handles, and every GC release is idempotent.
  5. Each handle has one of four lifetime classes: `owned`, `toplevel`, `borrowed`, `value`.
  6. When a toplevel dies natively (or anything else dies under us), the death cascades to every handle under it, and JS is told through a queued `destroyed` notification.
- **ABI v2:**
  - `on_destroyed` gains a `reason` argument.
  - A new `handle_info` call is added.
  - `destroy` is defined per lifetime class.
  - Re-entrancy rules are written down.
  - Everything else stays as it is (§4).
- **Metadata:**
  - A per-kind `lifetime` block.
  - Per-parameter and per-return `ownership`.
  - The existing `effects` vocabulary, plus `self` and `args`.
  - `internal_children` for borrowed sub-objects.
  - `hazards`.
  - Each fact gets a generated test (§2.3).

## 1. Archaeology

### 1.1 svelters v1 Qt prototype (2021)

There were four eras. All refs are `path@sha`; the `qt/` tree at f838fc8, 9eb173a and `v1-legacy` is identical.

| Era | Commits | Binding | Files |
|---|---|---|---|
| A, Jun 6–9 | 4b15e1f → dc4fa3b | napi (Node) + `cpp!` + qmetaobject | `src/{lib,widget,button,main_window}.rs`, `src/ts/widgets.ts` |
| B, Jun 20–21 | 9af951c | rquickjs + `cpp!` | `crates/qt/src/{object,document,elements/impls}.rs` |
| C, Jun 21 → Sep 10 | 787d3a3 … 1f1daab, ad421e2 | ritual (`qt_widgets` 0.5) | `crates/qt/src/impls/containers.rs`, `types/windows/main_window.rs`, `crates/core/src/{rt,init}.rs`, `runtime/subscription.rs` |
| D, Nov 21 | f838fc8 | ritual `QPtr` + QMetaObject reflection | `qt/src/{object,newtype,registry,meta}.rs` |

**Ownership: nobody deletes.**

- **Era A:** `cpp_class!(pub unsafe struct QWidget as "QPointer<QWidget>")` (`src/widget.rs@dc4fa3b`). The napi finalizer drops only the `QPointer`.
- **Era B:** `QPointer<QObject>* ptr = new QPointer<QObject>(); *ptr = new QPushButton();` is returned as Rust `Pin<Box<QPtr>>` (`crates/qt/src/elements/impls.rs@9af951c`). The pointer is allocated by C++ `new` and freed by Rust `Box`, and the widget itself leaks.
- **Era C:**
  - `enum Qt<CLS> { Owned(QBox<CLS>), Shared(QPtr<CLS>), Ref(Ptr<CLS>) }` (`impls/containers.rs@1f1daab`).
  - Instances live in a `SlotMap` (`core/src/rt.rs@1f1daab`) that is only `.clear()`ed on hot reload.
  - `removeChild` on QMainWindow calls `take_central_widget()` and drops the result (`types/windows/main_window.rs@1f1daab`).
- **Era D:**
  - `Object(QPtr<QObject>)` is non-owning.
  - The only liveness check is `if obj.0.is_null() { Err("QObject has been destroyed") }` (`qt/src/object.rs@f838fc8`).
  - `marshall` builds a new JS wrapper on every crossing (`registry.rs@f838fc8`), so there's no identity.

`deleteLater`, `setParent` and `destroyed` connections don't appear in any era.

**GC and handlers:**

- **Era B:** `ctx.register(func)` creates a permanent root, and only `removeEventListener` deregisters it (`object.rs@9af951c`).
- **Era C:**
  - `Subscription` roots the element with `ctx.register(...)` and never deregisters it (`runtime/subscription.rs@1f1daab`).
  - `Dispatcher`/`Runtime` have their `mark_refs` bodies commented out.
- **Era D:** `gc_mark` is an empty function, and a comment copied from the GTK code says "GC keeps destroying the prototypes and causing use after free" (`registry.rs@f838fc8`).

**What was janky:**

- Commit messages:
  - f838fc8 "epic fail on qt_metacall"
  - ad421e2 "screw it … switch to gtk-rs"
  - fa8399c "trying to figure out where the leaks are coming from"
  - ba201f1 "fixed mem leak issue (it was the registerkeys…)"
  - 11c9ac4 "cant figure out elemnt casting lifetimes"
  - 8ff7bc1 "stack overflow in virt cast"
- `// This prevents a segfault` above `std::mem::forget(args)` (`crates/qt/src/lib.rs@3c2ba28`).
- A `CString::into_raw` leak on every attribute call (`src/lib.rs@dc4fa3b`).
- An inverted `panic!` in `append_node` (`nodes/node.rs@1f1daab`).

The GTK side (`docs/notes/gtk-v1-archaeology.md`) adds toggle-ref crashes, QuickJS↔GObject cycles, and use-after-free on disconnect.

**Lesson:** every failure came from two heaps referencing each other with no explicit death protocol.

### 1.2 clippo

clippo is a Rust + cxx-qt + QML (Kirigami/KDDockWidgets) LLM chat client. It has 373 commits from 2023-04 to 2025-08 and is real GUI code. It has no handle table, no QPointer/Weak/generations, and no finalizers.

**The design:**

- Qt owns everything through the parent tree and the QML engine. Rust state lives inside the cxx-qt QObjects, and Rust `Drop` runs when Qt deletes the object.
- C++ helpers `setParent` a child and return a raw pointer, which Rust re-pins with `.expect("Model is null")` (`crates/core/src/list_model.cpp@3add413`, `sync_list_model.rs`).
- `src/main.rs@3add413`:
  > "we want to forget the UniquePtr because QGuiApplication takes over ownership / Without this block, there is a double free segfault / TODO: The C++ side should just return a Pin<&mut RustEngine> and parent the engine explicitly"

  The fix is `RustEngine::new().into_raw()` (since c9937ed).
- Background tasks poll `qt_thread().is_destroyed()` before queueing work (the `live_check!` macro, `crates/engine/src/macros.rs`). `Drop` impls abort them. `destroyed` is never connected.
- Rust-side signals use `signals2` RAII `ScopedConnection`s (`crates/commands_qml/src/command_impl.rs`). cxx-qt connections are `.release()`d for the object's lifetime.
- Hot reload calls `deleteLater()` on the old windows after tagging them `scheduledForDeletion` (`crates/engine/src/rustengine.cpp`, from 6c01e41).

**Problems:**

- The double free above.
- 212bedd "fixed the dumb is_destroyed() mistake": the liveness check was inverted.
- `Rc<RefCell<Node>>` stored in `QModelIndex::internalPointer` via `Rc::into_raw`/`from_raw`:
  - dab047e "tracking down segfault"
  - 6a1284b "about to switch to index method instead of this Rc crap"
  - f5e52d7 replaced it with a `Vec` arena and `usize` indices.

**Lesson:** two owners of one object cause double frees, and pointers smuggled through native code cause segfaults. Indices (handles) into an arena were the fix that stuck. We start there.

### 1.3 Current backends (master + P3 branch)

- **Qt** (`tools/metadata/qt/templates/backend.cpp.in`):
  - Each entry holds `QPointer<QObject>` + a `destroyed` connection, and `native_destroyed` calls `host->on_destroyed`.
  - `destroy` disconnects, erases, then calls `deleteLater()`.
  - Removed widgets are unparented by the `then: setParent(nullptr)` step, so the handle table is their only owner.
  - Gaps:
    - No `closeEvent` handling, and `quitOnLastWindowClosed` is left at its default `true`. Closing the last window natively hides it and ends the loop, with every handle still live.
    - No borrowed handles.
- **GTK** (`crates/svelters-gtk/src/backend.rs@4c832c3`):
  - The table holds a strong `hold` for non-toplevels and only a `WeakRef` for toplevels. The window's `destroy` signal drives `on_destroyed`.
  - The `by_obj` map is keyed by the raw pointer.
  - Gaps:
    - Native close of a window leaves strongly held descendants alive, orphaned, and reported as live (experiment G2a).
    - Pointer-keyed `by_obj` will alias handles once borrowed or weak handles exist, because an address can be reused after finalize.
- **Host** (`crates/svelters-host/src/core.rs`): `on_destroyed` only calls `forget_handle`, so JS is never told. Later ops fail with "stale or foreign handle".
- **Renderer** (`packages/renderer/src/index.js`):
  - A removed element goes into `sweepSet`.
  - At the end of the microtask it is `destroyTree`d (children first) unless it was re-attached.
  - `realize` on an element whose `h` is null creates a fresh native object.

## 2. Design

### 2.1 JS ↔ host

**When native objects die.** Explicit, tree-driven destruction, with GC as a backstop only:

- **Elements created by the renderer:** the renderer's `destroy` ends them.
  - The renderer already defers it to the end of the microtask, so a keyed move (remove, then insert within one flush) keeps the native object.
  - `out:` transitions keep the node attached until the transition ends, so they need nothing extra.
  - GC never decides when a widget dies: GC timing would make close/hide visibly nondeterministic. Both toolkits also need deletion to happen at a known point (Qt `deleteLater` timing, GTK dispose).
- **`bind:this` / `{@attach}` refs:** these are renderer element nodes, not raw numbers, so they can outlive the native object.
  - After `destroy` the node is `dead: "destroyed"`.
  - After a native death it is `dead: <reason>`.
  - Reads and calls (`getProp`, `call`) on a dead node throw `StaleHandleError(reason)`.
  - Teardown-shaped ops (`setProp`, `listen(off)`, `destroy`) are dev warnings and no-ops, because Svelte effects can legitimately run during teardown.
  - Re-inserting a dead node re-realizes it as a new native object. This is what you want for a closed window that the app reopens.
- **Borrowed handles and value handles** (§2.2) aren't tree elements:
  - JS gets them from `getProp`/`call`, wrapped in a small JS object that is registered with a `FinalizationRegistry`.
  - The registry callback calls `host.destroy(h)`, which only *releases* borrowed and value handles and never deletes them.
  - Explicit `.dispose()` (and `using` once QuickJS supports it) is the fast path.
  - Experiment J1: the `FinalizationRegistry` callback runs as a job after GC and is a no-op if dispose already ran.
- **Never use rquickjs class finalizers (`Drop`) to call the backend.**
  - J1 shows a class `Drop` runs **synchronously, in the middle of JS execution**, as soon as the refcount hits zero (`w = null` inside `eval`).
  - It also runs for every live object when the runtime is dropped.
  - A finalizer may only push the handle onto a release queue that the host drains at the next checkpoint.
  - Shutdown order: drop the JS runtime while the backend is alive, or set a `shutting_down` flag that makes releases no-ops.

**Cycles.** The rule is that **native code never holds a JSValue**:

- Backends connect natively and call `post_event(h, signal, args)`.
- The renderer looks up `byHandle` and runs the listeners.
- Every reference from JS to native code is a number, not a GC edge, so there are no cross-heap cycles, toggle refs, or `gc_mark` hooks.
- The cycle `byHandle → node → listener closure → node` is entirely inside JS, and `destroyTree` breaks it (`byHandle.delete`).

The exception is natives implemented in JS:

- JS-backed `GListModel`/`QAbstractItemModel`, list factories, custom draw, validators.
- These hold a host-side `u32` **JS ref id** that points into a host table of persistent values, never a raw JSValue.
- The id is released when the native object dies (GObject weak notify / `destroyed`), which goes through the host's queue.
- Such objects must be tree elements (or owned by one), so explicit destroy breaks the remaining cycle (native → JS ref → closure → handle number).
- In dev, the host reports owned handles that aren't in the tree when a window closes.

**Handle scheme:**

- `svl_handle` is a `u64` from a monotonic per-backend counter that is **never reused** and stays below 2^53, so JS numbers are exact.
- A stale handle is one that isn't in the table. That makes a late release (FR callback, Drop queue, duplicate destroy) always harmless, and no generation bits are needed.
- A backend that wants a slab can pack `index:32 | generation:21`. It's the same contract.
- The host keeps a bounded tombstone map (`h → reason`, about 4k entries) so errors can say "handle 12 died: window closed natively" instead of "unknown handle".
- Native identity:
  - One handle per native object, stored **on the object**: GObject qdata `svelters-handle`, and a Qt dynamic property or a `QHash<QObject*, svl_handle>` that is erased in `destroyed`.
  - Never a map keyed by raw address that outlives the object.

### 2.2 Host ↔ native

**Who holds the strong reference.** It depends on the handle's **lifetime class**, which the backend decides at mint time and which comes from metadata (§2.3):

| Class | Minted by | Strong ref held by | `destroy(h)` does | Native death → |
|---|---|---|---|---|
| `owned` | `create` | handle table (always); the native parent also holds one while the object is inserted (GTK/WinRT/ARC refcount; Qt parent tree) | GTK: disconnect, unparent if needed, drop the table ref. Qt: disconnect, `deleteLater`. WinRT: remove from parent, `Release`. Apple: `removeFromSuperview`, drop the strong ref | `on_destroyed(h, reason)` |
| `toplevel` | `create` of a window kind | the toolkit's toplevel list (GTK) or the table (Qt/WinRT/Apple); GTK's table holds it weakly | the native window destroy (`gtk_window_destroy`, `deleteLater`, `Close`, `close` + release) | `on_destroyed(h, CLOSED)` + cascade |
| `borrowed` | `get_prop`/`call` returning an object we didn't create (`QComboBox::lineEdit`, a ScrolledWindow viewport, template parts) | its native owner only; the table holds a **weak** ref | forget only | `on_destroyed(h, OWNER)` (Qt/GTK eager; WinRT/Apple may be lazy, see §2.5) |
| `value` | `get_prop`/`call` returning a refcounted value (QIcon, GdkTexture, GBytes, GVariant, GdkPixbuf) | the table (a copy or a ref) | unref / drop the copy | never |

**GTK floating refs:**

- `create` uses `g_object_new` (via `Object::with_type`). For `InitiallyUnowned` (widgets) the table must `ref_sink`; gtk-rs does this automatically.
- `gtk_box_append`/`set_child` and the like take their own ref.
- Methods annotated `transfer full` on an *input* (rare in GTK4, e.g. `gtk_widget_add_controller`, which takes the caller's reference) need the generated thunk to `ref` before the call, so the table keeps its own.

**Qt parent ownership:**

- An owned, inserted widget is deleted by its parent. That fires `destroyed`, then `on_destroyed(PARENT)`.
- A removed widget **must be unparented** (`removeWidget` leaves it parented; experiment Q1 confirms). Otherwise its old parent's death kills a node JS still considers alive and detached.
- After that the table is its only owner, and `destroy` deletes it.

**`deleteLater` vs `delete`, and deletion during emission:**

- Q4a: `deleteLater` of the sender inside its own `clicked`. Later slots still run, the handle is stale immediately (it's erased from the table at `destroy`), and the object goes at the next loop turn.
- Q4b: a synchronous `delete` in the same place doesn't crash Qt 6.11 (`QAbstractButton` guards with QPointer). But **it silently skips the remaining slots**. Never delete synchronously.
- Q4c: deleting the parent window from a child's signal with `deleteLater` is fine, and the child's `on_destroyed` fires.
- Caveat (Qt docs, not probed): deferred deletes posted from an outer loop level aren't processed inside a nested loop (`exec()`). Since the handle goes stale at `destroy` time, our semantics don't depend on when the delete actually happens.
- GTK: `g_signal_emit` holds a ref on the instance, so dropping the last table ref inside a handler is safe and finalization happens after the emission (G4).
- The host rule from `signals-and-slots-semantics.md` §1 still applies: tree ops issued from a sync handler are queued to the checkpoint.

**QObject `destroyed` timing (Q5, Qt 6.11):**

- For a **QWidget**, `~QWidget` emits `destroyed` *before* it deletes its children and its non-layout QObject children.
- At that point the widget's own QPointer is **still non-null** and `qobject_cast<QWidget*>` still succeeds.
- Its layout is deleted even earlier.
- For a plain QObject, the QPointer is already null inside `destroyed`.
- The order we saw: layout, then window, then button, then action.

Consequences:

- `native_destroyed` must erase the entry **before** it calls the host. Both backends already do this.
- `on_destroyed` may arrive for a parent before its children.
- The host must not run JS from inside `on_destroyed`: it only records the death and queues a notification.

**Detached but alive.** This is the window between a renderer `remove` and either a re-`insert` or the microtask sweep's `destroy`.

- GTK (G1): the table ref keeps the widget alive after `remove`. The old parent window can die in the meantime, and the widget can be re-inserted into another toplevel and mapped.
- Qt (Q1): the same, after `setParent(nullptr)`. Inserting a widget that sits in layout A into layout B removes it from A on reparent (`lay2->indexOf(a) == -1`).
- Rule: every slot's `remove` op must leave the child either unparented or `kept_child_hidden` (QDockWidget). Remove effects of `deleted` are only allowed on slots marked `on_remove: destroys_child`, and the renderer then marks the node dead.

**Toplevels and native close.** Apply React Native's `onRequestClose` pattern uniformly:

- **Controlled window** (the element has a `close-request`/`onclose` listener):
  - The backend vetoes the native close: GTK `close-request` returns `Stop` (G2c); Qt `closeEvent` calls `ignore()`.
  - It posts `close-request`.
  - JS updates state, Svelte unmounts the window, and the renderer calls `destroy`.
  - The window stays consistent with app state (`{#if open}`).
- **Uncontrolled window** (no listener):
  - The native default destroys it: GTK's default handler (G2a); Qt deletes it in the accepted `closeEvent`, not by hiding it.
  - The backend reports `on_destroyed(h, CLOSED)` and **cascades** it to every handle inserted below that window.
  - Qt cascades by itself because the children are deleted and each emits `destroyed`.
  - GTK does **not**: G2a shows a held child box survives, orphaned and undisposed, with its own children still attached. The GTK backend must walk its `slot` records and report and release each descendant with reason `PARENT`.
  - Detached (not inserted) handles aren't in the cascade, which matches Qt's behaviour.
- **Quit policy is the host's:**
  - Qt `setQuitOnLastWindowClosed(false)`, because Qt's default `true` quits while hidden windows are still alive.
  - GTK `GtkApplication` hold/release.
  - The host quits when the last `toplevel` handle dies, unless the app opted out (tray apps).
- **GTK zombie trap (G5):**
  - `gtk_window_destroy` on a window that anyone still holds strongly does **not** dispose it and doesn't emit `destroy`. It removes it from the toplevel list and unrealizes it, and its child stays attached.
  - `present()` brings it back with "A window is shown after it has been destroyed… inconsistent state".
  - So the GTK table must hold toplevels **weakly** (P3 does), and borrowed handles to toplevels must be weak too.

**Natively created children ("borrowed").**

- **JS can get them:** `combo.lineEdit`, `scrolled.viewport`, `entry.text` (GtkText, G3), dialog internals, but only through getters or methods whose metadata says `returns.ownership: borrowed`. The handle holds a **weak** reference.
- **Strong would be wrong:**
  - G3 shows a strongly held auto-viewport survives the ScrolledWindow dropping it, orphaned and useless.
  - Qt can't hold a strong reference at all.
- When the owner deletes the object:
  - Qt fires `destroyed` (Q3: `setEditable(false)`, `setViewport`, `QFormLayout::removeRow` both widgets, `setCentralWidget` old one). The handle becomes stale with reason `OWNER`.
  - A later `combo.lineEdit` returns a **new** handle for the new object, never the old one.
  - GTK uses weak-ref notify / qdata destroy.
- A borrowed handle can't be inserted or removed through the renderer. Ops that would reparent it fail unless metadata marks them as adopting it (`ownership: adopt`).
- The `wrapped:` slot children (ScrolledWindow's Viewport, Qt's internal popup container) stay hidden: the JS-visible child is always the user's widget (`signals-and-slots-semantics.md` open question 11).

**Non-widget objects:**

- **Controllers, actions, menus, layouts, columns:** tree elements with `owned` handles.
  - Their native parent ownership varies: GTK `add_controller` takes a ref; Qt `QAction(parent)`, `QLayout` owned by its widget, `addAction` *not owning*.
  - That variation is recorded per op in `effects`/`ownership`, and the table ref covers the gaps.
  - A Qt `QAction` added through `addAction(QAction*)` is referenced, not owned, so the table must keep it alive and delete it on `destroy`. Qt then removes it from widgets on deletion.
- **Models, delegates, selection models:** JS-created `owned` handles passed as **reference props** (`model={m}`).
  - GTK views ref their models.
  - Qt views don't own their models (`QComboBox::setModel` deletes the old model only if `parent == combo`, per the slots catalogue). If JS destroys a model that is still in use, Qt views reset themselves on `destroyed`.
  - Rule: reference props never transfer ownership. The JS object that created the model owns it, through `$effect` teardown or a component-scoped wrapper.
- **Value objects** (QIcon, QFont, QColor, GdkRGBA, GBytes, GVariant, GdkTexture, GdkPixbuf):
  - Small ones are marshalled by value in `svl_value` (strings, lists, plain records).
  - Large or immutable refcounted ones become `value` handles: shared and immutable, released through dispose or FinalizationRegistry, and `destroy` means unref.
  - Qt's implicitly shared values are kept as a copy in the table.

### 2.3 Ownership facts as metadata

This extends `svelters-element-kind/v0`. All fields are optional and additive, and every fact carries `origin` + `provenance`.

```yaml
lifetime:                       # per kind
  class: owned | toplevel | value | borrowed_only     # borrowed_only: kinds JS can't create (e.g. QComboBoxPrivateContainer)
  native_refcount: gobject_floating | gobject | qobject_tree | com | objc_arc | none
  destroy: { native: "QObject::deleteLater()" | gtk_window_destroy | "unref" | "Window::Close" }
  close:                        # toplevel only
    request_signal: close-request          # qt: closeEvent (generated event filter)
    veto: return_stop | event_ignore | cancel_arg   # GTK / Qt / WinRT AppWindow.Closing.Cancel
    default: destroy                       # what an uncontrolled close does in svelters (normalised)
  internal_children:            # borrowed sub-objects reachable from JS
    - name: lineEdit
      get: "QComboBox::lineEdit() const"
      exists_when: "editable == true"
      invalidated_by: ["QComboBox::setEditable(bool)", "QComboBox::setLineEdit(QLineEdit*)"]
methods / ctors / slot ops:
  params:  [{ name, ownership: borrowed | transfer_full | transfer_floating | parent_to_self | adopt | reference_kept | reference_weak }]
  returns: { ownership: none | full | floating | borrowed | caller_owns_unparented }   # takeCentralWidget: caller_owns_unparented
  effects:                      # existing slots vocabulary (signals-and-slots §2.5) + two keys
    child: parented | unparented | deleted | wrapped:<Kind> | kept_child_hidden
    old: deleted | unparented | unref | kept_child | referenced_only
    default: deleted | kept_child | null
    self: deleted | deleted_later                     # e.g. QWidget::close() with WA_DeleteOnClose
    args: { <param>: deleted | parented | referenced }  # QFormLayout::removeRow(int): {label: deleted, field: deleted}
  hazards: [ null_arg_crash, deletes_synchronously, blocking_nested_loop, invalidates:<getter>, emits_during_dispose ]
```

**Where the facts come from:**

| Field | GTK | Qt | Manual/probe |
|---|---|---|---|
| `params[].ownership`, `returns.ownership` | GIR `transfer-ownership="none\|full\|container\|floating"` plus the `InitiallyUnowned` base gives `floating` | PySide typesystem `<parent index="this" action="add\|remove">`, `<define-ownership owner="c++\|target">`, `replace_child`; qdoc phrases ("takes ownership", "is deleted") as *suggestions* only | anything the sources miss |
| `effects` | probe (GTK never deletes on replace: `old: unref`) | `qt_slot_ownership_probe.py` facts | yes |
| `lifetime.close` | `close-request` signal metadata | `closeEvent` virtual | yes |
| `internal_children` | `gtk_widget_get_template_child` / known composites | qdoc getters returning `T*` with no ctor arg | yes |
| `hazards` | — | `setTabBar(nullptr)` crash, `removeRow` delete | probe |

**What gets generated:**

- **Mint class:** each `get_prop`/`call` that returns an object mints `borrowed` or `value` according to `returns.ownership`. `full`/`floating` returns mint `owned`, after `ref_sink` on GTK.
- **Thunks:**
  - `transfer_full` inputs get `g_object_ref` first.
  - `params: adopt` re-classifies a borrowed handle as owned.
  - `effects.child: deleted` / `args.*: deleted` ops call `on_destroyed` eagerly when the toolkit doesn't (GTK can't delete; Qt fires `destroyed` anyway).
  - `null_arg_crash` turns into a generated guard that returns an error (`setTabBar(nullptr)`, `setLineEdit(nullptr)`, which is ignored with a warning).
- **Tests:** one per fact, run headless.
  - Liveness through `QPointer` / `GWeakRef` after flushing `DeferredDelete` (Qt) or **draining for about 600 ms** (GTK).
    - G4 found that a clicked, mapped `GtkButton` keeps 2 extra refs for about 250 ms from an internal timeout. Idle-only draining reports false leaks (`gtk_probe3.rs`).
  - Expected `on_destroyed` counts and reasons.
  - Cascades for toplevel close.
  - Borrowed invalidation (`invalidated_by` → old handle stale, new getter → new handle).
  - `hazards: null_arg_crash` is verified in a forked child that is expected to crash.

**Toolkit traps the generated tests must keep pinned (§2.4).**

### 2.4 Toolkit traps (facts the tests must keep pinned)

| Trap | Fact | Source |
|---|---|---|
| `QLayout::removeWidget` | leaves the widget parented and visible; remove must `setParent(nullptr)` | Q1, slots §2.1 |
| `QMainWindow::setCentralWidget` | deletes the old widget; `takeCentralWidget` unparents; `setCentralWidget(nullptr)` deletes | Q3, slots §2.1 |
| `setMenuBar(nullptr)` / `setStatusBar(nullptr)` | delete the bar; the getters lazily create a new one | slots §2.1 |
| `QFormLayout::removeRow` | deletes both label and field; `takeRow` keeps them | Q3 |
| `QTabWidget::setTabBar(nullptr)` | segfaults (no null check) | slots §2.1 |
| `QComboBox::setEditable(false)` | deletes the line edit; `true` makes a new one | Q3 |
| `QScrollArea::setWidget` | deletes the old widget, reparents the new one into the viewport; `setViewport` deletes the old viewport | Q3, P4 notes |
| `delete` vs `deleteLater` | sync delete in a slot skips later slots; `deleteLater` doesn't | Q4 |
| `QObject::destroyed` | for a QWidget: fires before children die, QPointer still set, still castable to QWidget; for a QObject, QPointer is null | Q5 |
| `quitOnLastWindowClosed` | defaults to true; a hidden last window quits the loop | Q2 |
| GTK dispose vs finalize | `destroy` signal = dispose; a widget is disposed only on its last unref, except toplevels via `gtk_window_destroy` *if nobody else holds them* | G2b, G5 |
| `gtk_window_destroy` with a held ref | no dispose and no signal; zombie window; re-present warns about inconsistent state | G5, `gtk_probe2.rs` |
| GTK container removal | drops the parent's ref; finalizes at once if the table held none (`b_fin`) | G1 |
| GTK window close | default handler destroys; held descendants survive orphaned | G2a |
| ScrolledWindow auto-Viewport | `set_child(NULL)` unparents the user child and drops the viewport | G3 |
| Transient GTK refs | ~250 ms after a click | G4 |

### 2.5 WinRT and Apple

The four lifetime classes and the cascade carry over. What changes is the refcount kind and how a death is noticed.

**WinRT / WinUI 3 (COM refcount, XAML tree):**

- `owned`: the table holds a COM ref (`winrt::com_ptr`/projected object). A `Panel.Children` / `ContentControl.Content` parent also holds one. `remove` drops the parent's ref, and `destroy` drops ours. It works like GTK without floating refs.
- `toplevel`: `Microsoft.UI.Xaml.Window`.
  - The veto is `AppWindow.Closing` with `args.Cancel(true)`.
  - After `Closed`, the object refuses calls (RO_E_CLOSED) even if it is still referenced. That's a zombie like GTK's, so the backend makes the handle stale on `Closed` and cascades.
- `borrowed`: template parts (`GetTemplateChild`) exist only after `OnApplyTemplate` and are replaced when the template changes. Hold `winrt::weak_ref`.
- **COM has no destroy notification.** Staleness is detected lazily, when `resolve()` fails at the next access. The ABI allows this: `on_destroyed` is "when known", and ops on a dead borrowed handle return an error with reason `OWNER`.
- `Unloaded` isn't death, because it also fires on reparent.
- Generated event handlers must capture `get_weak()` and use `auto_revoke` tokens, revoked at `destroy`. Otherwise native↔native delegate cycles leak the window. That's the WinRT version of "native never holds JS".

**Apple (AppKit/UIKit, ARC):**

- `owned`: the table holds a strong ref, and `superview` retains its subviews. `removeFromSuperview` releases the superview's ref, so a detached view survives on our ref (GTK-like).
- `toplevel`: `NSWindow` must be created with `releasedWhenClosed = NO`.
  - `windowShouldClose:` is the veto.
  - `windowWillClose:` is the death notification, then drop our ref.
- **View-controller containment:** `addChildViewController:` retains the child VC, and a VC owns its `view`.
  - Model VCs as elements whose `view` is a borrowed handle.
  - `removeFromParentViewController` + `view.removeFromSuperview` together are the `remove` op.
- `borrowed` (`NSScrollView.contentView`, `NSTableView` cell views): hold `__weak`.
  - To get an eager destroy notification, attach an associated-object sentinel (`objc_setAssociatedObject`) whose `dealloc` calls `on_destroyed`. That's the ObjC equivalent of GObject weak notify.
- **Recycled views** (table/collection cells, GTK ListView rows, WinUI `ItemsRepeater`) must **not** be handed out as borrowed handles, because identity changes under recycling. Rows are model-driven snippets (M5), and the factory owns them.

## 3. Experiments

Sources and outputs are in `docs/notes/scripts/ownership/`:

- `qt_ownership.cpp` + `.out.txt`: Qt 6.11.0, Xvfb. Build with `g++ -std=c++17 qt_ownership.cpp $(pkg-config --cflags --libs Qt6Widgets Qt6Test)`.
- `rs/`: gtk4-rs 0.11.5 (GTK 4.22.4) and rquickjs 0.14.0. Build with `CARGO_BUILD_BUILD_DIR=target/build cargo build -j4`, and run GTK with `GSK_RENDERER=cairo xvfb-run -a` (the Vulkan renderer aborts under Xvfb).

Everything passes (`FAILURES 0`).

| # | Case | Qt result | GTK result |
|---|---|---|---|
| 1 | remove, keep detached, parent dies, re-insert elsewhere | after `removeWidget` + `setParent(nullptr)` the widget survives the parent's `delete` and re-inserts into another layout; its sibling (still a child) dies and reports `on_destroyed` | survives on the table ref after `remove` and the window's death, re-inserts and maps; without a table ref it finalizes on `remove` |
| 2 | native close while handles are held | default close only **hides** (all handles live); `WA_DeleteOnClose` deletes on the next loop turn, window then child, with `on_destroyed` for both; `quitOnLastWindowClosed` default = true | default close destroys the weakly held window (`destroy` + finalize); **a held child stays alive, undisposed and orphaned** with its grandchild still attached; veto via `Stop` keeps it open; `hide-on-close` hides without destroy |
| 3 | owner deletes a borrowed child | `setEditable(false)` deletes the line edit (`on_destroyed` fires; a new line edit is a different object); `setViewport`, `QFormLayout::removeRow` (both), `setCentralWidget` (old) all fire; `takeCentralWidget` unparents | auto-Viewport held weakly is finalized when ScrolledWindow drops it; held strongly it survives, orphaned; Entry's internal GtkText dies with the Entry |
| 4 | delete during signal emission | `deleteLater` in its own `clicked`: later slots run, freed next turn; sync `delete`: no crash under freed-memory scribbling, **later slots skipped**; parent `deleteLater` from child signal: fine | dropping the last table ref in `clicked` after `remove`: second handler runs, object outlives the emission, finalized afterwards (**~250 ms later**, transient refs); `gtk_window_destroy` from its own `close-request` with a weak table ref: destroyed |
| 5 | GC finalizers after destroy (QuickJS-NG via rquickjs 0.14) | `FinalizationRegistry` + `WeakRef` available. FR callbacks run as **jobs after `run_gc`**; for an explicitly destroyed handle the callback's release is a stale no-op; for a never-destroyed one it is the only release. Closure cycles (node ↔ listener) are collected. A Rust class `Drop` runs **synchronously inside `eval`** when the last ref goes (`w = null`); after an explicit destroy its release is a stale no-op; remaining class objects are dropped at runtime teardown | — |
| — | destroyed timing | QWidget: `destroyed` before its children, QPointer non-null, castable; layout first; QObject child: QPointer null | `destroy` signal only at dispose; a held toplevel after `gtk_window_destroy` is a zombie (no signal, not in toplevel list, child still set; re-present warns) |

Not done: valgrind can't run here (no glibc debuginfo), so I used `MALLOC_PERTURB_` + `tcache_count=0` scribbling instead, and those runs were clean. Not probed: Qt deferred delete inside nested `exec()` loops, WinRT, Apple.

## 4. Proposed ABI changes (v2, to go through `backend-abi.md`)

```c
/* Handles: never reused by a backend instance; always < 2^53. */

typedef enum svl_lifetime {
  SVL_LIFE_OWNED    = 1,  /* created by `create`; table strong ref; destroy = native destroy */
  SVL_LIFE_TOPLEVEL = 2,  /* window kinds; natively closable; destroy = native window destroy */
  SVL_LIFE_BORROWED = 3,  /* minted from a getter/return; weak; destroy = forget only */
  SVL_LIFE_VALUE    = 4   /* refcounted immutable value; destroy = unref */
} svl_lifetime;

typedef enum svl_gone {
  SVL_GONE_UNKNOWN  = 0,
  SVL_GONE_CLOSED   = 1,  /* toplevel closed natively (uncontrolled close) */
  SVL_GONE_PARENT   = 2,  /* an ancestor died (cascade) */
  SVL_GONE_OWNER    = 3,  /* the native owner deleted/replaced a borrowed object */
  SVL_GONE_REPLACED = 4,  /* a slot op deleted it (effects old/child/args: deleted) */
  SVL_GONE_SHUTDOWN = 5
} svl_gone;

typedef struct svl_handle_info { svl_id kind; uint32_t lifetime; uint32_t flags; /* SVL_HI_INSERTED=1 */ } svl_handle_info;

/* backend vtable: added */
int (*handle_info)(void *self, svl_handle h, svl_handle_info *out);   /* -1 + last_error if stale */

/* host callbacks: changed signature */
void (*on_destroyed)(void *self, svl_handle h, uint32_t reason /* svl_gone */);
```

**Semantics:**

1. **`destroy(h)`:**
   - Idempotent: an unknown or stale `h` is a silent no-op.
   - `h` is stale as soon as the call returns, whatever the toolkit's deletion timing.
   - Its effect depends on the lifetime class (table in §2.2). It never calls `on_destroyed(h)` for `h` itself.
   - It *may* cascade `on_destroyed(d, PARENT)` to handled descendants. Qt does that through `destroyed`; GTK does it explicitly when it destroys a toplevel.
2. **`on_destroyed(h, reason)`:**
   - Called at most once per handle, only for handles still in the table, and only after the backend has erased `h`.
   - It can arrive inside any other vtable call, on the main thread.
   - The host must only record it (tombstone + a queued notification). It must not call into JS or into the backend for `h`.
   - Parents may be reported before children (Q5).
3. **Minting:**
   - `get_prop`/`call` results with tag `SVL_HANDLE` may name a handle the host hasn't seen. The host calls `handle_info` once to learn its kind and lifetime.
   - One handle per native object, identified through qdata, a dynamic property or a hash erased on `destroyed`. Never an address map that outlives the object.
4. **Toplevel close:**
   - If `listen(h, close-request, on)` is active, the backend vetoes and posts the event.
   - Otherwise it destroys natively and reports `CLOSED`, plus the cascade.
   - Qt backends set `quitOnLastWindowClosed(false)`. The host decides when to `quit` (the last toplevel handle dies, unless the app opted out).
5. **Host → JS:**
   - At checkpoint step 1, the host delivers `__svelters_destroyed(h, reason)` in queue order, interleaved with events.
   - The renderer marks the node and its subtree `dead`, calls `destroy` on the descendants (a no-op where they're already gone, a release of GTK holds otherwise), and runs the stale rules in §2.1.
6. **Releases from GC:**
   - Go through a host queue drained at checkpoints. They are never issued from a QuickJS finalizer directly.
   - They become no-ops after shutdown starts. The host drops the JS runtime before the backend.

**Bump `abi_version` to 2.** It can ship together with `schedule_wake`, which is already planned for v2.

**Follow-ups for the lanes (not done here):**

- **GTK backend:**
  - Cascade on toplevel destroy.
  - Replace the address-keyed `by_obj` with qdata.
- **Qt template:**
  - A `closeEvent` filter for toplevels.
  - `quitOnLastWindowClosed(false)`.
  - Mark `close()` paths `WA_DeleteOnClose`-equivalent when uncontrolled.
- **Host:** forward `on_destroyed` to JS through the queue, and keep tombstones.
- **Renderer:** `dead` state + `StaleHandleError`, and a `FinalizationRegistry` for borrowed and value wrappers.
- **Metadata:** the `lifetime`/`ownership`/`hazards` fields, with the GIR and typesystem ingestion into them.
