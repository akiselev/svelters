# NT2: Qt non-GUI types (N2 of plans/research/NON-GUI-TYPES.md)

Lane: Qt backend/build crates, `tools/metadata/qt`, `metadata/qt`, `metadata/overrides/qt`, `packages/types/qt`,
Qt docs pages, new fixtures. The GTK pilot it mirrors is `docs/notes/nt0-handoff.md`.

## Pipeline

```text
qdoc 6.11.0 indexes ─┐
PySide6 6.11 typesystems (core/gui/widgets) ─┤ ingest_types.py ─▶ metadata/qt/types/ingested/Qt{Core,Gui,Widgets}.json
moc metatypes (Qt 6.11.3) ─┤                  (raw, origin-tagged; checked in, 3.3 MB)
headers: `enum class` names ─┘
metadata/overrides/qt/types/qt-types.yaml ─┐
ingested ──────────────────────────────────┴─▶ types_qt_classify.py --report/--write/--check
                                                 ─▶ metadata/qt/types/Qt*.json + report.txt (pure: no Qt install needed)
facts ─▶ gen_types_cpp.py (via gen_cpp.py, in the counter-qt build) ─▶ C++ thunks over templates/types_core.hpp.in
facts ─▶ gen_types_js.py ─▶ packages/types/qt/{core,gui,widgets}/<Class>.js, <ns>.d.ts, manifest.json, docs pages
facts ─▶ gen_cpp.py Tests.types ─▶ generated ledger tests (svl_qt_verify.cpp) ─▶ metadata/verified/qt.json
```

- **Sources.** None of them is installed on a dev machine by default; `ingest_types.py` documents how to get them
  (`aqt install-doc linux 6.11.0`, `aqt install-qt linux desktop 6.11.3 linux_gcc_64 --archives qtbase icu`, PySide6
  6.11.0 from pip). Splitting ingest from classification means `--check` runs in `pnpm test:meta` without Qt.
- **sip**: PyQt6's annotations are GPL data, so they are not ingested; the one rule taken from them (a non-const
  `T *` of a basic or value type is an out parameter, a non-const `T &` an in-out) is applied as a rule.
- **libclang**: not run. Nested public-field structs (`QFont::Tag` 12 members, `QTextOption::Tab` 4,
  `QCalendar::YearMonthDay` 3, `QColorSpace::PrimaryPoints` 3) stay blocked with their reason in the report. `enum
  class` detection reads the headers with a regex instead.

## Design decisions

### Scope
Roots (60 classes in the overrides: QTimer, QSettings, QFile, QUrl, the models, QIcon, QFont, QTextDocument, ...)
plus the signature closure of their bound members plus ancestors: **118 classes** (QtCore 58, QtGui 59, QtWidgets 1).
The whole namespaces were not bound because every Qt member is a C++ case: generated size and the leaf-crate build
time grow with the scope (measured below). The roots list is the knob.

