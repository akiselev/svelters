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
- **Scope.** 30 roots (Uri, PropertySet/ValueSet/StringMap, JSON, Colors/ColorHelper, DispatcherQueue(Timer),
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

Branch `worktree-agent-a4956e8a76f5ce748`, pushed as `ci/n4-winui`, based on master bab98ff2 (master has since only
gained a STATUS line). Not merged.

### Commits

- 6852249d: `winmd-tool types`, ingest, classifier, facts, generated tables, platform-neutral core (invoker, values,
  model) with Linux unit tests.
- ed7f0fa2: the COM core (`rt.rs`, `delegate.rs`, `ops.rs`), backend wiring, `types_core` tests, workflow step.
- 47b1a87a: JS modules, typings, manifest, docs pages, generated ledger tests and facts, ledger merge.
- 4deb844b: `fixtures/winui-types`, `counter-win --fixture winui-types`, typings and tree-shaking tests, pytest.
- 7f5b2e63: counter-win walks toplevel windows (SHARED type handles are host roots too); factories released at
  shutdown, never by a TLS destructor; backend-abi and NON-GUI-TYPES notes.
- 16f74f9c: type events raised before the host attaches are queued and posted when it does.
- 54ce609c: ledger import (run 39573995327) and coverage.
- The commit after it adds this record only.

### CI (`ci/n4-winui`)

| Head | windows-winui | js | linux-gtk | linux-qt | macos-appkit |
|---|---|---|---|---|---|
| ed7f0fa2 | success 39570528975: lib 26/26, generated 495/495, **types_core 8/8** (first run) | failure (`gen_backend` facts stale: testing.rs is in the observe fingerprints) | success | success | success |
| 47b1a87a | failure 39571527229: types_core 8/8, types_verify 43/45 (two generated reader checks treated a native WinRT refusal as a failure: `JsonObject.getArray`, `parse("svelters")`) | failure (type facts not in the ledger yet) | success | success | success |
| 4deb844b | failure 39572396616: types_verify 45/45; `winui_types_selftest` read no widgets (counter-win took the first host root, now a SHARED Uri) | failure (ledger) | success | success | success |
| 7f5b2e63 | failure 39573359763: fixture `size 3, changes 1` (map-changed raised during the script, before `run` attached the host, was dropped) | failure (ledger) | — | — | success |
| 16f74f9c | **success 39573995327**: lib 26, generated 495, types_core 8, types_verify 45, counter selftest, `winui_types_selftest` | failure (ledger, imported next) | — | — | — |
| 54ce609c | **success 39574871402** (same counts) | **success 39574871453** (pytest 146 passed / 1 skipped, `ledger winui: 105 manual + 480 inferred verified`) | **success 39574871375** (GTK 2132/2132, GTK 4.14 1889/1889) | **success 39574871417** (95/95) | **success 39574871388** |

The fixture run (`counter-win --fixture winui-types --selftest`, 54ce609c) reads back `example.com:8080/docs` (Uri
props), `size 3, changes 3` (PropertySet events, including two raised before the loop), `brush 0,0,255 @ 0.5`
(SolidColorBrush built with props, then recolored by a click: the DependencyProperty getter re-rendered the text with
no `$state` mirror) and `pasted copied` (DataPackageView.GetTextAsync resolved after the hop).

### Ledger

`ledger.py import --toolkit winui` of 39573995327: 585 facts (540 before), 0 manual / 0 inferred not passing, 0
pending, 8 parked (pre-N4). The 45 new facts are `svelters-api:v1:type:winui:<Namespace>:<Type>:<group>` for the 12
curated types (construct, props, methods, statics, events, async, lifetime, thread). `coverage.py` counts them as a
`types` row. The 145 observe facts re-verified in the same import (their fingerprint covers `testing.rs`, which N4
extended).

### Gates (local, after the last code change, 16f74f9c; the later commits are ledger, coverage and docs)

| Gate | Result |
|---|---|
| `tools/dev/cargo-slot cargo check --workspace --all-targets` | rc 0 |
| `cargo check -p svelters-winui -p counter-win --tests --target x86_64-pc-windows-gnu` | rc 0 |
| `cargo test -p svelters-winui --lib` (Linux: invoker, model, values, tables) | 26/26 |
| `corepack pnpm -r build` (pnpm shim on PATH) | rc 0 (docs site 934 pages; `check-links`: all resolve) |
| `corepack pnpm test` (with `test:meta`) | rc 0; pytest 146 passed / 1 skipped; `@svelters/types` 25 node tests + strict `tsc` over GTK, Qt and WinUI typings; `ledger.py check` ok |
| `python3 tools/docs/build_ir.py`, `corepack pnpm docs:test` | rc 0 (11 Python tests, 1 skipped; 17 site tests) |

The msvc cross-check was not run here (the target is not installed on this machine); CI builds with msvc.

### Classification (`metadata/winui/types/report.txt`)

All 1,780 types of the 21 ingested namespaces: record 32, enum 133, flags 14, callback 46, object 385, model 36,
statics 44, args 67, interface 59, class-interface 930, collection 10, async 5, reference 2, widget 3, internal 2,
excluded 12, unclassified 0.

Bound: **84 kinds** (30 roots, 5 generic instances): Windows.Storage 13, DataTransfer 12, Xaml.Media 10,
Foundation.Collections 8, Xaml 7, FileProperties 7, Streams 7, Foundation 5, Data.Json 4, Dispatching 3,
Xaml.Media.Imaging 3, Microsoft.UI 2, Windows.UI 2, Composition 1. Thread classes: any 63, dispatcher 12, ui 9.

Members of bound kinds: **979 of 1,141 bound (85%)**: Microsoft.UI, Windows.UI, Streams, Imaging 100%; Data.Json
95%; DataTransfer 91%; Xaml.Media 89%; Storage 88%; Dispatching 88%; Foundation.Collections 87%; FileProperties
86%; Xaml 84%; Foundation 70%; Composition 10% (Compositor is bound for the thread check; the rest of Composition is
out of scope on purpose). Blocked 162: out-of-scope 84, array 28, collection argument 22, `IIterator` 13, delegate
parameter 8, static event 4, struct with a string field 3. Excluded: 3 deprecated, 3 by override. 22
DependencyProperty statics hidden (the instance props are reactive). Overloads: 649 JS method names, 44 overloaded by
arity, 0 renamed. Curated: 12 types, 45 ledger facts.

### Code size and build time

- Hand-written core: `src/types/` 2.6k lines (invoke 388, model 485, value 404 with their unit tests; rt 539,
  ops 567, delegate 185), backend wiring ~130 lines, harness ~280 lines.
- Generated Rust: `types_tables.rs` 2.2k lines / 240 KB of const data (84 kinds, 1,141 members, 200 IIDs, 166 ABI
  types); `bindings.rs` unchanged (64.9k lines). Generated tests: `types_verify.rs` 0.9k lines.
- Leaf crate rebuild (`cargo build -p svelters-winui --lib --target x86_64-pc-windows-gnu`, debug, touch `lib.rs`,
  three runs each on a shared, loaded machine): master 4.9 / 5.5 / 5.8 s, N4 5.8 / 6.5 / 7.7 s, so about +1 s.
  Debug rlib 40.5 MB -> 47.7 MB.
- JS: `packages/types/winui` 664 KB (84 classes, 16 namespace modules incl. types-only ones); a bundle keeps only its
  imports' signature closure (`fixtures/winui-types` imports 5 classes and bundles 36 of the 84).
- Facts: ingested winmd dump 2.3 MB, classified facts 0.7 MB (checked in).

### Shared files touched (additive)

`packages/types/vite.js` (`winuiDir`, the `svelters:winui/<ns>` alias), `tools/metadata/common/types_pkg.py` (the
`./winui/*` group and `./manifest-winui`), `packages/types/package.json` (generated exports),
`packages/types/test/types/tsconfig.json` (+ `winui_usage.ts`), `packages/types/test/shake-winui.test.mjs`,
`tools/metadata/ledger.py` (the winui manifest includes the type facts; a CI artifact's facts snapshots merge),
`.github/workflows/windows-winui.yml` (one step: `types_core` and `types_verify` append to the results file and copy
`types/facts.json`), `docs/site/astro.config.mjs` (sidebar group), `pnpm-lock.yaml` (the new fixture's importer),
`plans/architecture/backend-abi.md` and `plans/research/NON-GUI-TYPES.md` (new sections). `packages/types/src/core.js`
is unchanged: the WinUI helpers live in `packages/types/winui/_rt.js`.

### Open items

- **Arguments not marshalled yet:** JS arrays into `IIterable<T>`/`IVectorView<T>` parameters (22), arrays (28),
  delegate parameters (8; e.g. `DispatcherQueue.TryEnqueue`), static events (4), structs with string fields (3).
  Each needs a small core addition (a dynamic `IIterable` object, `CALLBACK` slots with `describe_type`).
- **aarch64 Windows:** the invoker refuses with an error (ARM64 passes floats and HFAs differently).
- **Async progress** is not reported; only the result.
- **Element interop:** an element can be passed where a type member expects a `UIElement`, but element props of
  object type (`Foreground: Brush`, `Margin` already takes a record) do not accept type handles yet (the element
  generator has no converter for them).
- **Element events before `run`** are still dropped (only type events are queued; changing it would change the
  element op logs).
- **The thread check per call** runs in debug builds only (section 4.1 rule 3), as planned.
- The 8 inferred WinUI facts parked before N4 stay parked.

