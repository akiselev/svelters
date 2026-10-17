# P4 Qt lane: working notes

Scope: `tools/metadata/qt/`, `metadata/qt/`, `metadata/overrides/qt/`, `crates/svelters-qt-build/`, `examples/counter-qt/`.

## Pipeline

```text
qdoc .index (core/gui/widgets) --+
PySide6 QMetaObject dump --------+--> tools/metadata/qt/ingest.py --> metadata/qt/<Class>.json   (192 classes, 6 MB, checked in)
PySide typesystem (ownership) ---+
metadata/qt/*.json + metadata/overrides/qt/*.yaml + templates/*.in
        --> tools/metadata/qt/gen_cpp.py --config <toml> --out <dir>
        --> svl_qt_backend.cpp  (one C++17 TU implementing svl_backend)
            svl_qt_verify.cpp   (one test per annotated fact; svl_qt_verify_run())
            qt.d.ts             (element descriptors; NOT written into packages/dom, see open issues)
crates/svelters-qt-build::Config  (build.rs of the leaf crate) runs gen_cpp.py into OUT_DIR, compiles with cc + pkg-config Qt6Widgets
examples/counter-qt  (leaf crate): backend() -> &'static svl_backend; tests/generated.rs, tests/counter.rs
```

Determinism: sorted keys/records; no clocks, env or absolute paths in output; an `inputs-digest` line (sha256 over the metadata/overrides/templates/script actually read) heads each generated file. Rerun, a different config path and the `build.rs` OUT_DIR all give byte-identical output.

## What is generated vs hand-written

Generated from metadata + overrides: kind/prop/signal/method/slot ids and the `resolve` tables, `create` (incl. composite box and `init` calls), `set_prop`/`get_prop` (direct setters from accessor facts, enum maps, `QMetaProperty` fallback for types without direct accessors), `call`, slot `insert`/`remove` (from op descriptors), `listen` connectors, the whole verification file, `qt.d.ts`.

Hand-written, shared by every build (`tools/metadata/qt/templates/*.in`): the handle table (QPointer + `destroyed` connection), error plumbing, `svl_value` conversions, vtable glue, `run`/`wake`/`quit`, and the test harness runtime tests (stale handles, native destroy, parent destroy, queued wake, wrong value type).

## Schema extensions (all additive, in overrides only)

- `elements[]` (per override file, keyed by the class it constructs): `tag`, `construct {class, composite.layout {class,args}, init[{method,args}]}`, `props[] {js, native | (type: enum, getter, setter, values), target: layout, suppress_signals}`, `methods[] {js, native, params}`, `signals[] {js, native, connect, collapses, args, trigger, expect_args}`, `children {...}`. An element is one (class, tag) pairing, so `box` lives in `QWidget.yaml`.
- `svmeta.validate`: overloaded records are identified by `id` when every record has one (additive; the shared `merge` is unchanged and still keys by name, so I do not merge overrides into kinds; `gen_cpp.py` reads the `elements` list directly).
- Ingested kinds carry extra optional fields: `qobject`, `ownership[]` (typesystem facts), signals' `metaobject_signatures`, properties' `metaobject`.

## Slot annotation format

Follows `docs/notes/signals-and-slots-semantics.md` section 2.5: `children {policy, default_slot, slots[]}` with `content`, `accepts`, `route_by_kind`, `cardinality`, `order`, `default`, `on_remove`, `ops{get,set,insert,remove,list}` as descriptors `{native: "Class::method(argtypes) [const]", args: [child|before|index|null|const:<lit>], variants[{when,native,args}], effects{child,old}, then[]}`, `provenance`.

What the generator validates: `native` parses and the method exists with that arity on the named class (when the class has metadata); `on_remove` agrees with `remove.effects.child`; typesystem `replace_child` is cross-checked (reported in the test comment). What the effects become: tests for `set`+`get` read-back, replace (`old: deleted` -> QPointer null after DeferredDelete and `on_destroyed` fired; `unparented` -> alive with null parent), remove (`child: unparented` -> alive, parent null, re-insertable; `deleted` -> QPointer null + `on_destroyed`), ordering (`before`, move of an existing child, `before == self`), default slot as slot 0, wrong-kind rejection, parent destroy deleting children.

Small extensions to section 2.5: `then` (extra steps after an op, e.g. `QWidget::setParent(nullptr)` after `removeWidget`), `receiver: child` on a descriptor (YAML 1.1 turns a key named `on` into a boolean), `index_of` op, and `list: {native: count, item: itemAt}` as the `many` read-back. The test DSL (`verify.steps`), `attrs`, `requires` (parsed, not enforced) and `restore` are not implemented.

## Facts re-checked by the generated tests

- `QMainWindow::setCentralWidget/setMenuBar/setStatusBar` delete the old widget; `takeCentralWidget` unparents. `setMenuBar(nullptr)`/`setStatusBar(nullptr)` delete the bar. `menuBar()`/`statusBar()` create one on demand, so the getters are `menuWidget()` and `findChild<QStatusBar*>(Qt::FindDirectChildrenOnly)`.
- `QScrollArea::setWidget` deletes the old widget, reparents the content into the viewport (so ancestor, not parent, checks), `takeWidget` unparents.
- `QBoxLayout::removeWidget` leaves the child parented: the descriptor's `then` step reparents to null. `insertWidget` of an existing child would duplicate the item, so the backend removes first (move semantics).
- `QAbstractButton::clicked` has one signal in Qt 6 but two metamethods; one connection to `clicked(bool)` delivers exactly once.

## P1 contract notes (fixtures merged from master)

`call present []` -> `show`; `destroy` children-first works (stale destroy is a no-op); a removed handle stays alive under the handle table until `destroy` and can be inserted again; `spacing` takes an integer, `orientation` a string; programmatic `entry.text` sets do not emit `changed` (`suppress_signals` -> `QSignalBlocker`); `scrolledwindow` (QScrollArea, `child` slot) was added because keyed-list needs it. `headerbar` is not implemented: `create`/`resolve` return 0/error with a message for unknown kinds, slots and props.

## Open issues

- `changed` posts `[text]` (the P1 shim mirrors it); `clicked` posts `[]`.
- `qt.d.ts` is generated into OUT_DIR only; placing it at `packages/dom/types/qt.d.ts` is outside this lane's directories.
- Only the P kinds (plus `scrolledwindow`, `menubar`, `statusbar`) are annotated; the other 185 classes are ingested, not annotated.
- No `headerbar`. A QToolBar-based mapping (window slot `titlebar` -> `addToolBar(TopToolBarArea)`) is the obvious route if P5 wants if-toggle on Qt.
- Property type mapping in ingest is textual (`neutral_type`); enums and flags from `qdoc` are `enum<...>` only when the spelling matches an enum name.
- The generated tests assume Qt 6.11 behaviour; a Qt upgrade should rerun them (they are the regression net for the ownership facts).
- `HEADER`: `svelters-qt-build` locates `crates/svelters-abi/include` from the repo root; a project outside the repo would need `repo_root()` or an `abi_include_dir` option.
