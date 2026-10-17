# S1 (ABI v3 / serdes lane): handoff

Written at the coordinator's request; the implementing agent stopped here with a very large context. Read
`docs/notes/serdes-impl.md` (decisions per area) and `docs/notes/js-native-serdes.md` (the design) with this.

## Branch state

- Branch `worktree-agent-aba56fbf823e6a71f` (worktree `/projects/svelters/.claude/worktrees/agent-aba56fbf823e6a71f`). HEAD is the commit that carries this file; the previous commit is "value model: variants ...".
- Master merged in up to **4f2e859** (ABI v2 complete, hot reload, bind hooks, docs site). Master is now **8997ea0+**: **not merged**. Expect conflicts there (see below).
- `SVL_ABI_VERSION` is **3**, applied on top of v2. v2's members sit where master put them (`handle_info` after `listen`, `schedule_wake` after `wake`); the v3 members (`caps, describe, describe_type, cancel`) and the two new `svl_host` members (`invoke_callback, release_callback`) are appended.
- Not done: nothing is half-applied. Everything is committed.

## Priority items

| Item | Status |
|---|---|
| 1. Main types (scalars, strings UTF-8/16, enums, flags, records Rect/Size/Point/Insets/Color, nullable, object handles, BOXED + dispose + checkpoint release, BYTES) | **Done on both toolkits**, with round-trip tests. |
| "Now" items (ids, borrowed strings, descriptor coercion, enums as names) | **Done.** Bench: see below. |
| 2. ERROR | **Done.** `SVL_ERROR` out value -> JS `NativeError {domain, code, message}` (code named via the error enum descriptor). Qt: C++ exceptions are caught at the ABI boundary into `ERROR` (domain `c++`). GTK: real `GError` from GIO (`gio-file` service). |
| 3. CALLBACK | **Done** in the host (`core/callbacks.rs`): call / async / notified scopes, `forever` refused, generation-tagged ids, owner invalidation, nested synchronous invocation, tree ops refused while a callback runs, JS functions dropped at checkpoints. Backend side proven by a Rust test backend and by GTK `gio-file` (`listNames` call-scope, `monitor` notified-scope). **Not done: Qt callbacks** (QtWidgets has ~1 callback parameter; no generator support for `CALLBACK` parameters or signals-as-callbacks). |
| 4. async/Promise | **Done** in the host (`core/asyncs.rs`): `SVL_M_ASYNC` methods, completion callback, `{signal}` -> `backend.cancel(id)`. Proven with the Rust backend and real `g_file_load_contents_async` + `GCancellable`. **No Qt async** (none in QtWidgets). |
| 5. MAP / variants | **Done**: host (plain object -> `MAP` in variant slots, Date, bytes), GTK `GVariant` (`a{sv}`/`av`/`ay`/scalars), Qt `QVariant` (`Conv<QVariant>`, QVariantMap/List). Not done: exact GVariant typing helper `variant(sig, v)`; GValue (`GParamSpecValue`) props. |
| 6. Out / in-out params | **Not done.** Design in the notes (method returns `LIST [ret, outs...]`, host shapes `{ret, name...}`), flags `SVL_D_OUT/INOUT` exist in the header and descriptors; the host does not read them yet. No generator support for out parameters on either toolkit. |
| 7. Dates / strings | **Mostly done**: Qt `QDate/QTime/QDateTime` (DateParts/TimeParts/Date), GTK `GDateTime` (Date), string forms: QKeySequence, QUrl, QIcon, QFont, QColor, GIcon, PangoFontDescription, GdkRGBA, GtkShortcutTrigger, GFile. Not done: `QDateTimeEdit` etc. only via the 3 added elements; no GTK time-only/date-only record types. |

## What exists, by layer

