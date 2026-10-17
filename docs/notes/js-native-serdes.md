# JS <-> native serialization ("serdes") research

Status: complete first pass (2026-10-17). Research note, not a plan; recommendations feed an ABI v2 proposal.

Experiments: `/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/serdes/` (scripts reproduced inline where load-bearing).

## 0. Summary

- **Measured surface (§1).**
  - T0 (scalars, strings, enums, flags, object handles) covers 92–94% of GTK properties and 90% of the GTK P kinds' methods.
  - It covers only 79% of the Qt P kinds' properties, because Qt exposes geometry as `QSize`/`QRect`/`QPoint`. Adding value structs lifts Qt P properties to 92%.
  - Everything else in the long tail is best treated as a handle, so T2 needs one generic "boxed handle" mechanism rather than many serializers.
  - 64-bit values beyond 2^53 essentially do not occur. Callbacks and async calls are GTK-only and few: 33 + 20 methods.
- **Encoding (§2).**
  - enums: names; flags: arrays of names;
  - value structs: plain objects with neutral field names shared by both toolkits;
  - rich values: strings where a canonical form exists;
  - boxed values: disposable wrappers released at checkpoints;
  - bytes: borrowed `Uint8Array`;
  - datetimes: `Date`, plus part records for dates and times;
  - callbacks: a host table tied to an owner handle;
  - async methods: Promise + AbortSignal;
  - native errors: `GError` → `NativeError`.
- **Value model (§5.1).** Keep nested `svl_value` (24 bytes, borrowed) as the one backend ABI. v2 splits the tag word into `tag/flags/type`, adds `ENUM/FLAGS/RECORD/BYTES/BOXED/CALLBACK/MAP/ERROR`, a UTF-16 string flag, and static descriptors published by the backend. No BIGINT, no CBOR/FlatBuffers on the backend ABI. A packed op buffer is an optional *front* encoding for N-API and Deno FFI only.
- **Codegen (§3).**
  - Converters are generated per type and shared by every member: `Conv<T>` templates in Qt, a GType/arg-plan registry in GTK dev, and Rust fns in GTK release.
  - Host-side conversion is type-directed, written once in `svelters-core` over a small per-engine reader trait. That is where validation and error messages live.
  - Tree-shaking keeps the type closure of the release roots.
- **Performance (§4).**
  - The JS→native call (~0.8 µs on a heavily loaded machine) dominates. Scalar encoding is noise.
  - The biggest avoidable cost today is passing names: 1.43 → 0.77 µs/op with ids.
  - Strings: ASCII is free with borrowing, and non-ASCII to Qt is ~20× cheaper as UTF-16.
  - Structs: an object costs the same as an array, but Svelte `$state` proxies add ~2.5 µs per 4-field read.
  - Batching does not pay in QuickJS, because JS-side encoding costs as much as a call.


## 1. Type inventory (measured)

Scripts: `docs/notes/scripts/serdes_gir_inventory.py` (parses `Gtk-4.0.gir` plus the 11 GIRs it references, stdlib only) and `docs/notes/scripts/serdes_qt_inventory.py` (reads the checked-in `metadata/qt/*.json`, 192 QtWidgets classes, and re-classifies each slot from its C++ spelling, using enum/flags/QObject facts from the qdoc `.index` files). The Qt metadata's own `type.ref` is too coarse for this: 2619 of 5295 parameter/property slots are `ref: "opaque"` (every enum parameter, `QSize`, `QIcon` …), so the neutral vocabulary is not yet filled in for Qt. GTK numbers come from GIR directly rather than `metadata/gtk/*.json`, because the GIR also covers Gdk/Gio/Pango types referenced from Gtk.

Scope: public, non-deprecated, introspectable members of `Gtk` classes/interfaces (GIR) and of the 192 QtWidgets classes (Qt). A "slot" is one property, one parameter, one return value or one signal argument. For GTK, `length`, `user_data` and `destroy` parameters are folded into their owning array/callback parameter, and `throws="1"` counts as an `error(out)` slot. Counts on 2026-10-17: GTK 4.22.4, Qt 6 (Arch `qt6-doc`).

### 1.1 Slot counts by category

All members, both toolkits (`(out)` = out or in-out parameter):

| Category | GTK props | GTK method slots | GTK signal slots | Qt props | Qt method slots | Qt signal slots |
|---|---:|---:|---:|---:|---:|---:|
| bool | 441 | 670 | 87 | 171 | 597 | 23 |
| integers (≤32-bit) | 209 | 421 | 57 | 111 | 1064 | 85 |
| i64/u64 | 2 | 5 | 0 | 0 | 2 | 0 |
| f32/f64 | 44 | 194 | 57 | 38 | 376 | 7 |
| string | 175 | 378 | 30 | 59 | 374 | 24 |
| enum | 111 | 259 | 34 | 105 | 562 | 11 |
| flags | 19 | 52 | 9 | 45 | 242 | 6 |
| object ref (GObject / QObject*) | 201 | 743 | 69 | 3 | 812 | 18 |
| value struct (see 1.2) | 12 | 72 | 3 | 50 | 531 | 15 |
| rich value class (QFont, QPalette, QBrush, QKeySequence, QUrl, QPainterPath …) | – | – | – | 23 | 328 | 12 |
| boxed/refcounted (PangoAttrList, GskTransform, PangoFontDescription …) | 19 | 83 | 7 | – | – | – |
| image/icon (GdkPaintable/Texture/GIcon/GdkPixbuf; QIcon/QPixmap/QImage) | 14 | 25 | 1 | 7 | 88 | 1 |
| bytes (GBytes, guint8[]; QByteArray) | 1 | 6 | 0 | 0 | 18 | 0 |
| string list (GStrv; QStringList) | 13 | 33 | 0 | 3 | 34 | 1 |
| other list (C array, GList/GSList, PtrArray; QList<T>) | 0 | 44 | 0 | 1 | 83 | 3 |
| GListModel | 18 | 46 | 0 | – | – | – |
| QModelIndex | – | – | – | 0 | 81 | 11 |
| iterator structs (GtkTextIter, GtkTreeIter/Path) | 1 | 64 | 36 | – | – | – |
| non-QObject pointers (QGraphicsItem*, QTreeWidgetItem*, QPainter*, QEvent*, QLayoutItem*) | – | – | – | 1 | 358 | 28 |
| variant (GVariant, GValue; QVariant) | 3 | 21 | 1 | 1 | 35 | 0 |
| dates/times (GDateTime; QDate/QTime/QDateTime) | 1 | 2 | 0 | 13 | 41 | 7 |
| GType | 16 | 11 | 0 | – | – | – |
| callback (non-async) | – | 33 | – | – | 1 | – |
| async (GAsyncReadyCallback + GCancellable) | – | 20 + 20 | – | – | – | – |
| GError (`throws`) | 2 | 56 | 2 | – | – | – |
| scalar out params | – | 99 | 2 | – | 51 | – |

GTK callback scopes (all Gtk callables): `notified` 23, `call` 9, `async` 21 (20 of them `GAsyncReadyCallback`), none without a scope after filtering non-introspectable ones. Qt's QtWidgets surface has a single callback-typed parameter (a functor template); Qt's callbacks are signals.

Most frequent non-trivial types:

- GTK: `GtkTextIter` 77, `GListModel` 64, `GStrv` 46, `GdkRGBA` 24, `GdkPaintable` 21, `GdkRectangle` 17, `graphene_rect_t` 16, `GVariant` 14, `PangoAttrList` 14, `GIcon` 13, `PangoTabArray` 13, `GtkTreeIter`/`GtkTreePath` 12 each, `GValue` 11, `PangoFontDescription` 10, `graphene_point_t` 10, `GList<GObject>` 9, `GBytes` 7, `GskTransform` 7.
- Qt: `QSize` 155, `QPointF` 117, `QGraphicsItem*` 93, `QRectF` 93, `QModelIndex` 92, `QRect` 85, `QPoint` 69, `QIcon` 58, `QTreeWidgetItem*` 56, `QPainterPath` 48, `QFont` 44, `QBrush` 39, `QStringList` 38, `QVariant` 36, `QColor` 34, `QPixmap` 32, `QDate` 30, `QUrl` 23, `QPalette` 22, `QByteArray` 18.

### 1.2 Coverage by tier

A member is covered by a tier if every slot of it is. Tiers:

- **T0**: bool, integers, i64, f64, string, enum, flags, object handle, void. This is today's `svl_value` plus enum/flag naming.
- **T1**: T0 + small value structs (QRect/QRectF/QSize/QSizeF/QPoint/QPointF/QMargins/QLine/QColor/QVector*; GdkRGBA/GdkRectangle/GtkBorder/GtkRequisition/graphene rect/point/size/vec/matrix/PangoRectangle) + string lists + scalar/struct out params.
- **T2**: T1 + handle-like non-objects (boxed refs, QIcon/QPixmap/QImage, rich value classes, bytes, lists of anything, GListModel, QModelIndex, iterators, non-QObject pointers, dates, variants, GType).
- **T3**: T2 + GError, callbacks, async.

