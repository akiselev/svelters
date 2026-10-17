# P4 - Qt lane (metadata-generated C++)

Owns `tools/metadata/qt/`, `metadata/qt/`, `metadata/overrides/qt/`, `crates/svelters-qt-build` and `examples/counter-qt`.

The C++ backend is generated from metadata from the start. The generators are Python for now; their port to Rust (M3) lets `svelters-qt-build` run them in-process from the leaf crate's `build.rs`. Keep them deterministic and dependent only on their inputs.

## Work
1. **Ingest:**
   - `tools/metadata/qt/ingest.py` merges three sources: the qdoc `.index` XML (full public API: signatures, default args, virtuals, property accessors), a QMetaObject dump via PySide6 (properties/signals/slots/enums), and the PySide typesystem XML (ownership rules).
   - It emits `metadata/qt/<Class>.json` in the `svelters-element-kind/v0` schema, with an `origin` on every fact.
   - Start from `docs/notes/scripts/qdoc_index_stats.py` and `qt_metaobject_dump.py`.
   - Generate all of QtWidgets. Only the P kinds must be correct and tested.
2. **Annotate:** `metadata/overrides/qt/*.yaml` holds child policy and slots (including the uic-derived QMainWindow mapping and layout-managed children), ownership/replace semantics (`setCentralWidget` releases the old widget), signal overload collapsing (`clicked()`/`clicked(bool)`), and element tag names.
3. **Generate:** `tools/metadata/qt/gen_cpp.py --config <toml> --out <dir>` emits:
   - one C++17 translation unit implementing `svl_backend`;
   - `resolve` tables;
   - a `create` switch;
   - a `set_prop`/`get_prop` switch: direct setters where known, `QMetaProperty` fallback;
   - a `call` switch for selected methods;
   - slot insert/remove ops (`setCentralWidget`, `QBoxLayout::insertWidget`, ...);
   - signal connectors using `QObject::connect` with lambdas that marshal arguments to `post_event`.
   It also emits `.d.ts` into `packages/dom/types/qt.d.ts`.
4. **Build:** `svelters-qt-build` is a `Config` builder for a leaf crate's `build.rs`:
   - for now it runs the Python generator into `OUT_DIR`, then compiles with `cc`;
   - Qt is found via pkg-config `Qt6Widgets`; avoid `Q_OBJECT`/moc in P;
   - it emits link directives and `rerun-if-changed` only for the build config, `metadata/qt/**`, the overrides and the generator scripts.
   `examples/counter-qt` is the leaf crate exposing `backend() -> &'static svl_backend`.
5. **Verify:** the generator emits a C++ or Rust test per annotated slot/ownership/signal fact, e.g.:
   - replacing the central widget destroys the old one (checked through `QPointer`);
   - layout insertion order matches;
   - `clicked` is delivered once.
   The tests run under `xvfb-run`.

P kinds that must be correct: `window` (QMainWindow, default slot central plus `menubar`/`statusbar`), `box` (QWidget + QBoxLayout by `orientation`, ordered), `label` (QLabel), `button` (QPushButton, `clicked`), `entry` (QLineEdit, `textChanged`).

## Acceptance
- Generator output is reproducible, i.e. a byte-identical rerun.
- The generated verification tests pass under `xvfb-run`.
- Editing a Rust source in a dependent crate doesn't rerun the leaf crate's build script. Check with `cargo build -vv`.
- The counter window appears from a hand-written op sequence.
