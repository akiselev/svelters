# E0 + G0: Rust exports registry core, non-blocking host start

Design: `docs/notes/egui-rust-exports.md` (§3 layer, §5 macros, §8 lifetimes, §9 codegen, §10 phasing) and
`plans/research/EGUI.md` (§3, §7 G0, §8 D1). This file records E0 and G0 as built.

## User decisions (2026-10-21)

1. Specifier: `svelters:app/<module>`.
2. `i64`/`u64` cross as a checked JS number (a `RangeError` outside ±2^53, in both directions); `#[js(bigint)]`
   opts a parameter, a record field or a method's result into `bigint`.
3. Op log: exported-object calls are logged in full, like native calls, so TestHost replay covers them.
4. E0 and G0 ship together (G0 = EGUI.md D1: non-blocking start + "started" hook).

Still open (not decided here, kept out of E0): the async executor default, settle-on-abort, foreign traits, the
rquickjs fast path (the measurement below feeds it), store granularity, subsecond, CUSTOM-WIDGETS Q9.

## As built

### Crates

| Crate | What |
| --- | --- |
| `crates/svelters-host/src/exports/` | the engine-neutral registry: `JsType`/`FromJs`/`IntoJs`, `TypeBuilder`, `ExportObject`/`UiObject`/`SharedObject`, `Obj<T>`, `ExportError`, `CallCx`, the `inventory` registrations (`ObjectDef`, `ImplDef`, `FnDef`), the schema (`schema()`), the per-host object table (`objects.rs`) and codegen (`json()`, `typescript()`, `javascript()`, `dump()`, `dump_if_debug()`) |
| `crates/svelters-host/src/core/apps.rs` | the host ops on app kinds and handles (construct, call, callStatic, getProp, setProp, destroy) |
| `crates/svelters-macros` | `#[derive(Record, Enum, Error)]`, `#[svelters::object]`, `#[svelters::export]` (syn 2) |
| `crates/svelters` | the facade apps depend on: re-exports the macros and `svelters_host::exports` |
| `examples/exports` | the acceptance app: `src/lib.rs` (`svelters:app/demo`), `src/main.rs` (GTK runner, `--dump`), `app/` (Svelte + TS, committed `.svelters/app` typings and stub), `tests/exports.rs` |

### Routing

- **Kinds** `app:<module>` (module functions) and `app:<module>.<Type>` (classes) get ids in pack 63
  (`63 << 22 | n`, CUSTOM-WIDGETS §3.2); record/enum/list type ids `63 << 22 | 0x20_0000 | n`. `op_kind_id`,
  `resolve`, `member` and `type_info` answer them from the schema, so the name caches, `convert::encode`
  and `convert::decode` work unchanged.
- **Handles**: `APP_HANDLE_BIT | n` with **bit 52** (deviation, below). `construct`/`call`/`callStatic`/
  `getProp`/`setProp`/`destroy` check the bit or the `app:` prefix first and never reach the backend;
  `listen` refuses (`on`) / ignores (`off`), `observe` answers `false` (E0 has no events or stores),
  `typeChain` answers the kind name, `adopt_handles` skips them.
- **Values**: arguments are encoded with the member descriptor (`convert::encode`, same validation and
  messages as backends: arity, integral ranges, enums by name, records by field, nullable/optional), copied
  to owned `Value`s and passed to the generated thunk. Results go through the new `convert::decode_slot`
  (slot-aware: `SVL_D_BIGINT` integers become `bigint`, also in records and lists). `encode_record` now lets an
  `SVL_D_OPTIONAL` field be missing (additive).
- **Objects**: UI objects are `Rc<RefCell<T>>` (`&self`/`&mut self`; a re-entrant borrow is
  `NativeError{code:'busy'}`; a panic in a `&mut self` method poisons the object: `code:'poisoned'`). Shared
  objects are `Arc<T>` with a compile-time `Send + Sync` check (`self: Arc<Self>` works). Identity: the table
  maps `(TypeId, allocation address) -> handle`, so returning the same `Arc`/`Obj` twice gives the same handle
  and, through `wrapObject`, the same JS object. Release: `dispose()` and garbage collection both reach
  `host.destroy(h)`, which drops the table's reference (idempotent; `Drop` runs on the UI thread for UI
  objects). With the renderer installed, results are renderer nodes released by its finalizer; without it the
  generated stubs register their own `FinalizationRegistry`.
- **Errors**: `Result<T, E: ExportError>` throws `NativeError{domain:'app:<module>', code, message}`
  (`#[derive(svelters::Error)]`: kebab-case variant names, `#[js(code)]`, `Display`); panics are caught
  (`domain:'svelters', code:'panic'`).
- **Reload**: a successful reload releases the old runtime's app objects; a failed one releases the objects
  the failed load created.
