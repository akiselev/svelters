---
title: Backend ABI
description: Reference for the fixed C ABI between the Rust host core and every toolkit backend, including values, lifetimes, checkpoints, observation and the op log.
---

The backend ABI is the L2/L3 boundary. A Rust host core drives every backend through one vtable, whatever the backend's language (Rust for GTK, AppKit, WinUI and egui, C++ for Qt). The header `crates/svelters-abi/include/svelters_backend.h` and its Rust mirror `crates/svelters-abi/src/lib.rs` are authoritative for exact spelling; `plans/architecture/backend-abi.md` holds the full semantics.

Goals:

- One fixed C ABI. It never grows per native method, property or widget kind.
- Names resolve to `u32` IDs once. Per-ID behaviour lives in backend code, generated from the metadata.
- JS never sees native pointers. The host keeps a handle table; backends see opaque `svl_handle` values that they map to their own objects.

:::note[Version]
The header is **ABI version 3** (`SVL_ABI_VERSION 3u`): the v3 value model on top of v2 (toolkit timers, handle lifetimes). Later additions are appended at the end of the structs and marked by minor version: 2.1 `request_frame` / `frame`, 2.2 `set_child_attr` / `get_child_attr` / `listen_ex`, 2.3 `style`, 2.5 `construct` / `call_static`, 2.6 `observe` / `prop_changed`, 2.7 `start`. Every addition is nullable or has a default, and all backends are in-tree and rebuilt together.
:::

## Data model

```c
typedef uint64_t svl_handle; /* 0 = null; issued by the backend. Objects and boxed values share the space. */
typedef uint32_t svl_id;     /* resolved kind/prop/signal/method/slot/attr/type id; 0 = unknown */

typedef struct svl_value {
  uint16_t tag;   /* svl_tag */
  uint16_t flags; /* SVL_F_UTF16, SVL_F_NUL_TERM, SVL_F_UNSIGNED */
  svl_id type;    /* type id for ENUM/FLAGS/RECORD/BOXED/CALLBACK, or an expected kind on HANDLE */
  union { bool b; int64_t i; uint64_t u; double f; svl_handle h; svl_str s; svl_list l; svl_bytes bytes; } u;
} svl_value; /* 24 bytes on LP64 */

typedef enum svl_name_kind { SVL_KIND = 0, SVL_PROP = 1, SVL_SIGNAL = 2, SVL_METHOD = 3, SVL_SLOT = 4, SVL_ATTR = 5 } svl_name_kind;
```

### `svl_value` and `svl_tag`

| Tag | Union member | JS side |
| --- | --- | --- |
| `SVL_NULL` (0) | none | `null` |
| `SVL_BOOL` (1) | `u.b` | boolean |
| `SVL_I64` (2) | `u.i` (`u.u` with `SVL_F_UNSIGNED`) | integer or integral float |
| `SVL_F64` (3) | `u.f` | other numbers |
| `SVL_STR` (4) | `u.s` | string (UTF-8, or UTF-16 with `SVL_F_UTF16`) |
| `SVL_HANDLE` (5) | `u.h` | element node or native object wrapper |
| `SVL_LIST` (6) | `u.l` | array (depth limit 16 in the host) |
| `SVL_ENUM` (7) | `u.i`, `type` | the member name |
| `SVL_FLAGS` (8) | `u.u`, `type` | an array of member names |
| `SVL_RECORD` (9) | `u.l` (fields in descriptor order), `type` | plain object (`Rect`, `Color`, `Date`, ...) |
| `SVL_BYTES` (10) | `u.bytes` | `Uint8Array` |
| `SVL_BOXED` (11) | `u.h`, `type` | a value-class wrapper |
| `SVL_CALLBACK` (12) | `u.u` (host callback id), `type` (signature) | a JS function |
| `SVL_MAP` (13) | `u.l` (alternating key, value) | plain object (variants and typed sync results) |
| `SVL_ERROR` (14) | `u.l` = `[domain, code, message]` | `NativeError` (out only) |

