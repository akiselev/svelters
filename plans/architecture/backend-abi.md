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
  /* synchronous handler call for return-value signals/virtuals; see "ABI v2.1 additive" */
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

- **`call_sync` re-entrancy rules.** Answered in "ABI v2.1 additive" (X2). JS may issue host ops during a sync handler and they apply at once; a nested sync signal is deferred; a returned Promise is ignored (the renderer warns); the turn budget is the timeout.
- **Slot routing:** answered by `docs/notes/signals-and-slots-semantics.md`.
  - `insert(…, slot)` is enough for placement.
  - Per-child attributes (grid cell, tab title, dock area) use `set_child_attr(parent, child, attr_id, value)`, added by X3 ("ABI v2.2 additive").
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
sync #<h> <signal> <json-args-array>         # an injected sync signal, logged before its handlers run
result #<h> <json>|busy|error                # what call_sync returned, logged after the handlers' own ops
gone #<h> <reason>                           # the native object died (ABI v2), logged when it happens
```

Kinds, props, signals and slots appear by their JS-facing names, never by IDs. A fixture's scripted interaction is a JSON array of `{"event": [<handle-number>, <signal>, <args>]}`, `{"sync": [<handle-number>, <signal>, <args>]}`, `{"gone": [<handle-number>, <reason>]}` and `{"checkpoint": true}` steps.

A `sync` step calls `__svelters_dispatch_sync(h, signal, args)` (Node) or `host.call_sync` (log backend) directly and does not checkpoint. The `result` line is the handler result as JSON (`null` = no decision, `true` = stop) or `busy`/`error` when the host refused. Ops the handler issues appear between the two lines; microtasks it queued run at the next `checkpoint` step. Toolkit replays skip fixtures with `sync` lines, because a result changes what the toolkit does next.

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
- **Classes.** The four lifetime classes exist in the ABI. `owned` and `toplevel` are minted by `create` and `borrowed` by getters and calls (X1, below); no `value` handle is minted yet (ABI v3). A toplevel is a window or dialog kind (GTK: `GtkWindow`; Qt: derived from `QMainWindow`/`QDialog`, decided by the generator).

### Borrowed handles and dynamic methods (X1)

No ABI change: the behaviour below is what a conforming backend does with the existing `SVL_HANDLE` tag, `handle_info` and `on_destroyed`.

- **Minting.** A `get_prop` or `call` result of `SVL_HANDLE` may name a handle the host has not seen. The host then asks `handle_info` for its kind and lifetime class and adds it to its handle table (a borrowed handle is never a toplevel for the quit decision). A result that names an object the app created carries that object's existing handle.
- **Identity.** One live handle per native object, found from the object itself, never by an address map that can outlive it (GTK: GObject qdata; Qt: a side table erased from `destroyed`). Handles are never reused, so asking again after the app forgot a handle returns a new number for the same object.
- **Weak.** A borrowed handle holds its object weakly (GTK: `GWeakRef` and a weak notify; Qt: `QPointer`). It never keeps the object alive and never deletes it.
- **`destroy` on a borrowed handle** only forgets it (silent, idempotent). The renderer calls it when the node for a borrowed handle is garbage-collected.
- **Death.** The owner deleting the object reports `SVL_GONE_OWNER`. When a toplevel closes natively, borrowed handles reachable below it (the QObject parent chain on Qt; the handle whose getter minted them on GTK) are reported as `SVL_GONE_PARENT` in the same cascade, after the window and its inserted children. Each handle is reported at most once.
- **Tree.** `insert` and `remove` fail with "borrowed" for a borrowed parent or child. `set_prop`, `get_prop`, `call` and `listen` work on them.
- **Kind.** The nearest ancestor type that has a generated kind, or 0 when none has. Property and signal lookups use the kind, so a borrowed `GtkText` supports the properties of `Text`/`Editable`/`Widget` and a borrowed plain `QWidget` (kind 0) supports only the universal getters.
- **JS.** Results are renderer nodes (`globalThis.__svelters_node(h)` is installed by `@svelters/renderer`; without a renderer the host returns the number). An object we created resolves to its own bound node, a native-made one to a single `borrowed` node per handle. Nodes passed as `call`/`setProp` arguments (anything with a numeric `svlHandle`) are converted back to handles.
- **Method ids** are not limited to the generated tables. A backend may mint more ids from `resolve(SVL_METHOD, kind, name)`:
  - GTK: ids past `METHODS.len()` are typelib methods, resolved through girepository-2.0 and invoked with `gi_function_info_invoke` (libffi). `snake_case` and `camelCase` names both work. Supported types: bool, 8..64-bit integers, float/double, UTF-8 strings, enums (nick or number in, nick out), flags (number, nick or `a|b` in, number out), objects and interfaces (handle or null in; existing or borrowed handle out; `transfer full` in gets its own reference, `transfer full` out becomes an owned handle). Callbacks, out/inout parameters, arrays, lists, hash tables, GType, structs/boxed values and static functions are rejected with an error that names the feature. The matrix and follow-ups are in `docs/notes/x1-borrowed-gi.md`.
  - Qt: ids from `1 << 20` are universal object getters generated from the qdoc metadata of every QObject-derived class (`parent`, `parentWidget`, `window`, `centralWidget`, `viewport`, `lineEdit`, ...); they apply to any kind and fail with "does not apply" on a receiver of the wrong class. The annotated-method generator also accepts `object<T>` parameters and results.
- **Deferred deletion.** Qt `destroy` hides a toplevel and calls `deleteLater`. GTK `destroy` hides a toplevel, then destroys it and drops the table's reference from idles.
- **Window close.** JS listens with `onclose` (the DOM alias of `close-request`). GTK's `close-request` handler returns `Stop` while listened. Qt has no such signal, so `close-request` is a synthetic signal declared in the `window` override (`synthetic: close_request`); the backend's event filter vetoes the `QCloseEvent` and posts it. Without a listener the window is closed for good and reported as `SVL_GONE_CLOSED`.
- **Quit.** `HostConfig::quit_on_last_window` (default true) makes the host call `backend.quit()` at the end of the checkpoint after the last toplevel handle is gone, once at least one existed. Not applied in scripted mode. GTK's `run` holds the application and Qt sets `setQuitOnLastWindowClosed(false)`.

### ABI v2.1 additive (X2: sync handlers and the frame clock)

Two tail members and one semantics definition, additive on top of v3. `SVL_ABI_VERSION` is whatever the value model set (3); `request_frame` is the last member of `svl_backend` (after the v3 `cancel`) and `frame` the last of `svl_host` (after `release_callback`). In the Rust `Backend` trait `request_frame` has a no-op default. All backends are in-tree and rebuilt together.

```c
/* backend, appended after last_error */
void (*request_frame)(void *self);   /* one host->frame on the toolkit's next frame; idempotent while pending */
/* host, appended after on_destroyed */
void (*frame)(void *self);           /* a requested frame is due; runs requestAnimationFrame callbacks */
```

**Frame clock.** The host calls `request_frame` when JS calls `requestAnimationFrame` and no request is pending, and the backend answers with exactly one `host->frame` from its frame clock: GTK `add_tick_callback` on a mapped toplevel, Qt `QWindow::requestUpdate` (the `UpdateRequest` event), the log backend a virtual frame on its next loop turn. A backend without a window to tick on falls back to a 16 ms timer. The host stamps callbacks with its own `performance.now()` clock, so the callback argument is comparable with `performance.now()`. A frame that arrives while JS is running is re-requested. Scripted mode (fixtures) never calls `request_frame`; `requestAnimationFrame` there is a virtual 16 ms timer.

**`call_sync`** (previously a stub). Return codes: `0` ok, `-1` failed (use the default), `-2` busy.

- The backend calls it from inside a toolkit signal emission or event filter, only for signals it connected because JS listens.
- On `0`, `out` is the handler result: `null` means no decision (toolkit default), a bool is the stop/handled flag of a `bool_stop` signal, other values are for `value` signals.
- `-2`: JS was already running (a host op that emits a sync signal synchronously, or a sync handler that triggers another sync signal). The host cannot re-enter JS, so it queued the event for ordinary async delivery. The backend returns the signal's deferred result: not-stop, except `close-request`, which vetoes (the ABI v2 behaviour).
- While a sync call runs the host holds its JS guard: a `checkpoint` that arrives (nested loop) is deferred; the turn budget applies; jobs are not drained. If jobs remain pending afterwards the host calls `wake`.
- A throwing handler yields `0` with a `null` result and an error in the host's error list.
- The result comes from the event object (`e.result`, `e.preventDefault()`, `e.stopPropagation()`), never from the handler's return value.

### Model-view protocol (M5)

No vtable change. A backend may provide the portable composite widget kind `virtuallist` (an "extra" kind resolved by name, like `gio-file`). It wraps the toolkit's native model-view widget and speaks one protocol. The model lives in JS: the native side holds only an integer *row id* per item.

```text
kind   virtuallist          a widget (insertable anywhere); also takes the generic widget props of its toolkit base
props  selected   i64       selected position, -1 for none (setting it never raises `select`)
       selection  string    "none" | "single"
       layout     string    "list" | "grid"
       rowHeight  i64       Qt: the uniform row height of the pooled rows. GTK: minimum height of a row slot (default 32; 0 = none),
                            which keeps a row that is still empty from measuring 0px and making the first layout create a slot per item
