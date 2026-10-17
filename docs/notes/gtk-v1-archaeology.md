# GTK v1 archaeology (2021)

Read-only survey of the 2021 GTK backends of svelters. All refs are `path@sha`.
Inspect with `git show <sha>:<path>`. Paths marked **[zig]** belong to the private repo
`github.com/akiselev/svelters-zig`. That repo is **not** in this repository's object store.
It was pinned as the `zig/` submodule from 22f9afc to 5a3641e, and you need a separate clone to read it.

## TL;DR

- Every GTK attempt used **GTK4**, never GTK3. Rust crates: `gtk4 = 0.3.x` (features `v4_2`),
  `glib/gio 0.14.x`, `gdk4 0.3.1`. The Zig build ran `linkSystemLibrary("gtk4")`.
- The repo had three GTK backends, and two of them really worked:
  1. **Rust gtk-rs v1** (Sep 2021, `gtk4/` crate). It only got as far as a window, a button and a click event. Superseded.
  2. **Zig + GTK4 C API + raw QuickJS** (Oct 8 to Nov 7 2021, svelters-zig). This one had
     **named native slots, reactivity, styles, hot reload and a working GtkListView** ("list view works!!!!").
  3. **Rust gtk-rs v2** (Nov 12 to 18 2021, `gtk/` crate, still present in HEAD). It covered GIR codegen,
     dynamic GObject props and signals, GtkListView, and Svelte `{#each}` + `<slot>` rows via `listViewProxy`.
     This one is "slots and dynamic lists" in the Svelte sense. The Qt rework came after it
     (f838fc8, 2021-11-21), so this is the GTK version that is "older than the Qt version in recent history".
- **Best commits to read:**
  - Rust v2: **04ea4eb** (2021-11-18, "fixing up listview"). This is the last GTK-only `src/main.rs` before the Qt main was split out.
    9eb173a (= the code in HEAD) adds the unfinished `gtk/marshall/` and comments it out.
  - Zig: **svelters 7f40742**, which pins **[zig] 8482af1** (2021-11-06, "list view works!!!!").

## Timeline

| Date | SHA | Event |
|---|---|---|
| 2021-06-06 | 4b15e1f | Project starts on Qt (cpp! macro, qode event loop) |
| 2021-09-01 | 9777649 | `gtk4/` lib created as a copy of the qt folder |
| 2021-09-10 | ad421e2 | "rearchitect to invert event loop control flow and switch to gtk-rs" |
| 2021-09-24 | 2204fea | "gtk4 remake" (`gtk4/Cargo.toml`: gtk4-macros 0.3, glib 0.14.5) |
| 2021-10-07 | 7bb80a7 | "click event on button works again" (end of Rust v1) |
| 2021-10-08 | [zig] fa7ddcd | svelters-zig: "zig + gtk works!" |
| 2021-10-23 | 98e0958 / 22f9afc | Rust gtk4 removed; zig added as submodule |
| 2021-10-27 | 9831812 / [zig] 3c31464 | reactivity; "window slots w/ finalizers" |
| 2021-10-28 | [zig] 1a1dee3 | "window children with titlebar child slot working!" |
| 2021-10-29 | 1557b1d / 9603e4e | layout children; CSS styles |
| 2021-10-31 | b4fdfb5 / 68e2e63 | two-way `bind:` works; child order updates correct |
| 2021-11-01 | 151d240 | hot reload works |
| 2021-11-04 | d0d10f7 / db11bd6 | generic signal marshaller; GC fixed |
| 2021-11-06 | **7f40742 / [zig] 8482af1** | **list view works** |
| 2021-11-07 | [zig] a2fd2b0..fdc7052 | GC rewrite following ptomato (GJS) blog posts; events/hooks ripped out (last zig commit) |
| 2021-11-12 | e290223 | "switching back to rust *facepalm*"; adds `docs/libs/gtk.md` (debug builds of GLib/GTK4) |
| 2021-11-12 | 1182083 | GIR codegen adopted from ts-for-gjs |
| 2021-11-13/14 | bb9a4ec, c0d6dc5 | button + handler; create/destroy buttons |
| 2021-11-14 | 50171ac | full static codegen gives 16-minute release builds, so the code switches to lazily loaded classes |
| 2021-11-17 | e6a4911 | Application subclass API; allocation fixes (rquickjs `rust-alloc`) |
| 2021-11-18 | **04ea4eb** | ListView with `listViewProxy` + `<slot>` |
| 2021-11-21 | f838fc8 | Qt again ("qt_metacall only works with Q_INVOKABLE") |
| 2021-11-24 | 9eb173a | qt and gtk/marshall commented out; end of 2021 work |

