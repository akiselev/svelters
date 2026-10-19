# NT1: GTK async members and GI closures (phase N1 of NON-GUI-TYPES)

Status: done on branch `worktree-agent-a13573a6ffaa278fe` (pushed as `ci/nt1`), not merged. Plan: `plans/research/NON-GUI-TYPES.md` §9, row N1. The section "N1 as built" there records where the build deviates from the plan.

## Goal

- GI closures by scope in the GTK dynamic invoker.
- `_async`/`_finish` pairs folded into Promise-returning members.
- The generated surface (modules, typings, manifest, docs, ledger facts) carries them.
- `gio_services.js` passes on the generic path, and the hand-written `gio-file` service is retired.

## Design decisions

### Scope handling (`crates/svelters-gtk/src/closures.rs`, `gi.rs`)

- **What the plan reads.** A callback parameter's GIR `scope`, `closure` (user data) and `destroy` (GDestroyNotify) annotations come from the typelib (`gi_arg_info_get_scope`, `_get_closure_index`, `_get_destroy_index`). The user-data and destroy arguments are hidden from JS. Older typelibs put `closure` on the user data instead; both forms are handled.
- **The native closure.** It is a libffi closure from `gi_callable_info_create_closure`. A `NativeClosure` owns:
  - its `ffi_cif` storage (32 × u64, larger than any platform's `ffi_cif`);
  - a ref on the `GICallbackInfo`;
  - the host callback id and owner;
  - a `Weak` to the backend;
  - the creating thread.

  The closure itself is the user data, so the destroy notify receives it.
- **When each scope ends:**

  | GIR scope | Native closure freed | Host callback released |
  |---|---|---|
  | `call` | when the native call returns (`Keep::drop`) | by the host after the call (unchanged S1 rule) |
  | `async` (generic) | from an idle after the first invocation | by the host after the invocation |
  | `notified` | from an idle after the callee's `GDestroyNotify` | by the closure's drop (`release_callback`) |
  | `forever` | never (as GJS does) | when the owner handle dies. It is described to the host as `notified`, because the host refuses `forever`. No member in the bound set has it. |

  A closure the callee never saw (a later argument failed to convert) is freed at once.
- **No use-after-free** (the 2021 failure mode):
  - frees are always deferred to an idle, never done inside the trampoline;
  - a destroy notify that arrives while the closure runs only marks it (`doomed`);
  - the trampoline checks the thread before it touches the backend;
  - it holds the backend weakly;
  - it returns the zero value when the backend is gone or the JS function was released.

  A callback can outlive its owner, for example a filter kept inside a list model after its JS wrapper was disposed. GTK then gets `false`/`0`/`NULL` back, not a crash.
- **Refused:**
  - callback signatures with untyped pointers other than the trailing user data;
  - out arguments, `GError`, arrays with a length;
  - list returns and nested callbacks;
  - an unpaired `GDestroyNotify`;
  - a `call`-scope callback of an async function. GIR's `call` on such callbacks is wrong: they run after the call returns.

### Finish-func folding

- **Pairing.** `<stem>_async` (or `<stem>` with a `GAsyncReadyCallback`) pairs with `<stem>_finish` in the same container. Pairing is by name because `gi_callable_info_get_finish_function` exists only from GLib 2.84, and the Ubuntu 4.14 CI job has 2.80. `types_facts.py` applies the same rule and blocks the 3 pairs whose GIR `glib:finish-func` disagrees.
- **What the JS member hides.** The `GCancellable`, the `GAsyncReadyCallback` and its user data are hidden. The JS member keeps the start's other parameters and adds a trailing `{ signal }` options object.
- **What it resolves with.**
  - The finish return value, or `{ret?, <out>...}` when the finish function has out parameters. This is the S2 out-parameter shape; array lengths are hidden.
  - The backend sends native out names, and the generated module renames them to camelCase (`etag_out` → `etagOut`).
- **Async statics and constructors** (`File.newTmpAsync`) go through `callStatic`. The host needed a small change for them.

### Cancellation

- **Decision: hide `GCancellable` and expose `AbortSignal`** (`{ signal }`). Each async call gets its own `GCancellable`. The backend keeps it in `pending` by completion id, and `backend.cancel(id)` (the host's abort wiring from S1) cancels it.
- GIO then completes with `G_IO_ERROR_CANCELLED`, and the Promise rejects with `code: 'cancelled'`. A signal that is already aborted cancels right after the start.
- Synchronous members keep their `cancellable` parameter (NT0 shape), because a JS caller cannot cancel a blocking call anyway.

### Error mapping

- A `GError` crosses as `ERROR`: `domain` is the quark string.
- `code` is the member of the domain's registered error enum, found through `gi_repository_find_by_error_domain`. This generalises S1's GIO-only rule: `GtkDialogError` codes, for example, now get names too (tested). A code with no enum member stays a number.
- The same mapping is used for synchronous calls.

### Completion delivery

- GIO calls the `GAsyncReadyCallback` on the main context of the thread that started the operation, never synchronously. `async_ready` then:
  1. checks that the operation is still pending (its owner may have been destroyed);
  2. runs the finish function;
  3. folds the result;
  4. calls `host.invoke_callback(done, [value])`.
- The host's completion function settles the Promise. Reactions run at the next checkpoint, because `invoke_callback` wakes the backend. Nothing blocks and nothing polls.
- A completion that arrives off the main thread is dropped with a message. It is not delivered.

### Lifetime of an operation

- **JS side** (`packages/types/src/core.js` `callAsync`): the wrapper an async call was made on is kept reachable until the Promise settles. `await File.newForPath(p).loadContentsAsync()` is therefore safe.
- **Owner destroyed.** If the owner is destroyed explicitly (`dispose()`, or the end of its component scope), the backend cancels its operations. The host settles the orphaned completion with `NativeError { domain: 'svelters', code: 'disposed' }`. The host used to drop the completion, which left the Promise pending forever.

### Widget slots

`FileDialog.open(parent)` needs a widget argument, and widgets are elements, not generated classes. The facts now bind widget-typed parameters and returns as `k: widget`: an element handle in, the renderer node out. This unblocked 72 members (async and sync), which NT0 had blocked as `unbound:Gtk.Window` and similar.

## ABI

No ABI change. Host changes, none of them ABI (`crates/svelters-host`):

- `invoke_callback` and event delivery adopt new handles found in arguments, including `MAP`/`RECORD` contents. Callback and signal arguments that carry a fresh object (`GFileMonitor::changed`'s file, a filter's item) used to be "stale or foreign" in JS. This was a pre-existing gap for non-widget signal arguments.
- Completions of `SVL_M_ASYNC` calls are marked. When their owner dies they are detached and rejected at the next checkpoint (`cb_settle_orphans`) instead of being dropped.
- `callStatic` of an `SVL_M_ASYNC | SVL_M_STATIC` member returns a Promise (`op_call_async(..., is_static)`).

## Generated surface

- **Facts** (`metadata/gtk/types/*.json`): async members carry `async: {finish, outs}`, and callbacks carry `{k: callback, t, scope, sig}`. They were regenerated on the installed GIR (GTK 4.24.1). NT0 had used 4.22; the GIR drift is listed separately below.
- **Modules** (`packages/types/gtk/<ns>/<Class>.js`): `callAsync`/`callStaticAsync` (new in `core.js`) with a result-shaping function. Callback arguments are wrapped so that object arguments arrive as generated classes. A trailing `ioPriority = 0`.
- **Typings:** `Promise<T>`, `options?: AsyncOptions`, callback function types, and `NativeElement` for widget slots (`core.d.ts`).
- **Manifest:** async outs and callback signatures count as signature references.
- **Docs:** async sections on the type pages, plus a guide page `types/gtk/async.md` under the auto-generated "Non-UI types (GTK)" sidebar group (`astro.config.mjs` untouched). The docs IR still carries widget kinds only (an NT0 design); the type pages are generated straight from the facts.
- **Ledger:**
  - an `async` fact group for the 4 curated types with async members (Gio.File, Gdk.Clipboard, Gdk.Texture, Gtk.FileDialog): 50 → 54 type facts;
  - generated `types_verify::*_async` tests check the descriptors, one completion per call, and a completion after a cancel, with the 4.14 `since` guard;
  - a new generated `types_members.rs` checks that every bound async member and every bound callback member resolves in the backend with matching async/static flags and argument count.

## Completion record

Filled in at the end of the lane; see below.