calls  splice(pos, removed, ids[])   replace `removed` ids at `pos` with `ids` (u64 as I64, no duplicates). One call = one
                                      items-changed (GTK) / beginRemoveRows or beginInsertRows (Qt). Never a reset
       count() -> i64                native item count
       scrollTo(pos)
       setRow(row, widget)           put a widget handle into row slot `row` (called from `setup`); null clears it
       rowAt(pos) -> i64             slot bound to `pos`, or -1
sync   setup(row)                    a row slot was created; give it content with setRow
       bind(row, pos, id, gen)       slot `row` now shows item `id` at `pos`; gen counts binds of that slot
       unbind(row, gen)              slot `row` is no longer showing that binding
       teardown(row)                 slot destroyed; JS releases its widget
async  activate(pos)                 user activated an item (double click, Enter)
       select(pos)                   user selection changed
```

- **Snapshot, not call_sync, for the model.** The native model answers `n_items`/`get_item`/`rowCount`/`data` from its own id vector; JS changes it only through `splice`. A toolkit may query the model at any time, including inside our own `splice`, where `call_sync` would answer busy.
- **Row content is lazy.** The four row signals use `call_sync` so content is in place before the first paint. They are listened like any other signal. When JS is already running (`-2`), the host queues them as deferred events; the `unbind`/`teardown` of rows that `splice` itself removes usually take that path. `gen` lets JS ignore a late `unbind` for a binding that was replaced meanwhile. A deferred `bind` is valid as long as the id is still in the list (JS re-finds its position).
- **Row slots are recycled** by the toolkit (GTK `GtkListItemFactory` recycling; Qt: a pool of `visible + 2` slots placed `slot = pos mod N`). The widget handed to `setRow` stays owned by the JS handle (destroying the handle drops only the host's reference).
- **Handles and accounting.** `setRow` takes a widget handle, so every row costs exactly one `create` for its container plus whatever the row component mounts; all of them are destroyed at `teardown`. Tests assert `live handles after teardown == baseline`.
- **Sort/filter** are done in JS: the app sorts or filters its array and the keyed diff turns the new order into minimal splices.

### ABI v2.2 additive (X3: child attributes and listen phases)

Three optional members appended after `request_frame` in `svl_backend` (nullable function pointers: a backend that does not implement one sets it to NULL), one name kind and one id convention. No existing member changes.

```c
#define SVL_ATTR 5   /* svl_name_kind: child attribute; resolve(SVL_ATTR, parent_kind, name) */

