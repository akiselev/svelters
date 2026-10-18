# Non-GUI types: how JS creates, holds and passes toolkit values and objects

Status: research proposal, 2026-10-17. Working notes, measurements and probe output: `docs/notes/types-research.md`. Probes: `docs/notes/scripts/types_gir_inventory.py`, `types_qt_inventory.py`, `types_qt_reflect_probe.cpp`, `types_gi_callable_blockers.py`.

Builds on: ABI v3 value model (`plans/architecture/backend-abi.md`, `docs/notes/js-native-serdes.md`, `docs/notes/serdes-impl.md`), X1 borrowed handles, X7 non-widget kinds, X8 Gdk values, `docs/notes/ownership-and-lifetimes.md`, `plans/research/CUSTOM-WIDGETS.md` (packs, reflective engines), `plans/research/QUICKJS-RUNTIME.md` (built-in `svelters:*` modules).

## Summary

- **The value model is mostly done; the object model is not.** ABI v3 already carries records, enums, flags, bytes, variants, dates, callbacks, async and errors. What is missing is everything about *non-widget objects*:
  - GObjects outside the Gtk namespace (GListModel, GMenuModel, GdkClipboard, GdkDisplay, GSettings, GFile) cross as opaque BOXED values. Each crossing mints a new handle, so they have no identity, and JS cannot reach their props, methods or signals. That is 286 GTK slot references across 32 types.
  - GBoxed and Qt value classes are opaque too (269 GTK slots, 407 Qt slots). JS can hold a QFont or a PangoAttrList, but it can't read or call anything on it.
  - JS can't construct anything except a kind with no arguments (`create(kind)`). There are no constructor arguments, no construct-only props, no static functions and no GType values.
  - 746 Qt slots across 83 types are rejected outright: QModelIndex, item pointers, QPainter, QPolygonF and others.
- **Two JS representations, nothing else:**
  - **Records**: plain objects, by value, snapshots. These are the existing Rect, Size, Point, Insets, Color and date parts, extended with a few more.
  - **Handle-backed class instances**: one canonical JS object per native object, created from generated classes in built-in modules (`svelters:gtk/gio`, `svelters:qt/core`, ...).
  - Today's methodless BOXED values become VALUE-class instances that have a kind and read-only methods.
- **ABI impact is small and additive:**
  - one new lifetime class, `SHARED`;
  - two nullable vtable members, `construct` and `call_static`;
  - one well-known type, `SVL_T_TYPE`;
  - two member flags, `SVL_M_STATIC` and `SVL_M_CTOR`;
  - semantic extensions that need no new members: VALUE handles get kinds and methods, and every native object crosses as a handle.
- **GTK is the pilot.** GIRepository already drives the X1 dynamic calls. Reached through handles, today's type matrix covers 60-75% of Gio/Gdk/Pango/Gsk callables. Adding out params, static functions and constructors takes that to 80-90%, and callbacks/async to about 95%, with no per-method code.
- **Qt needs generated C++.** QMetaObject reaches signals and item-model invokables, but not QSettings, QFile, QMimeData or QClipboard methods, and no value-type methods (probe, §1.3).
  - WinUI also needs generated glue, from winmd.
  - AppKit can be reflection-first (ObjC runtime + libffi), the NativeScript way.

## 1. Inventory

### 1.1 GTK (Gtk-4.0 API, GTK 4.22)

This counts every public, non-deprecated, introspectable property, parameter, return and signal argument of the Gtk namespace (widgets and non-widgets). It excludes primitives, strings, enums/flags and widgets. The scan found 263 distinct non-GUI types and 1422 slot references.

| Status today | Slots | Types | Examples (slot count) |
|---|---:|---:|---|
| Handle (Gtk-namespace kind) | 473 | 106 | Adjustment 35, ListItemFactory 33, Expression 24, PageSetup 24, PrintSettings 21, SelectionModel 18, Sorter 17, TextTag 17, TextMark 14 |
| **Opaque object** (BOXED, no identity, no members) | 286 | 32 | **GListModel 61**, GObject 39, **GMenuModel 36**, GCancellable 20, GAsyncResult 20, GdkDisplay 15, GdkDrop 11, GdkClipboard 8 |
| **Opaque boxed** (BOXED, no members) | 269 | 32 | **TextIter 96**, Bitset 16, PaperSize 14, PangoAttrList 13, PangoTabArray 13, GdkEventSequence 12, GBytes 10, GError 9, GskTransform 7 |
| String form in, opaque out | 101 | 7 | GFile 39, GdkPaintable 22, ShortcutTrigger 15, GIcon 12, PangoFontDescription 8 |
| Rejected: callbacks (GI path) | 79 | 33 | DestroyNotify 22, AsyncReadyCallback 20 |
| Record | 61 | 6 | GdkRGBA 18, graphene Rect 16, GdkRectangle 13, graphene Point 10 |
| GStrv | 49 | 2 | |
| Rejected: arrays, GList, GHashTable | 52 | 34 | |
| Variant (GVariant, GValue) | 28 | 2 | |
| Rejected: struct without GType | 16 | 7 | CssLocation |
| DateTime | 7 | 1 | |