- **Op log**: `Host::set_op_log(sink)`; `svelters_host::testing` appends to the log backend
  (`LogBackend::append_line`). Lines: `construct #a1 app:m.T [args]`, `call #a1 app:m.T.method [args]`,
  `static app:m.f [args]` (` #aN` when it returns an object), `set #a1 app:m.T.prop <json>`. `#a<n>` names app
  objects. Reads and releases are not logged (native reads are not either; GC releases are not
  deterministic).

### Macros

- `#[derive(svelters::Record)]` (named-field structs; camelCase fields, `#[js(rename)]`, `#[js(default [= e])]`,
  `Option<T>` optional, `#[js(bigint)]`), `#[derive(svelters::Enum)]` (unit enums, kebab-case strings),
  `#[derive(svelters::Error)]`.
- `#[svelters::object(module = "m")]`, `#[svelters::object(shared, module = "m")]` (`name = "..."` renames).
- `#[svelters::export]` on impl blocks (several per type): every `pub fn`; `#[js(constructor)]`, `getter`,
  `setter` (`set_x` -> `x`), no receiver = static, `rename`, `bigint`, `skip`. On a free function with
  `module = "m"`. Parameters: `FromJs` values, `&str`, `&[T]`, `&T`/`&mut T` of exported objects, `Arc<T>`,
  `Obj<T>`. E1 attributes (`worker`, `blocking`, `local`, `supersede`, `max_in_flight`, `buffer`) and `async fn`
  are compile errors that say "E1".
- Registration is `inventory` (re-exported through the facade). A crate whose exports are only reached
  from JS must be referenced from the binary so the linker keeps it (`exports_demo::link()`).

### Modules, typings, compiler

- `svelters::exports::dump(dir)` writes `exports.json` (`svelters-exports/1`), `app.d.ts`
  (`declare module 'svelters:app/<m>'`, classes extend `NativeObject`, `@throws` from error codes) and
  `<module>.js`, rewriting only changed files and deleting stale module files. `dump_if_debug` is the dev-host
  hook (the example calls it on start); `exports --dump <dir>` is the headless form.
- The QuickJS host serves `svelters:app/<module>` (generated from the schema of the running binary) and
  `svelters:core` (an embedded copy of `@svelters/types/core`) as built-in modules, for bundles that leave the
  import external and for plain ESM entries (the tests).
- Vite (`@svelters/native-runtime/vite.js`) aliases `svelters:app/<m>` to `<app root>/.svelters/app/<m>.js`, so
  bundled apps share one `@svelters/types/core` with the generated toolkit modules.
- `svelters-compile` needed no change: it passes `svelters:app/*` imports through, and
  `PlatformSemantics::native_type` answers `None` for them (silent), which is the E0 minimum. Reading
  `exports.json` there is E5.

## Deviations from the note

- **Handle bit 52, not 63.** JS handles are numbers below 2^53 (`MAX_HANDLE`); bit 63 cannot cross. App
  handles are `[2^52, 2^53)`; backends mint small counters, far below.
- **Bundled stubs by default, host-served built-ins as well.** A host-served module importing a second copy of
  the core would split the identity map and `NativeObject` from the bundle's copy, so Vite bundles the dumped
  stub; the host serves the same JS (with `svelters:core`) for unbundled entries. Staleness is caught by the
  example's `committed_typings_are_current` test instead.
- **Data enums (`#[js(tag)]`), tuples, `JsFn`, `Json<T>`, `Date`, foreign traits** are not in E0 (compile errors
  or unimplemented `JsType`). Maps (`HashMap/BTreeMap<String, T>`) cross with dynamic values (no per-value
  descriptor).
