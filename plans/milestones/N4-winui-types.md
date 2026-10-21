# N4-WinUI: WinUI non-GUI types (the WinUI half of N4 in plans/research/NON-GUI-TYPES.md)

Lane: `crates/svelters-winui` (`src/types/`, `tests/types_*.rs`), `tools/metadata/winui` (`types_winui_*.py`,
`gen_types_winui.py`, `winmd-tool types`), `metadata/winui/types`, `metadata/overrides/winui/types`,
`packages/types/winui`, `docs/site/src/content/docs/types/winui`, `fixtures/winui-types`, `examples/counter-win`.
The pilots it mirrors: `docs/notes/nt0-handoff.md` (GTK), `plans/milestones/NT1-gtk-async.md` (async),
`plans/milestones/NT2-qt-types.md` (Qt). Nothing here runs on Linux except the platform-neutral core and the tools;
every runtime result comes from the `windows-winui` CI job.

## Pipeline

```text
Windows App SDK 2.5.1 winmds (fetch.py) ─┐
Windows.winmd (windows-default 0.100) ───┴─ winmd-tool types <namespaces> ─▶ types_winui_ingest.py
      ─▶ metadata/winui/types/ingested/<Namespace>.json + _outside.json   (raw, origin winmd; checked in, 2.3 MB)
metadata/overrides/winui/types/winui-types.yaml (curated, roots, scope, exclusions, subjects, async checks) ─┐
ingested ──────────────────────────────────────────────────────────────────────────────────────────────────┴─▶
      types_winui_classify.py --report/--write/--check ─▶ metadata/winui/types/<Namespace>.json + report.txt  (pure)
facts ─▶ gen_types_winui.py ─▶ crates/svelters-winui/src/generated/types_tables.rs (const data)
                             ─▶ packages/types/winui/<ns>/<Class>.js, <ns>.d.ts, manifest.json, package.json exports
                             ─▶ docs/site/src/content/docs/types/winui/*.md
                             ─▶ crates/svelters-winui/tests/types_verify.rs + src/generated/types_facts.json (ledger)
```

- **Ingest.** `winmd-tool types` (new subcommand) dumps every type of 21 namespaces at the ABI level: interfaces with
  their methods in vtable order, GUIDs, generic parameters, `in`/`out` parameter flags, struct fields, enum
  underlying types, delegate signatures, and the class attributes (Activatable, Static, Composable, Deprecated,
  ExclusiveTo, Overload, DefaultOverload, ...), plus what they mention outside those namespaces (so the classifier can
  name a blocker and see widget bases). The namespaces: Windows.Foundation(.Collections, .Numerics), Windows.UI,
  Windows.Storage(.Streams, .FileProperties, .Search), Windows.ApplicationModel.DataTransfer, Windows.Data.Json,
  Microsoft.UI(.Dispatching, .Composition, .Windowing), Microsoft.UI.Xaml(.Media, .Media.Imaging, .Media.Animation,
  .Documents, .Data, .Input). Splitting ingest from classification keeps `--check` runnable without the SDK.
- **Classify** (section 3.1, WinUI column): struct -> record (`DateTime` -> `Date`, `TimeSpan` -> milliseconds);
  runtime class -> object, model when it implements `IVector`/`IObservableVector`/`IMap`/`IObservableMap`, statics
  when it has no constructor and no instance member, widget when it derives from `UIElement` (elements, not bound),
  args for `*EventArgs`; `IAsyncOperation*`/`IAsyncAction*` -> async; enum (`Flags` -> flags); delegate -> callback;
  `Deprecated` and the override `exclude:` list -> excluded; `ExclusiveTo` interfaces fold into their class.
