# X1: borrowed handles with stable identity, GTK dynamic method invocation, Qt method parity

Status: started 2026-10-17. Lane X1 of the framework work after M2a. Depends on ABI v2 (lifetime classes, `handle_info`, `on_destroyed` reasons). Does not touch `svl_value` (S1, ABI v3), event dispatch (X2) or `metadata/overrides` (A1).

## Goals

1. **Borrowed handles with stable identity** (GTK and Qt). A `get_prop` or `call` that returns a native object we didn't create mints a BORROWED handle. The same native object always maps to the same live handle, and an object we created returns its existing OWNED/TOPLEVEL handle.
2. **GTK dynamic method invocation.** `call(h, method, args)` for any introspectable method, through girepository-2.0 (`gi_function_info_invoke`, which drives libffi) with the typelib read at runtime.
3. **Qt method parity.** Object-returning getters mint borrowed handles; the generated dispatch gains object arguments and returns.
4. **Tests** for identity, borrowed death with its reason, no leaks, every supported GI argument/return type, and a `bind:this` + `call` fixture on both toolkits.
5. **Gaps** from `metadata/A1-GAPS.md` that fall in this area.

## Design

### Identity and lifetime

| Part | GTK | Qt |
| --- | --- | --- |
| object -> handle | GObject qdata `(backend id, handle)`, cleared when the handle is forgotten | `std::unordered_map<QObject*, svl_handle>` side table, erased in the `destroyed` slot before the address can be reused |
| borrowed reference | `glib::WeakRef` plus a `g_object_weak_ref` notify; no strong reference | `QPointer` plus the existing `destroyed` connection |
| native death | notify reports `SVL_GONE_OWNER`; a toplevel close cascades `SVL_GONE_PARENT` to inserted children and to borrowed handles minted below them | `destroyed` reports `SVL_GONE_OWNER`; the close filter cascades `PARENT` to every object with the window in its parent chain |
| `destroy` on borrowed | forget only: drop entry, qdata and notify | forget only: drop entry, side-table row and connection |
| insert/remove | error: borrowed handles are never part of the JS tree | same |
| `kind` of a borrowed handle | nearest ancestor GType that has a generated kind | nearest ancestor class that has a generated kind (0 if none) |

`transfer full` returns of an object nobody holds mint an OWNED handle (the table adopts the reference); `transfer none` returns mint BORROWED.

### Host and renderer

- Host: a handle that comes back inside a `get_prop`/`call` result and is not in the handle table is registered through `handle_info` (kind, lifetime class). Borrowed handles are not toplevels for quit purposes. A native death of a borrowed handle goes through the existing `on_destroyed` path.
- Host JS binding: handles in results become renderer nodes through `globalThis.__svelters_node(h)` (installed by `@svelters/renderer`). Nodes passed as `call`/`setProp` arguments convert back to handles.
- Renderer: `__svelters_node` returns the realized element for an owned handle (identity) or a borrowed node (`borrowed: true`, held weakly, forgotten through a `FinalizationRegistry` that calls `destroy`). Borrowed nodes throw if inserted or removed.

### GTK dynamic methods (`crates/svelters-gtk/src/gi.rs`)

- The generated `METHODS` table holds only annotated no-argument methods (`present`) and carries no signatures, so it cannot drive calls. Method ids beyond `METHODS.len()` are dynamic: `resolve(SVL_METHOD, kind, name)` looks the method up in the Gtk-4.0 typelib (`gi_repository_find_by_gtype` for the kind, then `find_method` up the parent chain and through interfaces), builds an argument/return plan once and caches it per `(kind, name)`.
- JS names may be `snake_case` or `camelCase` (`getParent` finds `get_parent`).
- Plan errors are stored with the cached entry and reported by `call`, so the message names the unsupported feature instead of "unknown method".
- Supported: bool, int8..uint64, float, double, UTF-8 and filename strings, enums, flags, GObject-derived objects and interfaces (handles), all nullable where the typelib says so; `transfer` annotations respected on arguments (ref before the call) and returns (free or adopt). Rejected with a clear error: varargs (not in the typelib), callbacks and their user_data/destroy parameters, out/inout parameters, arrays, lists, hash tables, structs/boxed/unions, GType, unichar. A `throws` GError is raised as an exception. Follow-ups are recorded in `docs/notes/x1-borrowed-gi.md`.
- Release builds (M6) replace this with generated thunks that read the same plan shape.

### Qt (`tools/metadata/qt`)

- The generator emits a second dispatch, `gen_object_getters`, from the qdoc metadata of every QObject-derived class: public non-static methods with no required parameters that return a pointer to a QObject-derived class we also include. Their ids live in a separate range so that `resolve` finds them on any kind. `take*`/`create*`/`new*`/`clone*` names are skipped because the caller owns the result.
- The generator's annotated-method path accepts `object<T>` returns and arguments.
- The result is interned by the backend: existing handle if the object has one, otherwise a BORROWED entry.

## Steps

1. GTK identity and borrowed handles (qdata, weak notify, `handle_info`, cascade, insert guard) with backend tests.
2. GTK GI invocation with tests per type.
3. Host registration of minted handles, JS node wrapping, renderer borrowed nodes.
4. Qt identity, borrowed handles, getters generator, tests.
5. Fixture `borrowed-walk` on both toolkits; docs; gaps.

Each step is committed on its own; gates before each commit worth merging.
