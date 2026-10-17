# Serdes lane (S1): implementation notes

Design: `docs/notes/js-native-serdes.md`. ABI: `plans/architecture/backend-abi.md` ("Planned ABI v3: value model").
This file records what was built, where it deviates from the design and why, and what is left.

## Layout

| Piece | Where |
|---|---|
| C header, Rust mirror, safe wrappers | `crates/svelters-abi/include/svelters_backend.h`, `src/lib.rs`, `src/safe.rs` |
| Host conversion (engine-agnostic) | `crates/svelters-host/src/convert.rs` (traits `JsReader`/`JsWriter`, `encode`, `decode`, `Arena`) |
| Host ops: name/descriptor caches, validation, boxed release | `crates/svelters-host/src/core/ops.rs` |
| QuickJS reader/writer | `crates/svelters-host/src/quickjs/conv.rs`, bindings in `quickjs.rs`, wrappers in `prelude.js` |
| GTK converters and descriptors | `crates/svelters-gtk/src/convert.rs` (GType-keyed), `types.rs` (type ids, descriptors) |
| Qt converters | `tools/metadata/qt/templates/serdes.hpp.in` (`Conv<T>`), emission in `tools/metadata/qt/gen_cpp.py` |
| Qt type classification | `tools/metadata/qt/ingest.py` (`EnumIndex`, `neutral_type`), enum table `metadata/qt/_enums/Qt.json` |
| Renderer | `packages/renderer/src/hostops.js` (ids), `coerce.js` (descriptor-driven coercion) |
| Typings | `tools/metadata/common/tsvalues.py` -> `packages/renderer/types/values.d.ts`; per toolkit in `gtk.d.ts`/`qt.d.ts` |
| Tests | `crates/svelters-host/tests/serdes.rs` (+ `tests/js/serdes_*.js`), `crates/svelters-gtk/tests/serdes.rs`, `examples/counter-qt/tests/serdes.rs`, `packages/renderer/test/serdes.test.mjs` |
| Benchmark | `crates/svelters-host/examples/setprop_bench.rs` |

## ABI v3 as built

- `svl_value` is 24 bytes: `u16 tag, u16 flags, u32 type, union u`. Tags ENUM, FLAGS, RECORD, BYTES, BOXED, CALLBACK, MAP, ERROR added; flags UTF16, NUL_TERM, UNSIGNED; well-known type ids `SVL_T_*`; backend types start at `0x10000`.
- Descriptors: `svl_slot_desc`, `svl_member_desc`, `svl_type_desc`, `svl_enum_member`, as in the design. `describe(what, kind, member)` is called with the handle's kind; `describe_type(id)`. Both may be NULL/return NULL (the host then converts dynamically, the P-era rules).
- `svl_backend` appends `caps, describe, describe_type, cancel` after `last_error`. `svl_host` appends `invoke_callback, release_callback`.
- Props: a prop's member descriptor has `params[0]` = the written slot (absent when read-only) and `ret` = the read slot.
- `SVL_D_STRING_FORM` (new, not in the design): a RECORD or BOXED slot that also accepts a string in the type's canonical form (`"#ff8800"`, `"Ctrl+S"`, `"document-new"`). `SVL_D_READONLY` is defined but not used: read-only is "no parameter".
- Datetimes: `F64` with type `SVL_T_DATETIME`, a `Value::DateTime` in Rust, a JS `Date` in the host.

### How this merges with the approved v2

v2 (timers + lifetime) landed on master first (`schedule_wake`, `handle_info`, `on_destroyed(reason)`, `SVL_ABI_VERSION 2`). This branch merged it and bumps the version to 3 on top. v2 put its two members in the middle of the vtable (after `listen` and `wake`), so the v3 members are simply appended after `last_error`; nothing of v2 moved. The Rust `Backend` trait keeps v2's required `schedule_wake`/`handle_info` and gains v3 methods that all have defaults (`caps`, `describe`, `describe_type`, `cancel`, `set_prop_ref`, `call_ref`), so an existing Rust backend only needs the v2 methods.

## Decisions and deviations from the design