- **Header / Rust mirror / safe wrappers** (`crates/svelters-abi`): complete v3 value model, descriptors, `Value` (owned, with `DateTime`/`Error`), `ValueRef`/`ArgsRef` (borrowed), `DescStore`, `Backend` trait with default v3 methods (`set_prop_ref`, `call_ref`, `describe`, `describe_type`, `cancel`, `caps`), `HostRef::invoke_callback/release_callback`, `TypeDesc.scope` (callback lifetime scope, was `reserved`).
- **Host** (`crates/svelters-host`): `convert.rs` (engine-agnostic `encode`/`decode` over `JsReader`/`JsWriter`/`JsPromises`), `core/ops.rs` (caches, validation messages, boxed release), `core/callbacks.rs`, `core/asyncs.rs`, QuickJS reader/writer in `quickjs/conv.rs`, `QjsCallbacks` + `RunningCtx` (nested re-entry through the raw context), prelude wrappers (`NativeBoxed`, `NativeError`, AbortController polyfill, completion promises).
- **GTK** (`crates/svelters-gtk`): GType-keyed converters (`convert.rs`), type ids and descriptors (`types.rs`), `backend/extras.rs` (`gio-file` service), harness extended (`Harness::named/get_value/set_value/...`).
- **Qt**: `tools/metadata/qt/templates/serdes.hpp.in` (`Conv<T>`, `EnumInfo`, type registry), generator changes in `gen_cpp.py` (Conv-based dispatch, `describe`, enum/boxed emission, d.ts), `ingest.py` (EnumIndex, record/boxed/bytes/date/cstring classification, enum table `metadata/qt/_enums/Qt.json`), overrides: typed props on `box/label/button/entry` and new elements `spinbox doublespinbox dateedit timeedit datetimeedit colordialog combobox`.
- **Renderer / typings**: `packages/renderer/src/hostops.js` (ids), `coerce.js` (descriptor-driven coercion, `INT_PROPS` removed from the renderer), objects allowed through; TestHost has ids + `schema.json` (generated by `gen_backend.py`), log backend publishes the schema via `set_default_schema`; typings `values.d.ts` (`tools/metadata/common/tsvalues.py`), `gtk.d.ts` (enums as unions, flags as arrays, records, boxed, `gio-file`), `qt.d.ts`.
- **Tests** (all pass): `svelters-host/tests/serdes.rs` (+ `tests/js/serdes_*.js`: validation messages, coercion, ids, UTF-16, boxed finalizers, native errors, callbacks, async, variants), `svelters-gtk/tests/serdes.rs`, `svelters-gtk/tests/gio_services.rs`, `svelters-gtk/src/convert.rs` unit tests, `examples/counter-qt/tests/serdes.rs` (15 cases incl. exceptions, variants), generated Qt verify tests, `packages/renderer/test/serdes.test.mjs`.
- **Bench**: `cargo run --release -p svelters-host --example setprop_bench` (see below).

## Gates (at HEAD, merged master = 4f2e859)

- `CARGO_BUILD_JOBS=4 xvfb-run -a cargo nextest run --workspace --test-threads 3`: **114 passed**.
- `pnpm test`: passes; **flaky**: `packages/renderer` test "reordering many rows is not quadratic" is timing based and failed once under load average ~30 (`1000 rows 22 ms, 4000 rows 210 ms` = 9.5x vs the 9x bound). It passed on rerun. Not related to this lane.
- `pnpm docs:test`: **OK** (10 tests). `pnpm -r build` fails only in `docs/site` (astro not available offline in the worktree); fixtures build with `pnpm --filter "./fixtures/*" --filter "./bench/*" build`.
- Flaky under load, seen once each and green on retry: `svelters-gtk::lifetime destroy_inside_a_signal_is_deferred`, `svelters-gtk::oplog_replay replay_all_fixtures`, `counter::integration keyed_list_gtk` (20 s timeout), one more integration test in one run.

## Benchmark (best of 7, ns/op including the ~90 ns JS loop; machine load average ~30, so read ratios)

| row | before (master) | after, UTF-8 backend | after, UTF-16 backend |
|---|---:|---:|---:|
| `setProp` int by name | 2337 | 2235 | 2456 |
| `setProp` int by id | n/a | 1607 | 1860 |
| 11 B ASCII by name | 2628 | 2284 | 2551 |
| 11 B ASCII by id | n/a | 1699 | 1864 |
| 1 KiB ASCII | 5965 | 2464 | 2774 |
| 1 KiB Latin-1 | 28702 | 7137 | 2747 |
| 1 KiB wide | 43722 | 13640 | 1834 |
| record `{x,y,width,height}` | unsupported (TypeError) | 4768 | 4862 |

By id is ~28% faster than by name; the owned-`Value` Rust backend after the change costs about what master did (see `setprop_bench` third block). The "before" numbers were measured from a `git archive` of master in the scratchpad with the same JS.

## Known conflicts with master (8997ea0+) to expect

- A1 is annotating widgets in both toolkits (`metadata/overrides/**`, probably `gen_cpp.py` and the GTK generator): my edits are additive keys inside the same YAMLs (`QWidget.yaml`, `QLabel.yaml`, `QPushButton.yaml`, `QLineEdit.yaml`) and new files (`QSpinBox.yaml`, `QDoubleSpinBox.yaml`, `QDateEdit.yaml`, `QTimeEdit.yaml`, `QDateTimeEdit.yaml`, `QColorDialog.yaml`, `QComboBox.yaml`). The Qt generator (`gen_cpp.py`) was largely rewritten around `Conv<T>`: if A1 changed `prop_cases`, `emit_call`, `emit_listen`, `_resolve_*`, merge by hand, keeping `conv_in`/`Conv<Plain<decltype(...)>>` dispatch.
- `core.rs` in the host: master's former `op_*_raw` block lives in `core/ops.rs` now (semantics ported: dead ring, `live_kind`, tree tracking, toplevels, MAX_HANDLE). Any later master change to those functions must be re-applied to `core/ops.rs`.
- `LogOptions` has no `schema` field on purpose (other code builds it with struct literals); the schema is process-wide via `svelters_backend_log::set_default_schema`.
- `TypeDesc` gained `scope`: any new Rust code that builds a `TypeDesc` literal needs `scope: 0`.
- `svl_backend` literals in new backends need the four v3 fields (`caps, describe, describe_type, cancel`); `svl_host` literals need `invoke_callback, release_callback`.