## Architecture: Zig version ([zig] 8482af1, host svelters 7f40742)

- **Host and event loop:** Rust `src/main.rs@7f40742` builds the rquickjs runtime. It calls
  `initialize_svelters_gtk(JSContext*, GtkApplication*)`, exported by the Zig static lib `svzig`
  (`src/lib.rs`@[zig]8482af1). GLib owns the loop: `GtkExecutor` spawns the rquickjs executor through
  `MainContext::default().spawn_local`. After `activate`, pending JS jobs are drained.
  Hot reload uses `hotwatch` + `flume` on `dist/index.js` and calls the module's `default(true)`.
- **JS bindings:** every GTK class is hand-written as a comptime Zig struct
  (`src/gtk/widgets/*.zig`, `src/gtk/widget.zig`, `object.zig`). Fields like
  `api.AutoProperty(gtk_list_view_get_x, gtk_list_view_set_x)`, `GObjectProperty`,
  `GtkEnumProperty{type_fn}` and `AutoMethod(fn)` infer their JS marshalling from the `@cImport`ed C signatures.
  `implements = .{Object, Widget}` gives the inheritance. `src/typedef/*.zig` builds QuickJS classes from these
  structs, including `getAttribute/setAttribute` (`typedef/attributes.zig`), and `typedef/codegen.zig`
  emits TS typings. No GIR is used.
- **Identity and GC:** `GObjHook` (`src/gtk/object.zig`) stores a `BackRef{JsValue, ctx}` in qdata
  `__svbackref__` and adds a **toggle ref**. It gcMarks the JS wrapper only while GTK holds the last
  ref. This is the GJS design ([zig] 3870f87, 4c25849). Getters return `backref.val.clone()`, which gives stable JS identity.
- **Signals ("on:"):** `src/gtk/signals.zig`, `SignalHandler`, uses a single `g_cclosure_new` per (object, signal)
  with `g_closure_set_meta_marshal`. `closure_marshal` converts each GValue through
  `Registry.toJs`, builds `{type, details:[...args]}`, dispatches to all JS subscribers, and writes the
  return value back through `Registry.fromJs`. It disconnects when the last listener goes away. It forces `rt.gc()` after every dispatch.
- **Fake DOM** (`app/dom/*@7f40742`): JS wrapper objects (`widgets/widget.ts`, `layout.ts`,
  `singleton.ts`) keep a parallel parentNode/sibling list and map DOM operations onto GTK:
  - Layout (Box): `insertChildAfter(node, target.prevSibling)` / `append` / `remove`.
  - Singleton (Window, ScrolledWindow, Viewport): `$.child = node`.
  - **Named slots:** `<child:titlebar>`, `<controller:motion>`, `<start>/<end>` (Paned) come through
    `document.createElement("ns:tag")` / `createElementNS` and become `nodes.Child(name, el)`, which the
    parent routes through `hooks[name]`. Examples: `$.titlebar = node`, `addController`, `startChild/endChild`.
    See `app/dom/widgets/index.ts@7f40742`.
  - `<style>` maps to a GtkCssProvider; `class` maps to `cssClasses`.
- **Lists:** a custom C GObject `GJsListModel` (`src/gtk/lib/GJsListModel.c`) implements
  `GListModel` on top of a JS array or a JS function `(index?) => count | item`. Items are `GJsValue` GObjects
  that box a JSValue (`GJsValue.c`). The JS side is `app/ListView.svelte@7f40742`:
  `new JsListModel(fn)` → `NoSelection` → `ListView(selection, SignalListItemFactory)`.
  `setup` creates an hBox and **instantiates a Svelte component `new Item({target: box})`** per row,
  `bind`/`unbind` call `comp.$set({data})`, and `teardown` calls `comp.destroy()`.

## Architecture: Rust v2 (04ea4eb; HEAD is equivalent plus gtk/marshall)

- **Metadata:** GIR XML parsed by a vendored **ts-for-gjs** (`gtk/codegen/*`, `gir-module.ts`).
  It generates `gtk/types/generated.rs` (type initialisers) and TS typings. Full per-class codegen was abandoned
  at 50171ac because of build time.