By namespace: Gtk 719, Gio 219, Gdk 153, GLib 79, GObject 68, Pango 62, Graphene 33, Gsk 30.

The non-GUI namespaces have about 4250 callables between them, which is far too many to annotate by hand. Gio alone has 106 classes, 37 interfaces and 1599 callables.

### 1.2 Qt (metadata/qt: 195 classes, 73 widgets)

The scan found 179 distinct non-GUI types and 2297 slots.

| Status today | Slots | Types | Examples |
|---|---:|---:|---|
| **Rejected (`opaque`)** | 746 | 83 | QGraphicsItem* 92, **QModelIndex 92**, QTreeWidgetItem* 56, QTableWidgetItem* 35, QPainter* 34, QPolygonF 33, QStyleOption* 31, QListWidgetItem* 30, QDataStream 22, `T*` out pointers 37 |
| Record | 589 | 9 | QSize 155, QPointF 117, QRectF 94, QRect 85, QPoint 69, QColor 34 |
| Boxed (opaque, QVariant copy) | 407 | 21 | QIcon 63, QFont 50, QPainterPath 48, QBrush 39, QPixmap 32, QUrl 29, QPalette 22, QTransform 21, QKeySequence 19, QTextCursor 18 |
| QObject with a kind | 200 | 18 | QAction 88, QLayout 24, QStyle 16, QAbstractItemDelegate 13, QTextDocument 9 |
| QObject without a kind | 111 | 15 | QObject 55, QAbstractItemModel 18, QValidator, QCompleter |
| Dates, variant, QStringList, QByteArray | 154 | 6 | |
| Lists (`QList<T>`) | 90 | 27 | |

QtCore objects are absent from metadata altogether, because the ingest covers only QtWidgets plus an allowlist. The missing classes include QTimer, QSettings, QFile, QStandardItemModel, QStringListModel and QSortFilterProxyModel.

### 1.3 What reflection gives (probe results)

- **GIRepository** describes every callable, including GType, `transfer`, `caller-allocates`, `scope` and `glib:finish-func`; Gio has 215 finish-func annotations.
  - The table below gives each tier's cumulative coverage of callables, when receivers are handles:

    | | today's matrix | + out/inout | + static/ctor | + callbacks/async |
    |---|---:|---:|---:|---:|
    | Gio | 66% | 68% | 82% | 97% |
    | Gdk | 74% | 79% | 89% | 94% |
    | Pango | 60% | 63% | 81% | 97% |
    | Gsk | 61% | 68% | 83% | 96% |

    The "+ callbacks/async" column also includes the records-by-value and arrays/lists tiers (`types_gi_callable_blockers.py`).
  - GLib and GObject are mostly gpointer/varargs utilities: only 55-82% are reachable even at the end. Bind a curated slice of them.
  - Graphene methods are out-struct math. They belong in pure JS on the records.
- **QMetaObject** (Qt 6.11) covers less:
  - Signals: everywhere.
  - Properties: where the class declares Q_PROPERTYs (QTimer 5, QAction 18, QCompleter 9).
  - Item models: their whole read/write API is Q_INVOKABLE (QStandardItemModel 46 invokables).
  - Nothing reachable at all: QSettings, QMimeData. QFile, QClipboard and QProcess have signals only.
  - Constructors: no class has a Q_INVOKABLE one.
  - Value types: no methods. Their Q_GADGETs carry only enums. QMetaType gives construct/copy/destroy/equals generically, and a QString converter only for QColor, QUrl, the date types, QUuid, QByteArray, QFont and QKeySequence.
- **WinRT:** winmd describes everything, but there's no general dynamic invoke without a projection, so the glue has to be generated (see §4.3).
- **ObjC runtime:** every method has a type encoding, so `objc_msgSend` through libffi reaches everything dynamically. The headers add nullability, generics and completion-handler shapes.

## 2. Taxonomy and representation