## Design decisions (details in `serdes-impl.md`)

- Descriptors drive everything; Qt descriptors are built from `Conv<T>::slot`, so they cannot disagree with the converters.
- Enums travel as native values with a type id; names are resolved by the host. Integer record variants (`QRect`, `GdkRectangle`, ...) are separate backend types named like the float ones.
- Strings: the host picks UTF-16 for backends with `SVL_CAP_UTF16_IN` (always, no sniffing); UTF-8 strings are NUL terminated.
- Boxed wrappers register with `FinalizationRegistry` without an unregister token (QuickJS holds tokens strongly).
- Callbacks: nested invocation re-enters through the raw `JSContext` kept in a thread-local while a binding runs (`Context::with` is not re-entrant). Tree ops are refused during a callback; no job drain; wake after idle invocation.
- Async: completion settles one microtask later so a synchronous completion cannot be reported as an unhandled rejection.
- `host->invoke_callback` failures carry their reason in an `ERROR` out value (domain `svelters`).
- GTK `gio-file` is hand-written because dynamic GI invocation is M3; it is the only place GTK has callback / async / GError members.
- Qt enum table lives in `metadata/qt/_enums/Qt.json` (not `metadata/qt/*.json`, which the docs generator reads as kinds).

## Open questions

- Should the generated GTK metadata/dynamic invocation (M3) own `gio-file`-like members, and how does it discover callback scopes (GIR `scope` annotations are available but unused)?
- Out params: shape in JS for `INOUT` and for several outs plus a return (design says `{ret, ...names}`); needs a Qt generator story (override key for `T*` out params).
- Qt `UTF16_OUT`: Qt still returns UTF-8 (`toUtf8`); the host would need a `JS_NewStringUTF16` fast path and a `Value::Str16`.
- `packages/dom` (older DOM shim) still has its own `INT_PROPS`.
- Event args are decoded from owned `Value`s; a borrowed fast path may matter for hot signals (`changed` text).

## Lessons

- The sandbox rejects `cd` + `git` or heredocs mixed with other commands: write helper scripts to the scratchpad with the Write tool and run them as separate commands; keep git commands plain.
- Merge conflicts concentrate in `core.rs`, `gen_cpp.py`, `backend.cpp.in`, renderer `index.js` and `testhost`: keep new logic in new files (`core/*.rs`, `serdes.hpp.in`, `extras.rs`, `hostops.js`, `coerce.js`).
- QuickJS: no `AbortController`, `FinalizationRegistry` tokens are strong, `Context::with` is not re-entrant, `JS_ToCStringLen2` is zero-copy only for ASCII 8-bit strings.
- Qt tests must not touch the user's D-Bus session (set `DBUS_SESSION_BUS_ADDRESS` to a dead path) or QtDBus can abort the process.
- Machine load (~30) makes timing tests flaky; rerun before chasing.

## Exact next steps

1. `git merge master`; resolve per "Known conflicts"; rebuild (`pnpm install --offline`, `pnpm --filter "./fixtures/*" --filter "./bench/*" build`), regenerate (`python3 tools/metadata/gtk/gen_backend.py`, `QT_QPA_PLATFORM=offscreen python3 -I tools/metadata/qt/ingest.py`, `cargo build -p counter-qt` for `qt.d.ts`), rerun the gates above plus `pnpm docs:test`.
2. Out / in-out params: read `SVL_D_OUT/INOUT` in `core/ops.rs::op_call` (exclude OUT-only params from JS args, expect `LIST [ret?, outs...]`, shape the result), add a Rust-backend test, a `gio-file` method (`query()` -> size/contentType/isDir via `g_file_query_info`), then the Qt generator key.
3. Qt callbacks / async if any QtWidgets member needs them (`QDialog::open` + `finished` as promise): generator support for `CALLBACK` slots and an `async` annotation.
4. `gtk` dynamic invocation (M3) should reuse `convert.rs` (`to_value`/`from_value`) and `types.rs` for argument plans; move `gio-file` into generated tables then.
5. Update `plans/architecture/backend-abi.md` (the v3 section still says CALLBACK/MAP/ERROR are M4) and `serdes-impl.md` "Open" with the list above.