/* backend, appended after request_frame */
int (*set_child_attr)(void *self, svl_handle parent, svl_handle child, svl_id attr, const svl_value *v);
int (*get_child_attr)(void *self, svl_handle parent, svl_handle child, svl_id attr, svl_value *out);
int (*listen_ex)(void *self, svl_handle h, svl_id signal, bool on, uint32_t flags);

enum { SVL_LISTEN_AFTER = 1, SVL_LISTEN_CAPTURE = 2 };   /* listen_ex flags; 4 is reserved for SYNC */
#define SVL_SIGNAL_FLAGS_SHIFT 28                         /* flags ride in bits 28..30 of the signal id of post_event/call_sync */
```

**Child attributes.**

- An attribute is a per-child fact owned by the child's parent: a grid cell, a tab label, a dock area, a stretch factor. `set_child_attr` is valid only while `child` is inserted into `parent` (any slot); otherwise it fails.
- Attribute ids are scoped by the parent's kind and named in JS camelCase (`row`, `columnSpan`, `tabLabel`). One id per (kind, name). When two slots of a kind have an attribute of the same name, the backend picks the implementation from the slot the child was inserted into, and fails if that slot has no such attribute.
- `v` is an ordinary `svl_value`: numbers, bools, strings, enums as nick/name strings. `SVL_NULL` resets the attribute to its documented default.
- The host calls `set_child_attr` right after `insert`, once per attribute the child already has, so a backend that fixes placement at insert time (Qt grid cells) re-places the child as the attributes arrive. Attribute changes while inserted call it again. Moving a child to another parent is `remove` + `insert` + attributes again.
- `remove` clears all per-child attribute state the backend holds (the layout child goes away with the child on GTK; Qt resets the tab text, stretch and so on by removing the widget). `get_child_attr` reads the live native value back; it is optional and used by tests and `bind:` support.
- A backend without the member answers every attribute op with "child attributes are not supported by this backend".

**Listen phases.** `listen_ex(h, signal, on, flags)` replaces `listen` when `flags != 0`. The host calls `listen` for flags 0, so old backends keep working for ordinary listeners.

- `SVL_LISTEN_AFTER`: run after the class/default handler (GTK `connect_after`).
- `SVL_LISTEN_CAPTURE`: run in the capture phase, before the target (event controllers on GTK; an event filter on Qt).
- Each (handle, signal, flags) is a separate native connection. The renderer turns listeners on at the first and off at the last listener of that triple.
- The backend reports which connection fired by or-ing `flags << 28` into the signal id it passes to `post_event` and `call_sync`. The host strips the bits, resolves the name from the base id and hands the flags to JS, which runs only the listeners registered with those flags.
- Support is per backend and per signal. A combination that has no honest native meaning fails with an error that names the reason; see "Listener phases per backend" in `docs/notes/x3-childattr-listen.md`.
- The op log gets one more word on `listen` lines (`listen #1 key-pressed on after+capture`, only when flags are set) and one line type `attr #<parent> #<child> <name> <json>` (`null` = reset).