| Category | Examples | JS form | Construction | Identity | Lifetime / GC | Equality | Mutation | Wire |
|---|---|---|---|---|---|---|---|---|
| **Plain value struct** | QPoint/QRectF, GdkRGBA, graphene_rect_t, Windows.Foundation.Point/Rect, Thickness, NSRect/NSEdgeInsets, NSRange | plain object `{x, y, width, height}` (shared portable shapes, `values.d.ts`) | object literal | none (by value) | JS GC only | structural (user code) | snapshot: change it, then set the prop again | `RECORD` (exists) |
| **Immutable value object** | QUrl, QFont, QKeySequence, QIcon, PangoFontDescription, PangoAttrList, GBytes, GVariant, GskTransform, Windows.Foundation.Uri, NSURL, NSFont, NSColor, NSImage, NSAttributedString | instance of a generated class (`QUrl`, `Pango.FontDescription`), read-only methods and getters, `toString()` where a string form exists | `new QUrl(s)`, static factories (`QIcon.fromTheme`), or the string form in place of the value (`SVL_D_STRING_FORM`, exists) | none: each crossing is a new handle; `equals()` | **VALUE** handle: strong ref or copy, released by GC (FinalizationRegistry → checkpoint), `dispose()` / `using` | `a.equals(b)` (QMetaType::equals, GIR `*_equal`, `isEqual:`, `Equals`) | immutable in JS; mutators are not exposed (§9 Q3) | `BOXED` + kind (extended) |
| **Mutable shared object** (refcounted, no tree) | GtkTextBuffer, GListStore, GSettings, GtkStringList, GdkClipboard, GFile, WinRT runtime classes (brushes, collections), NSMutableArray, NSTextStorage, NSUserDefaults | instance of a generated class: accessors for props, methods, `on(signal, fn)` | `new Gtk.TextBuffer({text})` (`construct`), `Gio.File.newForPath(p)` (`call_static`, ctor) | **one JS object per native object** while JS holds it | **SHARED** (new): strong ref; GC or `dispose()` drops our ref only, never destroys | `===` | live: getters read native, setters write native | `HANDLE` |
| **Owned QObject** (no refcount) | QTimer, QSettings, QStandardItemModel, QFileSystemWatcher, QSortFilterProxyModel created by JS | same as above | `new QTimer({interval: 500})` | one per object | **OWNED**: dies with the creating component scope or `dispose()`; never by GC (a view may still use it) | `===` | live | `HANDLE` |
| **Borrowed object** | `textview.buffer`, `view.model()`, `widget.display`, `qApp.clipboard()` | same class | n/a (returned by natives) | one per object | **BORROWED** (X1): weak; `gone` when the owner drops it | `===` | live | `HANDLE` |
| **Collections, by value** | GStrv, `QList<T>`, `NSArray<T>`, `IVectorView<T>`, GList returns | JS array (snapshot) of records, strings or instances | array literal | n/a | JS GC | n/a | snapshot | `LIST` (exists) |
| **Models** (live collections) | GListModel/GListStore/GtkStringList/GtkDirectoryList, QAbstractItemModel, IObservableVector, NSArrayController | a shared or owned object whose class adds `length`, `at(i)`, `[Symbol.iterator]` and an `items-changed` event; QModelIndex is a record `{row, column}` plus a persistent-index VALUE handle when it must survive changes | as for objects | as for objects | as for objects | `===` | live | `HANDLE` |
| **Enums, flags** | | names / name arrays (exists) | | | | | | `ENUM`/`FLAGS` |
| **Type values** | GType (`Gio.ListStore` item type), QMetaType | the class object (`StringObject`) or a native type name string | | | | | | `STR` + `SVL_T_TYPE` (new) |
| **Async results** | `_async`/`_finish` (GAsyncResult, GTask), Qt signal-completed ops, IAsyncOperation, completion-handler blocks | `Promise<R>`; trailing `{signal: AbortSignal}` replaces GCancellable; out params give `{ret, ...outs}`; errors reject with `NativeError` | method call | n/a | one-shot `CALLBACK` (exists) | | | `SVL_M_ASYNC` (exists) |
| **Iterators / transient pointers** | GtkTextIter, GtkTreeIter, QGraphicsItem*, QTreeWidgetItem*, QPainter*, QEvent*, QStyleOption* | not exposed as such: offsets instead of TextIter (`insert(offset, text)`), persistent indexes instead of QModelIndex, model APIs instead of item pointers; painter/event/style option are callback-only and stay native (canvas display list, event families) | | | | | | none |

### 2.1 The rules in prose

1. **Every native object crosses as a `HANDLE`, never as `BOXED`.** Extend X1 from "has a generated kind" to "every GObject / QObject / IInspectable runtime class / non-value NSObject". The kind is the generated kind if there is one. Otherwise GTK mints a *reflected kind* from the GType (GParamSpecs, `g_signal_query`, typelib methods: the engine the Gtk namespace already uses), and other backends fall back to the nearest ancestor with a kind. This fixes identity for GListModel, GMenuModel and GdkClipboard. As a result, keyed lists over native items get stable keys.
2. **Which lifetime class a native object gets:**
   - **Refcounted, outside the tree: SHARED.** The handle table holds a strong reference. Because that reference is strong, a pointer-keyed identity map is safe here: the address cannot be reused while it is held. This also covers COM, where there is no qdata.
   - **GC may release it.** Dropping our reference has no visible effect: a list view that uses the store holds its own reference. This is why SHARED differs from OWNED. "GC never decides when an owned element dies" (`ownership-and-lifetimes.md` §2.1) stays true for widgets and QObjects.
   - **Widgets stay as today:** weak BORROWED for natives we didn't create (the G5 zombie trap), OWNED for natives JS created.
3. **Listeners keep objects alive through scopes, not toggle refs.** `obj.on(signal, fn)` registers in the current component scope (native-runtime `Scope.handlers`). The wrapper therefore stays reachable while the component lives and is released at teardown. Native code still never holds a JS value. This replaces GJS's toggle-ref machinery, which was the 2021 versions' crash source. Cost: expando properties on a wrapper don't survive the wrapper being collected. In dev, wrappers are non-extensible (`Object.preventExtensions`), so misuse fails loudly.
4. **VALUE handles get kinds and read-only members.**
   - `handle_info` answers the value kind (today it answers 0).
   - `get_prop` and `call` accept VALUE handles.
   - `set_prop` and `listen` on them fail with "immutable value".
   - A returned value is a snapshot, as PySide documents for its value types. Changing a widget's font means setting `font` again.