- **Scope.** 31 roots (Uri, PropertySet/ValueSet/StringMap, JSON, Colors/ColorHelper, DispatcherQueue(Timer),
  Thickness/CornerRadius/GridLength helpers, SolidColorBrush/LinearGradientBrush/GradientStop, FontFamily,
  BitmapImage, StorageFile/StorageFolder/FileIO/PathIO, DataPackage/Clipboard, Compositor) plus the signature closure
  of their bound members within 13 scope namespaces, plus bases. A member that mentions a class or interface outside
  them is blocked `out-of-scope:<Type>` (Windows.Storage.Search, Windows.System.User, most of Composition). The
  roots list and the scope list are the knobs.

## Design decisions

### Dispatch: data tables and one dynamic invoker (no per-member native code)

The plan's C++/WinRT core with "one `case` per member" (section 2.3) would have meant either C++/WinRT in the
build or windows-rs typed bindings for every bound type. windows-rs repeats every base member on each derived class:
`bindings.rs` is already 64.9k lines (3.3 MB) for 17 element classes (AI1), and each typed member call is generic
code the leaf crate compiles. Instead:

- The generator emits **const data only**: per kind its method groups, props, events; per member the interface (an
  index into a table of 296 IIDs, parameterized ones computed by the generator with the WinRT UUIDv5 algorithm over
  the type signature), the vtable slot and the parameter types (an index into a table of ABI types). 84 kinds,
  1,141 members: `types_tables.rs` is 2.2k lines / 240 KB, no `unsafe`, no functions (a pytest enforces section 2.4).
- The hand-written core calls a slot through **one invoker** (`types/invoke.rs`). Every WinRT method is
  `HRESULT f(this, args..., [out, retval] T*)` with `__stdcall`, which on x86_64 is the Microsoft x64 convention:
  one 8-byte slot per argument; RCX/RDX/R8/R9 or XMM1-3 by position for the first four; structs of 1/2/4/8 bytes as
  integers, larger ones by pointer to a caller-owned copy; the caller owns and cleans the stack, so extra slots are
  harmless. So 27 function shapes (the float pattern of slots 1-3) with 15 slots each cover every method. The same
  convention exists on every x86_64 target as `extern "win64"`, so the invoker and the delegate thunks are
  unit-tested on Linux (`cargo test -p svelters-winui --lib`: integer, float register, stack, struct-by-value and
  struct-by-pointer cases, vtable dispatch, delegate thunks).
- Delegates (event handlers, async completions) are one COM object shape (`types/delegate.rs`) with a run-time IID
  and an `Invoke` thunk per argument count (0-6 integer-class arguments; events with float arguments are blocked
  in the facts).
- Cost: no bindings growth at all (bindings.rs unchanged), and the crate's compile time is the core's (~2.6k
  lines) plus const data. aarch64 Windows is not covered by the invoker yet (it answers an error); the CI runner and
  the gnu/msvc cross-checks are x86_64.

### Activation, statics, composition

- `new` is a static method group: `IActivationFactory.ActivateInstance` (Activatable without a factory interface)
  and every factory-interface method; composable factories (`Composable(..., Public)`) pass a null outer and release
  the inner. Factories come from `RoGetActivationFactory` (the bootstrapper makes the framework package's classes
  visible), with windows-rs's reg-free `DllGetActivationFactory` fallback, cached per thread.
- `construct(kind, props)` is the zero-argument constructor plus each prop's setter. In JS, `new X({...props})`
  goes to `construct` when every key is a prop, else to `new` with the arguments (so `new SolidColorBrush(color)`,
  a record, is the factory constructor).
- Statics go through the class's `Static` interfaces; static properties are static getters in JS (`Colors.red`).

### Async (the NT1 mapping)

