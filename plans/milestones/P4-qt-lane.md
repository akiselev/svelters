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

## Completion record

Versions: Qt 6.11.0, PySide6 6.11.0, Python 3.14.4, rust 1.97.0, cc 1.x. All cargo runs used `CARGO_BUILD_JOBS=4`. Notes: `docs/notes/p4-qt-lane.md`.

```text
$ python3 -I tools/metadata/qt/ingest.py          # 192 classes (115 QObject), rerun: 0 files changed
$ python3 -I tools/metadata/qt/gen_cpp.py --config tools/metadata/qt/gen.dev.toml --out A   # twice, and via build.rs OUT_DIR
$ diff -r A B  ->  byte-identical (dev config, rerun, build.rs OUT_DIR)
$ xvfb-run -a cargo test -p counter-qt
counter demo ok: 3 clicks delivered, label updated, screenshot at target/p4-counter.png
... 41 tests, 0 failed        # 6 runtime + 35 generated (props, methods, signals, slots, ownership)
```

- Reproducible: yes (see above). Generated verification tests: 41 pass under `xvfb-run` (QT_QPA_PLATFORM=xcb).
- Counter window: `examples/counter-qt/tests/counter.rs` replays the ops of `fixtures/counter/expected.oplog` through the ABI, drives three `click` calls inside `run()` from a `wake()`-queued checkpoint, checks the `clicked` events reach a stub `svl_host`, and writes `target/p4-counter.png` (label "Count: 3" and an "Increment" button).
- Leaf-crate property, `cargo build -vv` (count of `Running .../build-script-build` for counter-qt after each edit):

```text
baseline (no change)                                    build-script runs=0  counter-qt fresh
touch svelters-abi/src/lib.rs (its dependency)          build-script runs=0  (counter-qt lib recompiled, script not rerun)
touch svelters-host + counter sources (other crates)    build-script runs=0  counter-qt fresh
workspace build after touching abi/host/counter         no counter-qt build-script run (Dirty: only "dependency svelters-abi was rebuilt")
dependent crate (own workspace, bin -> counter-qt):
  first build                                           build-script runs=1
  edit the dependent's main.rs                          build-script runs=0  counter-qt fresh
touch metadata/overrides/qt/QLabel.yaml                 build-script runs=1
touch metadata/qt/QLabel.json                           build-script runs=1
touch tools/metadata/qt/gen_cpp.py                      build-script runs=1
```

The script only emits `rerun-if-changed` for `metadata/qt`, `metadata/overrides/qt`, `tools/metadata/qt`, `tools/metadata/common/svmeta.py` and the ABI header (plus `rerun-if-env-changed` for `SVELTERS_PYTHON`, `PKG_CONFIG_PATH` and the cc crate's compiler variables).

- Not done: `headerbar` (if-toggle stays GTK-only in P5); annotations beyond the P kinds plus `scrolledwindow`/`menubar`/`statusbar`; `qt.d.ts` is emitted to OUT_DIR only.
