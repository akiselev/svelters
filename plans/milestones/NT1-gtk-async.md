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

### Commits (branch `worktree-agent-a13573a6ffaa278fe`, pushed as `ci/nt1`)

- `a4e89004`: closures, folding, host adoption/orphans/async statics, facts and generators. `gio_services` passes on the generic path.
- `e01206d7`: async verify group, `types_members` cross-check, `async_callbacks` lifetime test, docs, NON-GUI-TYPES and backend-abi notes.
- `441fd8e7`: orphaned completions refuse backend invocations; GTK ledger refreshed.
- The final commit adds this record.

### Gates

Observed locally at `441fd8e7`, after the last code change; this record is the only later edit:

| Gate | Result |
|---|---|
| `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast` | 2006 run, 2006 passed, 7 skipped |
| `python3 tools/metadata/ledger.py refresh --toolkit gtk` | gtk: 1521 facts, 1546 tests, 0 manual not passing, 0 inferred not passing, 1 parked |
| `ledger.py check` | gtk 550 manual verified; inferred 971 verified, 0 pending, 1 parked (qt/appkit/winui unchanged) |
| `corepack pnpm test` (with `test:meta`) | rc 0; pytest 127 passed, 1 skipped (the opt-in `SVELTERS_CHECK_GIR` test) |
| `corepack pnpm -r build` | rc 0 (needs `python3 tools/docs/build_ir.py` first, and a `pnpm` on `PATH`: `experiments/native-compiler/compiled` spawns a bare `pnpm`) |
| `corepack pnpm docs:test` | rc 0; 11 Python tests (1 skipped), 17 node tests |
| `check-links` | 909 pages, all references resolve |

The N1 tests in the crate:

- `gio_services` 1/1;
- `async_callbacks` 1/1;
- `types_verify` 54/54, including 4 `*_async`;
- `types_members` 7/7;
- `@svelters/types`: 14 node tests plus `tsc` on the typings.

### CI (`ci/nt1` at `441fd8e7`)

All five workflows passed:

| Workflow | Run id | Result |
|---|---|---|
| js | 39301392419 | success |
| linux-gtk | 39301392420 | success: `gtk` (Arch) 1892/1892, 36 skipped; `gtk-4-14` (Ubuntu, required) 1675/1675, 29 skipped |
| linux-qt | 39301392440 | success |
| macos-appkit | 39301392478 | success |
| windows-winui | 39301392505 | success |

The first push (`e01206d7`, runs 39298739506 and 39298739581) failed in two places:

- `js`: the ledger was not refreshed yet.
- `linux-gtk` Arch: the host `serdes_async` late-completion expectation. `441fd8e7` fixes it.

`gtk-4-14` already passed on that first push.

### Bound members before and after

Both columns use the same GIR (GTK 4.24.1); "before" is the NT0 rules run on it:

| | before (NT0 rules) | after (N1) |
|---|---:|---:|
| bound members (8 namespaces) | 3552 | 3732 |
| blocked | 593 | 409 |
| blocked `async` | 106 | 6 |
| blocked `callback*` | 38 | 20 |
| blocked `unbound` | 86 | 13 |

- **What is bound now:** 94 async members (7 of them statics or constructors), 14 members with callback parameters, 72 members with widget slots.
- **Curated types:**
  - Gio.File 63 → 95 bound (4 blocked);
  - Gtk.FileDialog 25 → 33 (0 blocked);
  - Gdk.Clipboard 9 → 13;
  - Gtk.TextBuffer 84 → 85.
- **Still blocked async (6):** 3 starts with out parameters (`InputStream.read_async`, `read_all_async`, `DBusConnection.send_message_with_reply`) and 3 without a `<stem>_finish` (`File.copy/move_async_with_closures`, `replace_contents_bytes_async`).
- **GIR drift.** The checked-in NT0 facts were made on GTK 4.22 (3520 bound / 588 blocked). Regenerating on 4.24.1 with the NT0 rules gives 3552 / 593, including +2 Gtk models (EnumList, EnumListItem). The GTK 4.24 `cairo-1.0.gir` is not installed on this machine (Arch `gobject-introspection` is missing; only `-runtime` is installed). The facts were generated with `--girdir` pointing at the system GIRs plus that file, taken from the matching Arch package (1.86.0-2) under `target/`.

### Open items

1. **Facts and invoker disagree outside N1.** A cross-check of *every* bound member found NT0-era disagreements, so the generated `types_members.rs` is scoped to the N1 surface for now. Examples seen:
   - GLib 21/176: `GLib.Uri.*`, `KeyFile.load_*` (enums and flags without a GType, `UriFlags`/`KeyFileFlags`);
   - Gio 5/655: caller-allocated out arrays (`InputStream.read`, `DBusMessage.to_blob`), GType-less enums;
   - Graphene 9/232;
   - Gsk 2: struct by value;
   - many Gtk widget-interface members (`Gtk.Editable.*`, `Gtk.Actionable.*`) that do not resolve on the interface's own kind.

   The fix is in `types_facts.py`: block GType-less enums and flags and caller-allocated arrays, or teach `gi.rs` about them. Then the check can widen.
2. **Async options after defaulted parameters.** `{ signal }` must come after every declared parameter. With a defaulted `ioPriority` the caller passes it explicitly (`queryInfoAsync(a, f, 0, { signal })`).
3. **`forever` scope** is implemented (never freed natively; the JS function lives until its owner dies, forever for statics) but has no real-API test: no bound member has it.
4. **Callbacks that run off the main thread** get the zero value and a stderr message. GTK/GIO callbacks in the bound set run on the main context.
5. **Docs IR** (`tools/docs`, read by CL0) still has no non-GUI types. The type pages are generated straight from the facts, as in NT0.
6. **`types_facts.py --check` needs a complete GIR set** (cairo). Whoever regenerates next needs the same `--girdir` workaround or the `gobject-introspection` package.

### Conflict risks

- **NT2 / shared files:**
  - `packages/types/src/core.js` and `core.d.ts`: additive `callAsync`, `callStaticAsync`, `AsyncOptions`, `NativeElement`.
  - `packages/types/package.json`: `gen_types.py` rewrites `exports` and `scripts`. Unchanged content here, but NT2's `qt/` exports must survive the generator.
  - `plans/architecture/backend-abi.md`: a new "N1" section, no ABI change.
- **Host files:** `crates/svelters-host/src/core/{callbacks,asyncs,statics,ops}.rs` and `core.rs` (event adoption). NT2's Qt async work may touch `asyncs.rs`.
- **RT1:** none expected. `packages/types/vite.js` and the build alias wiring are untouched.
