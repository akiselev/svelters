# Non-GUI types: a hand-written object core, everything else generated from metadata

Status: research proposal, 2026-10-17. It was revised the same day to put metadata and automation at the centre and to draw the hand-written/generated boundary.

- **Working notes and raw numbers:** `docs/notes/types-research.md`.
- **Prototypes and probes** (`docs/notes/scripts/`):

  | Script | Covers |
  |---|---|
  | `types_gir_classify.py` | GTK auto-classifier |
  | `types_qt_classify.py` | Qt auto-classifier |
  | `types_objc2_classify.py` | Apple auto-classifier |
  | `types_gir_inventory.py`, `types_qt_inventory.py` | what the widget APIs use today |
  | `types_gi_callable_blockers.py` | GI invoker tiers |
  | `types_qt_reflect_probe.cpp` | what QMetaObject/QMetaType reach |

- **Builds on:**
  - ABI v3 value model (`plans/architecture/backend-abi.md`, `docs/notes/js-native-serdes.md`);
  - X1 borrowed handles;
  - X7 non-widget kinds;
  - X8 Gdk values;
  - `docs/notes/ownership-and-lifetimes.md`;
  - `plans/research/CUSTOM-WIDGETS.md` (reflective engines, packs);
  - `plans/research/QUICKJS-RUNTIME.md` (built-in `svelters:*` modules).
- **Widget annotation automation** is a separate lane (`docs/notes/annotation-automation-research.md`). This document covers non-GUI types only. The two share the facts schema and the ledger.

## Summary

- **The boundary:**
  - **Hand-written, per toolkit:** a small object-model core of about 2-3k lines each. It covers handles and lifetime classes, identity maps, refcount and weak-ref plumbing, GC integration, boxed copy/free and value tables, the marshalling primitives (GValue/GIArgument, `Conv<T>`/QVariant, HSTRING/IInspectable, NSValue/blocks) and thread checks.
  - **Generated from metadata, with rule-based exclusions and small override files:**
    - per-type classification;
    - constructors, statics, methods, props, signals, fields;
    - record conversions;
    - JS modules, typings, docs and verification tests.
  - **The rule for generated code:** it calls only the core API and the toolkit's own API. It never refs, unrefs, deletes or keeps a map. A CI check enforces this (§2.4).
- **Every toolkit has a machine-readable source of truth, and most of it is already on this machine:**

  | Toolkit | Sources |
  |---|---|
  | GTK | GIR plus the typelib |
  | Qt | qdoc indexes; PySide6 typesystem XML (value-type vs object-type, ownership, removals); moc metatypes JSON; PyQt6 sip annotations; libclang for the remainder |
  | Apple | objc2's header-translator output (super chains, main-thread-only, Send, NSCopying, safe vs unsafe, blocks); MAC1's libclang pass |
  | WinUI | winmd |

- **Rules classify most of the surface automatically.** The prototypes report:

  | Toolkit | Members auto-marshalled | Types needing an override entry |
  |---|---|---|
  | GTK | **96% of 5990** members of bindable types | 26 in the Gtk-reachable scope |
  | Qt | **92% of 7560** members (QtCore/QtGui and QtWidgets non-widgets) | **8**, plus a 17-entry record-shape list and one exclusion regex |
  | Apple | **82% of 9697** methods objc2 marks safe | dozens of misclassifications to correct |
  | WinUI | not run | — |

  WinUI rules are specified but the classifier wasn't run, because its winmd reader needs a windows-bindgen build.
- **JS sees two forms, unchanged from the first draft:**
  - plain records, by value;
  - handle-backed instances of generated classes, with one JS object per native object.
- **ABI impact is small and additive:**
  - a new `SHARED` lifetime class;
  - nullable `construct` and `call_static` members;
  - an `SVL_T_TYPE` well-known type;
  - `SVL_M_STATIC`/`SVL_M_CTOR` member flags;
  - VALUE handles gain kinds and methods;
  - every native object crosses as a handle.
- **Pilot: GTK.** The core is about 70% written already (`convert.rs`, `gi.rs`). The generated layer there is data plus JS, with no per-type native code at all.

## 1. Inventory: what the widget APIs need today

Measured from the GIR and from `metadata/qt` (`types_gir_inventory.py`, `types_qt_inventory.py`). It counts slots (props, params, returns, signal args) whose type is neither primitive, string, enum nor widget.

- **GTK (Gtk-4.0 API):** 263 types, 1422 slots.
  - **Handles:** 473 slots, all of them Gtk-namespace kinds such as Adjustment, ListItemFactory and Expression.
  - **Opaque GObjects:** 286 slots (GListModel 61, GObject 39, GMenuModel 36, GCancellable 20, GdkDisplay 15, GdkClipboard 8). Each crossing mints a new handle, so they have no identity, and their members are unreachable.
  - **Opaque GBoxed:** 269 slots (TextIter 96, Bitset 16, PaperSize 14, PangoAttrList 13, GBytes 10).
  - **String form in, opaque out:** 101 slots (GFile 39, GdkPaintable 22, GIcon 12).
  - **Supported:** records 61, GStrv/variant/datetime 84.
  - **Rejected:** callbacks 79, arrays/lists/structs without a GType 68.
- **Qt (195 classes, 73 widgets):** 179 types, 2297 slots.
  - **Rejected `opaque`:** 746 slots (QModelIndex 92, QGraphicsItem* 92, QTreeWidgetItem* 56, QPainter* 34, `T*` out pointers 37).
  - **Records:** 589.
  - **Opaque boxed:** 407 (QIcon 63, QFont 50, QPainterPath 48, QUrl 29).
  - **QObject handles:** 311 (111 of them have no kind).
  - **Missing from metadata entirely:** the QtCore objects QTimer, QSettings, QFile and the item models.