| Surface | members | T0 | T1 | T2 | T3 |
|---|---:|---:|---:|---:|---:|
| GTK P kinds (+Widget, Editable, Orientable): properties | 161 | 94% | 95% | 100% | 100% |
| — methods | 436 | 90% | 94% | 100% | 100% |
| — signals | 34 | 97% | 100% | 100% | 100% |
| GTK ~60 common kinds: properties | 439 | 94% | – | – | – |
| — methods | 1277 | 86% | – | – | – |
| — signals | 144 | 92% | – | – | – |
| GTK all: properties | 1303 | 92% | 94% | 100% | 100% |
| — methods | 2650 | 83% | 86% | 96% | 100% |
| — signals | 348 | 89% | 91% | 99% | 100% |
| Qt P kinds (+QAbstractButton, QFrame, QAbstractScrollArea): properties | 128 | 79% | 92% | 100% | 100% |
| — methods | 548 | 75% | 87% | 99% | 99% |
| — signals | 22 | 86% | 95% | 100% | 100% |
| Qt ~50 common kinds: properties | 399 | 87% | – | – | – |
| — methods | 1916 | 74% | – | – | – |
| — signals | 143 | 71% | – | – | – |
| Qt all: properties | 634 | 84% | 92% | 100% | 100% |
| — methods | 4016 | 66% | 77% | 98% | 98% |
| — signals | 273 | 73% | 78% | 100% | 100% |

