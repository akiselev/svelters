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

1. Deliver the batch of changed observed properties (`__svelters_changed`, ABI v2.6), then the queued native events.
2. Run Svelte effects and QuickJS jobs, up to a bounded number.
3. Apply the resulting tree/prop operations through the vtable.
4. If jobs remain, call `backend->wake()` and return to the toolkit loop.

The P milestone applies ops directly instead of batching. Batching comes later if measurements justify it.

## JS-side host ops

The L1 runtime (`@svelters/native-runtime`) calls these and the host forwards them to the vtable:

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
  // ABI v2.5 (optional on a backend):
  construct(kind: string, props?: Record<string, unknown>): Handle;
  callStatic(kind: string, method: string, args: unknown[]): unknown;
  typeChain(h: Handle): string[];
  // ABI v2.6: refcounted per (h, prop); answers whether (h, prop) is observed afterwards
  // (false for `on`: the backend has no change notification for this prop)
  observe(h: Handle, prop: string, on: boolean): boolean;
}
// events: globalThis.__svelters_dispatch(h, signal, args) is invoked at checkpoints
// ABI v2.6: globalThis.__svelters_prop_changed(h, prop) for an observed prop that may have changed
```

The JS host caches name→ID resolution per kind. The Node TestHost implements this interface in JS and records an op log. The QuickJS host implements it natively. A given fixture must produce the same op log in both.

**Widget creation timing.** Compiled components create each native widget where the element is declared (`$n.element`), insert it once its children are in place and present a toplevel after its subtree is complete (`docs/notes/native-compiler/runtime-contract.md`). (The retired DOM shim and renderer created widgets lazily, on first insertion under a native-backed parent, because Svelte cloned templates.)

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
construct #<h> <Kind> <json-props-object>    # ABI v2.5: a new instance with construct-time properties
static <Kind>.<method> <json-args-array> [#<h>]  # ABI v2.5: a static call; new/new_*/from_* answer a fresh handle #h
observe #<h> <prop> on|off                   # ABI v2.6: the backend starts/stops watching a prop (transitions only)
changed #<h> <prop> [<json>]                 # ABI v2.6: a scripted native change to <json>, announced to the host
```

Kinds, props, signals and slots appear by their JS-facing names, never by IDs. A fixture's scripted interaction is a JSON array of `{"event": [<handle-number>, <signal>, <args>]}`, `{"sync": [<handle-number>, <signal>, <args>]}`, `{"gone": [<handle-number>, <reason>]}`, `{"changed": [<handle-number>, <prop>, <value>?]}` (ABI v2.6) and `{"checkpoint": true}` steps.

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