- **WinUI/AppKit:** only P-scale samples exist (15 and 11 classes). The non-GUI types they reference are Thickness, Brush, Uri, Vector3, CornerRadius, NSSize/NSRect, NSColor, NSImage and NSAttributedString.

## 2. The boundary: hand-written core, generated everything else

### 2.1 What the core is

The core is code whose correctness depends on toolkit memory-management rules that no metadata states completely: refcount transfer edge cases, floating refs, toggle versus weak refs, `deleteLater` timing, COM apartment rules and ARC. It is written once per toolkit, reviewed and tested on its own, and does not grow with the number of types.

The toolkit-independent parts already exist:

- **Host (Rust):** handle table, lifetime classes, dead-handle ring, release queue drained at checkpoints, callbacks.
- **JS runtime:** `NativeBoxed`, `nodeFor`, the FinalizationRegistry release, `StaleHandleError`.

This proposal adds about 300 lines to the host (SHARED release on GC, `construct`/`callStatic` ops, `SVL_T_TYPE` encoding). It also adds about 400 lines of JS for the shared base classes:

- **`NativeObject`:**
  - `wrap(h)` through a WeakRef map, so each handle gets one wrapper;
  - `dispose` and `Symbol.dispose`;
  - `on(signal, fn)` registered in the component scope.
- **`NativeValue`:** `equals`, `toString`, `dispose`.

### 2.2 Core API per toolkit

**GTK** (Rust, `crates/svelters-gtk/src/core/`; about 3.0k lines, about 2.2k exist today as `convert.rs` 944, `gi.rs` 778, `types.rs` 193 and the identity parts of `backend.rs`)

| Function group | Responsibility | Lines |
|---|---|---:|
| `handles::for_object(&Object, Transfer) -> Handle`, `object(h)`, `release(h)` | qdata identity. Lifetime decision: widget → BORROWED/OWNED as in X1; non-widget → SHARED (strong ref). Weak notify → `on_destroyed`. Floating refs sunk. | 450 |
| `handles::for_value(GType, ptr, Transfer)`, `value_ptr(h, GType)` | VALUE handles. `g_boxed_copy`/`g_boxed_free`, or a fundamental's ref-func/unref-func found from the typelib, so there are no per-type cases | 150 |
| `marshal::to_gvalue`/`from_gvalue` | props and signal args, keyed by GType (exists) | 950 |
| `invoke::call(info, receiver, args)` | `gi_function_info_invoke`. GIArgument in/out/inout, caller-allocates scratch, arrays with a length, GList/GSList, string hash tables, `transfer` handling, GError → `ERROR` | 900 |
| `invoke::closure(callable_info, scope)` | callback trampolines (`gi_callable_info_create_closure`); call/async/notified scopes → `release_callback` | 250 |
| `invoke::async(info, finish_info)` | `_async` + `_finish` → one-shot `CALLBACK`; GCancellable ↔ `cancel` | 150 |
| `reflect::kind_for(GType)` | lazy reflected kinds: GParamSpecs, `g_signal_query`, typelib methods, constructors, statics; snake/camel resolution | 300 |
| `construct`, `call_static` | `g_object_new_with_properties`; static functions | 100 |

**Qt** (C++ in `tools/metadata/qt/templates/`; about 2.2k lines, about 1.3k exist today as `serdes.hpp.in` 974 and the handle/identity parts of `backend.cpp.in`)

| Function group | Responsibility | Lines |
|---|---|---:|
| `svl::handle_for(QObject*, Life)`, `svl::object<T>(h)` | QObject identity map erased in `destroyed`; QPointer for BORROWED; scope ownership and `deleteLater` for OWNED; `qobject_cast` check | 400 |
| `svl::value<T>(T)`, `svl::value_of<T>(h)` | VALUE table of `QVariant`. Copy, destroy and `equals` go generically through QMetaType, so there's no per-type code | 120 |
| `svl::unique<T>(T*)`, `svl::item<T>(T*)` | non-QObject C++ objects. `unique` uses a type-erased deleter and is JS-owned. `item` is an ownership token that moves to and from the container; the generator applies the facts that come from PySide/sip | 150 |
| `svl::Conv<T>` | scalars, UTF-16 strings, enums/flags, `RecConv` records, QList/QMap/QHash, QVariant, dates, bytes, object pointers, `std::function` from `CALLBACK`, `T*`/`T&` outs (exists) | 1000 |
| `svl::async_from_signal`, `svl::async_from_future<T>` | S2 signal adapter (exists); `QFuture::then(context)` | 150 |
| `svl::meta::*` | QMetaObject fallback for classes without generated facts: props, signals, invokables through `QMetaMethod::invoke` with QMetaType-created arguments | 350 |

**WinUI** (C++/WinRT; about 2.0k lines, none written)

- **Identity:** keyed by the default interface's `IUnknown*`. SHARED holds a strong ref, so the key can't be reused while it's held. BORROWED holds a `winrt::weak_ref`.
- **`Conv<T>`:** HSTRING, scalars, enums/flags, structs (`RecConv` over winmd field order), `IReference<T>`, `IVectorView`/`IIterable` → list, `IInspectable`/`PropertyValue` boxing, delegates ↔ `CALLBACK` with `auto_revoke` tokens, HRESULT → `ERROR`.
- **Async:** `IAsyncOperation`/`IAsyncAction` → completion, resumed on the DispatcherQueue.
- **Threading:** `DispatcherQueue::HasThreadAccess` checks.
- **Activation and statics:** go through C++/WinRT's own `get_activation_factory`, so the core only maps their errors.

