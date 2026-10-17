# Backend ABI (L2 ↔ L3)

Status: draft for the P lanes. Changes go here first, then into `crates/svelters-abi/include/svelters_backend.h`.

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
  void (*on_destroyed)(void *self, svl_handle h);
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
- **Slot routing.** How does a slot get chosen: attribute, wrapper element kind, or snippet? This determines whether `insert`'s `slot` argument is enough or per-child attributes need their own op.
- **Ownership.** What happens when the native side destroys a child the host still references? `on_destroyed` covers notification; the annotation pipeline covers which operations transfer ownership.
- **Batching.** Should mutations be batched per checkpoint or applied one at a time? Decide by measurement after P5.