(Tier columns marked "–" were not broken out; the T0 figure for "common" comes from the scripts' `members_simple`. The tier table comes from `docs/notes/scripts/serdes_tiers.py`.)

### 1.3 What the inventory says

- **T0 alone covers 90%+ of GTK and ~80% of Qt properties.** On GTK, the P kinds need almost nothing else. The 9 uncovered GTK P properties are `Entry`/`Label` `attributes` (`PangoAttrList`) and `tabs` (`PangoTabArray`), four `Entry` icon properties (`GIcon`, `GdkPaintable`), and `Widget:css-classes` (`GStrv`). The last one matters early: it is the natural target of a Svelte `class=` attribute.
- **Qt needs value structs right away.** `QWidget` alone exposes `geometry`, `minimumSize`, `maximumSize`, `sizeIncrement`, `baseSize`, `pos`, `size`, `rect`, `frameGeometry`, `childrenRect` and `contentsMargins`. Adding value structs moves Qt P properties from 79% to 92% and methods from 75% to 87%. The remaining 8% of Qt P properties are `QFont`, `QPalette`, `QCursor`, `QIcon`, `QSizePolicy`, `QLocale`, `QKeySequence`: handles or rich values.
- **i64/u64 essentially do not occur** (2 GTK properties, 5 GTK and 2 Qt method slots). Nothing in the widget surface needs values beyond 2^53. BigInt is not justified for the widget API.
- **Enums are common, flags much less so**: enums 111 vs flags 19 GTK properties, 105 vs 45 Qt properties. Both appear across every tier.
- **Callbacks and async are a GTK-methods-only concern** (33 callbacks + 20 async on 2650 GTK methods; Qt has essentially none in QtWidgets). The async ones are the GTK 4.10+ dialog APIs (`GtkFileDialog.open`, `GtkAlertDialog.choose`, `GtkColorDialog.choose_rgba`, …), which are exactly what apps use instead of the deprecated blocking dialogs.
- **Object references dominate the non-scalar surface** (743 GTK and 812 Qt method slots). They are already handles.
- **The long tail is handles anyway.** Everything in T2 that is not a value struct (QIcon, QFont, PangoAttrList, QModelIndex, GtkTextIter, QTreeWidgetItem*) is best represented in JS as an opaque handle with methods, not as serialized data (see §2). So T2 coverage costs one generic mechanism ("boxed handle") plus per-type method tables, not per-type serializers.


## 2. Encoding per category

### 2.0 Today, and the inconsistencies to fix

The P path handles `null | bool | number | string | handle | array`:

- `js_to_abi` (`crates/svelters-host/src/quickjs.rs`) maps an integral float below 2^53 to `I64`, `undefined` to an error, and objects and functions to errors.
- Handles are JS numbers.
- Backends then interpret by declared type:
  - Qt: `to_qstring/to_i32/to_variant` (`tools/metadata/qt/templates/backend.cpp.in`) and an if-chain of `QLatin1String` compares for enums.
  - GTK: `convert.rs` in the P3 lane, keyed by `ValueTy` and the property's `GParamSpec`. Enums go by nick through `EnumClass`, flags by nick or number.

Already divergent:

- **Enums out.** GTK returns the nick string; Qt returns the override table's name string. Flags out: GTK returns a **number**.
- **Typings.** `metadata/gtk/gtk.d.ts` types enum props as `string` and flags as `number`; the Qt `qt.d.ts` emits literal unions.
- **Attribute strings.** Svelte passes static attributes as strings (`spacing="8"`). The renderer fixes this with a hand-written `INT_PROPS` set (`packages/renderer/src/index.js` `coerce()`); GTK's `as_i64` would reject `"8"`. The coercion must come from the type descriptors (§3.2), not a hand list.
- **No type identity on the wire.** A JS object cannot cross at all, and an array cannot be told apart from a struct.

### 2.1 Category table

"Wire" is the proposed v2 `svl_value` (§5.1). Conversions in *italics* already exist.

| Category | JS value | TS type (generated) | Wire (v2) | GTK native | Qt native |
|---|---|---|---|---|---|
| bool | `boolean` (attribute strings `"true"/"false"/""` coerced) | `boolean` | `BOOL` | *`gboolean`* | *`bool`* |
| i8…u32 | `number`, integral, range-checked | `number` | `I64` | *range-checked `try_from`* | *`to_i32`*, add range check |
| i64/u64 | `number` within ±2^53; `bigint` accepted on input. Members annotated `bigint: true` return `bigint` | `number` (or `bigint`) | `I64`, `flags & UNSIGNED` for u64 | `gint64/guint64` | `qint64/quint64` |
| f32/f64 | `number` | `number` | `F64` (an integral `I64` is accepted) | *`gdouble/gfloat`* | *`double/float`* |
| string | `string`; `null` if nullable | `string` / `string \| null` | `STR` with encoding flag UTF-8 or UTF-16 and a `NUL_TERMINATED` flag; `len` in code units | `const char*`. Pass the borrowed pointer through when NUL-terminated (QuickJS ASCII), else copy once | `QString::fromUtf16` for UTF-16 input (zero transcoding), `fromUtf8` otherwise |
| enum | string name: GTK nick; Qt kebab-cased key (overridable) | literal union `'horizontal' \| 'vertical'` | `ENUM`: `type` = enum id, `i` = native value. The host maps name→value from the backend's member table | `g_value_set_enum`, or the plain C int | `static_cast<E>(i)` |
| flags | array of names (a single name and a raw `number` are also accepted) | `Array<'a' \| 'b'>` | `FLAGS`: `type` = flags id, `u` = mask | `g_value_set_flags` | `QFlags<E>::fromInt(i)` |
| object ref | wrapper object (today a number handle) | generated class/interface per kind | `HANDLE` (plus `type` = expected kind id for checking) | GObject via handle table, `is_a` check | `QObject*` via the table, `qobject_cast` check |
| value struct | plain object `{x, y, width, height}` (Svelte proxies accepted) | `interface Rect { x: number; y: number; width: number; height: number }` | `RECORD`: `type` = value type id, `l` = fields in schema order | per type: `GdkRectangle`, `graphene_rect_t`, `GdkRGBA`, `GtkBorder` | per type: `QRect(F)`, `QSize(F)`, `QPoint(F)`, `QMargins`, `QColor::fromRgbF` |
| string-coercible rich value | string, or record where natural: `QKeySequence` "Ctrl+S", `QUrl`, `QFont`/`PangoFontDescription` "Sans 12", `QCursor` shape name, icon name | `string` (+ record type) | `STR` or `RECORD` with `type` set | `pango_font_description_from_string`, `g_themed_icon_new` | `QKeySequence(s)`, `QUrl(s)`, `QFont::fromString`, `QIcon::fromTheme` |
| boxed / refcounted / implicitly shared | wrapper object with methods, `dispose()` and `[Symbol.dispose]`; finalized by GC | generated class per boxed type | `BOXED`: `h` = boxed handle, `type` = boxed type id | `g_boxed_copy/free` or ref/unref, held in the backend's table | heap copy `new QIcon(v)` (cheap, implicitly shared), `delete` on release |
| bytes | `Uint8Array`/`ArrayBuffer` in; `Uint8Array` out | `Uint8Array` | `BYTES` `{ptr, len}`, borrowed for the call | `g_bytes_new` (copy) or `g_bytes_new_with_free_func` over a pinned buffer | `QByteArray(ptr, len)` (copy) or `QByteArray::fromRawData` for borrowed-only use |
| string list | `string[]` | `string[]` | `LIST` of `STR` | `GStrv` built per call | `QStringList` |
| list of T | `T[]`; typed arrays accepted for numeric T | `T[]` | `LIST` of T | `GList/GSList/C array + length` per site | `QList<T>` |
| GListModel / item models | object handle (+ JS-backed models in M5) | model class | `HANDLE` | `GListModel` | `QAbstractItemModel*` |
| variant | any JSON-like value; `variant(sig, v)` helper for exact GVariant typing | `unknown` (or `Variant`) | the nested value itself; JS objects become `MAP` (keys `STR`); an optional GVariant signature rides in a wrapping `RECORD` of the built-in type `Variant` | `GVariant` from the signature, or inferred (`b,x,d,s,av,a{sv}`); `GValue` from the target GType | `QVariant` (*`to_variant`* extended to lists and maps) |
| date/time | `Date` for datetime; `{year, month, day}` for date-only; `{hour, minute, second, ms}` for time | `Date`, `DateParts`, `TimeParts` | `F64` epoch ms with `type` = DateTime, or `RECORD` | `GDateTime` (`g_date_time_new_from_unix_utc` + local) | `QDateTime::fromMSecsSinceEpoch`, `QDate(y,m,d)`, `QTime` |
| callback | JS function | the signature as a function type | `CALLBACK`: `h` = host callback id, `type` = signature id | trampoline with `user_data` = id and `GDestroyNotify` → `host->release` | `std::function` capturing id and an alive token, or a generated QObject receiver |
| async (`_async`/`_finish`) | method returns a `Promise`; optional `{ signal: AbortSignal }` | `Promise<R>` | a `CALLBACK` the backend completes once; `GCancellable` created by the backend | generated `_finish` + result conversion | rare in QtWidgets; dialogs use the signal → Promise adapter (`finished(int)`) |
| error | thrown `NativeError { domain, code, message }` | `class NativeError extends Error` | rc < 0 + `ERROR` out value `[domain STR, code ENUM\|I64, message STR]` | `GError` (`throws="1"`, 56 GTK methods) | Qt has no exceptions; `bool*`/`bool` result conventions become out params, and annotations can map them to throws |
| nullable | `null` | `T \| null` (only when nullable) | `NULL` | `NULL` | `nullptr`/empty `QString`/invalid `QVariant` |
| out / in-out | single out: the return value; several: object named by param names `{ ret, minimumSize, naturalSize }`; in-out: an argument plus a return field | object type | the backend returns a `LIST` [ret, out1, …] in `out` | caller-allocated structs on the stack | `T&`/`T*` locals |

### 2.2 Notes per category

**Enums as names, not numbers.**
- Names survive toolkit differences: `orientation="vertical"` works on both today. They are debuggable in op logs, and Svelte attribute strings need no coercion.
- The cost is one string→int lookup, which the host can cache by JS atom.
- Numbers stay accepted as an escape hatch (GTK's converter already does), but the TS type excludes them.
- The *backend* should never parse enum strings at runtime. The host resolves through a backend-published member table (`enum_members`, §5.1) and sends `ENUM{type, value}`. This removes the Qt if-chains and GTK's per-call `EnumClass` lookup, and gives one place for the error message listing valid names.
- Qt naming: kebab-case of the key, with the enum's common prefix stripped only when every member keeps a unique, non-empty name (`QLineEdit::PasswordEchoOnEdit` → `password-echo-on-edit`; `Qt::ScrollBarAsNeeded` → `as-needed`). Overrides can rename. GTK: the GIR nick, which is already in the metadata.

**Flags as arrays.**
- `['bold', 'italic']` reads well in Svelte (`align={['left', 'vcenter']}`).
- An object `{bold: true}` is the alternative. It is harder to type as a literal union and does not round-trip unknown bits.
- Output is always an array. Unknown bits produce a trailing number element, so nothing is silently lost.
- Flags are only 19 GTK / 45 Qt properties, so this is not hot.

**64-bit.** §1 found 2 i64 properties in all of GTK and none in Qt widgets. The rules:
- `number` everywhere, and a `RangeError` beyond ±2^53 on input.
- On output, a value beyond 2^53 throws with the member name, unless metadata marks the member `bigint: true`, in which case it always returns `bigint` (never "sometimes number, sometimes bigint").
- No BIGINT wire tag: `I64` plus an `UNSIGNED` flag carries every value.
- Handles stay below 2^53 by construction, since the backend issues them sequentially.

**Strings (UTF-8 ↔ UTF-16).** The measurements (§4) settle it:
- ASCII costs the same either way.
- Non-ASCII is ~20× cheaper to Qt as UTF-16, because QuickJS already stores wide strings as UTF-16 and `QString` *is* UTF-16.
- So `svl_str` gets an encoding flag. The host hands each backend what is zero-copy from the engine *and* acceptable to it: backends declare `accepts_utf16` in a capability word.
  - GTK accepts UTF-8 only. For it, the host transcodes wide strings once, and passes 8-bit ASCII strings through without a copy, NUL-terminated.
  - Qt accepts both.
- On the way out, Qt returns UTF-16 (`QString::utf16()`, zero-copy while the `QString` lives in the backend's per-call scratch) and the host uses `JS_NewStringUTF16`.
- Under Node/N-API the equivalents are `napi_get_value_string_utf16` and `napi_create_string_utf16`; they always copy into a caller buffer, but they avoid the transcode.
- Embedded NULs are fine because lengths are explicit. GTK C APIs that need NUL termination get a copy only when the borrowed buffer is not terminated.

**Value structs: plain objects, not classes.**
- Plain objects work with Svelte (`$state`, spreads, `{...rect, width: 10}`) and with JSON op logs. They need no import, and they are no slower than arrays (§4.3 item 5).
- Class instances (`new QRect(...)`, PySide style) would make `$state` reactivity awkward, because Svelte proxies only plain objects and arrays. They would also force a wrapper allocation on every `getProp`.
- One neutral JS shape per geometric concept, shared by both toolkits:

  | JS type | GTK | Qt |
  |---|---|---|
  | `Rect {x,y,width,height}` | `GdkRectangle` (int), `graphene_rect_t` (float) | `QRect` (int), `QRectF` (double) |
  | `Size {width,height}` | `GtkRequisition` | `QSize`, `QSizeF` |
  | `Point {x,y}` | `graphene_point_t` | `QPoint`, `QPointF` |
  | `Insets {top,right,bottom,left}` | `GtkBorder` | `QMargins` |
  | `Color {red,green,blue,alpha}` (0–1; CSS color strings also accepted on input) | `GdkRGBA`, `gdk_rgba_parse` | `QColor::fromRgbF`, `QColor::fromString` |

- Int-typed native structs validate integrality, with an error rather than a silent round, so `{x: 1.5}` into a `QRect` fails loudly.
- Converters read fields in schema order, once each, with pre-created atoms. Missing fields are an error, extra fields are ignored.

**Boxed and refcounted values as handles with finalizers.**
- A wrapper JS object owns one backend boxed handle. Release happens three ways:
  - explicitly: `dispose()`, or `using` (QuickJS-NG supports `using` and `Symbol.dispose`);
  - by GC finalization;
  - when the owning context goes away (hot reload, host shutdown).
- The finalizer must not call the backend. QuickJS class finalizers run *inside* GC, which can trigger during any allocation, including in the middle of a host op's conversion.
- So a finalizer only pushes the handle onto a release queue, and the host drains the queue at the next checkpoint through `backend->release(h)`.
- `FinalizationRegistry` (supported by QuickJS-NG, V8 and N-API engines) gives the same thing portably: callbacks already run as jobs. Use it in the shared JS layer so that the Node/Deno fronts behave identically, and keep the native class finalizer only as a QuickJS fast path.
- Qt value classes (`QIcon`, `QPixmap`, `QImage`, `QFont`, `QPalette`) are implicitly shared, so holding a heap copy is a refcount bump.
- Two semantic traps need documenting in generated docs. Both match PySide behaviour.
  - **Mutating a copy does nothing.** `w.font.bold = true` cannot affect the widget. Prefer string/record forms where §2.1 lists them; a returned boxed value is a snapshot.
  - **Iterator-like boxed types go stale.** `GtkTextIter`, `GtkTreeIter`, `QModelIndex` become invalid when the model or buffer changes, and the toolkit does not detect it. Map them to stable values instead:
    - TextIter ↔ character offset (`gtk_text_buffer_get_iter_at_offset` against the method's buffer instance);
    - `QModelIndex` ↔ `QPersistentModelIndex` behind a handle that checks `isValid()` before use;
    - GtkTreeIter only through the M5 model adapter.

**Binary data.**
- Inbound `ArrayBuffer`/`TypedArray` → `BYTES {ptr, len}` is zero-copy and borrowed for the call (`JS_GetArrayBuffer` / `napi_get_typedarray_info`).
- A backend that retains the data must copy (`g_bytes_new`, `QByteArray(ptr, len)`, `QImage(...).copy()`). Retaining a JS-owned buffer without a copy needs pinning: the host keeps the `ArrayBuffer` alive until a native free callback fires, and the buffer must not be detached or resized meanwhile. QuickJS gives no way to forbid detaching a JS-allocated buffer, so pinning is reserved for buffers the native side allocated.
- For large pixel data, offer `native.allocBytes(n)`: an `ArrayBuffer` over native memory (`JS_NewArrayBuffer` with a free func and opaque; `napi_create_external_arraybuffer`, which Electron's V8 sandbox forbids, so keep a copy fallback). JS fills it, then hands it to `GdkMemoryTexture`/`QImage` without a copy, and the native side keeps a reference.
- Outbound `GBytes` → an `ArrayBuffer` over the GBytes data that unrefs it in the free func. QuickJS-NG can mark it immutable (`JS_SetImmutableArrayBuffer`), which matches GBytes semantics. Outbound `QByteArray` → copy (`JS_NewArrayBufferCopy`), because a `QByteArray`'s data pointer is not stable across detach.

**Dates and times.**
- Datetimes → JS `Date` (epoch ms, local zone applied natively).
- Date-only and time-only → `{year, month, day}` / `{hour, minute, second, ms}` records. `Date` for a calendar day invites timezone off-by-one bugs, and Temporal is not available in QuickJS.
- Months are 1-based, matching both toolkits, not JS `Date`.
- Needed for `QDateEdit`/`QDateTimeEdit`/`QCalendarWidget` (13 Qt properties) and `GtkCalendar:date` (`GDateTime`).

**Callbacks.**
- Signals do not need callbacks: `listen` + `post_event` already keep functions on the JS side. Callback-typed *parameters* (33 GTK methods; 1 Qt) do.
- The host keeps a table `id → (persistent JS function, owner handle, scope, generation)` and passes `CALLBACK{id, type}`. The backend builds a trampoline per signature type (generated per `callback<Id>`).
- Lifetime by GTK scope:
  - `call`: valid for the call only; the host releases it on return.
  - `async`: released after the first invocation.
  - `notified`: released when GTK calls the `GDestroyNotify` → `host->release_callback(id)`.
  - `forever` / missing scope: refused (`unsupported_reason: callback_scope`).
- Every callback is also tied to its owner handle. When the owner is destroyed (`destroy` op or `on_destroyed`), the host drops the JS function and bumps the generation, so a late native invocation (common after dispose) finds a dead id and returns the type's default instead of calling into freed JS state.
- Invocation is synchronous (`host->invoke_callback`), with the same rules as `call_sync` (`signals-and-slots-semantics.md` §1.5): no job drain, and tree ops are deferred.

**Async results.**
- GTK `*_async/*_finish` pairs (20 in Gtk, all `GAsyncReadyCallback` + `GCancellable`) are generated as `Promise`-returning methods.
- The host allocates a Promise capability and a one-shot `CALLBACK`. The backend's trampoline calls `_finish`, converts the result or `GError`, and calls `host->invoke_callback`. The host resolves the Promise *without* running jobs. Reactions run at the next checkpoint, as everything else does.
- An optional `{signal}` maps to a backend-created `GCancellable` through a small `cancel(callback_id)` op.
- Qt dialogs (`QDialog::open` + `finished(int)`) get an annotation-driven "promise from signal" adapter instead. QFuture does not occur in QtWidgets.

**Errors.**
- **Conversion errors** are thrown synchronously as `TypeError`/`RangeError` from the op that received the bad value (§3.3).
- **Native errors** (`GError`) become `NativeError` with `domain` (the quark string, e.g. `gtk-dialog-error-quark`), `code` (an enum name when the domain is tied to an error enum, e.g. `Gtk.DialogError` → `'dismissed'`, else a number) and `message`.
  - For async methods the Promise rejects with it. `GtkFileDialog` cancel is a `dismissed` rejection, which apps must handle, so document it.
- **Backend failures** (stale handle, unknown id) stay as today: rc < 0 + `last_error` → a JS `Error`.

**Nullable.**
- `null` ↔ `NULL`. `undefined` is "not provided": an error for required slots, the native default for optional trailing parameters with a declared default.
- A `null` into a non-nullable slot is a `TypeError` in the host, using GIR `nullable`/`allow-none` or Qt's pointer/`QString` conventions. Today it silently becomes an empty `QString` (`to_qstring`) or `None` (GTK).
- Removing an attribute sends `null` (`packages/renderer` `removeAttribute`). For a non-nullable prop that should mean "reset to default": a `reset` accessor (Qt `RESET`) or the `GParamSpec` default. The host can do this when it sees `null` for a non-nullable property.

### 2.3 Structured values: nested `svl_value` vs a compact binary encoding

Options for RECORD/LIST/MAP on the backend ABI:

1. **Nested `svl_value`** (v1's LIST, extended with RECORD/MAP plus type ids). A tree of 24-byte values on the host's stack or arena. Self-describing, so debuggers and op logs can print it. Every backend language reads it with a switch. Cost per field: one 24-byte store; per call: zero allocations for small values (stack arrays).
2. **Generated per-type packed layout** (a C struct per value type: `struct svl_Rect { double x, y, width, height; }`). Fastest to read, but the ABI header would grow per type, which the "ABI never grows per method" rule forbids. Every engine front would also need the layouts.
3. **CBOR/MessagePack.** Self-describing bytes. An encode pass on the host *and* a decode pass in every backend; strings must be copied into the buffer, which loses the zero-copy borrowed pointers measured above; and C++/Rust backends need a decoder dependency. None of that buys anything in-process.
4. **FlatBuffers-like** schema'd buffers. Zero-copy reads, but schema compilation per type and a builder on the encode side. Overkill for 4-field values.

Measured context (§4): the call crossing (~0.8 µs) dwarfs value construction (a few ns). JS-side encoding in QuickJS costs as much as a call, and Rust-side encoding is the same work as building `svl_value`s. So:

- **Keep nested `svl_value` as the backend ABI.** Grow it with tags and a type id (§5.1). It is engine-independent: the Node/Deno fronts produce the same values, so every backend works under every engine unchanged.
- **Add a packed *front* encoding only where the engine boundary is expensive or pointer-free.** That means the Deno FFI front (`Deno.UnsafeCallback`/FFI cannot pass JS objects) and, if measurements justify it, N-API batching.
  - The format is the op buffer prototyped in §4: little-endian words, opcodes, `type` ids, strings as offsets into a UTF-8/UTF-16 byte area filled by `TextEncoder.encodeInto` (fast in V8).
  - `svelters-core` decodes it into `svl_value`s pointing into the buffer, still zero-copy, and calls the same backend vtable.
  - That keeps one value model and one backend ABI, with the batching decision made per engine front.


## 3. Codegen

### 3.1 Converters keyed by neutral type, shared by every member

Generators work per *type*, not per member (Shiboken's and gjs's lesson, §4.4). Each slot in the metadata carries a neutral type ref (`string`, `enum<Gtk.Orientation>`, `value<Rect>`, `boxed<Pango.AttrList>`, `list<string>`, `callback<Gtk.TickCallback>`, …) and its native spelling. The generator:

1. Collects the set of `(neutral type, native type)` pairs used by the selected members: props, method params and returns, signal args, child attributes.
2. Emits **one inbound and one outbound converter per pair**. Method, prop and signal dispatch code calls converters by name; it never inlines conversion logic.
3. Emits **type descriptors** as static data (§3.2) for the host's type-directed JS→wire conversion and the error messages.

Per toolkit:

- **Qt (C++, `svelters-qt-build`).** Templates keyed by the native type, with the neutral type in the descriptor:

  ```cpp
  template <class T> struct Conv;                       // primary: undefined -> compile error for unmapped types
  template <> struct Conv<QString> {
    static bool in(const svl_value *v, QString &o, Err &e);   // UTF-16 or UTF-8 by v->flags
    static void out(const QString &s, svl_value *o, Arena &a); // UTF-16 out, zero-copy into the arena
  };
  template <> struct Conv<QRect>  { /* RECORD type=Rect, 4 integral fields */ };
  template <> struct Conv<QRectF> { /* RECORD type=Rect, 4 numeric fields */ };
  template <class E> requires std::is_enum_v<E> struct Conv<E> { /* ENUM, type checked */ };
  template <class E> struct Conv<QFlags<E>> { /* FLAGS */ };
  template <class T> struct Conv<QList<T>> { /* LIST, element Conv<T> */ };
  template <class T> requires std::is_base_of_v<QObject, T> struct Conv<T*> { /* HANDLE + qobject_cast */ };
  ```

  The generated call site is `T0 a0; if (!Conv<T0>::in(&args[0], a0, err)) return err.fail("addTab", "icon");` followed by `obj->addTab(a0, a1)`. C++ instantiates only the converters that dispatch code uses, so tree-shaking (§3.4) is automatic. `to_variant`/`out_variant` become `Conv<QVariant>`, extended to LIST/MAP/RECORD. Overloads (`QAbstractButton::clicked()` vs `clicked(bool)`, `addTab(w, label)` vs `addTab(w, icon, label)`) dispatch on arity first, then on a generated `Conv<T>::accepts(const svl_value*)` predicate, like Shiboken's `isConvertible` tree.
- **GTK dev (Rust, dynamic).** Properties and signals go through `glib::Value`, with one converter per GType *class*:
  - fundamentals: bool, ints, floats, string, enum, flags, object;
  - boxed types: a registry `GType → fn` for the value structs (`GdkRGBA`, `GdkRectangle`, `graphene_rect_t`, `GtkBorder`), `GStrv`, and the generic boxed handle.

  This extends today's `convert.rs` `ValueTy` switch. GI-invoked methods (M3) use a per-callable **argument plan**, precomputed once and cached by method id like gjs's arg cache. The plan is a `Vec<ArgStep>` with steps `In(conv)`, `Out(conv)`, `Length(of)`, `UserData(cb)`, `Destroy(cb)` and `Error`, chosen from `GITypeInfo` plus our metadata overrides.
- **GTK release (Rust, generated thin bindings, M6).** The same per-type converters as Rust functions over `-sys` types (`fn rect_in(v: &svl_value) -> Result<gdk::Rectangle, ConvErr>`). Generated thunks call them, and the release linker keeps only the referenced ones.
- **Host (Rust, `svelters-core`, engine-agnostic).** One type-directed converter driven by descriptors, over a small engine trait. QuickJS, N-API and deno_core implement the trait in ~150 lines each. The conversion logic itself (validation, attribute-string coercion, enum name lookup, record field order, flags arrays, nullability) is written once, in the core:

  ```rust
  trait JsReader<'a> {
      fn kind(&self, v: Self::V) -> JsKind;                 // null/undefined/bool/number/string/bigint/array/object/function/arraybuffer
      fn number(&self, v: Self::V) -> f64;
      fn str_borrow(&self, v: Self::V) -> BorrowedStr<'a>;  // UTF-8 or UTF-16 view, zero-copy when possible
      fn field(&self, obj: Self::V, key: FieldKey) -> Self::V; // FieldKey pre-interned per engine (JSAtom / napi_value)
      fn index(&self, arr: Self::V, i: u32) -> Self::V;
      fn len(&self, arr: Self::V) -> u32;
      fn bytes(&self, v: Self::V) -> Option<&'a [u8]>;
  }
  ```

  Also generated: the **JS-side wrappers** (element classes, method stubs that call `host.call(h, id, args)` with ids resolved once per kind) and `.d.ts`. The JS side does no conversion work, which matters in QuickJS (§4.3 item 6).

### 3.2 Descriptors (data, not code)

The backend publishes static, generated tables (§5.1, `describe`). For each kind member:

- the name;
- the parameter list: neutral type id, wire tag, flags (`NULLABLE`, `OPTIONAL`, `OUT`, `INOUT`, `UNSIGNED`, `INTEGRAL`), numeric range, and default presence;
- the return type.

For each type id: enum/flags members `{name, value}`, record field names and types, and callback signatures. The host fetches each descriptor once on first `resolve`, caches it next to the name→id cache, and pre-interns field-name atoms.

This also lets the host coerce Svelte attribute strings, which replaces `INT_PROPS`:

- integer slot: `"8"` → 8;
- bool slot: `""`/`"true"`/`"false"`;
- enum: a name;
- flags: a space-separated list (`align="left vcenter"`);
- record: refused (attributes cannot carry objects meaningfully).

### 3.3 Validation and error messages

- Validate **in the host, before dispatch**, using descriptors. The message names the element, member and parameter, the expected type and the received value, abbreviated:

  ```text
  TypeError: <box>.spacing: expected integer, got "8px" (string)
  TypeError: QLineEdit.setEchoMode(mode): expected one of 'normal' | 'no-echo' | 'password' | 'password-echo-on-edit', got 'hidden'
  RangeError: <scale>.digits: 3.5 is not an integer (gint)
  TypeError: <window>.defaultSize: missing field 'height' in Size
  ```
- The exception is thrown from inside the host op, so the QuickJS stack starts at the renderer's `setProp` call. The user's callsite is one or two frames up: the compiled component's template effect, which source maps (M2) take back to the `.svelte` attribute.
- In dev, the renderer should catch host `TypeError`s in `setAttribute`/`setNativeProp` and rethrow with the component's element path. The element tag and attribute name are known there, which beats a bare stack.
- **Deferred ops.** Ops queued during a sync scope (`call_sync`) must be validated and fully converted at enqueue time, into owned values. Otherwise the error surfaces at the next checkpoint with no useful stack, and borrowed pointers would dangle.
- Backends still validate what only they can know: kind mismatch of a handle (`is_a`/`qobject_cast`), stale handles, and native-side `g_return_if_fail` (made fatal-to-error via a log handler in dev). They report through `last_error`, upgraded to a structured `ERROR` value when present.

### 3.4 Release binding linker (M6): only the converters you use

- The bindings manifest lists `(kind, member)` roots that survive JS DCE, plus `extra_roots` for dynamic calls.
- The generator computes the **type closure**: every neutral type reachable from those members' slots, recursively through list elements, record fields and callback signatures. It emits converters, descriptors and enum tables only for that closure.
  - In Qt this needs no extra work, because unreferenced `Conv<T>` specializations are never instantiated. The descriptor and enum-table emission must still be filtered to the closure.
  - In Rust, unreferenced converter fns are dead code anyway, but filtering at generation keeps compile time down: the 2021 lesson in `gtk-v1-archaeology.md`.
- The type closure goes into the manifest digest, so changing a used type's mapping rebuilds the leaf crate.
- Dev-fat builds include every converter for the annotated kinds.

### 3.5 Generated TypeScript per category

```ts
// scalars
spacing?: number;                         // i32, with JSDoc range from metadata: @minimum -1 @maximum 2147483647
opacity?: number;
label?: string | null;                    // nullable string
// enums & flags
orientation?: 'horizontal' | 'vertical';  // GtkOrientation / QBoxLayout::Direction (subset by override)
alignment?: Array<'left' | 'right' | 'hcenter' | 'justify' | 'top' | 'bottom' | 'vcenter' | 'baseline'>;
// value types (shared module @svelters/types)
export interface Rect { x: number; y: number; width: number; height: number }
export interface Size { width: number; height: number }
export interface Color { red: number; green: number; blue: number; alpha: number }
geometry?: Rect;
color?: Color | string;                   // CSS color strings accepted on input
// rich values with string forms
shortcut?: string;                        // QKeySequence, e.g. "Ctrl+S"
font?: string | FontDescription;
// handles
export declare class QIcon implements Disposable { static fromTheme(name: string): QIcon; dispose(): void; [Symbol.dispose](): void }
icon?: QIcon | string;                    // string = theme icon name
// lists, bytes, dates
cssClasses?: string[];
setData(data: Uint8Array): void;
date?: DateParts;                         // { year, month, day }, month 1-based
// methods with outs, async, errors
getPreferredSize(): { minimumSize: Size; naturalSize: Size };
open(parent?: Window | null, opts?: { signal?: AbortSignal }): Promise<File>;   // rejects NativeError
// callbacks
addTickCallback(cb: (clock: FrameClock) => boolean): number;
```

The P-era `gtk.d.ts` (`orientation?: string`, flags as `number`) and the Qt `qt.d.ts` (literal unions) converge on the form above. Both should be emitted by one shared TS emitter, keyed on the neutral types, so the two toolkits produce the same TS for the same concept.


## 4. Performance (measured)

### 4.1 Setup and caveats

Bench crate: `docs/notes/scripts/serdes_bench/` (path deps on `crates/svelters-abi` and `crates/svelters-host`; build with `CARGO_BUILD_JOBS=4 CARGO_BUILD_BUILD_DIR=target/build cargo build --release`). It has two parts:

- **A, the real path.** `svelters_host::Host` with QuickJS, `__svelters_host.setProp(h, name, value)` etc., into two dummy backends:
  - A2: a raw C-ABI backend that reads the borrowed `svl_value` (the Qt/GTK backends' situation);
  - A1: a Rust `Backend` trait backend through `BackendVtable` (the log backend's situation; it copies into an owned `Value`).
- **B, prototypes** on a bare rquickjs context:
  - pre-resolved numeric ids;
  - a v2-style value (`V2`: u16 tag, u16 flags, u32 type id, 16-byte union) with a typed `RECORD`;
  - flat-argument thunks;
  - a batched op buffer (`Float64Array` words + a string side table, one native call per flush);
  - borrowed strings via `JS_ToCStringLen2` / `JS_ToCStringLenUTF16`;
  - callback registration.

Each row is 10 000 ops. Each bench takes the best of 9 rounds within a process, and the table gives the min and median over 7 processes. All numbers are ns per op, including the JS loop (~150 ns/iteration). Machine: i7-8850H, `powersave` governor, **load average 27–31 on 12 threads during every run** (other sessions were building). Absolute numbers are therefore inflated, probably 2–3×: the QuickJS empty loop ran at ~140–270 ns/iteration here. Read the ratios, not the absolutes. The ordering was stable across all runs.

### 4.2 Results

Part A, current host path (ns/op, min / median):

| op | A2 raw C-ABI backend | A1 Rust-trait backend |
|---|---:|---:|
| JS loop only | 144 / 163 | 149 / 169 |
| `setProp(h,'spacing',i)` int | 1433 / 1567 | 1448 / 1615 |
| `setProp` f64 | 1423 / 1711 | 1472 / 1757 |
| `setProp` bool | 1505 / 1579 | 1632 / 1789 |
| `setProp` string 11 B ASCII | 1651 / 1704 | 1754 / 1998 |
| `setProp` string 1 KiB ASCII | 2044 / 2291 | 3906 / 4680 |
| `setProp` string 1 Ki chars non-Latin-1 | 22693 / 24745 | 34165 / 36344 |
| `setProp` enum as string `'vertical'` | 1721 / 1788 | 1965 / 2089 |
| `setProp` rect as LIST `[x,y,w,h]` | 2935 / 3086 | 3346 / 3571 |
| `getProp` int | 1403 / 1530 | 1486 / 1694 |
| `call(h,'m',[a,b])` | 2609 / 2818 | 2717 / 3057 |

Part B, prototypes (ns/op, min / median):

| op | min | median |
|---|---:|---:|
| JS loop only | 177 | 189 |
| JS only: allocate `{x,y,width,height}` | 1374 | 1558 |
| JS only: 4 `Float64Array` stores | 732 | 906 |
| native no-op call `(h, id, v)` (rquickjs `Function`, 3 args) | 837 | 941 |
| `setPropId(h, id, i)` int, pre-resolved id | 768 | 986 |
| `setPropId` string 11 B (borrowed UTF-8) | 812 | 1114 |
| `setPropId` enum as number | 802 | 1158 |
| `setPropId` rect as LIST `[i,2,300,400]` (array literal per op) | 1763 | 2048 |
| `setPropId` rect as object literal `{x,y,width,height}` per op → typed RECORD | 3171 | 4336 |
| `setPropId` rect, same object reused → typed RECORD | 1777 | 1902 |
| `setPropId` rect, object behind a `Proxy` get-trap (like Svelte `$state`) | 4337 | 5304 |
| `setRect4(h, id, x, y, w, h)` flat-args thunk | 1276 | 1633 |
| batched int (JS encode + flush + decode + dispatch) | 1530 | 1737 |
| batched string 11 B | 1983 | 2256 |
| batched rect object literal | 3724 | 4195 |
| batched mixed (int, string, rect) per op | 1470 | 2106 |
| batched string as UTF-16 code units copied into a `Uint16Array` in JS | 8702 | 10379 |
| register + unregister a JS function as a persistent root | 2170 | 2640 |

Strings, one native call per op (ns/op, min / median):

| JS string | `String` (rquickjs `to_string`, today) | borrowed UTF-8 (`JS_ToCStringLen2`) | borrowed UTF-16 (`JS_ToCStringLenUTF16`) |
|---|---:|---:|---:|
| 11 B ASCII | 801 / 866 | 686 / 737 | 668 / 853 |
| 1 Ki ASCII | 1345 / 1433 | 983 / 1088 | 1279 / 1338 |
| 1 Ki Latin-1 (`é`) | 10529 / 12764 | 5567 / 6167 | 1042 / 1363 |
| 1 Ki non-Latin-1 (wide) | 21889 / 23909 | 8639 / 12372 | 595 / 708 |

Qt side (`serdes_bench/qstr.cpp`, `-O2`, Qt 6, best of 7 × 100k, same loaded machine):

| string | `QString::fromUtf8` | `QString::fromUtf16` | `QString::toUtf8` (outbound) |
|---|---:|---:|---:|
| 11 B ASCII | 116–147 ns | 150–202 ns | 102–115 ns |
| 1 Ki ASCII | 437–459 ns | 324–375 ns | 407–469 ns |
| 1 Ki Latin-1 | ~14 000 ns | ~345 ns | ~9 900 ns |
| 1 Ki wide | ~15 100–16 200 ns | ~310–390 ns | ~10 900–12 800 ns |

### 4.3 What the numbers say

1. **The JS→native call dominates. The value encoding barely matters for scalars.** A native no-op with three arguments costs about as much as a full pre-resolved `setPropId` with int, string or enum (≈0.8 µs here). Building a 24-byte value on the stack is noise.
2. **Passing prop names as strings is the largest avoidable cost in today's path.** `setProp(h, 'spacing', i)` costs 1.43 µs; `setPropId(h, id, i)` costs 0.77 µs. The difference comes from:
   - the name `String` allocation per call (rquickjs `String` argument);
   - `resolve()` building a `(u32, u32, String)` key with `to_string()` for every lookup;
   - the handle-table lookup and `RefCell` traffic.

   `backend-abi.md` already says "the JS host caches name→ID resolution per kind". The implementation doesn't yet; generated JS tables should pass ids.
3. **Strings: ASCII is free, non-ASCII is not.** QuickJS-NG returns a pointer *into the JSString* for 8-bit ASCII strings (`JS_ToCStringLen2` scans and returns `str8(str)` without copying, NUL-terminated). Today's `to_string()` then copies into a Rust `String` and re-validates UTF-8. For non-ASCII, every UTF-8 transcode costs 5–12 µs per KiB in QuickJS, plus 14 µs per KiB in `QString::fromUtf8` on the Qt side. Wide JS strings are *already UTF-16* and `JS_ToCStringLenUTF16` returns them zero-copy (0.6 µs/KiB total), and `fromUtf16` is a memcpy. A 1 KiB non-ASCII label going to Qt today costs ~20 µs host + ~15 µs Qt. With UTF-16 passthrough it costs ~1 µs. ASCII UI strings (the common case for labels and ids) are within 10% either way.
4. **The Rust `Backend` trait's owned `Value` copy doubles the string cost** (1 KiB: 2.0 → 3.9 µs; wide: 23 → 34 µs). The GTK backend already reads raw `svl_value` and avoids this. The trait should hand out a borrowed view.
5. **Structs: an object costs about as much as an array, and most of the cost is JS allocation.** Allocating `{x,y,width,height}` in QuickJS costs ~1.2 µs over the loop, more than the native call. Converting a reused object (4 atom-keyed gets) costs ~1.8 µs total, the same as a LIST from a reused-ish array literal (1.76 µs). A flat-args thunk `setRect4` is the cheapest (1.28 µs), but it needs per-type entry points.
   - Choose the object representation for ergonomics. It is no slower than LIST.
   - Svelte `$state` proxies are the real hazard: reading 4 fields through a `get` trap adds ~2.5 µs. Generated converters should read fields once, in schema order. Users should be told to pass `$state.raw` or `$state.snapshot` objects for hot geometry.
6. **Batching does not pay in QuickJS.** Encoding into a `Float64Array` from interpreted JS costs about as much as a native call: 4 stores ≈ 0.6 µs vs. a call ≈ 0.8 µs. Batched int (1.53 µs) and batched rect (3.7 µs) are *slower* than direct calls (0.77 µs and 1.8–3.2 µs). Encoding strings in JS (`charCodeAt` loop into a `Uint16Array`) is 5–10× worse than one native call.
   - This matches the `backend-abi.md` stance: batch only if measurements justify it.
   - The trade flips for JIT engines (V8 under Node/Deno), where JS-side encoding is ~10× cheaper and an N-API or FFI crossing still costs ~50–200 ns plus argument wrapping. A packed op buffer belongs to the N-API/FFI fronts, not to the QuickJS path (§5.3).
7. **Callback registration is ~2 µs** (persistent root + table insert + removal). That is fine for signal-like registrations and too slow to do per item in a 10k-row list factory without pooling.

### 4.4 Prior art

- **React Native JSI / TurboModules / Fabric.**
  - JSI replaced the old bridge, which JSON-serialized batched async messages and paid for the encoding. JSI exposes `jsi::Value` (undefined/null/bool/number/string/symbol/bigint/object) and direct synchronous `HostFunction`/`HostObject` calls.
  - TurboModule codegen reads typed TS/Flow specs and emits C++ that converts each argument with type-directed templates (`react::bridging::fromJs<T>`/`toJs`, `Bridging<T>` specializations for strings, arrays, optionals, enums, functions and Promises).
  - Fabric parses props from `RawProps` into typed C++ props structs, using per-type `fromRawValue` converters (colors, edge insets, enums from strings).
  - JS callbacks become `AsyncCallback`/`jsi::Function` held with a `CallInvoker`, and must be released on the JS thread.
  - Lessons: typed per-type converters generated from a schema, enums as strings, a direct synchronous call instead of a serialized batch, and callbacks as owned objects with an explicit invoker/thread.
- **NativeScript.**
  - Metadata is generated at build time from ObjC headers and Java classes. The runtime marshals by the metadata type encoding: no per-method glue, much like our dev mode.
  - Native objects get one canonical JS wrapper (with weak tables and finalizers).
  - C structs are JS objects with record type info, and plain object literals are accepted where a struct is expected.
  - Out params use `interop.Reference`; JS functions become ObjC blocks/Java interfaces through FFI closures.
  - NativeScript's documented performance work is about avoiding repeated metadata lookups and wrapper creation on hot paths.
- **gjs argument cache.**
  - `gi/arg-cache.cpp` precomputes, per `GICallableInfo`, an array of `GjsArgumentCache` entries. Each entry's marshaller vtable (`in`/`out`/`release`) is chosen once from the `GITypeInfo`.
  - There are dedicated fast paths for bool/int/enum/flags/object/boxed/string. Array length and user-data arguments are "skip" entries filled by their owner.
  - Callbacks use `GjsCallbackTrampoline` with scope-specific freeing (`call` after return, `async` after first invocation, `notified` on `GDestroyNotify`).
  - `GError` becomes a JS `GLib.Error` exception. Enums and flags are plain numbers (namespace constants).
  - Signal closures (`GjsClosure`) are invalidated on object dispose. gjs had years of bugs from callbacks firing during GC sweep or after dispose, which is the same class of crash `gtk-v1-archaeology.md` lists for us.
  - Lesson: resolve the marshalling plan once per callable (our generated tables), never per call.
- **PySide6 / Shiboken.**
  - Every C++ type gets an `SbkConverter` with `pythonToCpp`/`cppToPython` function pointers and `isConvertible` checks, registered in a global registry keyed by type name (`Shiboken::Conversions::getConverter("QString")`).
  - Containers and primitives are described in the typesystem XML (`<primitive-type>`, `<container-type>`, `<conversion-rule>`). Implicit conversions are listed per type (e.g. `QColor` from `Qt::GlobalColor`, `QString` from `str`/`None`).
  - Overload resolution is a generated decision tree over the `isConvertible` checks.
  - Value types are copied into Python wrappers. Object types are wrapped pointers with parent-ownership heuristics from the typesystem.
  - Lesson: one converter per *type*, shared by every method, with a cheap "is convertible" predicate that also drives overload selection and error messages.
- **Other.**
  - wasm-bindgen generates JS glue per signature and copies strings through linear memory with `TextEncoder`. Its lesson: strings are the expensive part.
  - V8 Fast API calls (used by Deno ops) give near-free calls only for scalars and typed arrays, which argues for keeping hot paths scalar.


## 5. Recommendation

### 5.1 ABI v2 value model (proposal for `backend-abi.md`)

The struct stays 24 bytes. v1's 4-byte tag plus 4 bytes of padding become `tag`/`flags`/`type`. This needs an `abi_version` bump; it bundles with M2's `schedule_wake`.

```c
#define SVL_ABI_VERSION 2

typedef uint64_t svl_handle;   /* objects and boxed values share the backend's handle space */
typedef uint32_t svl_id;

enum {
  /* v1 */
  SVL_NULL = 0, SVL_BOOL = 1, SVL_I64 = 2, SVL_F64 = 3, SVL_STR = 4, SVL_HANDLE = 5, SVL_LIST = 6,
  /* v2 */
  SVL_ENUM = 7,      /* i: native value;            type: enum type id */
  SVL_FLAGS = 8,     /* u: bit mask;                type: flags type id */
  SVL_RECORD = 9,    /* l: fields in descriptor order; type: record type id (well-known or backend) */
  SVL_BYTES = 10,    /* b: {ptr,len} borrowed for the call */
  SVL_BOXED = 11,    /* h: handle of a boxed/refcounted/value-class instance; type: boxed type id */
  SVL_CALLBACK = 12, /* u: host callback id (generation in the high 16 bits); type: signature id */
  SVL_MAP = 13,      /* l: 2n values, alternating key (STR) and value; variants only */
  SVL_ERROR = 14,    /* out only. l: [domain STR, code ENUM|I64, message STR] */
};

enum {               /* svl_value.flags */
  SVL_F_UTF16    = 1u << 0,  /* STR: ptr is const uint16_t*, len in UTF-16 code units (else UTF-8 bytes) */
  SVL_F_NUL_TERM = 1u << 1,  /* STR: a terminating NUL follows ptr[len] */
  SVL_F_UNSIGNED = 1u << 2,  /* I64: read u.u */
  SVL_F_OWNED_BY_CALLEE = 1u << 3,  /* reserved: transfer instead of borrow (not used in v2) */
};

typedef struct svl_value {
  uint16_t tag;
  uint16_t flags;
  svl_id   type;            /* 0 unless ENUM/FLAGS/RECORD/BOXED/CALLBACK, or an expected kind on HANDLE */
  union {
    bool b; int64_t i; uint64_t u; double f; svl_handle h;
    struct svl_str   { const void *ptr; size_t len; } s;
    struct svl_list  { const struct svl_value *ptr; size_t len; } l;
    struct svl_bytes { const uint8_t *ptr; size_t len; } bytes;
  } u;
} svl_value;                 /* static_assert(sizeof(svl_value) == 24) on LP64 */

/* Well-known neutral value types; toolkit types get backend-assigned ids >= 0x10000. */
enum { SVL_T_RECT = 1, SVL_T_SIZE, SVL_T_POINT, SVL_T_INSETS, SVL_T_COLOR,
       SVL_T_DATE, SVL_T_TIME, SVL_T_DATETIME, SVL_T_VARIANT };

/* ---- descriptors: static, generated, owned by the backend for its lifetime ---- */
enum { SVL_D_NULLABLE = 1, SVL_D_OPTIONAL = 2, SVL_D_OUT = 4, SVL_D_INOUT = 8,
       SVL_D_UNSIGNED = 16, SVL_D_INTEGRAL = 32, SVL_D_BIGINT = 64, SVL_D_BORROWED_ARG = 128 };
enum { SVL_M_THROWS = 1, SVL_M_ASYNC = 2, SVL_M_SYNC_SAFE = 4, SVL_M_BLOCKING = 8 };

typedef struct svl_slot_desc {
  const char *name;          /* JS-facing name (param, field) */
  uint16_t tag;              /* expected wire tag */
  uint16_t flags;            /* SVL_D_* */
  svl_id   type;             /* type id for ENUM/FLAGS/RECORD/BOXED/CALLBACK/LIST element/HANDLE kind */
  double   min, max;         /* numeric range, NaN when unbounded */
} svl_slot_desc;

typedef struct svl_member_desc {
  const char *name;
  const svl_slot_desc *params; uint32_t n_params;
  svl_slot_desc ret;         /* tag SVL_NULL + name NULL = void; methods with outs return LIST [ret, outs...] */
  uint32_t flags;            /* SVL_M_* */
} svl_member_desc;

typedef struct svl_enum_member { const char *name; int64_t value; } svl_enum_member;

typedef struct svl_type_desc {
  const char *name;          /* "Gtk.Orientation", "Rect", "Pango.AttrList" */
  uint16_t kind;             /* SVL_ENUM | SVL_FLAGS | SVL_RECORD | SVL_BOXED | SVL_CALLBACK | SVL_LIST */
  uint16_t reserved;
  const svl_enum_member *members; uint32_t n_members;   /* ENUM/FLAGS */
  const svl_slot_desc   *fields;  uint32_t n_fields;    /* RECORD fields / CALLBACK params / LIST element */
  svl_slot_desc ret;                                     /* CALLBACK return */
} svl_type_desc;
```

`svl_backend` gains (appended, so v1 members keep their offsets):

```c
  uint32_t caps;   /* SVL_CAP_UTF16_IN = 1, SVL_CAP_UTF16_OUT = 2, SVL_CAP_BYTES_RETAIN = 4 */
  const svl_member_desc *(*describe)(void *self, svl_name_kind what, svl_id kind, svl_id member);
  const svl_type_desc   *(*describe_type)(void *self, svl_id type);
  int  (*cancel)(void *self, uint64_t callback);         /* async: g_cancellable_cancel; 0 if already done */
  void (*schedule_wake)(void *self, int64_t ms);         /* from the M2 plan */
  /* destroy(h) already means "host dropped its last reference"; it now also covers BOXED handles. */
```

`svl_host` gains:

```c
  int  (*invoke_callback)(void *self, uint64_t callback, const svl_value *args, size_t n, svl_value *out);
  void (*release_callback)(void *self, uint64_t callback);   /* GDestroyNotify, owner gone, async done */
```

Unchanged rules:
- Everything is borrowed for the call.
- Backend out values live in the backend's per-call scratch until the next ABI call.
- `rc < 0` + `last_error` signals failure. New: when the out slot holds `SVL_ERROR`, the host builds a `NativeError` from it.

Not proposed:
- **A BIGINT tag.** `I64` + `UNSIGNED` covers it.
- **A DATE tag.** Datetimes ride as `F64` with `type = SVL_T_DATETIME`; date-only and time-only values are `RECORD`s.
- **A packed binary encoding on the backend ABI.** See §2.3: it belongs to the N-API/FFI fronts if anywhere.

### 5.2 Host and JS conventions

1. **Pass ids, not names, on hot ops.** The JS layer (renderer and generated wrappers) resolves `(kind, name)` → id once and calls `setPropId/callId`. This was the largest measured win (1.43 → 0.77 µs/op).
2. **Type-directed conversion in `svelters-core`**, over a `JsReader` trait per engine, driven by backend descriptors. All validation, coercion and enum/flags naming happens there, so every engine and every backend behaves identically.
3. **Strings:**
   - borrow, never copy, on the way in (`JS_ToCStringLen2`, which is zero-copy for ASCII);
   - pass UTF-16 to backends with `SVL_CAP_UTF16_IN` (Qt) when the JS string is wide;
   - drop the owned `Value` copy from the Rust `Backend` trait in favour of a borrowed `ValueRef<'_>`.
4. **JS shapes:**
   - enums: kebab-case names;
   - flags: arrays of names;
   - value structs: plain objects with the neutral field names;
   - rich values: strings where a canonical string form exists;
   - boxed values: disposable wrapper objects;
   - bytes: `Uint8Array`;
   - datetimes: `Date`;
   - dates and times: part records;
   - nullable slots: `null`, with `undefined` meaning "not provided";
   - outs: return value or a named object;
   - async methods: `Promise` + `AbortSignal`;
   - native errors: `NativeError`.
5. **Handles become objects.** Today handles are bare numbers; boxed values need wrappers with finalizers. Make object handles wrappers too, with one canonical wrapper per handle, held in a host `WeakRef`/handle map. Then `bind:this` targets, method calls and typed `.d.ts` classes all hang off the same object. That is the "one canonical wrapper per native object" rule from `PLAN.md`.
6. **Finalizers only enqueue.** Releases are applied at checkpoints. `FinalizationRegistry` is the portable mechanism; QuickJS class finalizers are an optimization.

### 5.3 What is needed when

| Milestone | Categories | Why |
|---|---|---|
| P5 / M2 (now, no ABI change) | Pass ids; borrowed strings; descriptor-driven attribute coercion replacing `INT_PROPS`; enum names out on both toolkits; `string[]` as `LIST` of `STR` for `css-classes` | Cheap, and all of them fix measured costs or inconsistencies inside v1 |
| **M3** (ABI v2 lands with `schedule_wake`) | `ENUM`/`FLAGS` with type ids and member tables; `RECORD` for Rect/Size/Point/Insets/Color; UTF-16 strings; descriptors; host type-directed conversion with errors; nullability; out params; lists of strings, numbers and objects; `BOXED` handles with finalizers for icons/pixmaps/attr lists/fonts; string-coercible rich values (key sequences, URLs, fonts, icon names); one TS emitter for both toolkits | T1 coverage (Qt P properties 79% → 92%) and T2 for properties. Qt's `QWidget` geometry family, `QAbstractButton.icon`/`iconSize` and GTK `css-classes` are all in the P kinds' immediate neighbourhood |
| **M4** (native semantics) | Callbacks (`invoke/release_callback`, scope handling, owner invalidation); async → Promise for GTK `*Dialog` APIs; `GError` → `NativeError`; borrowed argument handles in sync signals (`GtkTooltip`, `QEvent`, invalidated after `call_sync`); dates/times (`QDateEdit`, `QCalendarWidget`, `GtkCalendar`); variants (QAction data, GAction targets) | All coupled to the sync-handler work, or to the first common widgets that need them |
| **M5** (models) | `GListModel`/`QAbstractItemModel` handles; `QModelIndex` as persistent-index handles; `GtkTextIter` ↔ offsets; numeric typed arrays; bytes with zero-copy textures (`allocBytes`) | Model/view and text-buffer APIs |
| M6 (release linker) | Type-closure tree-shaking of converters/descriptors | §3.4 |
| Later | `bigint: true` members; `MAP` beyond variants; drawing (`QPainterPath`, `GskPath`, cairo); packed op-buffer front for Deno FFI / N-API batching if measured | Rare in the widget surface (§1) |

### 5.4 Risks

1. **GC finalizers running inside a host op.** QuickJS runs class finalizers synchronously during any allocation, including while the host is converting arguments or holding a `RefCell` borrow.
   - Finalizers must only push to a queue: a `Cell`-based or lock-free push, never a `RefCell` borrow the op might hold.
   - They must never call the backend or JS.
   - This is the gjs "callback during GC sweep" bug class.
2. **Re-entrancy during conversion.** Reading a record field or array element can run user JS: getters, `Proxy` traps (every Svelte `$state` object), `valueOf`/`toString`. That JS can call host ops, destroy the target handle, or detach or resize an `ArrayBuffer` whose pointer was already taken. Rules:
   - Convert everything first, with no host state borrowed. Then re-check handle liveness, then dispatch.
   - Never call `valueOf`/`toString` implicitly: a string slot accepts only strings.
   - Take `ArrayBuffer`/string pointers *after* all JS-running steps, or copy.
   - Reject re-entrant ops on the same handle mid-dispatch with an error. Nesting is legitimate only through `call_sync`/`invoke_callback`, which defer tree ops.
3. **Callback lifetime and cycles.** A native closure roots a JS function. The function captures the wrapper of the native owner, so the cycle crosses the GC boundary and leaks. Tie every callback to an owner handle, drop it on `destroy`/`on_destroyed`, use generation-tagged ids, and refuse `scope=forever`. A late native call on a dead id returns the default result; it is never an error and never a crash. This is the 2021 failure mode ("use-after-free on signal disconnect", `gtk-v1-archaeology.md`).
4. **Boxed-value lifetime.**
   - Values must be released when their wrapper dies. GC is not timely for big pixmaps and textures, so expose `dispose()`/`using`.
   - Stale iterator-like values must be impossible by construction (§2.2).
   - Borrowed argument handles (`QEvent*`, `GtkTooltip`) must be invalidated when `call_sync` returns, by a generation bump on the handle.
5. **Borrowed data outliving the call.** Deferred ops in a sync scope, async starts, and backends retaining strings or bytes all must copy. Generated code must copy whenever the native signature takes ownership (`transfer full` in-args, 54 GTK widget callables, or a Qt setter that stores).
6. **Encoding mismatch between engines.** UTF-16 passthrough depends on what the engine stores. The backend must accept both encodings (UTF-8 is always available), so correctness never depends on the fast path.
7. **Type ids are per backend build.** They must not leak into persisted JS state or the op logs. Logs print names (`orientation: "vertical"`, `geometry: {"x":…}`) to keep the Node TestHost and the native host byte-identical.
8. **Qt overloads.** Arity-first plus `Conv<T>::accepts` dispatch can be ambiguous (`setText(QString)` vs `setText(const char*)`, `QIcon`/`QString` overloads that both take a string). Generate overload sets from the metadata, prefer one canonical overload per JS name, and require an override to expose a second one.


## 6. Scripts and reproduction

- `scripts/serdes_gir_inventory.py`: GTK slot categories from GIR (`python3 -I scripts/serdes_gir_inventory.py --json out.json`).
- `scripts/serdes_qt_inventory.py`: Qt slot categories from `metadata/qt/*.json` + qdoc indexes (run from the repo root).
- `scripts/serdes_tiers.py`: the T0–T3 coverage table in §1.2.
- `scripts/serdes_bench/`: the rquickjs bench crate (Part A: the real `svelters-host` path on two dummy backends; Part B: prototypes) and `qstr.cpp` (QString UTF-8/UTF-16 costs: `g++ -O2 -std=c++17 qstr.cpp $(pkg-config --cflags --libs Qt6Core) -fPIC`). Run the binary with no arguments for both parts, or pass a JS path to evaluate it in the Part B context. The process aborts in `JS_FreeRuntime` at exit, because the bench leaks persistent atoms by design. That happens after all output.