**Apple** (Rust over objc2, or ObjC++; about 2.5k lines, none written)

- **Lifetime:** retain/release.
- **Identity:** an associated-object sentinel; its `dealloc` reports death for BORROWED.
- **Threading:** MainThreadOnly checks.
- **Invoker:** a dynamic libffi `objc_msgSend` driven by `method_getTypeEncoding`, which is the dev path.
- **Conversions:** NSString, NSNumber, NSValue structs, NSArray/NSDictionary/NSSet, NSData, `NSError **` → `ERROR`, `Class` ↔ `SVL_T_TYPE`, nullability.
- **Blocks:** completion-handler blocks built as block literals whose invoke is a libffi closure (NativeScript's technique).

### 2.3 What is generated, and what it calls

| Artifact | GTK | Qt | WinUI | Apple | Calls |
|---|---|---|---|---|---|
| Facts JSON (`metadata/<tk>/types/*.json`): classification, lifetime, JS names, exclusions, record shapes, async folding, out directions, ownership | yes | yes | yes | yes | — (data) |
| Backend tables (policy for the reflective core: excluded members, lifetime overrides, record field maps) | `nongui.generated.rs` | part of the C++ | part of the C++ | `.generated.rs` | read by the core |
| Native member thunks | none in dev (reflective); release thin thunks (M6) | one `case` per member | one `case` per member | none in dev (reflective); release thunks (M6) | `Conv<T>`, `value_of`, `object<T>`, `handle_for`, `unique`, `fail` and the toolkit method itself |
| JS modules `svelters:gtk/gio`, `svelters:qt/core`, `svelters:winui/<ns>`, `svelters:appkit/<fw>` | yes | yes | yes | yes | host ops (`construct`, `callStatic`, `call`, `getProp`, `setProp`, `listen`, `destroy`) and `NativeObject`/`NativeValue` |
| `.d.ts`, docs cards, verification tests, ledger entries | yes | yes | yes | yes | — |

A generated Qt thunk, in full:

```cpp
case M_QUrl_host: {                                   // QString QUrl::host(QUrl::ComponentFormattingOptions) const
  const QUrl *self = svl::value_of<QUrl>(h);  if (!self) return svl::fail("stale QUrl");
  QUrl::ComponentFormattingOptions a0;
  if (!svl::Conv<decltype(a0)>::in(argc > 0 ? &args[0] : nullptr, a0, err)) return svl::fail(err);
  svl::Conv<QString>::out(self->host(a0), out);  return 0;
}
```

The same thunk for a QObject receiver uses `svl::object<QTimer>(h)`. A constructor returns `svl::value(QUrl(a0))`, `svl::unique(new QXmlStreamReader(a0))` or `svl::handle_for(new QTimer, svl::OWNED)`. These are the only forms.

### 2.4 Keeping the boundary honest

- **Compile-time check:** generated C++ includes only `svl_core.hpp` and toolkit headers.
- **Text check:** a generator test rejects generated sources containing `delete `, `new ` (outside the ctor forms above), `ref(`, `unref(`, `Release(`, `retain`, `QPointer`, `g_object_` or `CFRetain`.
- **Rust:** generated Rust tables are `const` data with no `unsafe`.
- **Reflection-first toolkits** (GTK, Apple): their generated part is data and JS only, so the core is the only native code by construction.

### 2.5 How much of the surface the generated layer covers

The member-level coverage is in §3.3: GTK 96%, Qt 92%, Apple 82%. Everything not covered falls into three groups:

- **Excluded by rule:** deprecated, non-introspectable, platform-native, templates, C++ operators (equality goes through the core's `equals`), special members.
- **Blocked by a type that needs an override entry:** listed in §3.3.
- **Deliberately unexposed:** transient types (iterators, painters, events, style options).

## 3. Metadata sources and the classification pipeline

### 3.1 One taxonomy, rules per toolkit

| Category | JS form / lifetime | GTK rule (GIR) | Qt rule | Apple rule (objc2) | WinUI rule (winmd) |
|---|---|---|---|---|---|
| **record** | plain object, by value | record with public, plain-number/enum/record fields, not opaque or disguised | value-type in the record-shape list (17); nested public-field structs via libclang | `#[repr(C)]` struct with pub fields | struct |
| **value** | VALUE handle, methods, `equals` | boxed record with a GType (copy/free, or ref/unref methods); fundamental with a ref-func up the chain (GskRenderNode, GdkEvent, GtkExpression, GParamSpec) | PySide `value-type` | has an `NSMutableX` counterpart; or NSCopying with no property setters and not MainThreadOnly | — (WinRT has no value classes; Uri is a runtime class) |
| **object** | SHARED (refcounted) / OWNED (JS-created QObject) / BORROWED | class or interface deriving from GObject | QObject in the qdoc bases or moc `object` | other NSObject subclasses | runtime class |
| **model** | object + `length`/`at`/iteration/`items-changed` | is or implements `Gio.ListModel` | QAbstractItemModel subclass | (NSArrayController and the like: override) | implements `IObservableVector`/`IVector`/`IMap` |
| **statics** | class with only static members | (functions on a namespace) | only static members, no public ctor (QStandardPaths, QDesktopServices) | class methods only | static-only class |
| **unique** | OWNED, deleted on dispose or scope end | — | PySide `object-type`, not a QObject, public ctor (QXmlStreamReader, QCommandLineParser, QTemporaryDir) | — | — |
| **item** | handle whose ownership moves into a container | — | `*Item`/`*Command` object-types; PySide/sip ownership facts say when ownership moves | — | — |
| **async** | `Promise` + `{signal}` | `glib:finish-func` pairs; GAsyncResult/GCancellable/GTask hidden | QFuture; signal-completed calls (S2 adapter, an override) | DynBlock param named `*handler`/`*completion*` | `IAsyncOperation*`/`IAsyncAction*` |
| **transient** | not exposed (offsets, persistent indexes, callback-only) | name `*Iter`, TreePath/TreeRowReference | QEvent/paint/style-option families | — | — |
| **excluded** | never bound | class structs, `*Private`; outside the scope closure | one machinery regex (templates, smart pointers, iterators, locks, JNI/Mac/Vulkan/RHI/OpenGL, accessibility) | objc2 `skipped`, `unsafe` | `Deprecated` attribute; `Windows.UI.Xaml` (system XAML) |
| **enum/flags, callback** | names / name arrays; JS function | enumeration/bitfield; callback | qdoc enums, QFlags | NS_ENUM/NS_OPTIONS newtypes | enum (`Flags` attribute); delegate |

Rules that can't be automated:

- **Mutability.** C doesn't spell it reliably: 140 of 182 GTK value types take a non-const `self` somewhere, including GBytes and GskRenderNode, which are immutable. So VALUE handles expose every generated method, and "snapshot" semantics come only from `transfer` (copy-boxed types copy; refcounted boxed types share). Docs flag the known traps (PangoAttrList shared with a label) from an override note.
- **Record shapes.** These are a JS design decision, not a fact: which value structs become the shared `Rect`/`Size`/`Point`/`Insets`/`Color` shapes. Other plain structs become records with their native field names, camelCased.

### 3.2 Sources

| Toolkit | Source of truth | Gives | Ingest status |
|---|---|---|---|
| GTK | GIR (`/usr/share/gir-1.0`), typelib at runtime | types, fields, GType, fundamentals and their ref-funcs, `transfer`, `caller-allocates`, `scope`, nullable, `glib:finish-func`, error domains, deprecation | `tools/metadata/gtk/ingest.py` covers Gtk only. Parameterise the namespace, as CUSTOM-WIDGETS §7 proposes |
| Qt | qdoc indexes (`/usr/share/doc/qt6/*/*.index`) | classes, bases, every public function with spellings, properties, enums, `static`, status | parsed for qtcore/qtgui/qtwidgets already (`parse_qdoc`) |
| Qt | PySide6 typesystem XML (`/usr/share/PySide6/typesystems`) | value-type 455 / object-type 1176 / primitive 54 / container 9; 34 class rejections; removed members; 483 ownership modifications | `parse_typesystem` reads `typesystem_widgets_common.xml` only; extend it to `core` and `gui` |
| Qt | moc metatypes JSON (`/usr/lib/qt6/metatypes/qt6*_metatypes.json`) | 2764 Q_OBJECT / 590 Q_GADGET with props, signals, slots, invokables | new; also covers app/pack classes through `moc --output-json` (CUSTOM-WIDGETS) |
| Qt | PyQt6 sip (`PyQt6/bindings`) | `/Transfer/` 239, `/TransferThis/` 304, `/TransferBack/` 45, `/Factory/` 69, `/KeepReference/` 71, `/In/` 26, `/Out/` 21. Its default rule (a non-const `T*` of a basic type is an out param) settles the 121 out-pointer blockers | new; cross-checks PySide |
| Qt | libclang over `/usr/include/qt6` (python-clang 21 is installed) | public-field nested structs qdoc doesn't model (QFont::Tag, QTextOption::Tab, QCalendar::YearMonthDay, QTextLayout::FormatRange), typedef resolution | new; shares its setup with MAC1's libclang pass |
| Apple | objc2 header-translator output (`objc2-foundation`, `objc2-app-kit` crates: generated Rust, MIT/Apache, readable on Linux), plus its `translation-config.toml` | super chains, `MainThreadOnly`, Send/Sync, NSCopying, property setters, objc2-safe methods (a reviewed whitelist), block params, structs, skip lists | new (`types_objc2_classify.py`); MAC1's `tools/metadata/appkit/ingest.py` stays the source for nullability/availability on a Mac |
| WinUI | winmd: Windows App SDK packages (`fetch.py`) + `Windows.winmd` (the `windows-default` crate) | every type and its attributes (activatable, statics, composable, exclusiveto, default interface, deprecated) | `winmd-tool dump` covers WinUI classes; extend it to Windows.Foundation and the Microsoft.UI non-XAML namespaces |

### 3.3 Measured: what the prototypes classify

**GTK** (`types_gir_classify.py`; GLib, GObject, Gio, Gdk, Gsk, Pango, Graphene, Gtk):

| | record | value | object | model | async-infra | transient | needs-override | enum | flags | callback |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| types | 26 | 182 | 326 | 24 | 3 | 8 | 97 | 244 | 104 | 159 |

- **Members of bindable types:** 5990, of which **5770 (96%) marshal generically**.
  - By namespace: Gio 97%, Gdk 97%, Gtk 97%, Gsk 94%, Pango 94%, Graphene 100%, GLib 90%, GObject 83%.
  - Excluded automatically: 390 deprecated or non-introspectable, 225 ref/unref/copy/free/qdata (the core's job), 150 `_finish` (folded into the async method).
  - Blocked: 220. Causes: needs-override types 75, gpointer 62, TextIter 36, non-string hash tables 17.
- **Scope policy.** Bind the closure of non-Gtk types reachable from the Gtk API: GLib 12, GObject 6, Gio 40, Gdk 28, Gsk 10, Pango 19, Graphene 16. Plus Gtk itself and a short explicit root list (GSettings, GApplication pieces).
  - The 97 needs-override types are mostly GLib/GObject internals outside that scope.
  - **Inside it, 28 types block members.** TextIter blocks 35 and gets the transient policy (offset forms); the rest block 1-4 members each.

**Qt** (`types_qt_classify.py`; qdoc + PySide + moc; QtCore, QtGui, non-widget QtWidgets):

| | record | value | object | model | statics | unique | item | async | transient | needs-override | excluded |
|---|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|---:|
| classes | 17 | 116 | 115 | 12 | 8 | 33 | 24 | 3 | 82 | **8** | 221 |

- **Members of bindable classes:** 7560, of which **7006 (92%) marshal generically** (QtCore 89%, QtGui 95%, QtWidgets non-widgets 92%).
- **Excluded automatically:** 1559 in all:
  - 679 C++ operators (`equals` comes from the core);
  - 431 special members;
  - 281 removed by PySide (a curated "unsafe or meaningless in a dynamic language" list we inherit);
  - 90 deprecated;
  - 69 templates or platform-native types (CF*, HWND, Vk*).
- **Blocked:** 554, from four causes:
  - **121 out-pointers:** the sip default rule plus its 26 `/In/` exceptions settles them;
  - **83 transient:** callback-only types;
  - **39 QBindable:** skipped, since properties cover them;
  - **174 from 90 distinct unresolved spellings:** mostly nested structs for libclang (QFont::Tag 17, QTextOption::Tab, QCalendar::YearMonthDay), QMetaObject::Connection (core) and std types.
- **The 8 needs-override classes:** QCollatorSortKey, QIODeviceBase, QItemEditorCreatorBase, QRandomGenerator64, QSurface, QTextBlockUserData, QTextObjectInterface, QXmlStreamEntityResolver. Most of them should simply be excluded.

**Apple** (`types_objc2_classify.py`; objc2-foundation and objc2-app-kit 0.3.2):

| | object | value | record | widget |
|---|---:|---:|---:|---:|
| Foundation | 190 | 82 | 8 | 0 |
| AppKit | 190 | 29 | 1 | 78 |

- 491 non-widget classes: 49 MainThreadOnly, 88 Send.
- Methods: 9697, of which **82% are objc2-safe** (Foundation 79%, AppKit 86%). 226 take blocks, and 113 of those are completion-handler shaped (the async candidates).
- **Misclassifications seen:**
  - NSColor lands in object, because of one class-property setter;
  - the NSURLSession task classes land in value.

  Expect a few dozen value/object overrides.

**WinUI:** rules only (§3.1). WinRT metadata is the most explicit of the four (struct, runtime class, activatable/statics/composable, async and collection interfaces are all declared), so I expect the fewest overrides. Running the classifier needs `winmd-tool` built, which takes a windows-bindgen build; that is the first step of the WinUI phase.

### 3.4 What needs a list or an override, and how many

| Kind | GTK | Qt | Apple | WinUI |
|---|---|---|---|---|
| **Scope roots** (whitelist): types bound beyond the reachable closure | ~10 (GSettings, GApplication pieces, GLib Uri/KeyFile) | allowlist of QtCore/QtGui roots, ~40 (QTimer, QSettings, QFile, models, QUrl, QFont, ...) | Foundation/AppKit roots, ~30 | Windows.Foundation + Microsoft.UI non-XAML roots, ~20 |
| **Exclusions** (blacklist): beyond the rules | ~10 (GLib MainContext/MainLoop/Source/Thread: JS timers stay authoritative, PLAN rule 6) | 1 regex + 8 classes | objc2 skips + ~10 | ~10 |
| **Record shapes** (design) | 6 to shared shapes; 20 more with native field names | 17 | ~8 (NSRect, NSSize, NSPoint, NSEdgeInsets, NSRange, ...) | ~8 (Point, Size, Rect, Thickness, CornerRadius, Color, Vector2/3) |
| **Type overrides** (needs-override in scope) | ~26 (most: exclude; Pango Attr* → a records-list form) | 8 + ~15 nested structs for libclang | a few dozen value/object fixes | few |
| **Semantics no metadata states** | transient forms (TextIter → offsets), trap notes (shared AttrList) | QModelIndex → record/persistent index, dialogs' signal-to-Promise | NSAttributedString input form, block-param async names | DependencyObject threading |
| **Typedef/alias table** | — | ~10 (`std::filesystem::path` → string, `QRgb` → number, ...) | — | — |

Overrides live where X7 put the widget ones: `metadata/overrides/<tk>/types/*.yaml`, in the same strict-merge format (every entry must match an ingested record). The annotation-automation lane decides how entries get drafted; this proposal only adds these keys:

- `class:` (taxonomy override)
- `lifetime:`
- `record: {shape, fields}`
- `exclude:`
- `transient: {replacement}`
- `async: {...}`
- `ownership:` (from PySide/sip)
- `verify_subject:` (`construct` or `static`)

### 3.5 The pipeline

```text
ingest (per toolkit, deterministic, checked in)              classify (rules, §3.1)       merge
 GIR ─────────────┐                                           ┌────────────────┐          ┌──────────────────┐
 qdoc + PySide +  ├─▶ metadata/<tk>/types/<Type>.json ───────▶│ category,      │─────────▶│ + overrides/<tk>/ │
 moc + sip +      │   (origin-tagged facts: fields, members,  │ lifetime, scope│          │   types/*.yaml    │
 libclang         │    transfer, ownership, thread, async)    │ closure, JS    │          │ (strict: must     │
 objc2 / headers  │                                           │ names, record  │          │  match a fact)    │
 winmd ───────────┘                                           │ shapes         │          └────────┬─────────┘
                                                              └────────────────┘                   │
       ┌───────────────────────────────┬──────────────────────────────┬────────────────────────────┤
       ▼                               ▼                              ▼                            ▼
 backend tables / thunks         svelters:<tk>/<ns> JS modules     .d.ts typings              docs IR + verification
 (call only the core API)        (call only host ops +            (classes, ctor props,       tests → ledger
                                  NativeObject/NativeValue)        statics, Promise, on())    (metadata/verified/<tk>.json)
```

- **Classification is a generator step, not an ingest step.** Rules change without re-ingesting, and `--report` prints the tables in §3.3 so drift shows up in review. The prototype scripts become this step.
- **Scope selection:**
  - **Dev:** whole namespaces in scope.
  - **Release:** the M6 binding linker narrows it to what the app imports. The module granularity (`svelters:gtk/gio`) gives the linker static roots.
- **Upstream updates** (new GTK/Qt/SDK): re-ingest, re-classify, then diff the classification report. New members in already-classified types need no human. New types either follow the rules or appear as needs-override in the report.

## 4. Taxonomy: JS representation

| Category | JS form | Construction | Identity | Lifetime / GC | Equality | Mutation | Wire |
|---|---|---|---|---|---|---|---|
| record | plain object (`values.d.ts` shapes or native field names) | object literal | none | JS GC | structural | snapshot; set the prop again | `RECORD` (exists) |
| value | generated class instance; methods, getters, `toString()` where a string form exists | `new QUrl(s)`, statics (`QIcon.fromTheme`), or the string form in a slot (`SVL_D_STRING_FORM`) | none: a new handle per crossing | VALUE: copy or ref; GC → checkpoint release; `dispose()` | `a.equals(b)` (core: `QMetaType::equals`, GIR `*_equal`, `isEqual:`, `Equals`) | all generated methods exposed; transfer decides copy vs share | `BOXED` + kind |
| object (refcounted) | generated class; prop accessors, methods, `on(signal, fn)` | `new Gtk.TextBuffer({text})` → `construct`; `Gio.File.newForPath(p)` → `call_static` + CTOR | one JS object per native object | SHARED: strong ref; GC or `dispose()` drops our ref only | `===` | live | `HANDLE` |
| object (QObject), unique | same | `new QTimer({interval: 500})` | one per object | OWNED: dies with its component scope or `dispose()`, never by GC | `===` | live | `HANDLE` |
| returned object | same | — | one per object | BORROWED (X1): weak, `gone` on owner death | `===` | live | `HANDLE` |
| model | object + `length`, `at(i)`, iteration, `items-changed` | as object | as object | as object | `===` | live | `HANDLE` |
| item (Qt) | object; ownership token follows the container | `new QTreeWidgetItem([...])` | one per object | OWNED until inserted, then BORROWED (core `item<T>`) | `===` | live | `HANDLE` |
| statics | namespace-like class | — | — | — | — | — | `call_static` |
| async | `Promise<R>`; `{signal}` replaces GCancellable; outs give `{ret, ...}`; `NativeError` rejections | method call | — | one-shot `CALLBACK` | — | — | `SVL_M_ASYNC` |
| transient | not exposed (offset/persistent forms; callback-only) | — | — | — | — | — | — |
| type value (GType, Class, QMetaType) | the class object or a native type name | — | — | — | — | — | `STR` + `SVL_T_TYPE` |

Rules carried over from the first draft:

- **Every native object is a `HANDLE`,** never a `BOXED` value. The kind is the generated kind, else a reflected kind (GTK, Apple), else the nearest known ancestor.
- **SHARED is a strong ref,** so a pointer-keyed identity map is safe, which matters for COM where there is no qdata. GC releasing a SHARED handle only drops our ref, so it has no visible effect.
- **Listeners live in the component scope.** That keeps listened objects reachable without toggle refs, and native code never holds JS values. In dev, wrappers are non-extensible.
- **All handles are main-thread.** The core marshals completions (WinRT, NSURLSession, GTask) to the UI loop before `invoke_callback`. JS workers never see handles.

## 5. ABI draft (additive on v3; "v2.6" in the doc's additive series)

```c
/* lifetime: one more class */
SVL_LIFE_SHARED = 5   /* strong ref to a refcounted object outside the tree; one handle per object;
                         destroy = drop our ref (never dispose/close); the host may release on GC */
/* well-known type */
SVL_T_TYPE = 11       /* STR: a native type name ("Gtk.StringObject", "QObject", "NSURL") */
/* member flags */
SVL_M_STATIC = 16     /* reached through call_static only */
SVL_M_CTOR   = 32     /* with STATIC: returns a new reference (OWNED/SHARED handle or VALUE) */

/* backend, appended after `style`; both may be NULL */
int (*construct)(void *self, svl_id kind, const svl_id *props, const svl_value *values, size_t n, svl_value *out);
int (*call_static)(void *self, svl_id kind, svl_id method, const svl_value *args, size_t n, svl_value *out);
```

- **`construct`** creates an instance with construct-time props:
  - GTK: `g_object_new_with_properties`, so construct-only props work;
  - Qt: default constructor, then setters;
  - WinRT: `ActivateInstance`, then setters;
  - ObjC: `alloc`/`init`, then KVC.

  `create(kind)` is `construct` with no props.
- **`call_static`** covers constructor functions and statics:
  - `g_file_new_for_path`
  - `QUrl::fromLocalFile`
  - `Uri(string)` (through the activation factory)
  - `+[NSURL fileURLWithPath:]`
- **No new members:**
  - `resolve(SVL_KIND, …)` accepts qualified native names (`Gio.ListStore`) and their kebab form.
  - VALUE handles answer their kind in `handle_info` and accept `get_prop`/`call`.
  - Objects never cross as `BOXED`.
  - GC release covers SHARED, VALUE and BORROWED handles, never OWNED or TOPLEVEL.
- **Host ops:** `host.construct(kind, props)` and `host.callStatic(kind, method, args)`.
- **Op log:** new lines `construct #h Kind {json}` and `static Kind.method [json]`. The TestHost implements both.
- **Not added:**
  - `get_type_info`: generated modules know the hierarchy, and `handle_info.kind` names the nearest known kind;
  - raw pointers;
  - toggle refs.

## 6. Typings, docs and tests from the same facts

- **JS modules:** one per namespace or module.
  - Class names are native names as JS identifiers.
  - Methods are camelCase. Properties are camelCase accessors.
  - Signals keep their native kebab-case names through `on('items-changed', fn)`.
  - Prototype chains mirror the native hierarchy; interfaces are mixins.
  - **Dev fallback:** when the toolkit is newer than the metadata, a `Proxy` resolves unknown members through `resolve` and warns once per member.
- **Typings:** `declare module 'svelters:gtk/gio' { export class ListStore<T extends GObject.Object = GObject.Object> implements ListModel<T> { constructor(props?: ListStore.ConstructProps); append(item: T): void; … } }`.
  - Async methods return `Promise`; `on` is typed through a signal map.
  - VALUE classes declare `equals` and `dispose`.
  - The emitter extends `tools/metadata/common` (`tsvalues.py`, `gen_bindings.py`).
- **Docs:** `svelters docs gtk text-buffer` and `svelters docs gtk Gtk.TextBuffer` resolve to the same card. The card's sections are `class` (taxonomy + lifetime, with the GC rule spelled out), `construct`, `props`, `methods`, `statics`, `events`, `examples`. It shows each fact's origin (`gir`, `qdoc`, `pyside`, `sip`, `objc2`, `winmd`, `override`).
- **Tests:** generated per member, with subjects from `verify_subject` (`construct`, `static`, or X7's `borrowed`). The checks run on the real host:

  | Category | Checks |
  |---|---|
  | object | prop round-trip, notify/echo rules, method calls, signal delivery, identity (`x.model === x.model`), `live handles after dispose == baseline` |
  | value | string form → value → `toString`, `equals`, copy-versus-share by `transfer` |
  | record | field round-trip, integrality errors |
  | async | resolve, reject with `NativeError`, abort |

  Results go to `metadata/verified/<tk>.json` with fact ids `svelters-api:v1:type:<tk>:<Ns>:<Type>:<member>`, versioned like the widget ledger.

## 7. Ergonomics in Svelte

These sketches use the proposed modules.

```svelte
<script>
  import { TextBuffer } from 'svelters:gtk/gtk';
  const buffer = new TextBuffer({ text: 'Hello' });                        // construct → SHARED
  let length = $state(buffer.text.length);
  $effect(() => buffer.on('changed', () => (length = buffer.text.length)));   // on() returns its disposer
</script>
<text-view buffer={buffer} />
<text-view buffer={buffer} monospace />
<label label={`${length} characters`} />
```

The Qt equivalent is `new QTextDocument({ plainText: 'Hello' })` into `<text-edit document={doc} />`. That object is OWNED and dies with the component.

A native model feeding `@svelters/lists`. Keys are stable because of identity, and no ABI change is needed:

```svelte
<script>
  import { ListView } from '@svelters/lists';
  import { DirectoryList } from 'svelters:gtk/gtk';
  import { File } from 'svelters:gtk/gio';
  const dir = new DirectoryList({ file: File.newForPath('/usr/share'), attributes: 'standard::display-name,standard::icon' });
</script>
<ListView items={dir} key={(info) => info}>
  {#snippet row(info)}<box spacing={6}><image gicon={info.getIcon()} /><label label={info.getDisplayName()} /></box>{/snippet}
</ListView>
```

An icon from a path. Strings keep meaning a theme name; files are explicit:

```js
import { FileIcon, File } from 'svelters:gtk/gio';
import appIcon from './icons/app.svg';
const icon = new FileIcon({ file: File.newForPath(appIcon) });        // <image gicon={icon} />
import { QIcon } from 'svelters:qt/gui';
const qicon = new QIcon(appIcon);                                      // <push-button icon={qicon} text="Open" />
```

Dates are values: `<date-time-edit bind:date-time={when} />` gives a `Date`, and `edit.call('dateTime')` does the same.

Async:

```js
const file = await new FileDialog({ title: 'Open' }).open(win);       // Promise<Gio.File>; cancel rejects with code 'dismissed'
const { contents } = await file.loadContentsAsync({ signal });        // _async/_finish → {ret, contents, etagOut}
```

On Qt, blocking `getOpenFileName` is refused (X7). `<file-dialog bind:this={dlg} />` with `await dlg.call('open')` uses the S2 signal-to-Promise adapter.

## 8. Prior art

These links weren't re-fetched in this session; the descriptions come from the projects' documentation.

- **GJS** ([Mapping.md](https://gitlab.gnome.org/GNOME/gjs/-/blob/master/doc/Mapping.md), [memory management](https://gjs.guide/guides/gjs/memory-management.html)) and **node-gtk** ([repo](https://github.com/romgrk/node-gtk)):
  - entirely typelib-driven, with no per-type code: the model for the GTK core;
  - one wrapper per GObject;
  - property-dict constructors;
  - `Gio._promisify`.

  Their toggle refs are what SHARED plus scope-held listeners replace.
- **PySide/Shiboken** ([type kinds](https://doc.qt.io/qtforpython-6/shiboken6/typesystem_specifying_types.html), [ownership](https://doc.qt.io/qtforpython-6/shiboken6/typesystem_ownership.html)):
  - `value-type`/`object-type` is our value/object split;
  - their typesystem is a curated whitelist we can ingest instead of re-creating;
  - runtime overload resolution.
- **PyQt/sip** ([annotations](https://www.riverbankcomputing.com/static/Docs/sip/annotations.html)): transfer and out annotations on every Qt class.
- **NativeScript** ([iOS marshalling](https://docs.nativescript.org/guide/ios-marshalling)): a metadata generator from headers drives a reflective ObjC runtime with libffi and blocks. That's our Apple core plus generated metadata.
- **objc2** ([repo](https://github.com/madsmtm/objc2)): its header-translator turns the SDKs into reviewed, machine-readable facts (thread kind, safety, copying, blocks). We ingest its output rather than redoing the review.
- **react-native-xaml** ([repo](https://github.com/microsoft/react-native-xaml)) and **NodeRT** ([repo](https://github.com/NodeRT/NodeRT)): WinRT surfaces generated from winmd.
- **N-API / Deno** ([object wrap](https://nodejs.org/api/n-api.html#object-wrap), [FFI](https://docs.deno.com/runtime/fundamentals/ffi/)): `napi_wrap`/`napi_add_finalizer`/`napi_create_reference` have the same shape as our handle table. Deno's `UnsafePointer` is what PLAN rule 4 forbids.
- **GObject-Introspection** ([annotations](https://gi.readthedocs.io/en/latest/annotations/giannotations.html)):
  - `transfer` → BORROWED/SHARED and copy/share;
  - `caller-allocates` → out records;
  - `scope` → callback lifetimes;
  - `glib:finish-func` → async pairs.

## 9. Phasing

| Phase | Hand-written (core) | Generated / data | Acceptance |
|---|---|---|---|
| **N0 GTK pilot** | handles for every GObject (SHARED); VALUE kinds via typelib copy/free or ref-funcs; invoker gains out/caller-allocates, statics/ctors, GList/arrays; reflected kinds; `construct`/`call_static`; host SHARED + ops; JS `NativeObject`/`NativeValue` | `ingest.py --namespace` for Gio/Gdk/Pango/Gsk/GLib/Graphene; `types_gir_classify.py` becomes the classify step with `--report`; scope closure + ~10 roots + ~10 exclusions; `svelters:gtk/*` modules, `.d.ts`, docs cards, generated tests for the in-scope set | §7 GTK examples run on the real host; the classification report is checked in; ledger facts for every in-scope type; identity/GC/dispose accounting tests |
| N1 GTK async | GI closures by scope, finish-func folding | async members appear in modules and typings | `gio_services.js` passes on the generic path; the hand-written `gio-file` is retired |
| N2 Qt | `svl::value`/`unique`/`item`, QMetaObject fallback, `async_from_future` | ingest of PySide core/gui typesystems, moc JSON, sip annotations, libclang nested structs; `types_qt_classify.py` as the classify step; thunks for value/object/unique/statics; overload dispatch; alias table | Qt twins of §7; QStringListModel/QStandardItemModel feeding lists; the CI boundary check passes |
| N3 models and reactivity | — | `@svelters/lists` takes native models; optional reactive accessors (`notify::`/NOTIFY via `createSubscriber`) | 10k-item DirectoryList scroll; the TextBuffer example needs no `$state` mirror |
| N4 Apple / WinUI | objc2-based or ObjC++ core; C++/WinRT core | objc2 and winmd ingest + classify; build `winmd-tool` for Windows.Foundation/Microsoft.UI | when those backends grow past the P set |

## 10. Open questions

1. **SHARED for every transfer-none non-widget GObject return?** The proposal says yes, except where metadata says `lifetime: borrowed`. Native resources (a GdkSurface) could otherwise outlive the UI that used them.
2. **Should getters be reactive by default (N3), or opt in?**
3. **VALUE mutators.** The proposal exposes them all, because mutability can't be derived from C (§3.1), and documents the traps. The alternative is an override list of mutating methods to hide, which is not small.
4. **Qt overloads:** split them by metadata or dispatch at runtime? This needs a measurement on the N2 allowlist; PySide's runtime dispatch is the prior art.
5. **Module granularity:** per namespace (proposed, for M6 tree-shaking) or per toolkit?
6. **Rich-text runs:** PangoAttrList, QTextCharFormat ranges and NSAttributedString each need an input form. They should be native-shaped records per toolkit, since no portability layer is allowed.
7. **Should the Apple classifier use objc2's generated sources as the primary ingest** (licence-clean on Linux, reviewed), with the SDK libclang pass as the authority on a Mac? Or the reverse?
8. **Can WinUI's DependencyObject-derived non-visual types** (Brush, Transition, FontFamily) be SHARED, or do they need XAML-thread rules beyond `HasThreadAccess`?
