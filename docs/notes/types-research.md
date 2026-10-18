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
