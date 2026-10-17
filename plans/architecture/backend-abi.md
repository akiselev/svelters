# Backend ABI (L2 ↔ L3)

Status: draft for the P lanes. Changes go here first, then into `crates/svelters-abi/include/svelters_backend.h`. The header and its Rust mirror `crates/svelters-abi/src/lib.rs` are authoritative for exact spelling: the value union is named `u`, and `svl_str`/`svl_list` are named structs.

## Goals

- One fixed C ABI between the Rust host core and every backend, whatever the backend's language (Rust for GTK/egui, C++ for Qt).
- The ABI never grows per native method, property or widget kind. Names resolve to `u32` IDs once, and per-ID behaviour lives in backend code (hand-written in P, generated from M3).
- JS never sees native pointers. The host keeps a handle table; backends see opaque `svl_handle` values that they map to their own objects.

## Data model

```c
typedef uint64_t svl_handle;   /* 0 = null; issued by the backend on create */
typedef uint32_t svl_id;       /* resolved kind/prop/signal/method/slot id; 0 = unknown */

typedef enum svl_tag { SVL_NULL, SVL_BOOL, SVL_I64, SVL_F64, SVL_STR, SVL_HANDLE, SVL_LIST } svl_tag;

typedef struct svl_value {
  svl_tag tag;
  union {
    bool b; int64_t i; double f; svl_handle h;
    struct { const char *ptr; size_t len; } s;              /* UTF-8, borrowed for the call */
    struct { const struct svl_value *ptr; size_t len; } l;  /* borrowed for the call */
  };
} svl_value;
```

Strings and lists are borrowed for the duration of a call. Values a backend returns go into a host-provided out slot and are copied by the host before the next ABI call.

## Backend vtable (implemented by each backend)

```c
typedef enum svl_name_kind { SVL_KIND, SVL_PROP, SVL_SIGNAL, SVL_METHOD, SVL_SLOT } svl_name_kind;

typedef struct svl_backend {
  uint32_t abi_version;
  void *self;

  /* names → ids. `kind` scopes prop/signal/method/slot lookups (0 for SVL_KIND). */
  svl_id (*resolve)(void *self, svl_name_kind what, svl_id kind, const char *name, size_t len);

  /* tree */
  svl_handle (*create)(void *self, svl_id kind);
  int  (*insert)(void *self, svl_handle parent, svl_handle child, svl_handle before /*0=end*/, svl_id slot /*0=default*/);
  int  (*remove)(void *self, svl_handle parent, svl_handle child);
  void (*destroy)(void *self, svl_handle h);              /* host drops its last reference */

  /* properties / methods */
  int  (*set_prop)(void *self, svl_handle h, svl_id prop, const svl_value *v);
  int  (*get_prop)(void *self, svl_handle h, svl_id prop, svl_value *out);
  int  (*call)(void *self, svl_handle h, svl_id method, const svl_value *args, size_t n, svl_value *out);

  /* signals: the backend connects natively and reports through host->post_event */
  int  (*listen)(void *self, svl_handle h, svl_id signal, bool on);

  /* loop: the backend owns the toolkit loop */
  int  (*run)(void *self, const struct svl_host *host);   /* returns when the app quits */
  void (*wake)(void *self);                               /* thread-safe: schedule a host checkpoint */
  void (*quit)(void *self);

  const char *(*last_error)(void *self);                  /* valid until the next call */
} svl_backend;
```

Return codes: `0` = ok, negative = error with `last_error`. A stale or foreign handle is an error, never a crash. Backends track liveness: `QPointer` for Qt, weak refs or a destroy notify for GObject.

## Host callbacks (implemented by the Rust core)

```c
typedef struct svl_host {
  void *self;
  /* queue an event; delivered to JS at the next checkpoint */
  void (*post_event)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n);
  /* run queued events + JS jobs + flush; the backend calls this from its loop after wake() */
  void (*checkpoint)(void *self);
  /* synchronous handler call for return-value signals/virtuals (M4; see open questions) */
  int  (*call_sync)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n, svl_value *out);
  /* native object went away underneath us (parent destroyed it, window closed) */
  void (*on_destroyed)(void *self, svl_handle h, uint32_t reason /* svl_gone */);
} svl_host;
```