5. **Thread affinity: all handles are main-thread.**
   - Backends marshal completions to the UI loop before calling `invoke_callback`:
     - WinRT `IAsyncOperation` (DispatcherQueue);
     - NSURLSession (main queue);
     - GTask worker threads (GTask already completes in the caller's main context).
   - JS workers (`QUICKJS-RUNTIME.md` #9) never receive handles. They exchange records, bytes and strings only.

## 3. ABI draft (additive on v3; "v2.6" in the doc's additive series after CUSTOM-WIDGETS' v2.5)

```c
/* lifetime: one more class */
typedef enum svl_lifetime { ..., SVL_LIFE_SHARED = 5 } svl_lifetime;
/* SHARED: a strong reference to a refcounted native object that is not in the tree (GObject non-widget,
   COM runtime class, ARC object). One handle per object while it lives. destroy = drop our reference
   (never dispose/close the object). The host may destroy it when the JS wrapper is collected. */

/* well-known type */
SVL_T_TYPE = 11,  /* STR: a native type name ("Gtk.StringObject", "GObject", "QObject"); JS passes a class or a string */

/* member flags */
SVL_M_STATIC = 16, /* a static function: reached through call_static only */
SVL_M_CTOR   = 32, /* with STATIC: returns a new reference (OWNED/SHARED handle or a VALUE) */

/* backend, appended after `style` (both may be NULL: "construction is not supported by this backend") */
/* A new instance of `kind` with construct-time properties (g_object_new_with_properties: construct-only
   props included; Qt: default constructor then setters in order; WinRT: ActivateInstance then setters;
   ObjC: alloc/init then KVC setters). `props` are SVL_PROP ids of `kind`. `out` receives a HANDLE
   (OWNED or SHARED per the kind) or a BOXED (VALUE kinds). create(kind) == construct(kind, 0 props). */
int (*construct)(void *self, svl_id kind, const svl_id *props, const svl_value *values, size_t n, svl_value *out);
/* A static function or constructor function of `kind` (g_file_new_for_path, QUrl::fromLocalFile,
   QIcon::fromTheme, Uri(string), +[NSURL fileURLWithPath:]). `method` comes from
   resolve(SVL_METHOD, kind, name) and its descriptor carries SVL_M_STATIC. Same out/async/throws
   rules as `call`. */
int (*call_static)(void *self, svl_id kind, svl_id method, const svl_value *args, size_t n, svl_value *out);
```

Semantic extensions with no new member:

- `resolve(SVL_KIND, ...)` accepts the qualified native name (`Gio.ListStore`, `QTimer`, `Windows.Foundation.Uri`, `NSURL`), its kebab form (`list-store`), and the existing tag. Kinds without a tag are not elements: the renderer refuses them in markup.
- `handle_info` on VALUE handles reports the value kind. `get_prop`/`call` take VALUE receivers. `describe(SVL_METHOD, valueKind, m)` describes their methods.
- An object result never mints `BOXED`. See §2.1 rule 1.
- **Equality.** Value kinds expose a method `equals(other)`. It comes from the GIR `*_equal` function, `QMetaType::equals`, `isEqual:` or `Equals`, so no new op is needed.
- **Host ops and op log.**
  - JS-side ops: `host.construct(kindOrId, props)` and `host.callStatic(kindOrId, methodOrId, args)`.
  - Op-log lines: `construct #h Kind {json}` and `static Kind.method [json]`.
  - The Node TestHost implements both, as it does every other op.
- **Release by GC** covers SHARED, VALUE and BORROWED handles, through the existing FinalizationRegistry → `queueRelease` → checkpoint path. It never covers OWNED or TOPLEVEL.

What the draft deliberately does not add:

- **No `get_type_info`.** Generated modules know the class hierarchy, and `handle_info.kind` names the nearest known kind of a private subclass (GLocalFile → `Gio.File`). A fully reflective pack without generated JS would need `kind_info(kind) → {name, parent, interfaces, flags}`. Defer it until CUSTOM-WIDGETS phase 2 asks for it.
- **No raw pointers.** No toggle refs. No JS values held natively.
- **No async op beyond `SVL_M_ASYNC` and `cancel`.**

## 4. Per-toolkit plan

### 4.1 GTK (reflection first; the pilot)

- **Typelibs.** gi.rs loads Gio, Gdk, Pango, Gsk and GLib next to Gtk (`gi_repository_require`).
- **Receivers.** Plans key on the receiver's GType rather than the kind table, which also closes X1's "methods on a more derived type" follow-up. Any GObject handle is a receiver, and so is any boxed VALUE handle.
- **Invoker growth, in coverage order.** All of it is host-ready since S1/S2:
  1. out/inout parameters and `caller-allocates` structs: records out, boxed out;
  2. static functions and constructors (`call_static`);
  3. GType through `SVL_T_TYPE`;
  4. GList/GSList/arrays with a length, as lists;
  5. callbacks by GIR scope (`call`, `notified`, `async`);
  6. `glib:finish-func` pairs as `SVL_M_ASYNC`, with GCancellable hidden behind `{signal}`.
  The hand-written `gio-file` service then becomes a regression test of the generic path and is retired.
- **Construction.** `construct` uses `g_object_new_with_properties`, so construct-only props such as `Gio.ListStore:item-type` and `Gtk.DirectoryList:file` work.
- **Reflected kinds.** These are minted lazily per GType: props from GParamSpecs, signals from `g_signal_query`, methods through the typelib. That is the same engine the Gtk namespace uses, with no new generated Rust.
- **Curated GLib slice.** Variant (exists), Bytes, DateTime/TimeZone (exists as Date), KeyFile, Uri, and `get_user_*_dir` statics. The rest of GLib and GObject stays unbound.
- **Graphene.** Records only. Their math lives in a small JS helper module, if anyone asks for it.

### 4.2 Qt (generated C++, QMetaObject as complement)

- **Ingest.** Ingest qtcore and qtgui classes (`parse_qdoc` already reads all three indexes), using a curated allowlist that replaces `EXTRA_CLASSES`:
  - QtCore objects: QTimer, QSettings, QFileSystemWatcher, QStandardItemModel, QStringListModel, QSortFilterProxyModel, QItemSelectionModel, QFileSystemModel (QtGui), QMimeData;
  - QtGui objects: QClipboard;
  - values: QUrl, QLocale, QTimeZone, QFont, QIcon, QPixmap, QImage, QBrush, QPen, QPalette, QKeySequence, QTransform, QPainterPath, QPolygonF, QTextCharFormat.
- **Value classes.** Generated thunks on the boxed table's `QVariant` (`qvariant_cast<T>(g.boxed[h]).method(args)`):
  - const methods only;
  - constructors and statics through `call_static`;
  - `equals` from `QMetaType::equals`.
  QPolygonF becomes `LIST` of Point.
- **QObjects.** These use the widget generator unchanged: props, signals, methods, `out:`. `construct` = default constructor + setters. JS-created objects are OWNED (scope-driven) and returned ones BORROWED.
- **QMetaObject.** Used at runtime only for signals and Q_PROPERTYs of classes without metadata, plus app-defined Q_OBJECTs (CUSTOM-WIDGETS §7). For item models it also gives the whole model API through invokables, at no cost.
- **QModelIndex.** `{row, column}` records when a parent-less index is enough (tables, lists). Otherwise a VALUE handle over a `QPersistentModelIndex` that checks `isValid()` (js-native-serdes §2.2).
- **Item pointers.** QTreeWidgetItem* and QListWidgetItem* stay unexposed. Their widgets are `QTreeWidget` convenience views; the M5 model path replaces them.
- **Dialogs.** Blocking statics (`QFileDialog::getOpenFileName`, `QInputDialog::getText`) are refused, as for every blocking call (X7). Their non-blocking forms get a Promise through the S2 `async: {signal}` adapter.
- **Overloads.** Qt is overload-heavy; JS has none. The generator emits one JS name per overload set and dispatches on arity, then on the descriptor tag. Where a set is ambiguous (`QIcon(QString)` path vs `QIcon::fromTheme`), the metadata gives a distinct `js:` name. This is Shiboken's approach, restricted to what can be decided statically (§9 Q4).

### 4.3 WinUI (generated glue from winmd)

- **Records.** Windows.Foundation Point/Size/Rect, Thickness (Insets), CornerRadius, Color, Numerics.Vector3. `_types.json` already lists their fields.
- **Runtime classes.** Uri, brushes, FontFamily, TransitionCollection, collections: SHARED handles, because COM is refcounted. Uri gets a string form; SolidColorBrush gets a CSS color string form.
- **Construction.** `construct` = `RoActivateInstance` or the activation factory + setters. `call_static` = statics interfaces.
- **Async.** `IAsyncOperation<T>`/`IAsyncAction` become `SVL_M_ASYNC`: `Completed` is marshalled to the DispatcherQueue, and `Cancel()` is driven by `{signal}`.
- **Collections.** `IVector`/`IObservableVector` become model classes. `IVectorView` becomes a JS array snapshot.
- **Glue.** Generated C++/WinRT thunks, the way react-native-xaml generates its props from winmd. There's no dynamic WinRT invoke worth building.

### 4.4 AppKit / UIKit (reflection first, headers for facts)

- **Dynamic invocation.** `objc_msgSend` through libffi, driven by `method_getTypeEncoding`. This is the NativeScript/PyObjC model; libclang header facts from MAC1 add nullability, generics and completion handlers.
- **Records.** NSRect, NSSize, NSPoint, NSEdgeInsets, NSRange. NSString is a string, NSNumber a number. Typed NSArray/NSDictionary results become JS arrays or objects by value.
- **Immutable value objects** (NSURL, NSColor, NSFont, NSImage, NSAttributedString) become VALUE handles with string forms, `equals` (`isEqual:`) and `toString`.
  - String forms: URL text; CSS color; a "Family size" font; an image name or path.
  - NSAttributedString becomes an *input* form: a string or a records list (§9 Q6). Today it maps to `string` and loses its attributes.
- **Mutable objects** (NSMutableArray, NSTextStorage, NSUserDefaults) are SHARED. Identity uses an associated-object sentinel (`ownership-and-lifetimes.md` §2.5).
- **Completion-handler blocks** become `SVL_M_ASYNC`, using Swift's async-import heuristic: the last block parameter named `completionHandler` or `withCompletion`, or `NS_SWIFT_ASYNC`.

## 5. Generation: modules, typings, docs, tests

- **Modules.** Built-in modules are served by the loader's `ModuleProvider` (QUICKJS-RUNTIME #5). Each module is generated from metadata and is thin JS over host ops, with no rquickjs classes (QUICKJS-RUNTIME #13).

  | Toolkit | Modules |
  |---|---|
  | GTK | `svelters:gtk/gtk`, `/gio`, `/gdk`, `/pango`, `/glib`, `/gsk`, `/graphene` |
  | Qt | `svelters:qt/core`, `/gui`, `/widgets` |
  | WinUI | `svelters:winui/windows.foundation`, `/microsoft.ui.xaml.media`, ... |
  | Apple | `svelters:appkit/foundation`, `/appkit` |

  - **Class names** are the native names as JS identifiers: `ListStore` (GIR `Gio.ListStore`), `QTimer`, `Uri`, `NSURL`.
  - **Methods** are camelCase (`insert_at_cursor` → `insertAtCursor`, which the GI resolve already accepts; WinRT `GetAt` → `getAt`).
  - **Properties** are camelCase accessors (`buffer.text`, `timer.singleShot`).
  - **Signals** keep their native kebab-case names through `on('items-changed', fn)`, the same names `on<event>` uses in markup.
  - **Element-free kinds** are classes only. Markup stays for elements.
  - **Prototype chains** follow the native hierarchy, so `instanceof Gio.ListModel` works. GObject interfaces are mixins checked through `handle_info`.
  - **Wrappers** are created from a handle with `Class.wrap(h)` (one per handle, through a WeakRef map). The existing `nodeFor` for borrowed widgets becomes the widget case of the same function.
- **Dev fallback.** When the native type is newer than the metadata (a typelib member missing from the generated module), a `Proxy` resolves unknown members through `resolve(SVL_METHOD, kind, name)` on first use. It warns once per member.
- **Typings.** `declare module 'svelters:gtk/gio' { export class ListStore<T extends GObject.Object = GObject.Object> implements ListModel<T> { constructor(props?: ListStore.ConstructProps); append(item: T): void; ... } }`.
  - Constructors take `Partial<ConstructProps>`, including construct-only props.
  - Async methods return `Promise`, and errors are typed `NativeError` in JSDoc.
  - `on` is typed through a signal map.
  - The emitter is `tools/metadata/common` (`tsvalues.py`/`gen_bindings.py`), extended.
- **Docs.** `svelters docs gtk text-buffer` and `svelters docs gtk Gtk.TextBuffer` resolve to the same card: the kebab-case form of the native name plus the JS class form. The card has sections `construct`, `props`, `methods`, `statics`, `events`, `lifetime` (shared/owned/value, with the GC rule spelled out) and `examples`.
- **Tests and ledger.** These follow the A1 pattern; the annotation-automation research owns how facts get drafted. This proposal needs these facts in the schema:
  - `lifetime: shared | owned | value`;
  - `construct: {props}`;
  - `static: true`;
  - `ctor: true`;
  - `equals`;
  - `verify_subject: {construct: {...}}` or `{static: newForPath, args: [...]}`, next to X7's `{borrowed}`.

  Generated tests construct the subject and do the following, recording each result in `metadata/verified/<toolkit>.json` by version:
  - round-trip props;
  - call methods;
  - check notify and signal echo rules;
  - for VALUE kinds, check `string form → value → toString` and `equals`;
  - for SHARED, assert identity (`x.model === x.model`);
  - assert `live handles after dispose == baseline`.
- **Reflection versus glue.**

  | Toolkit | Reflection-only (dev and release) | Generated |
  |---|---|---|
  | GTK | everything | release thin bindings (M6), as for widgets |
  | Apple | dynamic invocation | facts from headers |
  | Qt | signals and models | everything else |
  | WinUI | | everything |

  For Qt value types without a QMetaObject (QPoint, QUrl), QMetaType is only the carrier. Construct, copy, equals and the QVariant table come from it; members need generated C++ thunks.

## 6. Ergonomics in Svelte

These sketches use the proposed modules. Tags, props and events are native kebab-case; JS classes and methods are JS identifiers.

**A text buffer, shared and observed** (GTK):

```svelte
<script>
  import { TextBuffer } from 'svelters:gtk/gtk';
  const buffer = new TextBuffer({ text: 'Hello' });       // construct: SHARED handle
  let length = $state(buffer.text.length);
  $effect(() => buffer.on('changed', () => (length = buffer.text.length)));   // on() returns the disposer
</script>

<text-view buffer={buffer} />
<text-view buffer={buffer} monospace />                    <!-- two views, one native buffer -->
<label label={`${length} characters`} />
```

The Qt equivalent is `import { QTextDocument } from 'svelters:qt/gui'`, `const doc = new QTextDocument({ plainText: 'Hello' })`, `<text-edit document={doc} />`. `QTextDocument` is a QObject, so it is OWNED and dies with the component, never through GC.

**A native model feeding a list.** `@svelters/lists` takes any `ListModel` as `items`. The controller reads `length`/`at(i)` on `items-changed` and splices item handles, which works because identity is stable. No ABI change is needed. A native `virtuallist` that adopts the model directly is a later optimization for very large models.

```svelte
<script>
  import { ListView } from '@svelters/lists';
  import { DirectoryList } from 'svelters:gtk/gtk';
  import { File } from 'svelters:gtk/gio';
  const dir = new DirectoryList({ file: File.newForPath('/usr/share'), attributes: 'standard::display-name,standard::icon' });
</script>

<ListView items={dir} key={(info) => info}>
  {#snippet row(info)}   <!-- Gio.FileInfo, SHARED -->
    <box spacing={6}><image gicon={info.getIcon()} /><label label={info.getDisplayName()} /></box>
  {/snippet}
</ListView>
```

The Qt version: `const model = new QStringListModel({ stringList: ['a', 'b'] })`, then `<list-view model={model} />` for a plain QListView, or `<ListView items={model}>` for Svelte-rendered rows.

**An icon from a path.** A plain string keeps its current meaning, a theme name. Files are explicit.

```js
import { FileIcon, File } from 'svelters:gtk/gio';
import appIcon from './icons/app.svg';               // the loader turns an asset import into a path
const icon = new FileIcon({ file: File.newForPath(appIcon) });
// <image gicon={icon} />   or, for a texture: <picture paintable={appIcon} />  (X8 string form)

import { QIcon } from 'svelters:qt/gui';
const qicon = new QIcon(appIcon);                    // VALUE; QIcon.fromTheme('document-open') is a static
// <push-button icon={qicon} text="Open" />
```

**Reading a date-time.** Dates and times are values with no class: `Date`, or `{year, month, day}`.

```svelte
<script>
  let when = $state(new Date());
</script>
<date-time-edit bind:date-time={when} calendar-popup />
<label text={when.toISOString()} />
```

Reading the value from a ref: `edit.call('dateTime')` gives a `Date`.

**Async file APIs:**

```js
import { File } from 'svelters:gtk/gio';
import { FileDialog } from 'svelters:gtk/gtk';

const file = await new FileDialog({ title: 'Open' }).open(win);   // Promise<Gio.File>; cancel rejects NativeError code 'dismissed'
const { contents } = await file.loadContentsAsync({ signal });    // _async/_finish -> {ret, contents, etagOut}
```

Qt has no blocking `getOpenFileName`. Use the dialog element with the S2 signal-to-Promise adapter: `<file-dialog bind:this={dlg} file-mode="existing-file" />`, then `const files = await dlg.call('open')`. It resolves `selectedFiles` on accept and rejects on reject.

## 7. Prior art

These links weren't re-fetched in this session; the descriptions come from the projects' documentation.

- **GJS** ([Mapping.md](https://gitlab.gnome.org/GNOME/gjs/-/blob/master/doc/Mapping.md), [memory management](https://gjs.guide/guides/gjs/memory-management.html)):
  - one wrapper per GObject, found through qdata;
  - `new Gtk.TextBuffer({text})` property-dict constructors;
  - boxed values copied on transfer-none returns;
  - `Gio._promisify` for `_async`/`_finish`.

  **Toggle refs** keep a wrapper alive while native code holds the object, so expandos and handlers survive. That machinery (and its GC interplay) is what this design avoids, with SHARED handles and scope-held listeners.
- **node-gtk** ([repo](https://github.com/romgrk/node-gtk)): the same GI mapping on Node/N-API. Note its lazy per-namespace loading (`require('node-gtk').require('Gio', '2.0')`), which is the model for `svelters:gtk/gio`.
- **PySide/Shiboken** ([type kinds](https://doc.qt.io/qtforpython-6/shiboken6/typesystem_specifying_types.html), [ownership](https://doc.qt.io/qtforpython-6/shiboken6/typesystem_ownership.html)):
  - `value-type` (copied, compared with `==`) versus `object-type` (identity, parent ownership) is exactly VALUE versus OWNED/BORROWED;
  - it resolves overloads at runtime by type checks;
  - "mutating a returned value changes nothing" is documented behaviour.
- **PyQt/sip** ([annotations](https://www.riverbankcomputing.com/static/Docs/sip/annotations.html)): `/Transfer/`, `/TransferThis/` and `/TransferBack/` are ownership facts. We already plan to ingest them as `origin: sip` (CUSTOM-WIDGETS §7).
- **NativeScript** ([iOS marshalling](https://docs.nativescript.org/guide/ios-marshalling), [Android marshalling](https://docs.nativescript.org/guide/android-marshalling)):
  - a metadata generator (headers / bytecode) produces a compact blob;
  - the runtime builds JS classes lazily from it and calls through the ObjC runtime + libffi or JNI;
  - structs are JS objects and pointers are `interop.Reference`.

  This is the closest prior art for §4.4 and for "generated JS classes over a reflective engine".
- **React Native Windows / react-native-xaml** ([repo](https://github.com/microsoft/react-native-xaml)): XAML props and events are code-generated from winmd. **NodeRT** ([repo](https://github.com/NodeRT/NodeRT)) projects whole WinRT namespaces into Node modules, with async operations as callbacks or promises. Both generate code; neither invokes dynamically.
- **N-API / Deno** ([N-API object wrap](https://nodejs.org/api/n-api.html#object-wrap), [Deno FFI](https://docs.deno.com/runtime/fundamentals/ffi/)):
  - `napi_wrap`, `napi_add_finalizer`, `napi_create_reference` (weak/strong counts) and `napi_type_tag_object` are the same handle-table shape as ours. A future N-API front maps SHARED to a strong reference released in the finalizer.
  - Deno's `UnsafePointer` is the raw-pointer exposure we refuse (PLAN rule 4).
- **GObject-Introspection rules** ([annotations](https://gi.readthedocs.io/en/latest/annotations/giannotations.html)):
  - `transfer none|container|full` decides BORROWED or SHARED for objects, and ref or copy for boxed values;
  - `caller-allocates` is the out-record case;
  - `scope call|async|notified|forever` maps onto the existing callback scopes;
  - `glib:finish-func`/`glib:async-func` mark async pairs;
  - boxed copy/free are `g_boxed_copy`/`g_boxed_free`, so a VALUE handle owns one copy, or one ref for refcounted boxed types such as GBytes and PangoAttrList.

## 8. Phasing

| Phase | Scope | ABI | Acceptance |
|---|---|---|---|
| **N0 GTK pilot** (one lane) | Every GObject is a handle (SHARED, or reflected kinds); load the Gio/Gdk/Pango/Gsk/GLib typelibs; receivers by GType; `construct`, `call_static`, `SVL_T_TYPE`; out params, statics and GList in gi.rs; VALUE kinds with methods + `equals` for GBoxed. Generated `svelters:gtk/*` modules for a curated set (Gio.File, ListStore, Menu, Settings; Gtk.TextBuffer, TextTag, StringList, DirectoryList, FileDialog; Gdk.Texture, Clipboard; Pango.AttrList, FontDescription), `.d.ts`, docs cards, ledger tests | SHARED, construct, call_static, flags, SVL_T_TYPE | §6 GTK examples run in fixtures on the real host; identity, GC release and `dispose` accounting tests; ledger facts for the curated set |
| N1 GTK async | GI callbacks by scope, finish-func pairs as Promises with `{signal}`; retire hand-written `gio-file` | none | `gio_services.js` passes on the generic path |
| N2 Qt | qtcore/qtgui allowlist ingest; value-kind thunks (const methods, statics, `equals`); QObject non-widgets; QModelIndex records + persistent handles; overload dispatch; dialog Promises | uses N0's | Qt twins of §6; QStringListModel and QStandardItemModel feeding `<ListView>` and native `<list-view>` |
| N3 models and reactivity | `@svelters/lists` accepts native models; optional reactive accessors: getters subscribe to `notify::prop` / Qt NOTIFY through Svelte `createSubscriber`, so `buffer.text` is reactive in effects | none | 10k-item DirectoryList scroll test; no `$state` mirror needed in the TextBuffer example |
| N4 WinUI / AppKit | §4.3/§4.4 when those backends grow past the P set | uses N0's | — |

## 9. Open questions

1. **SHARED versus BORROWED for transfer-none GObject returns.** A strong ref is what GJS and PyGObject users expect, and it can't produce zombies for non-widgets. However, a strong ref on a GtkWindow-like non-widget (a GdkSurface) could keep native resources alive longer than intended. Proposal: SHARED for every non-widget GObject except kinds whose metadata says `lifetime: borrowed`.
2. **Reactive accessors (N3).** Should generated getters on objects be reactive by default, or opt in (`reactive(buffer).text`)? Default-reactive costs one notify connection per property read inside an effect.
3. **Mutators on VALUE kinds.** `QUrl::setScheme`, `pango_attr_list_insert` and `QFont::setBold` mutate. The options:
   - not exposing them (immutable, simplest);
   - exposing `with*` variants that return a new value;
   - a builder from records (`new AttrList([{type: 'weight', value: 'bold', start: 0, end: 5}])`).

   The proposal is no mutators in N0, plus records-list constructors for PangoAttrList, QTextCharFormat and NSAttributedString.
4. **Overload naming for Qt.** Should ambiguous overload sets be split by metadata or dispatched at runtime? This needs a measurement over the qtcore/qtgui allowlist.
5. **Module granularity.** Per namespace (`svelters:gtk/gio`) or one module per toolkit with namespace objects (`import { Gio } from 'svelters:gtk'`)? Per namespace tree-shakes better for the M6 binding linker, because static imports name the roots.
6. **NSAttributedString and rich text** need a shared "runs" record shape. PangoAttrList, QTextCharFormat ranges and NSAttributedString are similar, but no portability layer is allowed. Should each toolkit get its own native-shaped record?
7. **GLib main-context APIs** (`GLib.timeout_add`, `idle_add`) duplicate JS timers. Leave them unbound so that JS timers stay authoritative (PLAN rule 6).
8. **Expandos on wrappers.** Is `Object.preventExtensions` in dev too strict for user code that tags objects? Recommending a `WeakMap` instead is the GJS-documented workaround.
9. **Should native models ever round-trip through `call_sync`?** A JS-implemented `GListModel` (2021's design) is not needed if `@svelters/lists` keeps the model in JS. Keep it out unless a native API demands a model JS must implement, such as a `QAbstractItemModel` subclass for `QCompleter`.