- Strings, lists and bytes are borrowed for the duration of a call. A value a backend writes to an `out` slot stays valid until the backend's next call; the host copies it first.
- Well-known record types (`SVL_T_RECT` ... `SVL_T_TYPE`) have fixed ids below `0x10000`; toolkit types get backend-assigned ids above it. Backends describe members and types through `describe` / `describe_type`; the host converts by those descriptors (enums go out as names, records as objects).
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
  int (*handle_info)(void *self, svl_handle h, svl_handle_info *out);
  int (*run)(void *self, const svl_host *host); /* owns the toolkit loop; returns on quit */
  void (*wake)(void *self);                     /* thread-safe: schedule a host checkpoint */
  void (*schedule_wake)(void *self, int64_t ms);
  void (*quit)(void *self);
  const char *(*last_error)(void *self);        /* NUL-terminated, valid until the next call */
  /* v3 */
  uint32_t caps;
  const svl_member_desc *(*describe)(void *self, uint32_t what, svl_id kind, svl_id member);
  const svl_type_desc *(*describe_type)(void *self, svl_id type);
  int (*cancel)(void *self, uint64_t callback);
  /* 2.1 */ void (*request_frame)(void *self);
  /* 2.2 */ int (*set_child_attr)(void *self, svl_handle parent, svl_handle child, svl_id attr, const svl_value *v);
  /* 2.2 */ int (*get_child_attr)(void *self, svl_handle parent, svl_handle child, svl_id attr, svl_value *out);
  /* 2.2 */ int (*listen_ex)(void *self, svl_handle h, svl_id signal, bool on, uint32_t flags);
  /* 2.3 */ int (*style)(void *self, uint32_t op, svl_handle h, const svl_value *v);
  /* 2.5 */ int (*construct)(void *self, svl_id kind, const svl_id *props, const svl_value *values, size_t n, svl_value *out);
  /* 2.5 */ int (*call_static)(void *self, svl_id kind, svl_id method, const svl_value *args, size_t n, svl_value *out);
  /* 2.6 */ int (*observe)(void *self, svl_handle h, svl_id prop, bool on);
  /* 2.7 */ int (*start)(void *self, const svl_host *host);
} svl_backend;
```

<a id="resolve"></a>
### resolve

`svl_id resolve(self, what, kind, name, len)` maps a name to an ID. `what` is an `svl_name_kind`. `kind` scopes `SVL_PROP`, `SVL_SIGNAL`, `SVL_METHOD` and `SVL_SLOT` lookups to a kind ID, and `SVL_ATTR` lookups to the parent's kind (pass `0` for `SVL_KIND`). Returns `0` for an unknown name. The host caches name to ID resolution per kind, so each name is resolved once.

<a id="create"></a>
### create

`svl_handle create(self, kind)` makes a native object of the resolved kind and returns a new handle, or `0` on error. Compiled components create each widget where the element is declared, insert it once its children are in place and present a toplevel after its subtree is complete. `create` mints `owned` or `toplevel` handles (GTK widgets are `ref_sink`ed). `create(kind)` is `construct(kind)` with no properties.

<a id="insert"></a>
### insert

`int insert(self, parent, child, before, slot)` places `child` under `parent`. `before` is the handle of the next native sibling in the same slot, `0` meaning append. `slot` is a resolved `SVL_SLOT` ID, `0` meaning the kind's default slot. Inserting an already-parented child moves it. Fails with a message for an unknown slot or a wrong child kind. A toplevel (window, dialog) inserted anywhere becomes transient for the enclosing toplevel instead of a child (X7).

<a id="remove"></a>
### remove

`int remove(self, parent, child)` detaches `child` and leaves it alive and unparented, so it can be inserted again. Backends must leave it either unparented or hidden (`kept_child_hidden`); a native removal that deletes the child is only allowed on slots annotated `on_remove: destroys_child`.

<a id="destroy"></a>
### destroy

`void destroy(self, h)` is the host dropping its last reference. It is idempotent, and the handle is stale as soon as it returns. What happens natively depends on the lifetime class (see [Ownership](/concepts/ownership/#lifetime-classes)): owned deletes the native object, toplevel closes and destroys, borrowed only forgets, value and shared drop only our reference. It never calls `on_destroyed` for `h` itself, and it may cascade `on_destroyed(d, PARENT)` to descendants. Backends never delete synchronously inside a signal emission (Qt `deleteLater`, GTK an idle-deferred unref).

<a id="set_prop"></a>
### set_prop

`int set_prop(self, h, prop, v)` writes a property. `prop` is an `SVL_PROP` ID scoped by the handle's kind. The host converts JS values by the property's descriptor. The backend returns an error for a wrong value type; a property of another kind is an error naming the member and the handle's runtime class. Programmatic sets of text on entries do not emit change events to JS (see [Bindings](/guides/bindings/)).

<a id="get_prop"></a>
### get_prop

`int get_prop(self, h, prop, out)` reads a property into the host-provided `out` slot. An object the backend did not create comes back as a new borrowed handle with a stable identity (the same object gives the same handle while it lives).

<a id="call"></a>
### call

`int call(self, h, method, args, n, out)` invokes a method with `n` arguments and writes any result to `out`. Methods come from the generated tables, and on GTK also from GObject introspection. Out and in-out parameters come back as `[ret, out...]`, async methods take a `CALLBACK` and complete through `invoke_callback` (`cancel` aborts them), and errors are `SVL_ERROR` values. Blocking methods that spin a nested toolkit loop (`QDialog::exec`, the deprecated `gtk_dialog_run`) are not exposed; metadata marks them `unsupported_reason: blocking_nested_loop`.

<a id="listen"></a>
### listen and listen_ex

`int listen(self, h, signal, on)` connects (`on` true) or disconnects (`on` false) a native signal. The backend connects natively and reports through `host->post_event`, or through `host->call_sync` for signals whose handler returns a value or can veto. Only signals with a JS listener are connected.

`listen_ex(self, h, signal, on, flags)` (2.2) adds the phase flags `SVL_LISTEN_AFTER` (run behind every plain listener) and `SVL_LISTEN_CAPTURE` (ancestors first, where the toolkit has a capture phase). Events of such a connection carry `signal | flags << SVL_SIGNAL_FLAGS_SHIFT`. See [Events](/guides/events/).

<a id="run"></a>
### run and start

`int run(self, host)` hands the toolkit loop to the backend. The backend owns the loop and returns when the app quits; it checkpoints once when the loop starts, then after every `wake`.

`int start(self, host)` (2.7, nullable) attaches the host and brings the toolkit up without running the loop, then returns at once. The caller (an embedder, a test harness, a platform that owns `main`) drives the loop, and the backend calls the host exactly as it would from `run`. `start(self, NULL)` detaches. The host also uses `start` to attach before mounting, so callbacks made while a component mounts reach it.

### wake, quit, last_error

- `void wake(self)`: thread-safe. Schedules a host checkpoint on the loop thread. Wakes coalesce (one pending at a time) and are delivered at the toolkit's default priority, so a busy timer or input cannot starve them (see [Checkpoint semantics](#checkpoint-semantics)).
- `void quit(self)`: asks the backend to leave `run`.
- `const char *last_error(self)`: NUL-terminated, valid until the next call.

<a id="schedule_wake"></a>
### schedule_wake

JS timers run on the toolkit's native timing. `schedule_wake(self, ms)` (re)arms a single one-shot wakeup `ms` from now, replacing any pending one; `ms < 0` cancels. On expiry the backend calls `host->checkpoint`. Main thread only.

- The host keeps its timer heap and, after every checkpoint, arms only the earliest deadline (whole milliseconds, rounded up). The host starts no threads.
- GTK: `glib::timeout_add_local_once`. Qt: one single-shot `QTimer`, `Qt::PreciseTimer` under 20 ms and `Qt::CoarseTimer` otherwise. Scripted fixtures keep the host's own virtual clock.

<a id="request_frame"></a>
### request_frame

`request_frame(self)` (2.1) asks for one `host->frame` on the toolkit's next frame: GTK's frame clock, Qt `QWindow::requestUpdate`, AppKit `CADisplayLink`, WinUI `CompositionTarget.Rendering`. The host calls it for `requestAnimationFrame`. A backend without a window to tick on falls back to a 16 ms timer.

### handle_info

`int handle_info(self, h, out)` answers the kind and lifetime class of a handle the host has not seen, minted when `get_prop` or `call` returns an object the backend did not create. Returns `-1` with `last_error` if the handle is stale.

### Child attributes and styling

- `set_child_attr` / `get_child_attr` (2.2): a per-child fact owned by the parent (grid cell, tab label, dock area), valid while `child` is inserted into `parent`; `SVL_NULL` resets it. Written as `parent:<name>={v}` in a component.
- `style(self, op, h, v)` (2.3): one member, many ops (`TARGET`, `CLASSES`, `NAME`, `INLINE`, `OPACITY`, `SHEET`, `SCHEME`). See the [styling guide](/guides/styling/).

## Host callbacks

```c
typedef struct svl_host {
  void *self;
  void (*post_event)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n);
  void (*checkpoint)(void *self);
  int (*call_sync)(void *self, svl_handle h, svl_id signal, const svl_value *args, size_t n, svl_value *out);
  void (*on_destroyed)(void *self, svl_handle h, uint32_t reason /* svl_gone */);
  int (*invoke_callback)(void *self, uint64_t callback, const svl_value *args, size_t n, svl_value *out); /* v3 */
  void (*release_callback)(void *self, uint64_t callback);                                                 /* v3 */
  void (*frame)(void *self);                                                                               /* 2.1 */
  void (*prop_changed)(void *self, svl_handle h, svl_id prop);                                             /* 2.6 */
} svl_host;
```

<a id="post_event"></a>
### post_event

`post_event(self, h, signal, args, n)` queues a native event. It is delivered to JS at the next checkpoint as `__svelters_dispatch(h, signal, args)`, and the host wakes the backend so the loop runs one.

### checkpoint

`checkpoint(self)` delivers changed properties and queued events, runs JS jobs and applies the resulting operations. A checkpoint requested while JS is running (a backend op that spins a nested loop) is deferred and becomes a later `wake`.

<a id="call_sync"></a>
### call_sync

`int call_sync(self, h, signal, args, n, out)` is the synchronous handler call for signals and virtuals that return a value or can be vetoed (`close-request`, key handling, `GtkSpinButton::input`). The backend calls it from inside the toolkit's signal emission. Return codes: `SVL_SYNC_OK` (0, `out` holds the result: `null` = no decision, a bool = stop/handled, or a typed result), `SVL_SYNC_FAILED` (-1, use the toolkit default) and `SVL_SYNC_BUSY` (-2: JS was already running, so the host queued the event for ordinary delivery). The result comes from the event object (`e.result`, `e.preventDefault()`), never from the handler's return value. JS may issue host ops inside a sync handler and they apply at once; a nested sync signal is deferred; a returned Promise is ignored. See [Events](/guides/events/).

<a id="on_destroyed"></a>
### on_destroyed

`on_destroyed(self, h, reason)` reports that a native object went away underneath the backend (a window closed, a parent was deleted). It is called at most once per handle, after the backend dropped it, and the death cascades to every descendant handle (reason `PARENT`). The host only records deaths inside the callback and tells JS later (`__svelters_destroyed(h, reason)` at the next checkpoint); the handle's number stays in a bounded dead-handles ring so errors can say why it died.

| Reason | Meaning |
| --- | --- |
| `SVL_GONE_UNKNOWN` (0) | unspecified |
| `SVL_GONE_CLOSED` (1) | toplevel closed natively (uncontrolled close) |
| `SVL_GONE_PARENT` (2) | an ancestor died (cascade) |
| `SVL_GONE_OWNER` (3) | the native owner deleted or replaced a borrowed object |
| `SVL_GONE_REPLACED` (4) | a slot operation deleted it |
| `SVL_GONE_SHUTDOWN` (5) | app shutdown |

### invoke_callback, release_callback

`invoke_callback(self, callback, args, n, out)` (v3) runs the JS function behind a `CALLBACK` value: async completions, `CustomFilter` / `CustomSorter` functions, Qt functors. A call after the callback was released is a clean error (`-1`, with an `SVL_ERROR` in `out`), never a crash. `release_callback` reports that the native side dropped it (destroy notify, end of the call's scope, async completion); it is idempotent.

### frame

`frame(self)` (2.1): a frame requested with `request_frame` is due; the host runs the `requestAnimationFrame` callbacks.

## Lifetime classes

```c
typedef enum svl_lifetime { SVL_LIFE_OWNED = 1, SVL_LIFE_TOPLEVEL = 2, SVL_LIFE_BORROWED = 3, SVL_LIFE_VALUE = 4,
                            SVL_LIFE_SHARED = 5 /* 2.5 */ } svl_lifetime;
