# Non-GUI type bindings research (working notes)

Question: how JS creates, holds and passes the non-GUI types of each toolkit (GLib/Gio/Gdk/Pango/Graphene/Gsk; QtCore/QtGui; Windows.Foundation/Microsoft.UI; Foundation/AppKit values).
Final proposal: `plans/research/NON-GUI-TYPES.md`. Scripts: `docs/notes/scripts/types_*.py`.

## 0. What already exists (read 2026-10-17)

- ABI v3 (`crates/svelters-abi/include/svelters_backend.h`): tags ENUM/FLAGS/RECORD/BYTES/BOXED/CALLBACK/MAP/ERROR; slot/member/type descriptors; `describe`/`describe_type`; `cancel`; host `invoke_callback`/`release_callback`. Well-known records Rect/Size/Point/Insets/Color/Date/Time/DateTime/Variant/TypedVariant. `SVL_D_STRING_FORM` lets a RECORD/BOXED slot accept a canonical string ("#ff8800", "Sans 12", icon name).
- BOXED = VALUE lifetime class (S2): refcounted immutable value behind a never-reused handle; `destroy` = unref; `handle_info` -> lifetime VALUE, kind 0. JS wrapper has `dispose()`; FinalizationRegistry queues release, drained at checkpoint end (`Core::process_releases`). QuickJS-NG holds the unregister token strongly, so no token is used.
- Objects (X1): one handle per GObject/QObject (GTK qdata, Qt by_obj side table), borrowed handles weak (GWeakRef / QPointer). Objects with a generated kind travel as HANDLE; other GObjects (PangoLayout, GIcon) travel as opaque BOXED. Renderer `nodeFor(h)` gives one borrowed node per handle (WeakRef map + FinalizationRegistry).
- Only kinds can be *created* (`create(kind)`), and only Gtk-namespace GObjects + a few Qt extras (QTextDocument) are kinds. Gdk/Pango/Gio objects are not ingested as kinds (G11). The hand-written `gio-file` "extra" kind exists (async loadContents via SVL_M_ASYNC).
- GTK dynamic GI calls (X1/S2): methods on kinds through girepository; rejected: static functions/constructors, GList/GHashTable, unions, GType, callbacks on the GI path(?), methods on more-derived types.
- js-native-serdes.md §1 already counted the widget surface (Gtk namespace + QtWidgets). Most frequent GTK: GtkTextIter 77, GListModel 64, GStrv 46, GdkRGBA 24, GdkPaintable 21, ... Qt: QSize 155, QPointF 117, QGraphicsItem* 93, QRectF 93, QModelIndex 92, ... This research extends that to: which non-GUI types, what is supported today, and the construction/identity/ABI questions it left open.

## 1. Inventory (measured 2026-10-17)

### 1.1 GTK (`scripts/types_gir_inventory.py`, GIR of GTK 4.22 on this box)

Scope: public, non-deprecated, introspectable props, params, returns and signal args of every Gtk-namespace class/interface/record (widgets and non-widgets). Skipped: primitives, strings, enums/flags, GtkWidget subclasses. 263 distinct non-GUI types, 1422 slot references.

| status today | slots | types | meaning |
|---|---:|---:|---|
| handle:gtk-kind | 473 | 106 | Gtk-namespace GObject non-widgets (Adjustment, ListItemFactory, Expression, SelectionModel, TextBuffer, ...): kinds exist, creatable, props/signals/GI methods work |
| opaque:boxed-object | 286 | 32 | GObjects outside Gtk (GListModel, GObject, GMenuModel, GCancellable, GAsyncResult, GdkDisplay, GdkClipboard, ...): cross as BOXED, **no identity (new handle per crossing), no props/methods** |
| opaque:boxed | 269 | 32 | GBoxed records (TextIter, Bitset, PaperSize, PangoAttrList, PangoTabArray, GBytes, GskTransform, ...): BOXED, no methods |
| string-form + opaque | 101 | 7 | GFile, GdkPaintable/Texture, GIcon, GdkCursor, PangoFontDescription, GtkShortcutTrigger: string in, opaque out |
| rejected: callback (GI path) | 79 | 33 | DestroyNotify, AsyncReadyCallback, ... (only hand-written `gio-file` has async/callbacks) |
| record | 61 | 6 | GdkRGBA, GdkRectangle, graphene rect/point/size, GtkBorder |
| strv | 49 | 2 | |
| rejected: arrays/lists | 52 | 34 | |
| variant (GVariant, GValue) | 28 | 2 | |
| rejected: struct without GType | 16 | 7 | CssLocation, ... |
| datetime | 7 | 1 | |

