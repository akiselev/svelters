# S2 notes: ABI v3 remainder

## 1. Out and in-out params (done)

- Contract: `plans/architecture/backend-abi.md` "Out and in-out parameters (S2)". Host: `core/ops.rs::op_call` (`ins` = non-OUT params) and `decode_outs`.
- Result shape is always an object `{ret?, ...outs}`, even for a single out. Chosen over bare values so the shape does not change when a method gains an out parameter, and over arrays so keys are named (the descriptor's slot names).
- How a backend reports outs: return `LIST [ret, out...]`; ret is `NULL` for void; outs in parameter order. That is the only thing X1's GI invoker needs: for each GIArgInfo with direction OUT/INOUT, flag the slot, skip OUT args when reading JS args, and append the out values to the list.
- Qt generator: `out:`/`inout:` keys on a method entry (names of `T *` / `T &` params; types int/uint/bool/double/float/qint64/QString), `target: layout` for methods, calls go through the declaring class (QSpinBox re-declares `validate` as protected; the match now only takes public methods).
- Proofs: Rust backend (`out_and_inout_params_come_back_as_one_object`), Qt `QLayout::getContentsMargins` + `QAbstractSpinBox::validate`/`fixup` (counter-qt `serdes`), GTK `g_file_load_contents` etag (`gio-file.loadContentsEtag`).

## 2. Qt callbacks and async (done)

- Generator: `callbacks:`, `discard_return:` and `async:` keys on a method entry (contract in `backend-abi.md`, "Qt callbacks and async"). Template: `js_invoke`, `async_complete`, `new_callback_type` (serdes.hpp.in), `AsyncOp` + `b_cancel` (backend.cpp.in; the vtable's `cancel` slot is now set).
- Real APIs: `QMenu::addAction(const QString&, Functor, const QKeySequence&)` as a `notified` callback (`menu` element, new `QMenu.yaml`; a1 will want to extend it for G7), `QDialog::open()` + `finished(int)` as the Promise on `colordialog`.
- Tests: `examples/counter-qt/tests/serdes.rs` (descriptors, invocation record, completion, cancel), and `examples/counter-qt/tests/host.rs` (real host, JS: Promise resolve/reject/abort, callback fired from the Qt loop, throwing callback reported, no callback left alive). New test hook `hooks::trigger_actions`.
- Only the metadata classes (QtWidgets) are usable: `QTextDocument::setResourceProvider` (a std::function returning a value) would have been the better callback-with-result proof, but the Qt ingest only covers QtWidgets classes. A Qt API with a callback result is still unproven on Qt.
- `Conv<T>::out` for a boxed argument of a callback mints a boxed handle (the JS side owns and disposes it).

## 3. Qt UTF-16 strings (done)

- `Conv<QString>::out` returns the QString's own buffer flagged UTF-16 (`OutArena::wides` keeps a shared copy alive); Qt advertises `SVL_CAP_UTF16_OUT`. Host: `Value::Str16`, `Value::from_raw_wide` (used for get_prop/call results and the `HostCallbacks` shim), `JsWriter::string16` -> `JS_NewStringUTF16`.
- Raw readers in tests/templates had to learn the flag (`verify.cpp.in::str_of`, `counter.rs::get_str`).
- Benchmark (release, loaded machine, ns per `getProp` of a QLabel text through the host, best of 7, JS loop included; `SVL_BENCH=1 xvfb-run -a cargo test --release -p counter-qt --test host qt_getprop_bench`):

| string | UTF-8 out (before) | UTF-16 out (after) |
|---|---:|---:|
| 11 B ASCII | 1764 | 1635 |
| 1 KiB ASCII | 5370 | 1823 |
| 1 KiB Latin-1 | 15514 | 1728 |
| 1 KiB wide (BMP) | 32592 | 1792 |
| 1 KiB wide + astral | 24977 | 1974 |

## 4. GVariant exact typing and GValue props (done)

- `variant(sig, v)` / `NativeVariant` in `prelude.js`; `JsReader::typed_variant` (default `None`, QuickJS reads `__svlVariant`); `SVL_T_TYPED_VARIANT = 10`. GTK `typed_variant` in `convert.rs`; Qt `Conv<QVariant>::in` (basic signatures only). Typings: `values.d.ts` (`NativeVariant`, `variant`).
- GValue (`G_TYPE_VALUE`) props: `gvalue_type()`, `gvalue_in` / unwrap in `from_value`; descriptor is the dynamic variant slot. `glib::Value` has no `StaticType` in glib-rs, hence the raw `g_value_get_type`. No widget kind in the generated GTK tables has such a property today (`GtkDropTarget:value`, `GtkConstantExpression` are not kinds), so the proof is the round-trip unit test in `convert.rs`.
- Tests: `svelters-gtk` serdes `variants_dates_and_string_forms` (GtkActionable:action-target with 13 signatures), `convert::gvalue_properties_hold_an_inferred_value`, host `typed_variants_cross_as_records`, Qt `variants`.

## 5. packages/dom INT_PROPS (done)

`packages/dom` no longer has `INT_PROPS`; `setAttribute` calls the renderer's `coerceAttribute(host, tag, name, value)` (descriptor-driven via `host.propType`), exported as `@svelters/renderer/coerce`. Behaviour change: a host that publishes no descriptors gets the attribute text unchanged (before: `spacing` was always coerced to an integer). The native host and the Node TestHost both publish descriptors. `packages/dom` now has a test (`test/coerce.test.mjs`, fake host). Note: the shim still targets the older host API (`host.create`), which the Node TestHost no longer provides, so it only runs on the native host.