## Checkpoint semantics

1. Deliver queued native events to JS.
2. Run Svelte effects and QuickJS jobs, up to a bounded number.
3. Apply the resulting tree/prop operations through the vtable.
4. If jobs remain, call `backend->wake()` and return to the toolkit loop.

The P milestone applies ops directly instead of batching. Batching comes later if measurements justify it.

## JS-side host ops

The L1 adapter calls these and the host forwards them to the vtable:

```ts
interface SveltersHost {
  create(kind: string): Handle;
  insert(parent: Handle, child: Handle, before: Handle | null, slot?: string): void;
  remove(parent: Handle, child: Handle): void;
  destroy(h: Handle): void;
  setProp(h: Handle, name: string, value: unknown): void;
  getProp(h: Handle, name: string): unknown;
  call(h: Handle, method: string, args: unknown[]): unknown;
  listen(h: Handle, signal: string, on: boolean): void;
}
// events: globalThis.__svelters_dispatch(h, signal, args) is invoked at checkpoints
```

The JS host caches name→ID resolution per kind. The Node TestHost implements this interface in JS and records an op log. The QuickJS host implements it natively. A given fixture must produce the same op log in both.

**Widget creation timing.** Svelte clones each template with `cloneNode(true)` for every instance, so the shim creates the native widget when a node is first inserted under a native-backed parent, not when the node is constructed or cloned.

## Qt build: leaf crate

The Qt backend is C++. A Rust codegen crate generates it and compiles it inside a project-owned leaf crate.

```text
my-app/                     (bin; depends on svelters-host + my-app-qt)
my-app-qt/                  (leaf crate: no project deps, nothing depends on it except my-app)
  Cargo.toml                [build-dependencies] svelters-qt-build
  build.rs                  svelters_qt_build::Config::new()
                              .qt_from_pkg_config("Qt6Widgets")
                              .profile_dev_fat()              // all annotated kinds/methods
                              .release_roots("../dist/svelters-bindings.json") // M6
                              .extra_roots(["QTreeView::scrollTo"])            // dynamic calls tree-shaking can't see
                              .build();
  src/lib.rs                pub fn backend() -> &'static svl_backend
```

- `svelters-qt-build` loads the checked-in metadata (from M3), selects the root set, emits C++ (dispatch switch, generated subclasses for overridable virtuals, signal connectors), compiles it with the `cc` crate and Qt flags, and emits the link directives.
- The `build.rs` declares `rerun-if-changed` only on its own config, the metadata version and (in release) the bindings manifest. Editing app `.svelte`/`.ts` or app Rust code doesn't rebuild the leaf crate.
- In P4, the C++ is hand-written in the same crate layout so that M3 only swaps the source of the C++.
- GTK can use the same leaf-crate pattern later for release thin bindings (M6). In dev it uses dynamic GObject/GIR invocation and needs no per-project build step.

## Open questions

- **`call_sync` re-entrancy rules.** Can JS mutate the tree during a sync handler? What happens on a returned Promise, or on a timeout? See `docs/notes/signals-and-slots-semantics.md`.
- **Slot routing:** answered by `docs/notes/signals-and-slots-semantics.md`.
  - `insert(…, slot)` is enough for placement.
  - Per-child attributes (grid cell, tab title, dock area) need a new `set_child_attr(parent, child, attr_id, value)` op, planned for M4.
  - The shim routes a child to a slot in this order: the proxy elements `sv-slot`/`sv-item`, then the `slot` attribute, then the child's kind, then the default slot.
- **Ownership.** What happens when the native side destroys a child the host still references? `on_destroyed` covers notification; the annotation pipeline covers which operations transfer ownership.
- **Batching.** Should mutations be batched per checkpoint or applied one at a time? Decide by measurement after P5.

