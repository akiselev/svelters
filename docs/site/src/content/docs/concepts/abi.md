---
title: Backend ABI
description: Reference for the fixed C ABI between the Rust host core and every toolkit backend, including values, checkpoints, the op log and the planned v2 and v3 changes.
---

The backend ABI is the L2/L3 boundary. A Rust host core drives every backend through one vtable, whatever the backend's language (Rust for GTK and egui, C++ for Qt). The header `crates/svelters-abi/include/svelters_backend.h` and its Rust mirror `crates/svelters-abi/src/lib.rs` are authoritative for exact spelling.

Goals:

- One fixed C ABI. It never grows per native method, property or widget kind.
- Names resolve to `u32` IDs once. Per-ID behaviour lives in backend code (hand-written in P, generated from M3).
- JS never sees native pointers. The host keeps a handle table; backends see opaque `svl_handle` values that they map to their own objects.

:::note[Version]
The header in the repository is **ABI version 1** (`SVL_ABI_VERSION 1u`). Sections marked **planned** describe ABI v2 (approved 2026-10-17, lands at the start of M2) and v3 (M3). They are not in the header yet.
:::

## Data model

```c
typedef uint64_t svl_handle; /* 0 = null; issued by the backend on create */
typedef uint32_t svl_id;     /* resolved kind/prop/signal/method/slot id; 0 = unknown */

typedef enum svl_tag {
  SVL_NULL = 0, SVL_BOOL = 1, SVL_I64 = 2, SVL_F64 = 3,
  SVL_STR = 4, SVL_HANDLE = 5, SVL_LIST = 6
} svl_tag;

typedef struct svl_str  { const char *ptr; size_t len; } svl_str;   /* UTF-8, not NUL-terminated, borrowed for the call */
typedef struct svl_list { const struct svl_value *ptr; size_t len; } svl_list;   /* borrowed for the call */

typedef struct svl_value {
  uint32_t tag; /* svl_tag */
  union { bool b; int64_t i; double f; svl_handle h; svl_str s; svl_list l; } u;
} svl_value;

typedef enum svl_name_kind { SVL_KIND = 0, SVL_PROP = 1, SVL_SIGNAL = 2, SVL_METHOD = 3, SVL_SLOT = 4 } svl_name_kind;
```

### `svl_value` and `svl_tag`

| Tag | Union member | JS side |
| --- | --- | --- |
| `SVL_NULL` | none | `null` |
| `SVL_BOOL` | `u.b` | boolean |
| `SVL_I64` | `u.i` | integer or integral float |
| `SVL_F64` | `u.f` | other numbers |
| `SVL_STR` | `u.s` | string (UTF-8) |
| `SVL_HANDLE` | `u.h` | opaque handle |
| `SVL_LIST` | `u.l` | array (depth limit 16 in the host) |

- The union member is named `u`; `svl_str` and `svl_list` are named structs.
- Strings and lists are borrowed for the duration of a call. A value a backend returns goes into a host-provided `out` slot and stays valid until the backend's next call; the host copies it before the next ABI call.
- The host converts `undefined` and plain objects to a `TypeError` today. Property values in P never use `SVL_LIST` or nested objects.
- Return codes: `0` is ok, negative is an error with a message from `last_error`. A stale or foreign handle is an error, never a crash. Backends track liveness: `QPointer` for Qt, weak refs or a destroy notify for GObject.

## Backend vtable

```c
typedef struct svl_backend {
  uint32_t abi_version; /* SVL_ABI_VERSION */
  void *self;
  svl_id (*resolve)(void *self, uint32_t what /* svl_name_kind */, svl_id kind, const char *name, size_t len);
  svl_handle (*create)(void *self, svl_id kind);
  int (*insert)(void *self, svl_handle parent, svl_handle child, svl_handle before /* 0 = end */, svl_id slot /* 0 = default */);
  int (*remove)(void *self, svl_handle parent, svl_handle child);
  void (*destroy)(void *self, svl_handle h);
  int (*set_prop)(void *self, svl_handle h, svl_id prop, const svl_value *v);
  int (*get_prop)(void *self, svl_handle h, svl_id prop, svl_value *out);
  int (*call)(void *self, svl_handle h, svl_id method, const svl_value *args, size_t n, svl_value *out);
  int (*listen)(void *self, svl_handle h, svl_id signal, bool on);
  int (*run)(void *self, const svl_host *host); /* owns the toolkit loop; returns on quit */
  void (*wake)(void *self);                     /* thread-safe: schedule a host checkpoint */
  void (*quit)(void *self);
  const char *(*last_error)(void *self); /* NUL-terminated, valid until the next call */
} svl_backend;
```