### Lifetimes (decision Q1, adapted to QObject)
- **QObject made by JS** (`construct`, or a constructor through `call_static(k, "new")`): `SHARED`. QObject has no
  refcount, so JS's reference *is* the ownership: `destroy` (dispose or GC) deletes a parentless object, and only
  forgets one that a QObject parent owns now (Q1's "parent ownership respected").
- **QObject returned by a getter**: `BORROWED` (QPointer + `destroyed`), as since X1. A result the PySide typesystem
  hands to the caller (`define-ownership owner="target"`) is promoted to `SHARED` when parentless.
- **Value classes**: `VALUE` handles with a kind; const members only (decision Q3: non-const members of value
  classes are hidden; residual hand list 0); `equals` via QMetaType equality. `QModelIndex` is held as a
  `QPersistentModelIndex`, so a removed row makes it invalid rather than dangling.
- **unique** (QMimeDatabase, QTemporaryDir, QXmlStreamReader, ...): `OWNED`, deleted on destroy through a
  type-erased deleter in the core. A borrowed pointer to a unique class from a getter is blocked.
- **item** (QStandardItem, QUndoCommand): made as `svl::Tracked<T>` (or `TrackedClone<T>`), whose destructor reports
  native death (`SVL_GONE_OWNER`). `OWNED` until a PySide `parent index="this" action="add"` argument fact moves it
  into the container (`BORROWED`); a `parent action="remove"` result gives it back. Models with
  `setItemPrototype` get a tracked prototype, so items the model clones are watched too. An untracked native item
  is refused (no safe way to know its lifetime).

### Overload dispatch (decision Q4)
One JS name per (class, method). The thunk tries the overloads in a fixed order derived from the facts
(`types_qt_classify.overload_key`): fewer required JS arguments first, then more specific JS types first (handles,
values, enums, flags, records, bool, integer before float, string, bytes, list, variant), then declaration order. An
overload is taken when the argument count fits and every argument passes `Accept<T>` (a trial conversion; `bool`
only from a JS boolean, integers only from integral numbers). Members with several overloads describe dynamic
slots, so plain objects arrive as maps (records accept maps by field name, flags a list of names).
- Measured: 2079 JS method names, 112 overloaded; 45 C++ spelling variants merged (QStringView next to QString,
  short/long next to int, float next to double: same JS arguments, same result); 2 const/non-const twins merged;
  **3 truly ambiguous**, renamed (`QPolygon.pointXY`, `QTextDocument.redoCursor` / `undoCursor`: same JS arguments,
  different result shape because of out parameters); 0 left ambiguous.
- Statics that share a name with an instance method are reached as `static:<name>`.

### Async mapping
`svl::async_from_future<T>` turns a `QFuture<T>` result into the host's one-shot completion (`SVL_M_ASYNC`):
`then(context)` resolves, `onFailed` rejects with a `NativeError`, `onCanceled` and `backend.cancel` reject with
`operation cancelled`. No bound Qt 6.11 member returns a `QFuture` (only QtConcurrent and platform APIs do), so the
generator support exists but binds nothing; the adapter is tested through a test hook.

### The boundary (section 2.4)
`templates/types_core.hpp.in` (hand-written) holds identity, lifetimes, the item tracker, the QMetaObject fallback
(props via QMetaProperty, signals via a relay on `QMetaObject::connect`, invokables via `QMetaMethod::invoke`, used
for every signal and prop of a bound class and for names without facts) and the async adapter. Generated code
calls only that, the converters and Qt: `tools/metadata/qt/test_types_cpp.py` rejects `delete`, `deleteLater`,
`QPointer`, ref/unref/retain, the core's tables and arena, and any `new` that is not `new <BoundClass>(`; the core
has a line budget.

## Completion record

Branch `worktree-agent-a0c7f1e79811c8505`, pushed as `ci/nt2`. Master (CL0, NT1, RT1, up to 1d39e648) merged in.
Gates and CI below were observed on **3e87d51f**; the commit after it only adds this record.

**Commits:** a5f4db77 (ingest + classifier), 128b3cfe (core, thunks, construct/call_static, JS modules, typings,
docs, boundary check), 1952af7a (fixtures, host/core tests, replay), the follow-ups up to 08ebaede (member table
order, dynamic fallback after getters, omitted optional arguments, unknown-member wording, CI bundles), fc553711
(ledger), the master merges b5d8a8f3 / 354fc05e, dcf3c7ba (post-RT1 fixtures), 3e87d51f (signal hook survives
later mounts).

**Gates (local, after the last code change):**
- `tools/dev/headless tools/dev/cargo-slot cargo nextest run --workspace --no-fail-fast`: **2082 passed, 0 failed,
  7 skipped**. Earlier runs that guided fixes: `counter::integration borrowed_walk_qt` (probe errors now say
  "unknown method") and `host_types native_models_feed_lists` after RT1 (see 3e87d51f).
- `counter-qt` alone: 64/64 before the master merge (types_core 9, host_types 3, generated incl. 38 type facts,
  oplog_replay replaying `qt-types` with its construct/static/call/event lines).
- `python3 tools/metadata/ledger.py refresh --toolkit qt`: 749 facts, 1826 tests, 0 manual / 0 inferred not
  passing, 0 parked; `ledger.py check`: qt 634 manual facts verified (595 before: +38 type facts, +1 `text-edit`
  `document` prop), inferred 115; gtk 550, appkit 79, winui 60 unchanged.
- `corepack pnpm -r build` (with a `pnpm` shim on PATH and `cargo build -p svelters-compiler` first): ok.
- `corepack pnpm test` (incl. `test:meta`: pytest 135 passed / 1 skipped, ledger check, coverage check): ok.
- `python3 tools/docs/build_ir.py` then `corepack pnpm docs:test`: unittest 11 (1 skipped) OK, site tests 17/17.
- `@svelters/types` node tests 19/19 and strict `tsc` on GTK + Qt typings.

**CI on `ci/nt2` @ 3e87d51f:** js 39336650956 success, linux-gtk 39336651034 success, linux-qt (Qt 6.11.3)
39336650902 success, macos-appkit 39336650906 success, windows-winui 39336651076 success.

**Classification** (`metadata/qt/types/report.txt`): all public classes — record 12, value 108, object 113,
model 12, statics 8, unique 33, item 23, transient 82, needs-override 8, unclassified 12, excluded 291,
widget 73. Bound: 118 classes (QtCore 58, QtGui 59, QtWidgets 1; 60 roots). Members: QtCore 1229/1336 (91%),
QtGui 1378/1488 (92%), QtWidgets 40/40; 412 hidden (value mutators and prop readers); overloads: 2079 JS names, 112
overloaded, 45 spelling variants merged, 2 const twins, 3 renamed, 0 ambiguous. Curated (ledger-tested): QTimer,
QStringListModel, QStandardItemModel, QStandardItem, QTextDocument, QUrl, QFileInfo, QIcon, QMimeDatabase,
QTemporaryDir.

**Generated size and leaf build time** (same machine, debug `-O0 -g`, single g++ per file, load ~4):
- `svl_qt_backend.cpp` 39,449 → 97,674 lines (1.83 → 4.72 MB); compile 64.9 s → 174.4 s.
- `svl_qt_verify.cpp` 24,539 → 25,521 lines; compile 94.6 s → 98.0 s.
- The leaf build script used to compile the two units one after the other (~160 s); it now compiles them in
  parallel (`svelters-qt-build`), so the leaf rebuild is bounded by the backend unit (~175 s + generator).
  `tools/dev/qt-leaf-timing` measures a forced rebuild (its wall time includes waiting for a cargo slot).
- JS: `packages/types/qt` 678 KB (137 files); a bundle imports only the classes it uses (shake test).

**ABI:** no change. The Qt backend implements the v2.5 members; conventions are recorded in
`plans/architecture/backend-abi.md` ("Qt non-GUI types (NT2, no ABI change)").

**Shared files touched outside the Qt lane:** `packages/types/src/core.js` / `core.d.ts` (constructor arguments
forwarded, `isProps`/`constructObject`, TestHost-shaped values, the dispatch hook kept in front of later runtime
installs — that last one also fixes GTK listeners registered before a ListView mounts), `packages/types/vite.js` and
`package.json` (qt aliases/exports via `tools/metadata/common/types_pkg.py`, used by both generators),
`tools/metadata/gtk/types_closure.py` (reads the Qt manifest too), `serdes.hpp.in` (records from maps, flags from
name lists, kinds on VALUE handles), `metadata/overrides/qt/QTextEdit.yaml` (`document` accessor prop; gen_cpp
accepts object-typed accessor props), `.github/workflows/linux-qt.yml` (builds `tests/qt-types-*`).

**Open items:**
- libclang pass for nested public-field structs not done (QFont::Tag 12 members, QTextOption::Tab 4,
  QCalendar::YearMonthDay 3, QColorSpace::PrimaryPoints 3 stay blocked).
- `async_from_future` has no Qt 6.11 member to bind (only QtConcurrent / platform APIs return QFuture); tested
  through `svl_qt_test_future`.
- The backend translation unit roughly tripled; splitting the type layer into its own unit (shared core header)
  would let it compile in parallel too. The roots list is the other knob.
- QMap/QHash/QSet parameters (29 members: `itemData`, `roleNames`, ...) and `QBindable` are blocked.
- `concrete:` / `platform_enum_members:` / `platform_members:` are hand lists where qdoc lacks the fact.
- Qt returned QObjects stay BORROWED (no refcount to take); only factory results become SHARED.