typedef struct svl_handle_info { svl_id kind; uint32_t lifetime; uint32_t flags; } svl_handle_info;
```

<a id="abi-v2-planned"></a>
The lifetime model came with ABI v2 and is described in [Ownership](/concepts/ownership/):

- Handles are never reused and stay below 2^53.
- Borrowed handles are minted when a getter or call returns a native object we did not create; they never keep it alive or enter the tree.
- Shared handles (2.5) are strong references to refcounted objects outside the widget tree (`Gio.File`, `Gtk.TextBuffer`, `QTimer`), one handle per object; the host may `destroy` one when its JS wrapper is garbage collected.
- Native code never holds a JS value: events are (handle, signal, args), and JS-implemented native objects hold an integer ID into a host table.
- Window close follows `onRequestClose`: if JS listens for the close signal the backend vetoes the native close and asks JS (`call_sync`); otherwise it destroys natively and reports `SVL_GONE_CLOSED`. The host, not the toolkit, decides when the app quits.

## Non-UI types: construct and call_static (2.5)

- **`construct(kind, props, values, n, out)`:** an instance with construct-time properties (GTK: `g_object_new_with_properties`, so construct-only properties work). The new handle is written to `out`.
- **`call_static(kind, method, args, n, out)`:** static functions and constructor functions. Member flags `SVL_M_STATIC` and `SVL_M_CTOR` mark them in the descriptor.
- **`SVL_T_TYPE` (11):** a string slot that carries a native type name, for type-valued properties and parameters.
- **`describe(SVL_KIND, 0, kind)`:** the qualified name of a kind and its nearest bound ancestor, for `host.typeChain(h)`.

The JS side is the generated `svelters:<toolkit>/<namespace>` modules, one named export per class; see [Non-UI types](/types/gtk/).

## Property observation (2.6)

`observe` and `prop_changed` let the host learn that a native property changed without a signal of its own. The compiler lowers `bind:` on an observable property onto them, and the generated getters of non-UI types read through them, so a native change re-runs the effects that read the property.

- **`observe(h, prop, on)`** (backend): start or stop watching one property natively. Calls nest per (handle, prop); `destroy` and the native death of the handle end every observation; an observation never keeps the object alive. A property with no change notification on the backend answers `SVL_OBSERVE_UNSUPPORTED` (-2).
- **`prop_changed(h, prop)`** (host): "this property may have changed". It carries no value. UI thread only; a backend hops there first.
- **`SVL_M_OBSERVABLE`** (64) on a property descriptor: `observe` can watch it. **`SVL_M_PROPORTIONAL`** (128): reading costs O(size) (whole text), so the host delivers an invalidation without reading and the backend drops the echo of our own `set_prop`.

| Backend | Mechanism |
|---|---|
| GTK | `notify::<prop>` on any GObject handle, and on the object a forwarded property lives on (`GtkRange` `value` on its adjustment) |
| Qt | NOTIFY signals, `QBindable` notifiers, a per-widget event filter, signal pairs, and a metaobject fallback; refused without any of these |
| AppKit | key-value observing, with notification and target-action fallbacks |
| WinUI | `RegisterPropertyChangedCallback` on dependency properties, with event fallbacks |
| egui | the retained scene's own values (`text`, `checked`, `value`) |

The host refcounts observations per (handle, prop) and calls the backend only on 0→1 and 1→0. Announced pairs go into an ordered, deduplicated dirty set; at the checkpoint each still-observed pair is read once, compared with the last value delivered (or read back after our own `set_prop`, so the echo of a write is dropped), and the changes reach JS as one `globalThis.__svelters_changed([h, prop, value, ...])` batch before the queued events. Which properties are observable, and how completely, comes from the observe facts (`exact`, `partial`, `set_only`, `none`); see [Reactivity](/concepts/reactivity/).

## Checkpoint semantics

1. Deliver the batch of changed observed properties (`__svelters_changed`), then the queued native events.
2. Run Svelte effects and QuickJS jobs, up to a bounded number (`max_jobs_per_checkpoint`).
3. Apply the resulting tree and property operations through the vtable.
4. If jobs remain, call `backend->wake()` and return to the toolkit loop.

Operations are applied as they are issued, not batched: P5 measured 11,000 operations costing 16 to 18 ms of host and ABI time.

**Wake delivery.** A `wake` must not be starvable by the toolkit's ordinary sources, and wakes coalesce (one pending at a time). GTK uses a `G_PRIORITY_DEFAULT` idle with a redraw guard (after 4 wakes in a row, the next one waits just below the redraw priority, with a 4 ms backup); Qt a queued call (a posted event); AppKit a version-0 `CFRunLoopSource`; WinUI `DispatcherQueue.TryEnqueue` at `Normal`; egui a flag read at the start of the next pass.

Scripted mode (`HostConfig::scripted`) steps differently for fixtures: an `event` step dispatches at once with no job drain, a `checkpoint` step drains jobs and fires the earliest timer on a virtual clock until both are empty, and the run ends with a final job-only drain.

## JS-side host ops

The runtime (`@svelters/native-runtime`) calls these and the host forwards them to the vtable:

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
  construct(kind: string, props?: Record<string, unknown>): Handle;   // 2.5
  callStatic(kind: string, method: string, args: unknown[]): unknown; // 2.5
  typeChain(h: Handle): string[];                                     // 2.5
  observe(h: Handle, prop: string, on: boolean): boolean;             // 2.6: observed afterwards?
}
// events: globalThis.__svelters_dispatch(h, signal, args) at checkpoints
// changes: globalThis.__svelters_changed([h, prop, value, ...]) at checkpoints, before events
```