<a id="resolve"></a>
### resolve

`svl_id resolve(self, what, kind, name, len)` maps a name to an ID. `what` is an `svl_name_kind`. `kind` scopes `SVL_PROP`, `SVL_SIGNAL`, `SVL_METHOD` and `SVL_SLOT` lookups to a kind ID (pass `0` for `SVL_KIND`). Returns `0` for an unknown name; unknown kinds, slots and props also set a `last_error` message. The host caches name to ID resolution per kind, so each name is resolved once.

<a id="create"></a>
### create

`svl_handle create(self, kind)` makes a native object of the resolved kind and returns a new handle, or `0` on error. The renderer creates a widget lazily, when a node is first inserted under a native-backed parent, so a `create` appears in the op log next to its first `insert`. Both backends number handles 1, 2, ... in creation order. Under the planned lifetime model, `create` mints `owned` or `toplevel` handles (GTK widgets are `ref_sink`ed).

<a id="insert"></a>
### insert

`int insert(self, parent, child, before, slot)` places `child` under `parent`. `before` is the handle of the next native sibling in the same slot, `0` meaning append. `slot` is a resolved `SVL_SLOT` ID, `0` meaning the kind's default slot. Inserting an already-parented child moves it. Which native call runs comes from the slot's `ops.insert` descriptor in the annotations. Fails with a message for an unknown slot or a wrong child kind.

<a id="remove"></a>
### remove

`int remove(self, parent, child)` detaches `child` and leaves it alive and unparented, so it can be inserted again. Backends must leave it either unparented or hidden (`kept_child_hidden`); a native removal that deletes the child is only allowed on slots annotated `on_remove: destroys_child`. `remove` of a child that was not inserted through a slot is an error on GTK.

<a id="destroy"></a>
### destroy