- **Runtime reflection** (`gtk/core/from_gtype.rs@04ea4eb`): for each GType, `init_gtype` walks
  `Class::list_properties()`. For every ParamSpec it defines a camelCase JS accessor (`Prop<T>`, `EnumProp`,
  `ObjectProp`). `addEventListener` calls `obj.connect(name, false, handle_event)`
  with generic GValue↔JS conversion (`core/generic_val.rs`). Callbacks are kept in qdata `svEvents`.
- **Registry** (`gtk/registry.rs@04ea4eb`): it keeps lazy prototypes per GType and assigns the prototype in `marshall()`
  for each object. `marshall()` creates a **new JS wrapper on every crossing**, so per-object JS state lives in qdata
  (`gtk/obj_data.rs`, exposed as `svExtra`). That explains the ListView comment
  "To preserve GTK Object identity between calls".
- **Fake DOM** (`app/dom/proto/base.ts`, `index.ts@04ea4eb`): DOM methods are patched straight onto the
  native GObject prototypes (`initBaseNode("Box", GtkBox)`), with no wrapper objects. Tree state lives in `svExtra`.
- **Lists + slots:** custom GObjects `CallbackListModel` / `ListItemValue`
  (`gtk/types/custom/list_model.rs`, `list_item.rs@04ea4eb`). `app/ListView.svelte@04ea4eb` keeps a
  reactive set of live GtkListItems from setup/teardown and renders
  `{#each items as item (item.svExtra.id)} <listViewProxy child={item.child}><slot item=.../>`.
  `ListViewProxy` (`app/dom/proto/proxies.ts`) is a **portal** that forwards
  insertBefore/appendChild/removeChild to the row's GtkBox, queueing them until the box exists.
  HEAD factors it into `app/ItemProxy.svelte` with a per-row writable store set on bind/unbind.
- **Event loop:** `gtk/application.rs`, again `MainContext::spawn_local` + `MainContext::channel`.

## GTK4 bugs: what the history records

No commit message, comment or note in either repo names a specific upstream GTK4 bug. The evidence is indirect:
- `docs/libs/gtk.md` (added in e290223, the "switching back to rust" commit) describes building **debug GLib and GTK4
  from source** into `zig/lib/{glib,gtk}` and running against them through `LD_LIBRARY_PATH`. The author was stepping into GTK itself.
- ListView rows: `onSetup` guards `if (!item.child)`, and teardown has to null both `item.child` and `item.item`.
  This suggests factory setup/teardown and recycling ran out of the expected order (`app/ListView.svelte@04ea4eb`).
- Windows: `hideOnClose={true}` + explicit `window.$.destroy()` in `onDestroy` (`app/App.svelte@7f40742`).
  Event controllers (`<controller:motion>`) are commented out in `app/App.svelte@9eb173a`, and `GtkLabel` registration is commented out
  (`app/dom/proto/index.ts`).
- Most of the recorded pain is in the **bindings, not GTK**:
  - toggle-ref crashes (d1f2629)
  - QuickJS↔GObject reference cycles and GC: db11bd6, d5bcadd, 4c25849, and [zig] 330dbb7/c7ca0f0 "GJsValue GC problem remains"
  - use-after-free on signal disconnect ([zig] f28b406, c9f6568)
  - uninitialised GValue segfault ([zig] bab2b44)
  - refcount assertion fixed by cloning ([zig] 7794552)
  - allocator mismatch (e6a4911)

## Lessons for a rewrite

1. Use runtime GObject introspection (ParamSpec and signal queries) for props and signals, plus GIR only for typings and docs.
   Static per-class codegen made release builds take 16 minutes.
2. Keep one canonical JS wrapper per GObject (qdata backref + toggle ref, as in GJS). Re-wrapping on
   every crossing forced the `svExtra` workaround. Write down the GC contract up front.
3. Use one generic closure marshaller per (object, signal) and fan out to JS listeners. The `{type, details}` event shape worked.
4. Named native slots through namespaced tags (`child:titlebar`, `controller:*`), handled by per-parent hooks,
   are a clean way to map Svelte onto non-ordered GTK children.
5. For virtualised lists, use a GListModel backed by a JS callback, then a SignalListItemFactory, then a portal (`listViewProxy`)
   so that rows are ordinary Svelte `{#each}`/`<slot>` content driven by a per-row store. This fits SVR16.
6. Let GLib own the main loop and drive QuickJS jobs from `MainContext::spawn_local`.