- **Borrowed values in the Rust trait.** `Backend::set_prop_ref(h, prop, ValueRef)` and `call_ref(h, method, ArgsRef)` default to the owned methods. A backend overrides them to avoid the copy. The raw `BackendRef` gained `set_prop_raw`/`get_prop_raw`/`call_raw`, which is what the host uses.
- **Qt `Conv<T>` is typed by C++, descriptors by Conv.** The generator does not spell tags into descriptors: `describe()` builds each slot from `Conv<decltype(getter())>::slot(name)`, so the descriptor cannot disagree with the converter. Python only classifies the metadata ref (for TS and for the enum closure) and registers the enums, boxed classes and owners a slot needs.
- **Records have per-backend integer variants.** `QRect`/`QSize`/`QPoint`/`QMargins` and `GdkRectangle`/`GtkBorder` are separate record types (backend ids) with integral fields and the same name (`Rect`); the float forms use the well-known ids. The host rejects `{x: 1.5}` for an integral field with a `RangeError`.
- **Enums on the wire are native values.** `ENUM{type, i}`; the host owns the name lookup (design 2.2). Both toolkits therefore return names once the host decodes. The raw GTK/Qt vtable no longer returns nick strings; the GTK test harness (`Harness::named`, `Harness::get`) plays the host's part for tests that drive the vtable by name.
- **Qt enum names** are kebab-case with the common leading words stripped only when names stay unique (`AlignHCenter` -> `hcenter`, `PasswordEchoOnEdit` -> `password-echo-on-edit`). An override `values:` table (e.g. `QBoxLayout::Direction` -> `horizontal|vertical`) replaces the generated table for that enum everywhere.
- **Enum table in the metadata.** The ingest writes `metadata/qt/_enums/Qt.json` (kind `namespace`) with the members and C++ expressions of every enum or flags type a slot mentions, whichever class declares it (QtCore, QtGui, QtWidgets). Class JSONs keep their own enums.
- **FinalizationRegistry without an unregister token.** QuickJS-NG holds the unregister token strongly, so `register(this, h, this)` keeps the wrapper alive forever (covered by a test). `dispose()` therefore releases immediately and the registry callback of a disposed wrapper is ignored by the core (`boxed` set), which also makes double release harmless.
- **Release at checkpoints.** The registry callback only calls `Core::queue_release` (a `RefCell` push, no backend call); `Core::process_releases` runs at the end of every checkpoint, outside any JS entry. `Host::collect_garbage` is a test hook.
- **Exceptions from user JS during conversion** (a getter, a Proxy trap) propagate unchanged: the reader keeps the pending exception and the op rethrows it instead of wrapping its message.
- **One host op per verb for names and ids.** `setProp(h, nameOrId, v)`, `getProp`, `call`, `listen`, `create(kindOrId)`, plus `kindId/propId/signalId/methodId/slotId` and `propType(kindId, propId)`. The renderer resolves once per (kind, name) and falls back to names for hosts without ids.
- **Coercion.** Attribute strings are coerced in two places that agree: the renderer, for hosts that publish `propType` (the TestHost via `packages/testhost/src/schema.json`, generated from the GTK metadata), and the host core, from the descriptors (so the renderer is not needed for correctness on a native host). The renderer's hand-written `INT_PROPS` is gone. `packages/dom` (the older DOM shim) still has its own `INT_PROPS`; it is not part of the renderer path.
- **Log backend.** `LogOptions::schema` makes the log backend publish integer/number/boolean property descriptors from the same schema file, so the fixtures' op logs coerce `spacing="8"` to `8` on the native host exactly as the Node TestHost does. Typed values (enum, record, ...) print their raw payload in the log; names would need type descriptors in the log backend (not needed by any fixture).

## What is covered

See the tables in the final report for the per-toolkit type list; each is exercised by the round-trip tests listed above.

## Open

Closed by S2 (`docs/notes/s2-abi-v3-rest.md`): out / in-out params, Qt callbacks and async, Qt UTF-16 string results, exact GVariant typing and GValue props, VALUE-class handles, `packages/dom` coercion.

Still open:

- Events decode from owned `Value`s; a borrowed fast path may matter for hot signals (`changed` text).
- A Qt callback API that returns a value is unproven on a real Qt class (the ingest covers QtWidgets classes only; `QTextDocument::setResourceProvider` would be the proof).
- GTK time-only / date-only records; list models (M5).
- Typed variants do not round-trip their signature on read.
- The `gio-file` service should move into the generated tables once the GI invoker owns callback scopes.