Top 25: TextIter 96, GListModel 61, strv 47, GObject 39, GFile 39, GMenuModel 36, Adjustment 35, ListItemFactory 33, Expression 24, PageSetup 24, GdkPaintable 22, DestroyNotify 22, PrintSettings 21, GCancellable 20, GAsyncReadyCallback 20, GAsyncResult 20, GdkRGBA 18, SelectionModel 18, Sorter 17, TextTag 17, GVariant 16, Bitset 16, graphene Rect 16, GdkDisplay 15, ShortcutTrigger 15.

By namespace (slots): Gtk 719, Gio 219, Gdk 153, GLib 79, GObject 68, Pango 62, Graphene 33, Gsk 30.

Size of the non-GUI namespaces themselves (non-deprecated, introspectable): Gio 106 classes / 37 interfaces / 215 records / 1599 callables; GLib 75 records / 926 callables; Gdk 42 classes / 522 callables; Gsk 42 classes / 285; Pango 10 classes / 42 records / 357; Graphene 19 records / 382; GObject 30 classes / 183. Total ~4250 callables: hand-annotation is out of the question, reflection is the only scalable path.

Key observation: the biggest *practical* gap is not unsupported types but **non-Gtk GObjects having no identity and no member access** (GListModel/GListStore, GMenuModel, GdkClipboard, GdkDisplay, GSettings, GFile beyond its string form). `lookup::is_handle_type` = "has a Gtk kind or is a widget"; everything else goes through `mint_boxed`, a fresh strong-ref VALUE handle per crossing.

### 1.2 Qt (`scripts/types_qt_inventory.py`, metadata/qt: 195 classes, 73 widgets)

179 distinct non-GUI types, 2297 slots.

| status | slots | types |
|---|---:|---:|
| rejected: opaque | 746 | 83 |
| record (QSize/QPoint(F)/QRect(F)/QMargins/QColor) | 589 | 9 |
| boxed (QIcon, QFont, QPainterPath, QBrush, QPixmap, QUrl, QPalette, QTransform, QKeySequence, QTextCursor, QSizePolicy, ...) | 407 | 21 |
| handle: QObject with kind (QAction, QLayout, QStyle, QTextDocument, ...) | 200 | 18 |
| handle: QObject without kind (QObject*, QAbstractItemModel*, QValidator*, QCompleter*, ...) | 111 | 15 |
| date/time | 58 | 3 |
| lists (QList<T>) partial/rejected | 90 | 27 |
| variant / QStringList / QByteArray | 96 | 3 |

Top rejected: QGraphicsItem* 92, QModelIndex 92, QTreeWidgetItem* 56, QTableWidgetItem* 35, QPainter* 34, QPolygonF 33, QGraphicsLayoutItem* 33, QStyleOption* 31, QListWidgetItem* 30, QLayoutItem* 22, QDataStream 22, qreal*/int*/bool* out pointers 37, QStyleOptionViewItem 14, QEvent* 12, QVector3D 9, QTextBlock 9.
Boxed values are opaque too: the generic `Conv<T>` boxed converter (serdes.hpp.in) holds a QVariant copy, string form for QIcon (fromTheme)/QKeySequence/QUrl/QFont, **no methods** on them.
The ingest only covers QtWidgets + an allowlist (QAction, QActionGroup, QTextDocument), so QtCore objects (QTimer, QSettings, QFile, QFileSystemWatcher, QStandardItemModel, QStringListModel, QSortFilterProxyModel) are absent from metadata entirely.

### 1.3 WinUI / AppKit (checked-in metadata is a P-scale sample)

- WinUI (15 classes): non-GUI refs are `value<Thickness>` 11, `Brush` 10 (+SolidColorBrush 3), `DependencyProperty` 9, `Vector3` 5, `FlyoutBase` 5, `TextPointer` 6, `Size`/`Rect`/`Point`/`CornerRadius`, `Uri` 2, `FontFamily` 2, `TransitionCollection` 3, `DataTemplate` 3, `object` (IInspectable boxed) 14. `_types.json` already records WinRT structs with fields.
- AppKit (11 classes): `NSSize` 14, `NSRect` 12, `NSColor` 8, `NSEdgeInsets` 4, `NSImage` 3, `NSAttributedString` 5 (mapped to `string` today), anchors/layout guides 14, `NSArray<...>` 8, `id` 9, `CIFilter` 1 (unsupported).