`void destroy(self, h)` is the host dropping its last reference. After it returns the handle is stale. What happens natively depends on the lifetime class (see [Ownership](/concepts/ownership/#lifetime-classes)).

- **Version 1 (implemented):** Qt disconnects, erases the entry and calls `deleteLater`. GTK drops the table's reference. A stale destroy is a no-op.
- **ABI v2 (planned):** `destroy` is idempotent and defined per class: owned deletes the native object, toplevel closes and destroys, borrowed only forgets, value unrefs. It never calls `on_destroyed` for `h` itself, and it may cascade `on_destroyed(d, PARENT)` to descendants.

<a id="set_prop"></a>
### set_prop

`int set_prop(self, h, prop, v)` writes a property. `prop` is an `SVL_PROP` ID scoped by the handle's kind. The host converts JS values: integers and integral floats to `SVL_I64`, other floats to `SVL_F64`. The backend coerces to the native type (enums are passed as names) and returns an error for a wrong value type. Programmatic sets of text on entries do not emit change events to JS (see [Bindings](/guides/bindings/)).

<a id="get_prop"></a>
### get_prop

`int get_prop(self, h, prop, out)` reads a property into the host-provided `out` slot. The value stays valid until the backend's next call.

<a id="call"></a>
### call

`int call(self, h, method, args, n, out)` invokes a method with `n` arguments and writes any result to `out`. In P only `window.present` exists on GTK (`call #h present []`); Qt maps `present` to `show` and has `click` on buttons. General dynamic method invocation is M3. Blocking methods that spin a nested toolkit loop (`QDialog::exec`, the deprecated `gtk_dialog_run`) must not be exposed; metadata marks them `unsupported_reason: blocking_nested_loop`.

<a id="listen"></a>
### listen

`int listen(self, h, signal, on)` connects (`on` true) or disconnects (`on` false) a native signal. The backend connects natively and reports through `host->post_event`. Only signals with a JS listener are connected. A signal whose listeners differ by capture, after or sync would need separate connections, which the version 1 signature cannot express.

**Proposal, not applied:** an optional `flags` argument, `{phase?: 'capture'|'target'|'bubble', after?: bool, sync?: bool}`, with `off` carrying the same flags and the dispatch back into JS naming the connection that fired. See [Events](/guides/events/#listen-flags-abi-proposal-not-applied). The v2 notes list "listen flags for phase, once, after and sync" as a later addition, not part of v2.

<a id="run"></a>
### run

`int run(self, host)` hands the toolkit loop to the backend. The backend owns the loop and returns when the app quits. From its loop it calls `host->checkpoint` after `wake`. GTK registers one shared `GtkApplication` per UI thread; the Qt `run` always queues exactly one initial checkpoint.

### wake, quit, last_error

- `void wake(self)`: thread-safe. Schedules a host checkpoint on the loop thread.
- `void quit(self)`: asks the backend to leave `run`.
- `const char *last_error(self)`: NUL-terminated, valid until the next call.

<a id="schedule_wake"></a>
### schedule_wake (planned)

**ABI v2, approved.** JS timers must run on the toolkit's native timing. The P2 host currently runs its own timer thread that calls `wake` at each deadline; that is a stopgap to be removed.

```c
/* backend: (re)arm a single one-shot wakeup `ms` from now; replaces any pending one.
   ms < 0 cancels. On expiry the backend calls host->checkpoint. Main thread only. */
void (*schedule_wake)(void *self, int64_t ms);
```

- The host keeps its timer heap and, after every checkpoint, arms only the earliest deadline.
- GTK: `glib::timeout_add_local_once` (or `timeout_add_seconds` for coarse timers). Qt: one single-shot `QTimer`, `Qt::PreciseTimer` for short timeouts and `Qt::CoarseTimer` otherwise. egui: `request_repaint_after`. The log backend implements it against its virtual clock, so scripted fixtures stay deterministic.
- `wake` stays for thread-safe "run a checkpoint soon" requests.
- `requestAnimationFrame` gets the same treatment later through a backend `request_frame` driven by the frame clock.

## Host callbacks

```c
typedef struct svl_host {
  void *self;
  void (*post_event)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n);
  void (*checkpoint)(void *self);
  int (*call_sync)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n, svl_value *out);
  void (*on_destroyed)(void *self, svl_handle h);
} svl_host;
```

<a id="post_event"></a>
### post_event

`post_event(self, h, signal, args, n)` queues a native event. It is delivered to JS at the next checkpoint as `__svelters_dispatch(h, signal, args)`. After a native event the host calls `backend.wake()` (outside scripted mode) so the loop runs a checkpoint. `clicked` posts no arguments; `changed` posts `[text]`.

### checkpoint

`checkpoint(self)` runs queued events, JS jobs and a flush. The backend calls it from its loop after `wake()`. A checkpoint requested while JS is running (a backend op that spins a nested loop) is deferred and becomes a later `wake`.

<a id="call_sync"></a>
### call_sync

`int call_sync(self, h, signal, args, n, out)` is the synchronous handler call for signals and virtuals that return a value or can be vetoed. **Planned (M4):** the host returns an error until then. The design: the host builds an event object, dispatches it to the JS handlers, and computes `out` from `defaultPrevented`, `cancelBubble`, `returnValue` and metadata-declared out-fields, not from the handler's return value. See [Events](/guides/events/#the-event-object-design-m4). Open questions: re-entrancy rules, returned Promises, and timeouts.

<a id="on_destroyed"></a>
### on_destroyed

`on_destroyed(self, h)` reports that a native object went away underneath the backend (a window closed, a parent was deleted). Version 1 signature as in the header.

- The backend must erase its table entry before calling it. For a QWidget the callback can arrive for a parent before its children.
- The host only forgets the handle (`Core::forget_handle`); it does not enter JS or call the backend for `h`.
- **ABI v2 (planned):** the signature becomes `void (*on_destroyed)(void *self, svl_handle h, uint32_t reason)`, called at most once per handle after the backend has dropped it, with the death cascading to every descendant handle. See [ABI v2](#abi-v2-planned).

## Checkpoint semantics

1. Deliver queued native events to JS.
2. Run Svelte effects and QuickJS jobs, up to a bounded number (`max_jobs_per_checkpoint`).
3. Apply the resulting tree and property operations through the vtable.
4. If jobs remain, call `backend->wake()` and return to the toolkit loop.

P applies operations directly as they are issued instead of batching. P5 measured 11,000 operations costing 16 to 18 ms of host and ABI time, so batching to save ABI crossings is not justified; reducing operation count could still help toolkits that relayout per operation (Qt).

Scripted mode (`HostConfig::scripted`) steps differently for fixtures: an `event` step dispatches at once with no job drain, a `checkpoint` step drains jobs and fires the earliest timer on a virtual clock until both are empty, and the run ends with a final job-only drain.

## JS-side host ops

The L1 renderer calls these and the host forwards them to the vtable:

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

`__svelters_dispatch` is synchronous and returns `{ results, event }`. The bundle defines `__svelters_root` and `__svelters_dispatch`; the host provides `__svelters_host`, `queueMicrotask`, `setTimeout` and `clearTimeout`.

## Op-log format

The Node `TestHost` and the Rust log backend must produce identical logs for the same fixture and scripted interaction. UTF-8 text, one op per line. Handles are numbered `#1, #2, ...` in creation order per run. Values are compact JSON.

```text
create <kind> #<n>
insert #<parent> #<child> before=#<h>|end slot=<name>|default
remove #<parent> #<child>
destroy #<h>
set #<h> <prop> <json>
call #<h> <method> <json-args-array>
listen #<h> <signal> on|off
event #<h> <signal> <json-args-array>        # an injected native event, logged when delivered
```

Kinds, props, signals and slots appear by JS-facing names, never by IDs. A scripted interaction is a JSON array of `{"event": [<handle-number>, <signal>, <args>]}` and `{"checkpoint": true}` steps. Fixtures store the expected log in `expected.oplog`.

## Open questions

- `call_sync` re-entrancy rules (see [Events](/guides/events/)).
- Per-child attributes (grid cell, tab title, dock area) need a `set_child_attr(parent, child, attr_id, value)` operation, planned for M4. `insert(..., slot)` is enough for placement.
- Batching: decided by measurement after P5; the measurement says it is not needed.

## Planned extension: foreign runtimes

From `docs/notes/node-deno-integration.md`, post-P5. Optional members so stock Node (N-API) or Deno (FFI) can drive a backend while the toolkit loop stays authoritative: host callbacks `foreign_fd()` and `foreign_timeout_ms()` watched by the backend's loop, backend `poll_prepare`/`poll_dispatch` for runtimes that insist on owning the main thread, and a rule that nested toolkit loops may re-enter `checkpoint`. JS freezes during nested modal loops in the poll mode.

<a id="abi-v2-planned"></a>
## ABI v2 (planned)

Approved 2026-10-17, lands at the start of M2. `SVL_ABI_VERSION` becomes 2. It bundles toolkit-native timers (`schedule_wake`, above) and the lifetime model from [Ownership](/concepts/ownership/).

```c
/* lifetime */
typedef enum svl_lifetime { SVL_LIFE_OWNED = 1, SVL_LIFE_TOPLEVEL = 2, SVL_LIFE_BORROWED = 3, SVL_LIFE_VALUE = 4 } svl_lifetime;
typedef enum svl_gone { SVL_GONE_UNKNOWN = 0, SVL_GONE_CLOSED = 1, SVL_GONE_PARENT = 2, SVL_GONE_OWNER = 3,
                        SVL_GONE_REPLACED = 4, SVL_GONE_SHUTDOWN = 5 } svl_gone;
typedef struct svl_handle_info { svl_id kind; uint32_t lifetime; uint32_t flags; } svl_handle_info;
int  (*handle_info)(void *self, svl_handle h, svl_handle_info *out);   /* backend, new */
void (*on_destroyed)(void *self, svl_handle h, uint32_t reason);       /* host, changed: adds reason */
```

### handle_info

The host calls it once for a handle it has not seen, minted when `get_prop` or `call` returns an `SVL_HANDLE` for a native object the backend did not create. It returns the handle's kind and lifetime class. Returns `-1` with `last_error` if the handle is stale. The proposal defines the flag `SVL_HI_INSERTED = 1`.

### Gone reasons

| Reason | Meaning |
| --- | --- |
| `SVL_GONE_UNKNOWN` (0) | unspecified |
| `SVL_GONE_CLOSED` (1) | toplevel closed natively (uncontrolled close) |
| `SVL_GONE_PARENT` (2) | an ancestor died (cascade) |
| `SVL_GONE_OWNER` (3) | the native owner deleted or replaced a borrowed object |
| `SVL_GONE_REPLACED` (4) | a slot operation deleted it (`effects ... deleted`) |
| `SVL_GONE_SHUTDOWN` (5) | app shutdown |

### Rules

- Handles are never reused and stay below 2^53. The host keeps a bounded dead-handles ring (about 4k entries in the research) so errors can say why a handle died.
- `destroy` is idempotent. The handle is stale as soon as it returns.
- `on_destroyed` fires at most once per handle, after the backend dropped it. Death cascades to every descendant. The host only records deaths inside the callback and tells JS later through the event queue (`__svelters_destroyed(h, reason)` at checkpoint step 1).
- Borrowed handles are minted when a getter or call returns a native object we did not create.
- Native code never holds a JS value.
- Window close follows `onRequestClose`: if JS listens for the close signal the backend vetoes the native close and posts the event; otherwise it destroys natively and reports `SVL_GONE_CLOSED`. The host, not the toolkit, decides when the app quits (Qt: `setQuitOnLastWindowClosed(false)`).
- Backends never delete synchronously inside signal emission: Qt uses `deleteLater`, GTK an idle-deferred unref.
- GC releases go through a host queue drained at checkpoints and become no-ops after shutdown starts; the host drops the JS runtime before the backend.

Later additions, not part of v2: `set_child_attr` (M4), `listen` flags, `request_frame`, the foreign-runtime hooks, and the v3 value model.

## ABI v3: value model (planned, M3)

From `docs/notes/js-native-serdes.md` section 5.1, which has the exact C proposal. Not implemented.

- Keep the nested, borrowed `svl_value` and extend it in place. It stays 24 bytes, with the tag word split into tag, flags and type id.
- New tags: `ENUM`, `FLAGS`, `RECORD`, `BYTES`, `BOXED`, `CALLBACK`, `MAP`, `ERROR`. Flags mark UTF-16 versus UTF-8 strings and NUL termination.
- Backends publish generated type descriptors (`describe`, `describe_type`), and the host gains `invoke_callback` and `release_callback`.
- No BigInt and no binary op buffer at this ABI. A packed buffer belongs only in future N-API or Deno-FFI fronts.

Things done now without an ABI change: JS passes cached IDs rather than names (1430 to 770 ns per `setProp`), strings are borrowed, descriptor-driven prop coercion replaces the renderer's hand-written `INT_PROPS`, and enums go out as names on both toolkits. The S1 milestone (ABI v3 value model and converters) is IN PROGRESS and merges after P5.

## ABI v2.3: styling (X6)

One optional member, `int (*style)(void *self, uint32_t op, svl_handle h, const svl_value *v)`, appended after `listen_ex`. `v` is a string or null. The ops are `TARGET` (which dialect the backend renders: CSS or QSS), `CLASSES`, `NAME`, `INLINE`, `OPACITY`, `SHEET` (install, replace, remove or clear a component sheet, `@tokens` being the token layer) and `SCHEME` (colour scheme plus palette pairs). The renderer asks `TARGET` first and then renders only that dialect. Payload formats and the per-toolkit behaviour are in `plans/architecture/backend-abi.md#abi-v23-additive-x6-styling`; the user-facing side is the [styling guide](/guides/styling/).

## ABI v2.5: shared handles, construct and call_static (NT0)

Additive members for non-UI types (`Gio.File`, `Gtk.TextBuffer`, `Pango.FontDescription`), all optional on a backend:

- **`SVL_LIFE_SHARED = 5`:** a strong reference to a refcounted object outside the widget tree, one handle per object. `destroy` drops only our reference; the host may call it when the JS wrapper is garbage collected. Every GObject now crosses as a handle (widgets stay `owned` / `borrowed`, everything else is `shared`); `BOXED` is for boxed structs only, and those `value` handles have a kind and take read-only calls.
- **`construct(kind, props, values, n, out)`:** an instance with construct-time properties (GTK: `g_object_new_with_properties`, so construct-only properties work). `create(kind)` is `construct` with no properties.
- **`call_static(kind, method, args, n, out)`:** static functions and constructor functions. Member flags `SVL_M_STATIC` and `SVL_M_CTOR` mark them in the descriptor.
- **`SVL_T_TYPE = 11`:** a string slot that carries a native type name, for GType-valued properties and parameters.
- **`describe(SVL_KIND, 0, kind)`:** the qualified name of a kind and its nearest bound ancestor, for `host.typeChain(h)`.

The JS side is the generated `svelters:gtk/<namespace>` modules, one named export per class; see [Non-UI types](/types/gtk/). The full text, including the op-log lines `construct` and `static`, is in `plans/architecture/backend-abi.md`.

Sources: `plans/architecture/backend-abi.md`, `crates/svelters-abi/include/svelters_backend.h`, `docs/notes/ownership-and-lifetimes.md`, `docs/notes/p2-host-lane.md`, `docs/notes/p5-integration.md`, `docs/notes/js-native-serdes.md`.