- **Objects inside records** convert, but the stub does not wrap them into classes when nested in a record
  declared for another module (records' class references are typed per first use).
- **Releases are not logged** (decision 3 covers calls; GC timing would make logs flaky).
- **`svelters exports` CLI**: not built; the app binary's `--dump` (and the check test) cover it, since only the
  app binary has the schema.

## Measurements

Per call, release build, QuickJS-NG via rquickjs 0.14, log backend, best of 5 × 200k calls
(`cargo test --release -p exports-example --test exports -- --ignored --nocapture per_call`), load average
≈ 10–12 on 16 cores (other lanes building):

| Path | ns/call |
| --- | --- |
| plain JS function call | 197 |
| registry: module function `noop(x)` (stub -> `callStatic` -> thunk) | 2781 |
| registry: getter `t.count` (`NativeObject` -> `getProp`) | 3214 |
| registry: method returning a 4-field record with an enum | 7240 |
| C ABI (log backend): `setProp(w, 'title', 'x')` | 2607 |
| C ABI (log backend): `call(w, 'present', [])` | 2600 |

The first version formatted kind names, argument prefixes and the op-log JSON on every call (4.7 µs for
`noop`); making them lazy brought the registry path level with the ABI path. It is ~14x a JS call. The rquickjs
`#[class]` path measured 0.72–1.04 µs in the probe, so a fast path would win about 2.5–3x on scalar calls:
worth it only for chatty per-frame APIs (open question 5 stays open, with this number).

Build and size (release, `examples/exports`: 3 classes, 3 functions, 1 record, 1 enum, 1 error type):

- `exports` binary 5.77 MB; registry runtime + inventory + the demo's thunks: 255 symbols, 152 KB, of which the
  demo's generated code is 95 symbols, 38 KB (≈ 2.5 KB per exported member).
- Incremental rebuild of the exports crate after a touch: 0.9 s (debug).
- `inventory` 0.3 is the only new runtime dependency of `svelters-host`; `svelters-macros` adds syn/quote
  (already in the lock file).

## G0: non-blocking start and the started hook

See "ABI v2.7 additive (G0)" in `plans/architecture/backend-abi.md`.

- **ABI v2.7 (additive):** one nullable member at the end of `svl_backend`,
  `int (*start)(void *self, const svl_host *host)`: attach the host and bring the toolkit up without running
  the loop; the caller (embedder, kittest, a platform that owns `main`) drives the loop and the backend calls
  host callbacks as from `run`. `start(self, NULL)` detaches. `svl_host` is unchanged. Safe layer:
  `BackendRef::{has_start, start, detach}`, `Backend::{supports_start, start}` (defaults: unsupported).
- **Started rule:** the host treats its first callback from `run`/`start` (or the return of `start`) as
  "toolkit up" and fires the hook first, before that callback takes the re-entry guard. Every desktop `run`
  already checkpoints when its loop starts (GTK activate, Qt, AppKit, WinUI).
- **Host API:** `on_started(f)`, `run_with(f)` (arms a `wake` before `run` only when a hook is set; `run()`
  without a hook is unchanged), `start()`, `start_with(f)`, `stop()`, `is_started()`, `frame()` (embedders
  that own the frame clock). `Drop` stops a started host. `Host` became invariant in `'a` (it stores the hook).
- **Attach before mount** (coordinator request from N3, `plans/milestones/N3-native-models.md` open item 1):
  `load_bundle` first attaches the host through `start` when the backend has one and the host is not running,
  so JS that runs during mount (a GTK `CustomFilter` evaluated while a filter model is built) has a host in the
  desktop order too. The started-hook orders (`run_with`/`start_with`) load after the attach by construction.
- **Backends:** GTK `b_start` stores the host pointer (GTK is initialised by `GtkBackend::new`); `b_wake`'s idle
  re-reads the pointer when it fires, so a wake after `stop`/the end of `run` reaches nobody. Qt `b_start` sets
  the host, creates the app and delivers a wake that came before the waker existed. The log backend supports
  `start` (an `attached` flag). AppKit and WinUI keep NULL (their `attach_host` still works).

## Tests

- New: `crates/svelters/tests/exports.rs` (6: calls/values/errors/panics/poisoning/op log, identity + dispose +
  GC, GC release, schema + typings + JSON, dump rewrites only changes, unknown module/member errors);
  `examples/exports/tests/exports.rs` (2 + 1 ignored: the Svelte app on real GTK calling the exports and
  clicking through them, committed typings current; `per_call_cost` is the benchmark);
  `crates/svelters-host/tests/started.rs` (5) and `crates/svelters-gtk/tests/host_start.rs` (3) for G0.
- `examples/exports/app`: `pnpm test` = `vite build` + `svelte-check` against the dumped `app.d.ts`
  (0 errors; a deliberately mistyped file gives 3 errors, so the typings are enforced).
- Local gates on the merged tree (c5ce909d): `cargo check --workspace --all-targets` clean;
  `cargo nextest run --workspace` 2358/2358 passed (8 skipped); `pnpm -r build`, `pnpm test` (incl. `test:meta`),
  `pnpm docs:test` all exit 0.

## CI

Branch `ci/e0`, head c5ce909d (merge of origin/master 3479e2da): js 39598151437, linux-gtk 39598149384
(gtk 2155/2155, gtk-4-14 1897/1897, portable 48/48), linux-qt 39598149415, macos-appkit 39598149744,
windows-winui 39598149683: all success.

## Open items

- E1–E5 (offload, buffers, stores/services/events, widgets, compiler types).
- Node TestHost: `svelters:app/*` stubs are not available in Node (the design's "native export not available"
  stubs are not generated yet); exports fixtures run on the QuickJS host only.
- Records holding objects from other modules (wrapping), data enums, tuples, `Date`.
- `svelters exports --check` as a CLI command (the example's test does it per app).
- Conflict risks with other lanes: `crates/svelters-host/src/core/ops.rs`/`statics.rs` (routing branches at
  the top of each op), `convert.rs` (`decode_slot`, optional record fields, `JsWriter::bigint`),
  `quickjs.rs` (built-in module resolver), `reload.rs`, `packages/native-runtime/vite.js` (one alias),
  `pnpm-workspace.yaml`, `.github/workflows/linux-gtk.yml` (package list), the ABI files (G0).