## 2. Qt reflection reach (probe `scripts/types_qt_reflect_probe.cpp`, Qt 6.11, offscreen)

Value types: every one is default/copy constructible and equality comparable through QMetaType (QIcon/QPixmap are not equality comparable). A QString converter is registered only for QColor, QUrl, QDate/QTime/QDateTime, QUuid, QByteArray, QStringList, QJsonValue, QFont, QKeySequence. **No value type exposes methods through a metaobject**: the Q_GADGETs (QLocale, QFont, QIcon, QImage, QPalette, QKeySequence, QSizePolicy, QTextCharFormat, QColorSpace, QEasingCurve) carry only Q_ENUMs; QMimeType is the only gadget with properties (13) and an invokable. So QMetaType gives construct/copy/destroy/equals/(some) toString generically, nothing else: members of QUrl/QFont/QDateTime need generated C++ (qdoc-driven, the same generator as widgets).

QObjects, QMetaObject members below QObject (props / signals / slots / invokables):

| class | props | signals | slots | invokables |
|---|---:|---:|---:|---:|
| QTimer | 5 | 1 | 3 | 0 |
| QSettings | 0 | 0 | 0 | 0 |
| QFile | 0 | 6 | 0 | 0 |
| QFileSystemWatcher | 0 | 2 | 0 | 0 |
| QStandardItemModel | 1 | 24 | 4 | 46 |
| QSortFilterProxyModel | 11 | 32 | 15 | 50 |
| QItemSelectionModel | 5 | 5 | 7 | 13 |
| QMimeData | 0 | 0 | 0 | 0 |
| QClipboard | 0 | 4 | 0 | 0 |
| QAction | 18 | 8 | 8 | 0 |
| QCompleter | 9 | 4 | 8 | 0 |
| QTextDocument | 14 | 10 | 5 | 1 |
| QProcess | 0 | 13 | 2 | 0 |

