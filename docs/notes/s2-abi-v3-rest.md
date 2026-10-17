# S2 notes: ABI v3 remainder

## 1. Out and in-out params (done)

- Contract: `plans/architecture/backend-abi.md` "Out and in-out parameters (S2)". Host: `core/ops.rs::op_call` (`ins` = non-OUT params) and `decode_outs`.
- Result shape is always an object `{ret?, ...outs}`, even for a single out. Chosen over bare values so the shape does not change when a method gains an out parameter, and over arrays so keys are named (the descriptor's slot names).
- How a backend reports outs: return `LIST [ret, out...]`; ret is `NULL` for void; outs in parameter order. That is the only thing X1's GI invoker needs: for each GIArgInfo with direction OUT/INOUT, flag the slot, skip OUT args when reading JS args, and append the out values to the list.
- Qt generator: `out:`/`inout:` keys on a method entry (names of `T *` / `T &` params; types int/uint/bool/double/float/qint64/QString), `target: layout` for methods, calls go through the declaring class (QSpinBox re-declares `validate` as protected; the match now only takes public methods).
- Proofs: Rust backend (`out_and_inout_params_come_back_as_one_object`), Qt `QLayout::getContentsMargins` + `QAbstractSpinBox::validate`/`fixup` (counter-qt `serdes`), GTK `g_file_load_contents` etag (`gio-file.loadContentsEtag`).