The Node `TestHost` implements this interface in JS and records an op log; the QuickJS host implements it natively.

## Op-log format

The Node `TestHost` and the Rust log backend must produce identical logs for the same fixture and scripted interaction. UTF-8 text, one op per line. Handles are numbered `#1, #2, ...` in creation order per run. Values are compact JSON.

```text
create <kind> #<n>
insert #<parent> #<child> before=#<h>|end slot=<name>|default
remove #<parent> #<child>
destroy #<h>
set #<h> <prop> <json>
call #<h> <method> <json-args-array>
listen #<h> <signal> on|off [after|capture]
attr #<parent> #<child> <name> <json>             # 2.2 child attribute
style #<h>|sheet|scheme ...                        # 2.3 styling
event #<h> <signal> <json-args-array>              # an injected native event, logged when delivered
sync #<h> <signal> <json-args-array>               # an injected sync signal, logged before its handlers run
result #<h> <json>|busy|error                      # what call_sync returned
gone #<h> <reason>                                 # a native death (lower-case svl_gone name)
construct #<h> <Kind> <json-props-object>          # 2.5
static <Kind>.<method> <json-args-array> [#<h>]    # 2.5
observe #<h> <prop> on|off                         # 2.6, transitions only
changed #<h> <prop> [<json>]                       # 2.6, a scripted native change
```

Kinds, props, signals and slots appear by JS-facing names, never by IDs. A scripted interaction is a JSON array of `{"event": [...]}`, `{"sync": [...]}`, `{"gone": [...]}`, `{"changed": [h, prop, value?]}` and `{"checkpoint": true}` steps. Fixtures store the expected log in `expected.oplog`. The GTK and Qt test suites replay these logs against the real toolkit: `event` and `changed` lines are produced natively and must reach the host (a `changed` line is announced exactly when the property is observed).

## Planned extension: foreign runtimes

From `docs/notes/node-deno-integration.md`. Optional members so stock Node (N-API) or Deno (FFI) can drive a backend while the toolkit loop stays authoritative: host callbacks `foreign_fd()` and `foreign_timeout_ms()` watched by the backend's loop, backend `poll_prepare`/`poll_dispatch` for runtimes that insist on owning the main thread, and a rule that nested toolkit loops may re-enter `checkpoint`. Not implemented.

Sources: `plans/architecture/backend-abi.md`, `crates/svelters-abi/include/svelters_backend.h`, `docs/notes/ownership-and-lifetimes.md`, `docs/notes/js-native-serdes.md`, `plans/research/REACTIVITY.md`.