Reflection gives signals everywhere and props where the class has Q_PROPERTYs, and item models are unusually well covered (QAbstractItemModel's `index/data/rowCount/setData/...` are Q_INVOKABLE). But QSettings, QFile, QMimeData, QClipboard and QProcess are almost entirely plain C++ methods. No class has a Q_INVOKABLE constructor. Conclusion: Qt non-GUI types need the same qdoc → generated C++ thunk path as widgets; QMetaObject is a complement (signals, dynamic props, app-defined Q_OBJECTs as in CUSTOM-WIDGETS), not the primary engine.

## 3. GI reach for the non-GUI namespaces (`scripts/types_gi_callable_blockers.py`)

Each non-deprecated introspectable callable, by the first invoker tier that covers all of its slots. "now" = the type matrix gi.rs supports today, *assuming the receiver is a handle the invoker accepts* (today only Gtk-kind receivers are). Cumulative %:

| ns | callables | now | +out/inout | +static/ctor | +records by value (GType, unichar, unions, no-GType structs) | +arrays/lists | +callbacks/async | never (gpointer, varargs) |
|---|---:|---:|---:|---:|---:|---:|---:|---:|
| GLib | 1383 | 37% | 38% | 67% | 70% | 78% | 82% | 18% left |
| GObject | 301 | 32% | 32% | 46% | 52% | 54% | 55% | 45% left |
| Gio | 1668 | 66% | 68% | 82% | 82% | 88% | 97% | 3% |
| Gdk | 543 | 74% | 79% | 89% | 89% | 92% | 94% | 6% |
| Gsk | 290 | 61% | 68% | 83% | 88% | 94% | 96% | 4% |
| Pango | 419 | 60% | 63% | 81% | 89% | 96% | 97% | 3% |
| Graphene | 382 | 48% | 84% | 95% | 95% | 100% | | |
| Gtk | 2944 | 81% | 84% | 95% | 95% | 97% | 99% | 1% |

Reading: for Gio/Gdk/Pango/Gsk the generic invoker already covers 60-75% once receivers are handles; out params (S2 has the host side) + static functions/constructors take them to 80-90%; callbacks/async (host side exists, S1) to ~95%. GLib/GObject are mostly not worth binding (gpointer, varargs, low-level utilities): expose a curated slice (GLib.Variant, Bytes, DateTime/TimeZone, KeyFile?, uri utils) and leave the rest. Graphene is almost all out-param structs (caller-allocates `graphene_rect_t *res`): those are value records in JS, so its methods are better as pure-JS helpers on the records than as native calls.

## 4. Design decisions (draft, feeding plans/research/NON-GUI-TYPES.md)

- **Two representations only**: plain JS records (by value, snapshot) and handle-backed JS class instances. Opaque boxed without methods (today) becomes a handle-backed instance *with* a kind.
- **Every native object gets a handle, never BOXED** (GObject, QObject, IInspectable runtime class, NSObject that is not a value type). Kind = generated kind if any, else a reflected kind (GTK: from GType + typelib, lazily), else nearest ancestor. Fixes identity for GListModel/GMenuModel/GdkClipboard/...
- **New lifetime class SHARED** for refcounted non-tree objects (GObject non-widgets, COM, ARC): table holds a strong ref; identity by qdata / side table (safe to key by address *because* the ref is strong); `destroy` = drop our ref; the host may release it from FinalizationRegistry. OWNED keeps "JS decides death, scope-driven, never GC" (widgets, JS-created QObjects). Qt QObjects cannot be SHARED (no refcount): JS-created = OWNED tied to the component scope; returned = BORROWED.
- **VALUE handles get kinds and methods** (read-only calls), so `font.family`, `url.host()`, `attrs.toString()` work; `set_prop` on VALUE is refused (immutable snapshot semantics).
- **Construction**: `construct(kind, props)` (GObject construct props, construct-only included) and `call_static(kind, method, args)` (constructor functions, static methods, `Gio.File.newForPath`, `QUrl.fromLocalFile`, `Uri.EscapeComponent`, `+[NSURL fileURLWithPath:]`). Both additive, nullable vtable members.
- **Type values** (GType for `Gio.ListStore.new(itemType)`, `Gtk.StringObject`): well-known type id `SVL_T_TYPE` on an `STR` carrying the kind name; JS passes the class object.
- **JS surface**: built-in modules `svelters:gtk/<ns>` (`gio`, `gdk`, `pango`, `glib`, `gtk`, `gsk`, `graphene`), `svelters:qt/<module>` (`core`, `gui`, `widgets`), `svelters:winui/<namespace>`, `svelters:appkit/<framework>`. Classes keep native names as JS identifiers (`ListStore`, `QTimer`, `Uri`, `NSURL`); methods camelCase; signals kebab-case through `obj.on('items-changed', fn)`; props as accessors (camelCase JS identifiers) — kebab-case stays for markup.
- **Thread affinity**: all handles are main-thread; backends marshal async completions (WinRT IAsyncOperation, NSURLSession, GTask threads) to the UI loop before `invoke_callback`. JS workers never see handles.

## 5. Status

Proposal written: `plans/research/NON-GUI-TYPES.md` (taxonomy, ABI draft: SHARED lifetime + `construct` + `call_static` + `SVL_T_TYPE` + `SVL_M_STATIC/CTOR`, per-toolkit plan, generation, Svelte examples, prior art, phasing N0-N4, open questions). No production code changed. Probe binary for Qt was built in the session scratchpad, not in the repo.

## 6. Follow-up (lead, 2026-10-17): metadata + automation, hand-written core only

Request: no per-type hand bindings. Find each toolkit's machine-readable source of truth, auto-classify into the taxonomy by rules, keep whitelists/overrides small, and draw the boundary: hand-written object-model core per toolkit; everything else generated and calling only that core. (Widget facts belong to the annotation-automation agent, `docs/notes/annotation-automation-research.md`; this lane covers non-GUI types only.)

### 6.1 Sources found on this machine

| Toolkit | Source | Where | What it gives |
|---|---|---|---|
| GTK | GIR (+ typelib at runtime) | `/usr/share/gir-1.0`, `/usr/lib/girepository-1.0` | everything: GType, fundamental + ref/unref funcs, boxed get-type, record fields + `private`, `transfer`, `caller-allocates`, `scope`, `glib:finish-func`, nullable, error domains |
| Qt | qdoc indexes | `/usr/share/doc/qt6/<mod>/<mod>.index` | classes, bases, every public function with parameter/return spellings, properties, enums, status (deprecated), `static` |
| Qt | PySide6 typesystem XML | `/usr/share/PySide6/typesystems/*.xml` (101 files) | value-type 455 / object-type 1176 / primitive-type 54 / container-type 9; 34 class rejections; removed members; 483 ownership modifications (parent/define-ownership/reference-count) |
| Qt | moc metatypes JSON | `/usr/lib/qt6/metatypes/qt6*_metatypes.json` | 2764 Q_OBJECT + 590 Q_GADGET classes with props/signals/slots/invokables (moc --output-json, shipped) |
| Qt | PyQt6 sip | `site-packages/PyQt6/bindings/Qt*/*.sip` (1002 classes) | `/Transfer/` 239, `/TransferThis/` 304, `/TransferBack/` 45, `/Factory/` 69, `/KeepReference/` 71, `/In/` 26 / `/Out/` 21 (sip treats non-const `T*` of basic types as out by default) |
| Qt | libclang over headers | `/usr/include/qt6`, `python-clang` 21 installed | nested public-field structs qdoc does not describe as records (QFont::Tag, QTextOption::Tab, QCalendar::YearMonthDay, QTextLayout::FormatRange) |
| Apple | objc2 header-translator output | `~/.cargo/registry/src/*/objc2-foundation-0.3.2`, `objc2-app-kit-0.3.2` (MIT/Apache, readable on Linux) | super chains, `thread_kind = MainThreadOnly`, Send/Sync, NSCopying, property setters ("Setter for"), safe vs unsafe methods, DynBlock params, `#[repr(C)]` structs; plus `translation-config.toml` skip lists |
| Apple | SDK headers via libclang | MAC1 `tools/metadata/appkit/ingest.py` (on a Mac) | nullability, generics, availability, NS_SWIFT_ASYNC |
| WinUI | winmd | `target/winsdk/**.winmd` (fetch.py) + `windows-default` crate (Windows.winmd) through `winmd-tool` | every type: struct/enum/runtime class/interface, activatable + statics + composable factories, IAsync*, IVector*, IReference, Exclusive/Default interfaces |

### 6.2 Auto-classifier prototypes (no hand lists except record shapes / exclusion regexes)

- `scripts/types_gir_classify.py` (GTK stack, GIR): 26 record, 182 value, 326 object, 24 model, 3 async-infra, 8 transient, 97 needs-override, 244 enum, 104 flags, 159 callback (all namespaces). Members of bindable types: **5990, 96% auto** (Gio 97%, Gdk 97%, Gsk 94%, Pango 94%, Graphene 100%, Gtk 97%, GLib 90%, GObject 83%); 765 excluded automatically (deprecated/non-introspectable 390, ref/unref/copy/free/qdata 225 = core, `_finish` 150 folded). Blockers: needs-override types 75, gpointer 62, transient (TextIter) 36, non-string hash tables 17.
  - Scope policy "closure of types reachable from the Gtk API": GLib 12, GObject 6, Gio 40, Gdk 28, Gsk 10, Pango 19, Graphene 16 bindable types (of 36/34/154/62/53/26/17). In scope: 26 needs-override types; 28 distinct types block in-scope members (TextIter 35 by far; then OutputVector 4, RoundedRect 4, Pango Attr* subtypes).
  - Mutability is **not** derivable from C: 42 value types take `const self` everywhere, 140 do not, including immutable GBytes and GskRenderNode. So VALUE handles expose all generated methods; "snapshot" comes only from transfer (copy-boxed copies, refcounted boxed shares).
- `scripts/types_qt_classify.py` (QtCore/QtGui/QtWidgets, qdoc + PySide + moc): 17 record (shape whitelist), 116 value, 115 object, 12 model, 8 statics-only, 33 unique (non-QObject C++ objects with a ctor), 24 item, 3 async, 82 transient (events, paint, style options), **8 needs-override**, 8 unclassified, 221 excluded by one machinery regex (templates, smart pointers, iterators, locks, JNI/Android/Mac/Vulkan/RHI/OpenGL, accessibility). Members: **7560, 92% auto** (QtCore 89%, QtGui 95%, QtWidgets non-widgets 92%); 1559 excluded automatically (special members 431, operators 679, removed-by-PySide 281, deprecated 90, templates/platform 69). Blockers: unknown spellings 174 (90 distinct, mostly nested structs and std types), out-pointers 121 (sip default rule settles them), transient 83 (callback-only types), QBindable 39 (skip).
- `scripts/types_objc2_classify.py` (Foundation + AppKit via objc2): Foundation 190 object / 82 value / 8 record; AppKit 190 object / 29 value / 1 record / 78 widget. 491 non-widget classes, 49 MainThreadOnly, 88 Send. Methods 9697, **82% objc2-safe** (Foundation 79%, AppKit 86%); 226 with block params, 113 completion-handler shaped. Rule misses spotted: NSColor (one class-property setter) lands in object; NSURLSession*Task in value. Overrides needed in the dozens.
- WinUI: rules only (no prototype run; winmd-tool needs a windows-bindgen build). WinRT metadata is the most explicit of the four: struct → record; runtime class → object (SHARED, COM); `IAsyncOperation*`/`IAsyncAction*` → async; `IVector`/`IObservableVector`/`IMap` → model; `IVectorView`/`IIterable` → list; `IReference<T>` → nullable T; activatable/statics/composable attributes → construct/call_static. Expect few overrides beyond DependencyObject-specific rules.

### 6.3 Core / generated boundary (sizing from existing code)

Existing core-like code: GTK `convert.rs` 944 + `gi.rs` 778 + identity/lifetime parts of `backend.rs` (~300) + `types.rs` 193 ≈ 2.2k lines; Qt `serdes.hpp.in` 974 + handle/identity parts of `backend.cpp.in` (~350). Estimates for the full non-GUI core: GTK ~3.0k (≈2.2k exist), Qt ~2.2k (≈1.3k exist), WinUI ~2.0k, Apple ~2.5k. Details in the proposal §2.

### 6.4 Status
Proposal rewritten around the core/generated boundary and the metadata pipeline: `plans/research/NON-GUI-TYPES.md` (§2 core API per toolkit with sizes, §3 sources/rules/measured coverage/override counts/pipeline). Classifier prototypes: `types_gir_classify.py`, `types_qt_classify.py`, `types_objc2_classify.py`.

## 7. User decisions (2026-10-17) and follow-up measurements

Decisions: Q1 SHARED for transfer-none non-widget returns; Q3 hide VALUE mutators (metadata-derived list); Q7 objc2 generated sources are the primary Apple ingest; Q8 investigate WinUI threading. Q2/Q4/Q5/Q6 still open. Recorded in the proposal §10.

### 7.1 Correction

`types_gir_classify.py` read `c:type` without the XML namespace, so every instance parameter looked non-const. Fixed (`CNS`). Corrected count: 79 GTK value types take `const self` everywhere, 103 do not (earlier note said 42/140).

### 7.2 Mutator hide list (`scripts/types_value_mutators.py`)

- GTK, bound scope (Gtk + closure): keep 598 (const self, reading names, `self` transfer full = consuming chain), hide 209 (mutating names), residual **22 methods in 12 types**; 1 builder (`*Builder`) kept mutable. Whole stack before scoping: residual 140 methods in 46 types, mostly GLib utilities that are out of scope anyway.
- Qt: keep 2345 const/static, hide 978 non-const, 30 non-const twins dropped; 27 non-const methods with reading-like names flagged (QImage `load*` etc.) and all genuinely mutate: **0 hand entries**.
- Apple: keep 2018, hide 34 (property setters, set/add/remove/insert/replace/append selectors); 12 value classes are immutable halves of NSMutable pairs: **0 member entries**.

### 7.3 WinUI threading (`scripts/winrt_threading_probe/`, Rust, windows-metadata 0.100)

- Built through cargo-slot (standalone crate, own `[workspace]`). Inputs: Microsoft.UI.Xaml.winmd (WinUI 2.3.9), Microsoft.UI.winmd + Microsoft.Foundation.winmd (interactiveexperiences 2.1.9), Windows.winmd (windows-default 0.100).
- Every attributed class in Microsoft.UI.*, Windows.Foundation is `Threading(Both)` + `MarshalingBehavior(Agile)`; 218 classes carry only `MarshalingBehavior(Agile)`. That includes UIElement (185) and non-visual DependencyObjects (365). The attributes describe COM marshaling only.
- Microsoft docs (DependencyObject class reference, fetched 2026-10-17): all DependencyObject instances must be created on the UI thread of the current Window; every API except `DispatcherQueue` throws from other threads. CompositionObject (also Both/Agile) exposes `DispatcherQueue` too. The Windows App SDK threading migration guide: standard STA (not ASTA), `DispatcherQueue.HasThreadAccess` / `TryEnqueue`.
- Structural rule: a class with `DispatcherQueue` in its chain is dispatcher-affine — 696 classes (DependencyObjects, Composition, Window, AppWindow, input sources). Windows.Foundation (23 classes) and static helpers are free-threaded. Non-DO XAML types (FontFamily, TransitionCollection, event args) treated as UI-thread conservatively.
- Proposal §4.1: everything SHARED; single UI thread (no thread-per-window); HasThreadAccess at minting (all calls in debug); completions hop to UI before conversion; releases at checkpoints on UI; IClosable.Close as an explicit method.
