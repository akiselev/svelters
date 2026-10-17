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