**Status: implemented in M2a** (host, log backend, GTK, Qt, the then JS renderer; today `@svelters/native-runtime`). Deviations from the text below are listed under "As implemented" at the end of this section.

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
- **Native death.** `on_destroyed(h, reason)` only records the death and queues a `gone` event; the next checkpoint calls `globalThis.__svelters_gone(h, reasonName)` (installed by `@svelters/native-runtime`'s `mount`; before RT1 by `@svelters/renderer`). The runtime drops the handle from its tables and sends nothing more for that node. Signals queued for a handle that died are dropped silently.
- **Cascade.** GTK reports the window (`closed`) first, then every handle inserted below it (`parent`), from its slot records, because GTK has already unparented the children when `destroy` is emitted. Qt does the same eagerly in the close filter while the object tree is intact.
- **Classes.** The four lifetime classes exist in the ABI. `owned` and `toplevel` are minted by `create` and `borrowed` by getters and calls (X1, below); no `value` handle is minted yet (ABI v3). A toplevel is a window or dialog kind (GTK: `GtkWindow`; Qt: derived from `QMainWindow`/`QDialog`, decided by the generator).

### Borrowed handles and dynamic methods (X1)

No ABI change: the behaviour below is what a conforming backend does with the existing `SVL_HANDLE` tag, `handle_info` and `on_destroyed`.

- **Minting.** A `get_prop` or `call` result of `SVL_HANDLE` may name a handle the host has not seen. The host then asks `handle_info` for its kind and lifetime class and adds it to its handle table (a borrowed handle is never a toplevel for the quit decision). A result that names an object the app created carries that object's existing handle.
- **Identity.** One live handle per native object, found from the object itself, never by an address map that can outlive it (GTK: GObject qdata; Qt: a side table erased from `destroyed`). Handles are never reused, so asking again after the app forgot a handle returns a new number for the same object.
- **Weak.** A borrowed handle holds its object weakly (GTK: `GWeakRef` and a weak notify; Qt: `QPointer`). It never keeps the object alive and never deletes it.
- **`destroy` on a borrowed handle** only forgets it (silent, idempotent). The runtime calls it when the node for a borrowed handle is garbage-collected.
- **Death.** The owner deleting the object reports `SVL_GONE_OWNER`. When a toplevel closes natively, borrowed handles reachable below it (the QObject parent chain on Qt; the handle whose getter minted them on GTK) are reported as `SVL_GONE_PARENT` in the same cascade, after the window and its inserted children. Each handle is reported at most once.
- **Tree.** `insert` and `remove` fail with "borrowed" for a borrowed parent or child. `set_prop`, `get_prop`, `call` and `listen` work on them.
- **Kind.** The nearest ancestor type that has a generated kind, or 0 when none has. Property and signal lookups use the kind, so a borrowed `GtkText` supports the properties of `Text`/`Editable`/`Widget` and a borrowed plain `QWidget` (kind 0) supports only the universal getters.
- **JS.** Results are runtime nodes (`globalThis.__svelters_node(h)` is installed by `@svelters/native-runtime`'s `mount`; without it the host returns the number). An object we created resolves to its own bound node, a native-made one to a single `borrowed` node per handle. Nodes passed as `call`/`setProp` arguments (anything with a numeric `svlHandle`) are converted back to handles.
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

### ABI v2.4 additive (X8: typed sync results, event families, canvas)

No vtable change.

**Typed sync results.** A `call_sync` result (`out`) may be a `MAP` (from a JS object) or a `LIST` (from an array) of primitives, besides null, bool, number and string. The handler sets it as `e.result = {ret?, <out>: v}`; it is still never read from the return value. Signals with out-parameters declare them in metadata (`out_params: [{index, name, type}]`, GTK `SignalInfo.outs`): the `gpointer` parameters GTK passes to the handler (`GtkSpinButton::input` has `gdouble *new_value`). The backend does not pass those to JS (the listener gets `(event)` only) and writes them from the result:

- `{ret?, name: n}`: `name` is the out-parameter's name; `ret` is the signal's own return value (bool or int), default 1 ("handled") when an out-parameter was written, else 0.
- a bare number is shorthand for `{ret: 1, <first out>: n}`.
- `null` (or anything else): no decision; the toolkit default runs and nothing is written.
- Busy (-2) and failed (-1) replies behave as `null`.

`GtkSpinButton::input`: `ret` 1 = converted, 0 = not handled (GTK parses the text), -1 = GTK_INPUT_ERROR; out `newValue`. `output` is `bool_stop`: the handler displays the value with `text` and calls `e.preventDefault()`.

**Event families.** Qt widgets (every `QWidget` element) have synthetic signals raised by an event filter, besides `key-press`: `key-release`, `mouse-press`, `mouse-release`, `mouse-move`, `double-click`, `wheel`, `context-menu` (sync: `e.preventDefault()` consumes the event before the widget sees it) and `focus-in`, `focus-out`, `enter`, `leave`, `resize` (posted, never consumed). Payloads (listeners get `(event, ...args)` for sync ones, `(...args)` for posted ones):

| signal | args |
|---|---|
| `key-release` | `key`, `text`, `modifiers` |
| `mouse-press`, `mouse-release`, `mouse-move`, `double-click` | `position` (Point), `button`, `buttons`, `modifiers` (Qt enum masks as integers) |
| `wheel` | `position` (Point), `angleDelta` (Point), `buttons`, `modifiers` |
| `context-menu` | `position` (Point), `globalPosition` (Point), `reason` |
| `focus-in`, `focus-out` | `reason` |
| `enter` | `position` (Point) |
| `leave` | none |
| `resize` | `size` (Size), `oldSize` (Size) |

`listen_ex` with `SVL_LISTEN_CAPTURE` works for every family (an application-level filter calls the listeners of proper ancestors first, outermost first, then the target's own); `SVL_LISTEN_AFTER` is refused. Listening to `mouse-move` turns on mouse tracking for the widget.

GTK: the same events are signals of declarative controller elements, not of widgets: `keycontroller` (`key-pressed`, `key-released`, `modifiers`), `clickgesture` (`pressed`, `released`, `stopped`), `motioncontroller` (`enter`, `motion`, `leave`), `scrollcontroller` (`scroll`, `scroll-begin`, `scroll-end`; prop `flags` must include `vertical` or `horizontal` to receive scrolls), `focuscontroller` (`enter`, `leave`). A controller element is a child of the widget it controls (inserted with `add_controller`, X2); `bind:this` and props work as for any element. `on<signal>capture` on a controller puts it in the capture phase (X3).

**Recording canvas (draw func).** JS never runs while a toolkit paints. A `canvas` element (GTK: the `drawingarea`, also spelled `canvas`; Qt: the hand-written `canvas` kind, a `QWidget` whose `paintEvent` replays) takes one value, the property `ops`: a JSON array of `[name, ...args]` arrays, the display list recorded by `@svelters/canvas` (`Canvas2D`). Ops: `save restore translate rotate scale transform setTransform globalAlpha fillStyle strokeStyle lineWidth lineCap lineJoin setLineDash font textAlign textBaseline beginPath moveTo lineTo bezierCurveTo quadraticCurveTo arc rect closePath fill stroke clip fillRect strokeRect clearRect fillText strokeText`; colors are CSS colors, fonts the CSS shorthand (`bold 14px sans-serif`); unknown ops and bad arguments are skipped. Setting `ops` repaints (`queue_draw` / `update`); an invalid list is an error and keeps the old one. The size is not an argument of a draw callback: the widget posts `resize` (GTK `(width, height)`, Qt `(Size, oldSize)`) and JS records a list for it. `contentWidth`/`contentHeight` set the requested size (both toolkits), `hexpand`/`vexpand` on Qt. The Qt canvas also has the event-family signals above, so it is interactive.

**ListBox headers.** `GtkListBox` sort, filter and header funcs are not exposed (they would call JS once per row pair during layout, X5's decision). JS renders the rows in the order and subset it wants, keyed, and a row child carries the attribute `parent:header="Section"` (child attribute `header` of the `children` slot, `""`/`null` removes it): a native header func turns the stored text into a heading label.

**Gdk values.** A string converts to a `GdkCursor` (CSS cursor name) and, where a `GdkPaintable` or `GdkTexture` is wanted, to a texture loaded from a path or `file:` URI; a `BYTES` value is decoded as an image (PNG, JPEG, ...).

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

### Native models (N3, protocol extension, no vtable change)

The `virtuallist` kind can also show a *native* model the app holds as a handle (NON-GUI-TYPES §9 N3; design in `plans/milestones/N3-native-models.md`). Additive: an id-mode list (the M5 protocol above) behaves exactly as before. Nothing in `svl_backend` or `svl_host` changes.

```text
props  model      handle|null  GTK: any object implementing GListModel; Qt: any QAbstractItemModel (column 0, root rows).
                               Set: the list shows that model and the id vector is emptied (model mode). null: back to the
                               (empty) id model. In model mode `splice` fails.
sync   bind(row, pos, id, gen, epoch, item)
                               two arguments appended: `epoch` is the number of model changes the list has reported so far
                               (see `items-changed`); `item` is the item's object handle on GTK (g_list_model_get_item,
                               adopted by the host like any handle in an event), null on Qt and in id mode. `id` is 0 in model mode
async  items-changed(pos, removed, added, epoch)
                               model mode only: the model replaced `removed` items at `pos` with `added` new ones; epoch
                               increments by one per event. GTK: the model's items-changed. Qt: rowsInserted / rowsRemoved
                               (one event each), rowsMoved (a remove then an insert), modelReset and layoutChanged
                               (pos 0, removed = the old count, added = the new count)
       data-changed(first, last, epoch)
                               model mode, Qt only: dataChanged over rows first..last (no epoch increment). GTK items
                               announce their own changes (notify), and a replaced item is an items-changed
```

- **The model never calls JS.** The toolkit's view reads the native model directly (GtkListView over the model or over a `GtkSingleSelection`/`GtkNoSelection` wrapping it; `QListView::setModel`). JS holds no copy of the list, no keys and no ids; it hears only the row lifecycle and the change reports.
- **A selection model is used as is.** On GTK, a model that implements `GtkSelectionModel` (`SingleSelection`, `MultiSelection`, `NoSelection`) is shown directly and `selection` is ignored; `selected`/`select` work when it is a `GtkSingleSelection`. Qt uses the view's own `QItemSelectionModel` (`currentIndex`) as before.
- **Ordering by epoch.** A sync `bind` can run before JS has seen a change event that was posted earlier (the host runs `call_sync` at once while events wait for the checkpoint), and a deferred `bind` can run after change events that were posted later. The backend reports its change counter with each bind; JS applies a change to a bound slot only when the change's epoch is newer than the slot's. The backend connects its change handler to the model *before* the view's own, so the epoch a bind reports already includes the change that caused it.
- **Host:** `call_sync` adopts handles in its arguments (as `post_event` already did since NT1), so a sync `bind` can carry the item.
- Qt items are resolved by JS at bind time (the default: `QStandardItemModel.item(row)`, else `index(row, 0)`); Qt binds run from the pool's relayout timer, never inside a model mutation, so the model is consistent when JS reads it.

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
- Each (handle, signal, flags) is a separate native connection. The runtime turns listeners on at the first and off at the last listener of that triple.
- The backend reports which connection fired by or-ing `flags << 28` into the signal id it passes to `post_event` and `call_sync`. The host strips the bits, resolves the name from the base id and hands the flags to JS, which runs only the listeners registered with those flags.
- Support is per backend and per signal. A combination that has no honest native meaning fails with an error that names the reason; see "Listener phases per backend" in `docs/notes/x3-childattr-listen.md`.
- The op log gets one more word on `listen` lines (`listen #1 key-pressed on after+capture`, only when flags are set) and one line type `attr #<parent> #<child> <name> <json>` (`null` = reset).


### Non-widget kinds: menus and actions (X4, no ABI change)

Menu nodes are ordinary kinds of the existing ops; nothing is added to the vtable. The conventions both backends follow:

- **Kinds.** `menubar` and `toolbar` are widgets (insertable into any widget slot). `menu`, `action`, `menuseparator` and `contextmenu` are non-widget kinds of lifetime class `owned`. `menuseparator` is not called `separator` because GTK's `separator` is a widget.
- **Placement.** The slot is always the default (0); the backend decides by the kinds of parent and child:
  - `action`, `menuseparator` and `menu` go into a `menu`, `menubar`, `toolbar` or `contextmenu`;
  - a `menu` also goes into a `menubutton` (GTK `menu-model`);
  - a `contextmenu` goes into any widget and attaches to it.
- **Order.** `insert(parent, child, before)` with a `before` that is already in the parent is a move, so a keyed `{#each}` reorder is `insert` with an anchor and needs no `remove`. `before` that is not an item of the parent fails. `remove` releases the child (Qt: parentless, not deleted); `destroy` ends it.
- **Properties.** `label`, `icon` (theme icon name), `shortcut` (`Ctrl+O`), `tooltip`, `enabled`, `visible`, `checkable`, `checked`, `group` on `action`; `label`, `icon`, `enabled` on `menu`. A bare attribute (`<action checkable>`) arrives as the text `""` when the host has no descriptor, which both backends accept for booleans.
- **Signals.** `activate(checked)` (a click, Enter or the shortcut) and `toggled(checked)` (the check state changed because of the user). Writing `checked` from JS never posts `toggled` for that action (no echo). In a radio `group` the user's pick posts `toggled(false)` for the previous member and `toggled(true)` for the new one; Qt also posts `toggled(false)` for the previous member when JS selects another one (the member really changed state).
- **Radio.** Actions of one parent with the same `group` name are exclusive; each keeps its own `checked`. GTK: one string-state `GSimpleAction` per (parent, group). Qt: an exclusive `QActionGroup` per (parent, group), a child of the parent.
- **Methods.** `contextmenu.popup(...)` and `popdown()` (never a blocking `exec`). GTK takes `popup(x, y)`; Qt takes a `Point` record.

### ABI v2.3 additive (X6: styling)

One optional member appended after `listen_ex` (a backend without it answers every styling op with "styling is not supported by this backend"). No existing member changes. Design: `plans/milestones/X6-styling.md`, user guide: `docs/site` styling page.

```c
/* backend, appended after listen_ex */
int (*style)(void *self, uint32_t op, svl_handle h, const svl_value *v);
```

`v` is an `SVL_STR` or `SVL_NULL` (clear). The return value is `>= 0` on success and `< 0` on error with `last_error`. Fields inside one string are separated by `\x01` (`SVL_STYLE_SEP`). JS reaches it as `host.style(op, handle, text)`, which returns the integer.

| Op | `h` | Payload | Meaning |
| --- | --- | --- | --- |
| `SVL_STYLE_TARGET` 0 | 0 | none | Returns the dialect the backend renders: `1` CSS (GTK), `2` QSS (Qt), `0` none. The log backend and TestHost answer `1`. JS then renders and sends only that dialect. |
| `SVL_STYLE_CLASSES` 1 | widget | `a b c` | The element's user classes (the `class` attribute with Svelte's scope hash, plus `sv-k-<tag>` for kinds whose CSS name / Qt class another kind shares). GTK diffs against the previous set with `add_css_class`/`remove_css_class` so the widget's own classes (`text-button`, `toggle`) survive. Qt sets the dynamic property `class` to `"<QtClass> a b c"` and re-polishes (`.QPushButton` keeps matching; see `docs/notes/styling-gtk-css-qss.md` 3.2). |
| `SVL_STYLE_NAME` 2 | widget | `id` | The `id` attribute: `gtk_widget_set_name` / `setObjectName`, so `#id` selectors match. |
| `SVL_STYLE_INLINE` 3 | widget | declaration lines | `style=` / `style:` translated to the backend's dialect, one `prop: value;` per line. GTK dedupes by content into a shared class (`sv-s<hash>`) in an append-only inline provider flushed once per main-loop turn. Qt sets `".QClass {\n...}"` as the widget's own sheet. `NULL` removes it. |
| `SVL_STYLE_OPACITY` 4 | widget | `0`..`1` | Inline `opacity` through the widget API (`gtk_widget_set_opacity`, `QGraphicsOpacityEffect`), never CSS. |
| `SVL_STYLE_SHEET` 5 | 0 | `id\x01text` | Install or replace the component sheet `id`. `id` alone (no separator) removes it; `NULL` clears every sheet. `@tokens` is the token layer (GTK: a `:root { --sv-*: v }` provider below the component providers; Qt does not get it: tokens are substituted before sending). GTK: one `GtkCssProvider` per id. Qt: a map of id to text, concatenated in registration order into the application stylesheet, set once per main-loop turn. |
| `SVL_STYLE_SCHEME` 6 | 0 | `light\|dark` then `\x01role=#rrggbb`... | GTK: sets `prefers-color-scheme` on every provider it owns (so `@media (prefers-color-scheme: dark)` in component CSS applies) and the toolkit's dark preference. Qt: sets the application `QPalette` from the role pairs (`window`, `button`, `windowText`, `text`, `buttonText`, `base`, `highlight`, `highlightedText`, `placeholderText`) derived from the token set, then re-polishes. |

Ordering: the first styling use of an app sends `SHEET` with `NULL` (so a hot-reloaded bundle drops sheets of modules that no longer exist), then `SCHEME`, then the token layer, then sheets, then per-element ops. Apps that never use styling send nothing. Dead widget handles are a no-op in the host.

Op log lines: `style #h classes|name|inline|opacity <json|null>`, `style sheet "<id>" <json text|null>` (id `*` = clear), `style scheme "<scheme>" <number of palette pairs>`.

### Toplevels in the tree (X7)

No new vtable member and no new name kind. Convention for `insert`/`remove` when the **child** is of the TOPLEVEL lifetime class (a window or dialog):

- `insert(parent, child, before=0, slot=0)` makes `child` transient for the window that contains `parent` (`parent` itself when it is a window). GTK: `gtk_window_set_transient_for`. Qt: `QWidget::setParent(parent->window(), windowFlags())`, which keeps the window type. `before` and `slot` are ignored. The child is not shown by the insert; `present` (windows), `open` (Qt dialogs, window-modal, non-blocking) or `modal` + `present` (GTK) show it. Blocking calls (`exec`, run loops) are never exposed.
- `remove(parent, child)` undoes it (`set_transient_for(NULL)`, `setParent(nullptr, flags)`). Inserting an already inserted toplevel, a window into itself, or into a parent that is not in a window fails.
- The tree edge is real for lifetime purposes: when the parent window dies, its transient toplevels die with it and are reported `SVL_GONE_PARENT` (Qt through the QObject parent; GTK destroys them from an idle). A transient toplevel that is closed on its own reports `SVL_GONE_CLOSED` as any toplevel.
- JS side (`@svelters/native-runtime`, `$n.toplevel`; the compiler emits it for every toplevel kind): a toplevel kind may appear anywhere in a component; it is created, inserted under the nearest enclosing toplevel element once the appends that contain it are complete (none at the root), and shown by its static `show` policy (`modal`, `modeless`, `none`; dialogs default to modal, using `TOPLEVELS` in `bindings.generated.js` for the modal method and the `modal` property) after that window was shown.

Later ABI additions, not part of v2:

- `listen` flags `once` and `sync` (reserved bit 4);
- the foreign-runtime hooks;
- the value-model changes from the serdes research.

### ABI v2.5 additive (NT0: SHARED handles, construct, call_static, type values)

Everything here is additive within v3; a backend that implements none of it keeps working (the new members are `NULL`).

- **Lifetime class `SVL_LIFE_SHARED = 5`.** A strong reference to a refcounted object outside the widget tree (GObject non-widgets, later COM and ARC objects). One handle per object (identity: a GTK qdata tag, safe to key because the reference is strong). `destroy` drops only our reference, never disposes or closes the object. The host may call `destroy` when the JS wrapper is garbage collected, so GC release covers `SHARED`, `VALUE` and `BORROWED` handles, never `OWNED` or `TOPLEVEL`. Transfer-none returns of non-widget objects are `SHARED` (decision Q1); widgets keep `OWNED` / `BORROWED`.
- **Every GObject crosses as a `HANDLE`,** never `BOXED`. The handle's `kind` is the generated kind when the type has one, else a reflected kind minted from the GType (`handle_info` answers it). `BOXED` stays the `VALUE` class for boxed structs; a `VALUE` handle now has a kind too and takes `call` (read-only members; mutators are hidden by the JS modules, not by the ABI).
- **`describe(SVL_KIND, 0, kind)`** answers the kind itself: `name` is the qualified native name (`Gio.ListStore`), `ret.type` the id of the nearest bound ancestor kind (0 at the root). The host reads it for `host.typeChain(h)`.
- **`construct(self, kind, props, values, n, out)`** (nullable) creates an instance with construct-time properties. GTK: `g_object_new_with_properties`, so construct-only properties work; widgets get `OWNED` / `TOPLEVEL` handles like `create`, other objects `SHARED`. `create(kind)` is `construct` with no props. The new handle is written to `out` as an `SVL_HANDLE`.
- **`call_static(self, kind, method, args, n, out)`** (nullable) calls a static function or constructor function (`g_file_new_for_path`, `pango_font_description_from_string`). `resolve(SVL_METHOD, kind, name)` gives the id; the member descriptor carries `SVL_M_STATIC`, and `SVL_M_CTOR` when it returns a new reference. Out parameters follow the S2 convention (`LIST [ret, outs...]`).
- **`SVL_T_TYPE = 11`:** a `STR` slot (or value) carrying a native type name (`Gio.ListStore`, or the GType name), for GType-valued properties and parameters (`Gio.ListStore:item-type`, `g_list_store_new`). JS passes the generated class; the module turns it into its `native` name.
- **Host ops** `host.construct(kind, props)`, `host.callStatic(kind, method, args)` and `host.typeChain(h)`. The op log has two new lines, `construct #h Kind {json}` and `static Kind.method [json]`; a constructor-shaped method name (`new`, `new_*`, `from_*`) answers a fresh handle and the line ends with ` #h`. The Node TestHost and the log backend both implement them.
- **GTK invoker** (`crates/svelters-gtk/src/gi.rs`) now loads the Gtk, Gdk, Gsk, Gio, GLib, GObject, Pango and Graphene typelibs and dispatches on the receiver's GType (objects, boxed values, interfaces the runtime class implements). It marshals out and inout parameters, caller-allocated out structs, `GList` / `GSList` and C arrays of strings, objects, boxed values and numbers (length arguments hidden), `GType`, `gunichar`, statics and constructors (N1 added callbacks by scope and `_async`/`_finish` pairs, below). Still refused with a clear error: `GVariant`, hash tables, `GArray` / `GPtrArray`, fundamental classes, unions, structs passed by value.

### N1: GTK async members and GI closures (no ABI change)

NT1 (`plans/milestones/NT1-gtk-async.md`) uses the v3 `CALLBACK` / `SVL_M_ASYNC` model unchanged.

- **GTK invoker.**
  - Callback parameters of typelib functions are `CALLBACK` slots. Their type descriptor (`describe_type`, ids from `SVL_T_FIRST_BACKEND + 0x0100_0000`) carries the argument slots, the result slot and the scope. `forever` is described as `notified`, because the host refuses `forever`.
  - `_async` functions with a `GAsyncReadyCallback` fold with their `_finish` into one `SVL_M_ASYNC` member. Its completion type is `SVL_T_FIRST_BACKEND + 0x00FF_0000`.
  - The completion value is the finish result, or a `MAP` `{ret?, <native out name>...}` when the finish function has out parameters. A `GError` crosses as `ERROR`, with the code named through the domain's registered error enum (`gi_repository_find_by_error_domain`).
  - Async statics are `SVL_M_STATIC | SVL_M_ASYNC`.
- **Host** (not ABI):
  - Handles inside `invoke_callback` arguments and `post_event` arguments join the handle table, as call results already did.
  - When an owner dies, an async completion tied to it is detached and settled at the next checkpoint with `NativeError { domain: 'svelters', code: 'disposed' }`, instead of being dropped. Other scopes are released as before.
  - `callStatic` of an `SVL_M_ASYNC` member returns a Promise; it has no owner.

### Qt non-GUI types (NT2, no ABI change)

The Qt backend now implements the v2.5 members (`construct`, `call_static`, `describe(SVL_KIND)`); nothing is
added to the header. Conventions a host or test may rely on (`plans/milestones/NT2-qt-types.md` has the design):

- **Kinds.** Every bound class of QtCore/QtGui/QtWidgets non-widgets (`metadata/qt/types/`) is a kind named by
  its C++ class (`QTimer`, `QUrl`), ids from `0x20000`; `describe(SVL_KIND, 0, k)` answers the name and the
  nearest bound base. Element kinds keep their tags; `resolve(SVL_METHOD, element_kind, name)` also finds the
  methods of the element class's bound type after the element's own tables and getters.
- **Lifetimes.** `construct` (default constructor, then each prop through its `QMetaProperty`) and a
  constructor called as `call_static(k, "new", args)` give QObjects a `SHARED` handle: `destroy` deletes a
  parentless object and only forgets one a QObject parent owns now. Objects a getter returns stay `BORROWED`.
  Value classes are `VALUE` handles whose `handle_info.kind` is their class (`call` reaches their const members,
  and `equals(other)` compares with QMetaType equality). Non-QObject objects JS makes (`QMimeDatabase`) are
  `OWNED`; container items (`QStandardItem`) are `OWNED` until a PySide `parent add` fact moves them into a
  container, then `BORROWED`, and their native deletion is reported through `on_destroyed` (`SVL_GONE_OWNER`).
- **Overloads (decision Q4).** One method id per JS name. A member with several overloads describes dynamic
  parameter slots (`SVL_NULL` typed `SVL_T_VARIANT`, optional beyond the smallest arity), so the host encodes
  arguments by their JS type (plain objects as `MAP`); the generated thunk takes the first overload, in a fixed
  metadata-derived order, whose arity and argument types fit. Records also accept a `MAP` by field name and
  flags a `LIST` of member names.
- **Statics.** `call_static` resolves `resolve(SVL_METHOD, k, "static:<name>")` to the static when an instance
  method has the same name (`QFileInfo.exists`); plain `<name>` otherwise.
- **Meta fallback.** On QObject type kinds, a prop, signal or method name with no facts resolves to a dynamic
  id (from `0x1000000`) served through the object's `QMetaObject` (`QMetaProperty`, a signal relay,
  `QMetaMethod::invoke`).
- **Async.** A `QFuture<T>` result completes the host's one-shot callback (`SVL_M_ASYNC`); no bound Qt 6.11
  member returns one yet, so only the adapter exists (tested through a test hook).

### AppKit non-GUI types (N4, no ABI change)

The AppKit backend now implements the v2.5 members (`construct`, `call_static`, `describe(SVL_KIND)`) and
`cancel`; nothing is added to the header (`plans/milestones/N4-appkit-types.md` has the design).

- **Kinds.** Any loaded Objective-C class is a kind named by its class (`NSURL`, `NSPasteboard`), ids from
  `0x10000`, registered on first use; element tags resolve first. An object's kind is the nearest class of the
  bound scope (`metadata/appkit/types/`) along its class chain, so a cluster's private class reports the public
  one. `describe(SVL_KIND, 0, k)` answers the class name and its superclass's kind (element kinds too).
- **Lifetimes.** Every non-widget object crosses as a `SHARED` handle holding one strong reference, one handle
  per object (identity by pointer). That includes objects element getters return (`font`, `text-color`): only
  views, windows, cells and other responders stay `BORROWED`. Immutable value classes (`NSURL`, `NSColor`) use
  the same `SHARED` handles; nothing crosses as `BOXED`.
- **Members by selector.** `resolve(SVL_METHOD, type_kind, name)` takes the selector itself (`setString:forType:`)
  for instance or class methods; `call` invokes on the object and `call_static` on the class (`NSInvocation`).
  `call_static` of an `init…` selector is alloc + init (the result is +1); results of the new/copy families are
  +1. A selector ending in `error:` hides its `NSError **`: a failure is `Ok(ERROR)`, i.e. a `NativeError`
  (domain, integer code, `localizedDescription`). Props resolve by KVC key or kebab name (`levelsOfUndo`).
- **Async.** A member of the generated `ASYNC` table describes itself with `SVL_M_ASYNC` (plus `SVL_M_STATIC`)
  and a trailing `CALLBACK` slot; the backend builds a completion block of the member's shape, queues its
  retained arguments from whatever thread runs it, and completes the callback on the main thread with the
  result or the `NSError`. `cancel` cancels a returned `NSProgress` and rejects at once with
  `NSCocoaErrorDomain` 3072; destroying the owner drops its operations.
- **Op-log hosts.** The TestHost answers `callStatic(kind, "init…:")` with a fresh handle, like `new`.

### WinUI non-GUI types (N4, no ABI change)

The WinUI backend implements the v2.5 members (`construct`, `call_static`, `describe(SVL_KIND)`) and `cancel`;
`plans/milestones/N4-winui-types.md` has the design.

- **Kinds.** Every bound Windows Runtime class or interface (`metadata/winui/types/`) is a kind named by its full
  native name (`Windows.Foundation.Uri`), ids from `0x20000`; a used generic collection instance is a kind too
  (`Windows.Foundation.Collections.IVector<string>`). Kind id `0x20000` is `Windows.Foundation.IInspectable`, the
  root an object of an unbound class wraps as. `describe(SVL_KIND, 0, k)` answers the name and the nearest bound
  base. Element tags resolve first, so a tag never collides with a type name.
- **Lifetime.** Every runtime class object is `SHARED` (one COM reference, identity by IUnknown pointer in the same
  table as the elements, so an element passed back is its element handle). `destroy` drops our reference and never
  calls `Close()`; `IClosable.Close` is a method (`close`).
- **Members.** Methods and props are camelCase JS names, events kebab-case; a static that shares its name with an
  instance method is `static:<name>`; constructors are the static `new`. Every member describes dynamic parameter
  slots (`SVL_NULL` typed `SVL_T_VARIANT`, optional beyond the narrowest overload), so the host encodes arguments
  by their JS type, and the backend picks the overload by argument count (trailing nulls, which the host pads,
  select the narrowest overload that takes every non-null argument). Records are `MAP`s by camelCase field name,
  enums kebab-case names (native and camelCase names and numbers are accepted), flags lists of names, `DateTime`
  `SVL_T_DATETIME`, `TimeSpan` milliseconds.
- **Async.** `IAsyncOperation<T>` / `IAsyncAction` (and the `WithProgress` variants) are `SVL_M_ASYNC`; the
  completion handler hops to the UI dispatcher queue before anything is converted, then settles the callback with
  `GetResults` or a `NativeError { domain: 'winrt', code }` (`file-not-found`, `cancelled`, or `0x8xxxxxxx`).
  `cancel(id)` is `IAsyncInfo.Cancel`; destroying the owner cancels its operations.
- **Errors.** A failing WinRT call answers `rc -1` with an `ERROR` value in the same shape (`SVL_M_THROWS` on every
  member).
- **Threads.** `dispatcher`-class objects (a `DispatcherQueue` property on the class or a base) are refused at
  minting unless their queue has thread access on the UI thread, and checked on every call in debug builds.

### ABI v2.6 additive (property observation)

Status: R1a (2026-10-19) added the members; R1 (2026-10-20) implemented the host contract of
`plans/research/REACTIVITY.md` §5.2 (decisions §13). Everything here is additive within v3: both new
members are nullable, and the new member flag defaults to 0.

```c
/* backend, after call_static. NULL = no property can be observed. */
int (*observe)(void *self, svl_handle h, svl_id prop, bool on);
/* host, after frame. NULL = a host that never observes. UI thread only. */
void (*prop_changed)(void *self, svl_handle h, svl_id prop);
/* member flags on SVL_PROP descriptors */
SVL_M_OBSERVABLE = 64     /* observe can watch it, and the platform announces its changes (exact/partial) */
SVL_M_PROPORTIONAL = 128  /* R1: reading costs O(size) (whole-text props): invalidation only, see below */
/* observe's answer for a prop with no change notification on this backend (other failures: -1) */
#define SVL_OBSERVE_UNSUPPORTED (-2)
```

- **`observe(h, prop, on)`** starts or stops watching one property natively. Calls nest: the backend
  counts per (h, prop), connects on the first `on` and disconnects on the matching last `off`; `off`
  for an unobserved prop or a dead handle is `0`. `destroy(h)` and the native death of `h` drop every
  observation of it. An observation never keeps the object alive and never holds a JS value.
  `SVL_OBSERVE_UNSUPPORTED` means the prop is valid but has no change notification here; `-1` is a
  dead handle or an unknown, write-only or constant (construct-only, `CONSTANT`) prop.
- **`prop_changed(h, prop)`** says "this may have changed": no value, and not a promise that it did
  (GTK re-notifies identical sets). A backend may report props nobody observes; the host ignores them.
  UI thread only: a backend hops there first. It never runs JS and may be called while JS runs (a host
  op that changes the object emits it synchronously).
- **`SVL_M_OBSERVABLE`** on a prop descriptor: `observe` can watch it and the platform's observe fact
  (`metadata/observe/<tk>.json`) is `exact` or `partial`. GTK reads the facts (a rule-made `constant`,
  `set_only` or `none` prop is not observable; a prop without a fact keeps the mechanical rule: readable,
  not construct-only). Qt does not set it yet (R3); AppKit and WinUI set it from their facts (R4).
- **`SVL_M_PROPORTIONAL`** on a prop descriptor: the fact's `value_cost: proportional` (GTK
  `TextBuffer:text`, `TextView:text`). The host never reads or compares such a prop after a change and
  delivers it as an invalidation (`value` undefined); the backend suppresses the echo of the host's own
  `set_prop` of it (REACTIVITY.md §5.4), since no compare can drop it.

#### Host contract (crates/svelters-host, R1)

| Rule | Implementation |
|---|---|
| refcount | `host.observe(h, prop, on)` counts per (h, prop id) and calls the backend on 0→1 and 1→0 only, so wrapper getters, `observeProp` callbacks and element bindings share one native connection. It answers `true` while observed and `false` when the backend has no mechanism (or no member). At 0→1 a non-proportional prop is read once as the baseline `last`. |
| coalescing | `prop_changed` inserts (h, prop) into an ordered, deduplicated dirty set (first-announcement order) and wakes the backend on the first insert into an empty set. Unobserved pairs are ignored. Nothing is read here. |
| read | at the checkpoint each dirty pair still observed is read with `get_prop` (no JS), unless it is proportional. A failed read (the object died) delivers nothing. |
| compare | with `last`, the last value delivered or read back after our own `set_prop`. Equal values are dropped. Handles compare by id, strings by content (UTF-8 and UTF-16 alike), lists, records and maps deeply, f64 by bits (NaN equals NaN, -0 differs from 0). A value holding a `BOXED` handle always counts as changed and is never kept as `last`; a read whose value nobody will see is released at once. |
| batch | one `globalThis.__svelters_changed([h, prop, value, h, prop, value, ...])` per checkpoint (prop as JS first named it; `value` undefined for proportional props), **before** the queued signal events, with no job drain between the batch and the first event. Svelte then flushes once for the whole batch. |
| set path | after a successful `set_prop` on an observed, non-proportional (h, prop) the host reads it back once and stores it as `last`: the native echo of our write is dropped. If the read-back differs from the value written (the toolkit normalized it), the pair is forced dirty and the normalized value is delivered once at the next checkpoint. |
| sync handlers | before `call_sync` runs JS, the dirty set is read, compared and delivered as a batch (no drain), so `$derived` values the handler reads are fresh. |
| death | `destroy(h)` and `on_destroyed(h)` drop the observations, `last` values and dirty entries of `h`; `observe(off)` to 0 drops the pair, and its pending dirty entry is skipped. |
| scripted mode | a `changed` step is delivered like an event: the checkpoint the log backend runs after it delivers the batch, then any events, without draining. |

R1 replaced R1a's per-notification `__svelters_prop_changed(h, prop)` path: no runtime installs it any more.

#### JS runtime

`@svelters/native-runtime` installs `__svelters_changed` and the tracker of `@svelters/types`
(`installTracker`) when it loads (`src/observe.js`):

- `observeProp(h, prop, fn) -> unsubscribe`: `fn(value)` once per delivered change (`value` undefined for a
  proportional prop); `unsubscribe.observed` says whether the host observes it natively.
- `trackProp(h, prop)`: subscribes the running effect (Svelte's `createSubscriber` semantics, through the
  core adapter's `subscriber`); `trackLocal(h, prop)` for `set_only` props (re-runs on our writes only);
  `invalidateProp(h, prop)`: re-runs the readers now.
- The table is keyed by handle number and canonical prop name (`cursor-position` = `cursorPosition`) and never
  holds a wrapper. The last subscribed effect's teardown (one microtask later) calls `host.observe(off)`.
- `@svelters/types` core: `observed(self, prop)` (exact/partial getters), `observedLocal` (set_only),
  `observedSignal(self, prop, signal, read)` (content accessors with no property behind them, Qt
  `QTextDocument.plainText`), `setProp` invalidates locally, `NativeObject.peek(prop)` is the snapshot read.
  Without a tracker (plain scripts) every getter is a plain read.

#### Op log and scripts

`observe #h prop on|off` (the transitions) and `changed #h prop <json>` (a scripted native change; the
value is omitted when the step has none). Scripts take `{"changed": [h, prop, value?]}`: the value becomes
the property's native value, then the change is announced. The log backend stores it, logs the line, calls
`host.prop_changed` and checkpoints; the Node TestHost implements the same compare (baseline at the first
observation, read-back after `setProp`, deep equality) and calls `__svelters_changed([h, prop, value])` when the
value differs. Neither has proportional props. `tools/oplog-compare.mjs` keeps the observed props per node in
the compared state and treats `changed` as a boundary.

#### Backends

- **GTK** (`crates/svelters-gtk/src/backend/observe.rs`): `notify::<prop>` on any GObject handle, widgets
  (generated ids) and non-widgets (reflected ids) alike, and on the object a `via` property is forwarded to
  (GtkRange `value` on its adjustment). The handler captures ids and the host pointer cell only; a `via` target
  is held weakly, and the forwarding prop (`adjustment`) is watched too: when it changes, the observation moves
  to the new target and the prop is reported changed (rule O9). A notify on another thread is moved to the main
  context with an idle. The descriptor flags come from the observe facts generated into the tables. Refused:
  virtual (svelters-computed) props, menu, `virtual-list` and service-kind own members (unsupported);
  write-only and construct-only props (-1). `listen('notify::x')` stays a separate path: an ordinary listener
  that posts an event.
- **Qt** (`tools/metadata/qt/templates/backend.cpp.in`): the prop's JS name in camelCase is looked up on the
  object's metaobject; a `Q_PROPERTY` with a NOTIFY signal is connected to a per-(h, prop) relay object through
  `QMetaObject::connect(sender, notifyIndex, relay, slot)` with no receiver metaobject, so Qt calls the relay's
  `qt_metacall` (the QSignalSpy technique: no moc). A coarse NOTIFY (`QAction::changed`) reports every observed
  prop that shares it. No NOTIFY: `SVL_OBSERVE_UNSUPPORTED` (QBindable, events and signal pairs are R3).
  `CONSTANT` and write-only: -1. Relays are deleted with the handle (`drop_listeners`).
- **AppKit** (`crates/svelters-appkit/src/observe.rs`, milestone R4): from the prop's observe fact (generated
  `OBSERVE` table, `metadata/observe/appkit.json`, looked up along the element's chain): KVO on the KVC key through
  one backend observer object, `context` = a per-registration (h, prop) record; notifications
  (`NSControlTextDidChangeNotification`, `NSWindowDidResize/DidMove`) and the target-action pair through the
  handle's target, user-counted together with `listen`. Teardown: OWNED/TOPLEVEL remove KVO in `destroy`/close
  before the reference is released; BORROWED objects of autonotifying classes are not retained, an identity
  sentinel (associated object) reports their death (`SVL_GONE_OWNER`), the runtime cleans up the registration
  (macOS ≥ 10.13; the minimum is 11); other classes are retained while KVO-observed. KVO from another thread is
  queued, deduplicated and delivered by the next wake. Facts `none`/`set_only` and props without a fact:
  `SVL_OBSERVE_UNSUPPORTED`; constant and write-only: -1. `SVL_M_OBSERVABLE` is set on the scalar/record props
  `describe` covers.
- **WinUI** (`crates/svelters-winui/src/backend.rs`, generated `OBSERVE_CONNECT`, milestone R4): the DP change
  callback (`RegisterPropertyChangedCallback`, `(dp, token)` per (h, prop)) and event fallbacks (`SizeChanged`,
  `SelectionChanged`, `Loaded`, `ActualThemeChanged`, `VisibilityChanged`) with their revokers; delegates capture
  the ids only and reach the backend through its thread-local weak reference. Unregistered by `observe(off)`,
  `destroy` (before the element reference is released) and native death. UI thread only
  (`DispatcherQueue.HasThreadAccess`, else -1). Codes as AppKit; `SVL_M_OBSERVABLE` from the facts.
- **Echo (AppKit, WinUI):** while `set_prop` writes (h, prop), a `value_cost: proportional` prop is not reported
  (REACTIVITY.md §5.4); cheap props are reported and the host's compare drops the echo. They do not set
  `SVL_M_PROPORTIONAL` yet (no AppKit/WinUI fact is proportional today).

### ABI v2.7 additive (G0): non-blocking start and the started hook

Status: G0 (2026-10-21). One nullable member at the end of `svl_backend`; `svl_host` is unchanged.

```c
/* backend, after observe. NULL = only the blocking `run` is supported. */
int (*start)(void *self, const svl_host *host);
```

- **`start(self, host)`** attaches the host and brings the toolkit up (initialises it, creates the
  application object) **without running the loop**, and returns at once: `0`, or `< 0` with
  `last_error`. The caller (an embedder, a test harness, a platform that owns `main`) then drives the
  loop, and the backend calls the host exactly as it would from `run` (checkpoint on `wake`, timers,
  frames, events). `start(self, NULL)` detaches: no host callback afterwards (queued wakes and timers
  skip the host); it is `0` when nothing is attached. `run` after `start` stores the same host again.
- **Started** is the host's notion, not a callback: the toolkit is up at the first host callback the
  backend delivers from `run`/`start` (checkpoint, frame, post_event, call_sync, prop_changed,
  on_destroyed, invoke_callback; not release_callback), or when `start` returns if none came during it.
  `Host::on_started(f)` fires `f` once at that point, before the callback is handled (a callback made
  while JS runs defers it to the next one), so `f` may load the bundle. Every desktop `run` already
  checkpoints once when its loop starts; with a hook pending, `Host::run` also calls `wake` just
  before `run`, for loops that only checkpoint on a pending wake (the log backend).
- **Two host orders.** Desktop: `load_bundle` then `run` (unchanged). Platforms that own `main`,
  embedders and kittest-style harnesses: `run_with(|h| h.load_bundle(..))` (blocking) or
  `start_with(|h| h.load_bundle(..))` (non-blocking; the caller pumps the toolkit and/or calls
  `Host::checkpoint`/`Host::frame`), then `Host::stop` (also done by `Drop`). `Host::is_started`.
- **Attach before mount.** On a backend with `start`, `Host::load_bundle` on a host that is not
  running first calls `start(host)` (once), so callbacks made during the desktop-order mount (a GTK
  `CustomFilter`/`CustomSorter` calling `invoke_callback` while a model is built, a wake) reach the
  host; that pre-attach does not count as started. `Drop` detaches.
- **Backward compatibility:** a NULL `start` means unsupported (`Host::start` errors; the
  backend-specific `attach_host`/`set_host` hooks keep working); `run` without a hook is unchanged.
- **Backends:** GTK stores the host pointer (GTK is initialised by `GtkBackend::new`; the caller
  iterates the default main context), and `wake` re-reads the pointer at dispatch. Qt sets `g.host`
  and creates the `QApplication` (`ensure_app`), delivering a wake that came before the waker existed.
  The log backend records the attachment only (no op logged; in start mode its script is not played
  and the embedder checkpoints). AppKit (FU1) stores the host (`AppKitBackend::new` already set up
  `NSApplication` and the run-loop sources; the caller pumps the run loop) and queues a checkpoint for
  the first loop turn, as `run` does. WinUI (FU1) stores the host (`WinUiBackend::new` already
  initialised XAML and the dispatcher queue; the caller pumps messages), posts type events raised
  before it attached, and queues a checkpoint. Both read the host when a wake, timer or frame fires,
  so nothing reaches a detached host; `attach_host` keeps working. All four desktop backends
  implement `start`. The iOS and
  Android backends implement `start` and checkpoint once the platform has brought the UI up
  (docs/notes/ios-uikit-backend.md §2.2 decision I1, docs/notes/android-backend.md A2); egui
  embedding uses it as plans/research/EGUI.md §8 D1 describes.

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
- Descriptor-driven prop coercion replaces the renderer's hand-written `INT_PROPS` (since RT1 the compiler types static values from the semantic spec).
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