## Op-log format

The P1 TestHost (Node) and the P2 log backend (Rust) must produce identical logs for the same fixture and scripted interaction. They write UTF-8 text, one op per line. Handles are numbered `#1, #2, …` in creation order per run. Values are compact JSON (`JSON.stringify` / serde_json with no spaces).

```text
create <kind> #<n>
insert #<parent> #<child> before=#<h>|end slot=<name>|default
remove #<parent> #<child>
destroy #<h>
set #<h> <prop> <json>
call #<h> <method> <json-args-array>
listen #<h> <signal> on|off
event #<h> <signal> <json-args-array>        # an injected native event, logged when delivered
gone #<h> <reason>                           # the native object died (ABI v2), logged when it happens
```

Kinds, props, signals and slots appear by their JS-facing names, never by IDs. A fixture's scripted interaction is a JSON array of `{"event": [<handle-number>, <signal>, <args>]}`, `{"gone": [<handle-number>, <reason>]}` and `{"checkpoint": true}` steps.

`gone` is a native death. `<reason>` is the lower-case `svl_gone` name: `unknown`, `closed`, `parent`, `owner`, `replaced`, `shutdown`. A scripted `gone` step kills the handle with the given reason, and the log gets one `gone` line for it followed by one line per descendant (pre-order, reason `parent`). The Node TestHost and the log backend both do the cascade themselves. On a real toolkit the replay produces the first death natively (closing the window) and checks the rest against what the backend reports. Ops the host swallows for dead handles (`setProp`, `listen off`, `remove`, `destroy`) are not logged.

Prop values never use the `SVL_LIST` tag or nested objects in P.

## Planned extension: foreign runtimes (post-P5)

From `docs/notes/node-deno-integration.md`. To let stock Node (N-API addon) or Deno (FFI) drive a backend while the toolkit loop stays authoritative, add these optional members:

- host callbacks `foreign_fd()` and `foreign_timeout_ms()`, which the backend's loop watches (GLib source / `QSocketNotifier` + `aboutToBlock`), calling `checkpoint` when either fires;
- backend `poll_prepare`/`poll_dispatch` for runtimes that insist on owning the main thread (stock Deno), with the known limitation that JS freezes during nested modal loops;
- the rule that nested toolkit loops (modal dialogs, DnD) may re-enter `checkpoint`.

Priority after QuickJS: (1) stock Node via N-API on Linux, (2) embedded deno_core as a cargo feature, (3) stock Deno via FFI. Bun is not supported (no libuv, no async FFI).

## Note from P2: nested loops and blocking calls

The host defers a `checkpoint` that arrives while JS is running, and turns it into a later `wake`. This happens when a backend op spins a nested toolkit loop. It's safe, but JS is frozen until the outer call returns. So JS-facing APIs must not expose blocking modal calls (`QDialog::exec`, the deprecated `gtk_dialog_run`). Expose dialogs as present + response signal instead, and let generated metadata mark blocking methods as `unsupported_reason: blocking_nested_loop`.

## Planned change: timers belong to the toolkit (before M2)

Decision (2026-10-16): JS timers must run on the toolkit's native timing. The P2 host currently runs its own timer thread that calls `wake` at each deadline. That's a prototype stopgap and must be removed. Native timing gives us:

- correct behaviour in nested/modal loops;
- coalescing with frame clocks (GdkFrameClock, Qt's animation/vsync timing);
- power-friendly idle wakeups (`g_timeout_add` slack, `Qt::CoarseTimer`);
- no extra thread.

ABI addition (version 2):

```c
/* backend: (re)arm a single one-shot wakeup `ms` from now; replaces any pending one.
   ms < 0 cancels. On expiry the backend calls host->checkpoint. Main thread only. */
void (*schedule_wake)(void *self, int64_t ms);
```

- The host keeps its timer heap and, after every checkpoint, arms only the earliest deadline.
- GTK uses `glib::timeout_add_local_once` (or `timeout_add_seconds` for coarse timers).
- Qt uses one `QTimer` with `setSingleShot(true)`, `Qt::PreciseTimer` for short timeouts and `Qt::CoarseTimer` otherwise.
- egui uses `request_repaint_after`.
- `requestAnimationFrame` gets the same treatment later: a backend `request_frame` that is driven by the frame clock.
- `wake` stays for thread-safe "run a checkpoint soon" requests from host threads.
- The log backend implements `schedule_wake` against its virtual clock, so scripted fixtures stay deterministic.

## ABI v2 (approved 2026-10-17, lands at the start of M2)

**Status: implemented in M2a** (host, log backend, GTK, Qt, renderer). Deviations from the text below are listed under "As implemented" at the end of this section.

ABI v2 bundles two approved changes: toolkit-native timers (above) and the lifetime model from `docs/notes/ownership-and-lifetimes.md`. `SVL_ABI_VERSION` becomes 2.

```c
/* timers */
void (*schedule_wake)(void *self, int64_t ms);                         /* backend, new */

/* lifetime */
typedef enum svl_lifetime { SVL_LIFE_OWNED = 1, SVL_LIFE_TOPLEVEL = 2, SVL_LIFE_BORROWED = 3, SVL_LIFE_VALUE = 4 } svl_lifetime;
typedef enum svl_gone { SVL_GONE_UNKNOWN = 0, SVL_GONE_CLOSED = 1, SVL_GONE_PARENT = 2, SVL_GONE_OWNER = 3,
                        SVL_GONE_REPLACED = 4, SVL_GONE_SHUTDOWN = 5 } svl_gone;
typedef struct svl_handle_info { svl_id kind; uint32_t lifetime; uint32_t flags; } svl_handle_info;
int  (*handle_info)(void *self, svl_handle h, svl_handle_info *out);   /* backend, new */
void (*on_destroyed)(void *self, svl_handle h, uint32_t reason);       /* host, changed: adds reason */
```

Rules:

- **Handles are never reused.** They stay below 2^53, so JS numbers are exact. The host keeps a bounded "dead handles" ring so errors can say why a handle died.
- **`destroy` is idempotent.** The handle is stale as soon as it returns, and the effect depends on the lifetime class: owned → delete the native object; toplevel → close and destroy; borrowed → just forget it; value → unref.
- **`on_destroyed` fires at most once per handle,** after the backend has dropped the handle. When a node dies natively (a window closing, a parent being deleted, the owner deleting a borrowed child), the death cascades to every descendant handle. The host only records deaths inside `on_destroyed` and tells JS later through the event queue. It never re-enters JS from that callback.
- **Borrowed handles are minted** when a getter or call returns a native object we didn't create.
- **Native code never holds a JS value.** Events are (handle, signal, args), and JS-implemented native objects (models, factories) hold an integer ID into a host table.
- **Window close** follows React Native's `onRequestClose`. If JS listens for the close signal, the backend vetoes the native close and posts the event. Otherwise the window is destroyed natively and reported as `SVL_GONE_CLOSED`. The host, not the toolkit, decides when the app quits (Qt: `setQuitOnLastWindowClosed(false)`).
- **Backends never delete synchronously** inside signal emission: Qt uses `deleteLater`, GTK an idle-deferred unref.

### As implemented (M2a)

- **Vtable order.** `handle_info` follows `listen`; `schedule_wake` follows `wake`. `svl_handle_info.flags` is 0 for now.
- **Timers.** After every checkpoint (and when a timer is set or cleared) the host arms `schedule_wake` for the earliest deadline, in whole milliseconds rounded up, and cancels it when the host is dropped. The host starts no threads. The `wake` entry stays for thread-safe requests (the dev loop's file watcher uses it).
  - GTK: `glib::timeout_add_local_once`, one pending source, replaced on every call.
  - Qt: one single-shot `QTimer`, `Qt::PreciseTimer` under 20 ms and `Qt::CoarseTimer` otherwise.
  - Log backend: scripted fixtures (`HostConfig::scripted`) keep the host's own virtual clock, which is deterministic and instant and never calls `schedule_wake`. Unscripted runs on the log backend honour `schedule_wake` in wall time, so the real-timer test exercises the same path as the toolkits. A virtual clock inside the backend would need the host to read the backend's time, which the ABI does not offer.
- **Handles.** Backends mint monotonic handles that are never reused. The host rejects a handle at or above 2^53. The dead-handle ring keeps the last 4096 dead handles. Errors read `handle #12 was destroyed (its window was closed)`; a handle that fell out of the ring or was never issued reads `stale or foreign handle 12`.
- **Destroy.** The host's `destroy` is idempotent and silent (no `on_destroyed` for the app's own destroy). `setProp`, `listen(off)`, `remove` and `destroy` on a dead handle are no-ops; `getProp`, `call`, `insert` and `listen(on)` throw.
- **Native death.** `on_destroyed(h, reason)` only records the death and queues a `gone` event; the next checkpoint calls `globalThis.__svelters_gone(h, reasonName)` (installed by `@svelters/renderer`). The renderer drops the handle from its tables and sends nothing more for that node. Signals queued for a handle that died are dropped silently.
- **Cascade.** GTK reports the window (`closed`) first, then every handle inserted below it (`parent`), from its slot records, because GTK has already unparented the children when `destroy` is emitted. Qt does the same eagerly in the close filter while the object tree is intact.
- **Classes.** The four lifetime classes exist in the ABI. Only `owned` and `toplevel` are minted today; no `get_prop`/`call` returns a borrowed or value handle yet (M4/M5). A toplevel is a window or dialog kind (GTK: `GtkWindow`; Qt: derived from `QMainWindow`/`QDialog`, decided by the generator).
- **Deferred deletion.** Qt `destroy` hides a toplevel and calls `deleteLater`. GTK `destroy` hides a toplevel, then destroys it and drops the table's reference from idles.
- **Window close.** JS listens with `onclose` (the DOM alias of `close-request`). GTK's `close-request` handler returns `Stop` while listened. Qt has no such signal, so `close-request` is a synthetic signal declared in the `window` override (`synthetic: close_request`); the backend's event filter vetoes the `QCloseEvent` and posts it. Without a listener the window is closed for good and reported as `SVL_GONE_CLOSED`.
- **Quit.** `HostConfig::quit_on_last_window` (default true) makes the host call `backend.quit()` at the end of the checkpoint after the last toplevel handle is gone, once at least one existed. Not applied in scripted mode. GTK's `run` holds the application and Qt sets `setQuitOnLastWindowClosed(false)`.

Later ABI additions, not part of v2:

- `set_child_attr` (M4);
- `listen` flags for phase, once, after and sync (pending the feature survey);
- `request_frame`;
- the foreign-runtime hooks;
- the value-model changes from the serdes research.

## Planned ABI v3: value model (M3)

From `docs/notes/js-native-serdes.md` §5.1, which has the exact C proposal.

- Keep the nested, borrowed `svl_value` and extend it in place. It stays 24 bytes, with the tag word split into tag / flags / type id.
- New tags: `ENUM`, `FLAGS`, `RECORD`, `BYTES`, `BOXED`, `CALLBACK`, `MAP`, `ERROR`. Flags mark UTF-16 vs UTF-8 strings and NUL termination.
- Backends publish generated type descriptors (`describe` / `describe_type`), and the host gains `invoke_callback` / `release_callback`.
- No BigInt and no binary op buffer at this ABI. A packed buffer belongs only in the future N-API/Deno-FFI fronts.

Things to do now, with no ABI change:

- JS passes cached IDs, not names (1430 → 770 ns per `setProp`).
- Strings are borrowed.
- Descriptor-driven prop coercion replaces the renderer's hand-written `INT_PROPS`.
- Enums go out as names on both toolkits.

### As implemented (S1, S2): the value model

The header (`crates/svelters-abi/include/svelters_backend.h`) is the reference; `docs/notes/serdes-impl.md` records the decisions. This section records additions made after the first v3 merge. All are additive within v3.

#### Out and in-out parameters (S2)

A method descriptor may flag a parameter `SVL_D_OUT` (the callee fills it in; JS does not supply it) or `SVL_D_INOUT` (JS supplies the input, the callee returns the updated value).

- **Arguments.** JS supplies the parameters that are not `OUT`, in declaration order. The host encodes only those and calls `call(h, method, args, n)` with `n` = their count. `OUT` parameters are never sent, so a backend does not need placeholder values.
- **Result.** If the method has at least one `OUT` or `INOUT` parameter, the backend answers a `LIST` of `1 + k` values: the return value first (`NULL` when the method is void), then one value per `OUT`/`INOUT` parameter, in declaration order. A list of any other length is an error reported by the host.
- **JS shape.** The host turns the list into one object: `ret` (omitted when the descriptor's return is void), then one key per out parameter, named by the slot's `name` (`out<i>` when unnamed). `getContentsMargins()` gives `{left, top, right, bottom}`; `validate(text, pos)` gives `{ret, input, pos}`. The shape is the same for one out parameter or many.
- **Errors.** A `SVL_M_THROWS` method that fails answers `ERROR`/error status as usual; no out list is built.
- **Not supported.** `SVL_M_ASYNC` methods with out parameters (an async completion carries one value), and `OUT` parameters on callback types.
- **Backend recipe.** Report outs by building `[ret, out1, out2...]` as an ordinary `LIST` out value: Rust `Value::List(vec![ret, ...])`; Qt generated code (`out:` / `inout:` keys, below); GTK a `StrBuf::list_value(SVL_LIST, 0, ...)` (`gio-file` `loadContentsEtag`). Descriptors of `OUT` slots describe the output type (so enums become names, records objects, and so on) exactly like a return slot.
- **Qt overrides.** A method entry may carry `out: [names]` and `inout: [names]` naming C++ parameters of type `T *` or `T &` (`int`, `uint`, `bool`, `double`, `float`, `qint64`, `QString`); `target: layout` calls the method on the element's layout. Example: `{ js: getContentsMargins, target: layout, native: getContentsMargins, out: [left, top, right, bottom] }`.

#### Qt callbacks and async (S2)

Both are generated from the Qt override YAML (`tools/metadata/qt/gen_cpp.py`), with the host side unchanged.

- **Callbacks.** A method entry lists `callbacks: [{ param, scope, args: [{name, cpp, ref}], returns?: {cpp, ref} }]` for `Functor` / `std::function` parameters (`scope`: `call` or `notified`). The parameter becomes a `CALLBACK` slot whose type descriptor (`describe_type`) carries the argument slots, the result slot and the scope. The generated call passes a C++ lambda that runs `js_invoke<R>(id, args...)`, which calls `host->invoke_callback`; a failure (the function threw, was released, or its owner died) is reported by the host and Qt sees a default-constructed result. `discard_return: true` hides a C++ return value JS cannot use (`QMenu::addAction` returns the `QAction`). Proof: `menu.addItem(text, fn, shortcut)` on `QMenu::addAction(text, Functor, shortcut)`.
- **Async.** A method entry with `async: { signal, connect, result?, cancel? }` starts a native operation that ends with a signal. The descriptor is `SVL_M_ASYNC` with the host's one-shot completion callback appended as the last parameter. The generated call connects the signal, calls the method, and on emission passes the named signal argument to `invoke_callback`. `backend.cancel(id)` calls the `cancel` method (a zero-argument method of the same object) and completes the callback with `ERROR` (domain `qt`, message `operation cancelled`), once. Proof: `colordialog.open()` on `QDialog::open()` + `finished(int)` (`accept()` resolves `1`, `reject()` resolves `0`, an `AbortSignal` rejects).
- Not covered: callbacks with a result on a real Qt API (the host/ABI path is covered by the Rust test backend), `forever` scope (refused by the host).