`IAsyncOperation<T>`, `IAsyncAction` and their `WithProgress` variants are `SVL_M_ASYNC` members: the host appends
its one-shot completion; the backend sets `Completed` to a dynamic delegate whose IID is the parameterized handler's.
The completion **always** hops to the UI `DispatcherQueue` first (rule 4 of section 4.1; it also avoids re-entering
the host when an operation completes synchronously inside `put_Completed`), then calls `GetResults`: its value, or
its failure with the operation's error info, becomes the result or a `NativeError { domain: 'winrt', code }`.
Cancelled operations reject with `cancelled`. `{ signal }` maps to `cancel(id)` -> `IAsyncInfo.Cancel`; destroying
the owner cancels its operations and forgets their completions (the host settles them `disposed`). Progress is not
reported (the Promise covers the result only).

### Threading (decision Q8, section 4.1)

- Thread class from the structural signal: `dispatcher` when the class or a base has an instance `DispatcherQueue`
  property (12 bound kinds: the DependencyObjects, Compositor), `ui` for the rest of Microsoft.UI.Xaml (9), `any`
  otherwise (63).
- One UI thread; at minting a `dispatcher` object's queue must have thread access (`HasThreadAccess`), else the
  handle is refused with "belongs to another UI thread" (tested with a Compositor made on a second dispatcher
  thread); `ui` objects need the UI thread. Debug builds repeat the check on every call.
- Events raised on another thread copy their arguments (a reference per object, a copy per string) and hop before
  conversion; completions always hop.

### Lifetime and identity

Every runtime class object is SHARED (one strong COM reference; identity by IUnknown pointer, in the same handle
table as the elements). The runtime class name (`GetRuntimeClassName`) picks the kind when it is bound (a
`StorageFile` returned as `IStorageItem` is a `StorageFile`), else the declared kind, else the `IInspectable` root.
`destroy` releases our reference; `IClosable.Close` is the explicit `close()` method (rule 6).

### Overload naming

One JS name per native name; the backend picks the overload by argument count, WinRT's own rule (overloads of one
interface differ in arity, except through `DefaultOverload`). Same-arity overloads would keep the `DefaultOverload`
under the name and take the `Overload` attribute's name otherwise: none occur in the bound scope (0 renamed, 44
names overloaded by arity). The host pads omitted optional arguments with nulls, so trailing nulls select the
narrowest overload that still takes every non-null argument; the generated JS splits a trailing `{ signal }` off
async calls and pads to the widest overload so the host recognizes it.

### Values

Records are plain objects with camelCase field names (`Color {a, r, g, b}`, `Thickness {left, top, right, bottom}`);
enums are kebab-case names (as the WinUI element props; native and camelCase names and numbers are accepted), flags
lists of names. `object` (IInspectable) arguments box JS scalars as Boolean/Double/String/DateTime through
`PropertyValue` (the JS projection's rule); results unbox `IPropertyValue` scalars, other objects are handles.
`IReference<T>` is `T | null`; `IVectorView`/`IIterable` results are arrays, `IMapView` plain objects,
`IKeyValuePair` `{key, value}`; `IVector`/`IMap`/`IObservableMap` results are live objects of a generic kind.

### Reactivity

Props backed by a DependencyProperty (a `FooProperty` static on the class: 22 in the bound scope) are reactive
getters (`observed`): `observe` registers `RegisterPropertyChangedCallback` with the DP object fetched through the
static getter (the R4 mechanism), and their descriptors carry `SVL_M_OBSERVABLE`. Other props are plain reads.

### JS modules (decisions Q5, Q9)

One module per native namespace, `svelters:winui/<namespace in lower case>` (`svelters:winui/windows.foundation`),
aliased next to the GTK and Qt ones in `typesAliases()`. One file per class, `index.js` re-exports, no top-level
side effects; a namespace that only has records or enums gets a types-only module. Generic collection interfaces are
one class per definition (`IVector<T>`). `packages/types/winui/_rt.js` (hand-written) holds the WinUI helpers
(async argument normalization, props-or-arguments constructor test, `IInspectable` wrapping). `manifest.json`
carries imports and signature references for the M6 linker; a vite test proves import-driven tree shaking.

## Completion record

(filled in below)
