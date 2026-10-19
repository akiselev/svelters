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

(filled in below)
