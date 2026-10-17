# S2: ABI v3, the remainder

Picks up from `docs/notes/s1-handoff.md` (S1 merged at 8071bb2). Notes are written incrementally to `docs/notes/s2-abi-v3-rest.md`. ABI changes stay additive within v3 and are documented in `plans/architecture/backend-abi.md` first.

## Steps (one commit each)

1. **Out and in-out params.** Host: OUT params are not sent, the backend answers `LIST [ret, outs...]`, the host returns `{ret?, name: value...}`. Qt: `out:`/`inout:` override keys, `target: layout` for methods. GTK: `gio-file` `loadContentsEtag` as the real GIO proof; the contract is generic so X1's GI invoker can use it. Tests: Rust test backend, Qt `QLayout::getContentsMargins` (out) and `QAbstractSpinBox::validate/fixup` (in-out), GTK `g_file_load_contents`.
2. **Qt callbacks and async.** Generator support for `CALLBACK` params and async methods; a real Qt API proving each.
3. **Qt UTF-16 strings.** `SVL_CAP_UTF16_OUT` for QString returns (`Value::Str16` / host fast path), measured with `setprop_bench`.
4. **GTK GVariant exact typing and GValue props.** `variant(sig, v)` helper; `GParamSpecValue` props.
5. **`packages/dom` INT_PROPS.** Remove the duplicate; derive from descriptors or generated tables.
6. **Handoff leftovers.** `backend-abi.md` v3 section, `serdes-impl.md` "Open", VALUE-class handles if BOXED + dispose does not cover them, A1 gaps (G4 value types; closed ids are noted in the notes).

## Gates

`pnpm test`, `pnpm docs:test`, `CARGO_BUILD_JOBS=2 xvfb-run -a cargo nextest run --workspace --test-threads 2` before each merge-worthy commit; targeted tests while iterating.