Later ABI additions, not part of v2:

- `listen` flags `once` and `sync` (reserved bit 4);
- the foreign-runtime hooks;
- the value-model changes from the serdes research.

## ABI v3: value model (M3)

**Status: implemented (S1, S2).** The text below is the original plan; "As implemented" at the end of this section records what was built and the additions made later.

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

#### UTF-16 string results (S2)

A backend that advertises `SVL_CAP_UTF16_OUT` may return `STR` values flagged `SVL_F_UTF16` (pointer to `char16_t`, length in code units) from `get_prop`, `call`, and in event, sync-handler and callback arguments. The host (`Value::from_raw_wide`, `Value::Str16`) builds the JS string with `JS_NewStringUTF16` and does not transcode. Consumers of raw values must honour the flag (`str_of_raw` does); `Value::from_raw` still transcodes to `Value::Str` for callers that want text. Strings inside `ERROR` values and `MAP` keys may stay UTF-8. The Qt backend returns every `QString` this way and shares the `QString` buffer, so no copy is made.

#### Exact variant types and GValue-typed properties (S2)

- **Typed variants.** `variant(signature, value)` (a global in the JS prelude; class `NativeVariant`) wraps a value with an exact GVariant type signature. On the wire it is a `RECORD` of well-known type `SVL_T_TYPED_VARIANT` (10) with fields `[signature: string, value]`, valid wherever a variant slot accepts a value (and nested inside variants and maps). Without it a variant's type is inferred (integers `x`, floats `d`, strings `s`, bytes `ay`, lists `av`, objects `a{sv}`).
  - GTK builds exactly that type: basic types with range checks, `aX`, `a{sV}` from objects (string-like keys only), `(..)`, `mX` (null = nothing), `v`, `ay` from bytes. A mismatch fails the set with a message naming the signature and the path.
  - Qt has no signatures: a basic one fixes the `QMetaType` and the range (`y n q i u x t d b s o g`); a container signature passes the value as it is.
  - Reading a variant still yields the inferred JS shape; the signature does not come back.
- **GValue props.** A property of boxed type `G_TYPE_VALUE` (e.g. `GtkDropTarget:value`) has a variant-like dynamic slot: the JS value is converted to a GValue by inference (bool, int64, double, string, bytes -> `GBytes`, boxed or object handles as themselves, typed variants as `GVariant`) and read back by unwrapping the contained GValue.

#### VALUE-class handles (S2)

A `BOXED` value is the v2 VALUE lifetime class: a refcounted, immutable native value behind a handle in the same never-reused handle space. `destroy(h)` (the wrapper's `dispose()`, or the finalizer at the next checkpoint) is the unref and is idempotent. `handle_info(h)` answers `lifetime = SVL_LIFE_VALUE` with `kind = 0` (the value's type is the `BOXED` type descriptor it travelled with) until it is released; afterwards the handle is stale. No tree operation accepts a VALUE handle. Both backends implement it.
