# N4-AppKit: AppKit non-GUI types (N4 of plans/research/NON-GUI-TYPES.md)

Lane: `crates/svelters-{appkit,apple}`, `tools/metadata/appkit`, `metadata/appkit/types`, `metadata/overrides/appkit/types`,
`packages/types/appkit`, the AppKit type docs pages, `fixtures/appkit-types`. The GTK pilot and the Qt slice it mirrors:
`docs/notes/nt0-handoff.md`, `plans/milestones/NT1-gtk-async.md`, `plans/milestones/NT2-qt-types.md`.

Nothing here runs on Linux except the metadata tools and the JS package: the Apple crates are checked for
`aarch64-apple-darwin` without an SDK, and every native result below comes from the `macos-appkit` CI job.

## Pipeline

```text
objc2-foundation / objc2-app-kit 0.3.2 generated sources (crates.io; MIT/Apache; readable on Linux)
   │ types_objc2_ingest.py        (registry needed; deterministic; --check)
   ▼
metadata/appkit/types/ingested/{Foundation,AppKit}.json     raw facts: 587 classes, 14k methods, protocols,
   │                                                         structs, enums, typedefs, statics (4.2 MB)
   │ + metadata/overrides/appkit/types/appkit-types.yaml     roots, curated set, 4 class corrections, string
   │ + metadata/probe/appkit/types-<SDK>.json (optional)     initializers, test subjects, one exclusion regex
   ▼ types_appkit_classify.py --report/--write/--check       (pure: no registry, no Mac; in pytest)
metadata/appkit/types/{Foundation,AppKit}.json + report.txt
   │ gen_types.py --write/--check
   ├─▶ packages/types/appkit/{foundation,appkit}/<Class>.js, enums.js, index.js, <fw>.d.ts, manifest.json
   ├─▶ crates/svelters-appkit/src/generated/types.rs        bound classes + async members (const data)
   ├─▶ docs/site/src/content/docs/types/appkit/             index + a page per curated type
   └─▶ examples/counter-mac/tests/types.rs + metadata/appkit/types/facts.json   ledger tests and facts
```

- **Decision Q7 as built.** objc2's header-translator output is the only ingest. It gives the superclass chains,
  `MainThreadOnly`, `Send`/`Sync`, conformances (NSCopying), objc2's reviewed safe/unsafe split with its `# Safety`
  notes, method families, read-write property pairs ("Setter for"), `NSError **` outs (objc2 folds them into a
  `Result` and marks the selector with a trailing `_`), block parameters with their signatures, nullability
  (`Option`), enums/options with values, typedefs. What it does not mark: **read-only properties** (an `@property
  (readonly)` is an ordinary zero-argument method) and string constants' values. The first comes from a runtime
  probe on the macOS runner (`crates/svelters-appkit/examples/types_probe.rs`, `class_copyPropertyList`), committed
  like MAC1's probe; MAC1's libclang pass was not needed.
- **Unsafe members.** objc2 marks 18% of the methods `unsafe`. Its safety notes that the core covers at run time
  (arguments are converted against the runtime method signature, ObjC exceptions are caught, pointers and
  selectors are refused by the type mapping, completion blocks only queue to the main thread) are accepted: "should
  be of the correct type", "might not allow None", "has further requirements", "block must be sendable", "must be a
  valid pointer/selector". Any other note ("not retained internally", threading requirements) excludes the member.

## Design decisions

### Dispatch: reflective (KVC + NSInvocation), not generated objc2 calls

The core reaches every member by its selector at run time: props through KVC (`valueForKey:`/`setValue:forKey:`),
methods and class methods through `NSInvocation` driven by the runtime method signature (`bridge::invoke_ex`). The
generated layer is data and JS only, as for GTK (§2.4 "reflection-first toolkits"): `generated/types.rs` lists the 73
bound classes and the 23 async members, nothing else.

Measured (release, `aarch64-apple-darwin`, this machine, no SDK, sccache off):

| | before (master bab98ff2) | after |
|---|---:|---:|
| rebuild of svelters-apple + svelters-appkit after a touch | 6.1 s | 7.5-8.5 s |
| `libsvelters_appkit.rlib` | 1.33 MB | 1.74 MB |
| `libsvelters_apple.rlib` | 0.59 MB | 0.75 MB |

The generated-objc2 alternative (one typed `msg_send!` thunk per bound member, as NT2's Qt thunks) needs the objc2
header features of the bound scope: a clean release build of objc2-foundation + objc2-app-kit with those features
(32 Foundation + 46 AppKit headers) takes **30.5 s** against **13.6 s** with today's features, before a single thunk
is compiled; NT2's 2.6k Qt thunks added 110 s and 2.9 MB to their unit. The reflective path costs about 1.6 s and
0.6 MB of rlib (most of it the block shapes and conversions), and the scope can grow without native code.
`NSInvocation` costs a few microseconds per call (signature lookup + boxing), which only matters in loops over
thousands of members; release thunks for hot members stay possible later (M6).

### Lifetimes (decision Q1)

- **Every non-widget object is a `SHARED` handle** holding one strong reference (retain/release), one handle per
  object: identity through the pointer map, which is safe because our reference keeps the pointer from being
  reused. `destroy` (dispose or GC) drops our reference only.
- **Value classes use the same handles.** An immutable ObjC object is shared without copying, and sharing it is
  indistinguishable from copying it, so nothing crosses as `BOXED`. The JS side still treats them as values
  (`NativeValue`: no identity contract, `equals()` is `isEqual:`, `toString()` is `description`, mutators hidden).
  A declared value whose runtime object is mutable (an `NSMutableAttributedString` where an `NSAttributedString` is
  declared) needs no special case.
- **Widgets stay BORROWED** (X1): views, windows, cells, responders, and any class with an element kind.
- **Element props changed too.** A non-view object an element getter returns (`font`, `text-color`, `image`) used to
  be BORROWED with kind 0; it is now SHARED with its type kind, so it can be passed back and called.
- **Ownership of results**: `init…` (through `call_static`: alloc + init) and the new/copy/mutableCopy families are
  +1; everything else +0 and retained. Class clusters implement initializers on the placeholder `+alloc` returns, so an
  `init…` selector resolves by name and is checked on the allocated object.

### Kinds

Any loaded ObjC class is a kind named by its class name, registered on first use (ids from `0x10000`). An object's
kind is the nearest class of the bound scope along its chain (`__NSCFString` → no; `NSComponentColor` → `NSColor`), so
`describe(SVL_KIND)` and the host's `typeChain` name public classes.

### Async mapping (the NT1 model)

- An async member is a method whose last parameter is a completion block (`completion`, `completionHandler`,
  `handler`, `reply`, `resultHandler`) returning void. JS gets a Promise (`SVL_M_ASYNC`, a trailing `{ signal }`).
- **Blocks without libffi.** The core builds blocks of a fixed set of **shapes** (`svelters_apple::blocks::SHAPES`:
  one letter per argument, `o` object, `e` NSError, `b` BOOL, `i` integer, `d` double; nine shapes). The classifier
  binds only members whose block has one of them; the bound scope uses `(e)` and `(oe)`. A test checks the lists agree.
- **Threads.** A block may run on any queue (NSItemProvider completes on a background queue). It retains its object
  arguments, queues them and wakes the run loop; the main thread converts them (minting handles) and completes the
  host's callback. A block called twice is ignored after the first call.
- **Results.** The non-error argument resolves (bytes, objects, ...); a non-nil `NSError` rejects with a `NativeError`.
- **Cancel.** `{ signal }` → `backend.cancel(id)`: an `NSProgress` the method returned is cancelled, and the Promise
  rejects at once with `NSCocoaErrorDomain` 3072 (`NSUserCancelledError`); a late completion is dropped.
- **Owner death.** Destroying the receiver drops its operations; the host rejects them with `disposed`.

### Errors

A selector ending in `error:` (objc2's `Result`) hides its `NSError **`. A failure (NO, nil, or a set error with a void
result) is a `NativeError` (`domain`, integer `code`, `localizedDescription`): a sync call throws, an async one rejects.

### Names (overloads and selectors)

- **Methods join the selector's pieces in camelCase** (`setString:forType:` → `setStringForType`), dropping a trailing
  `error:` and the completion piece. This is objc2's Rust name camelCased: deterministic, stable when Apple adds a
  sibling selector (a first-piece-only rule would rename members on upstream changes), and unambiguous (0 collisions in
  the bound scope; `renames:` in the overrides exists for future ones). Long names are the price.
- **Initializers are statics named `new…`**: `initWithString:` → `NSURL.newWithString(s)`. A short `string_ctor`
  list maps `new X(string)` to one initializer (`NSURL`, `NSAttributedString`, `NSUUID`, `NSLocale`,
  `NSURLComponents`); `new X(props?)` is alloc/init plus KVC.
- **The JS name is only for typings and docs**: generated code passes the selector (`call(this, 'setString:forType:',
  [s, t])`), and the core resolves selectors exactly, so naming never affects dispatch.
- **Properties** are accessors named by their KVC key (read-write pairs from objc2; read-only ones from the probe).
  A value class reads them with its getter selector; an object class through `getProp` (reactive with `observed`
  when an observe fact exists; no type class has one yet, so they are plain reads).
- **Hierarchy**: a JS class extends its bound superclass when both are values or both objects; across the boundary
  (`NSMutableAttributedString` → `NSAttributedString`) the object class flattens the value class's members.
- **VALUE mutators (Q3)**: setters and `set*/add*/remove*/insert*/replace*/append*` selectors of value classes are
  hidden (14 in the bound scope); the immutable half of an `NSMutable*` pair keeps everything. Residual hand list: 0.

### Conversions (hand-written, `svelters_apple::objc::bridge`)

NSString ↔ string, NSNumber ↔ number (CFBoolean ↔ boolean), NSArray/NSSet/NSOrderedSet → list (lists in: NSArray),
NSDictionary ↔ object map, NSData ↔ bytes, NSDate ↔ Date, NSNull → null, NSRect/NSSize/NSPoint/NSEdgeInsets/NSRange ↔
records, `Class` ↔ type name, NSError → `NativeError`, any other object → handle.

## Completion record

Branch `worktree-agent-a4dd501cd92964802`, pushed as `ci/n4-appkit`, based on master bab98ff2. Not merged.
Gates and CI below were observed on **8c6bec5b**; the commit after it only adds this record and the "N4 as built"
note in `plans/research/NON-GUI-TYPES.md`.

### Commits

- 94a79334: ingest, classifier, Apple core (`types.rs`, `blocks.rs`, bridge `invoke_ex` and conversions), generator,
  modules, typings, docs, generated type tests, `types_core.rs`, fixture `appkit-types`, types probe, CI steps.
- 232aa359: CI round 1 fixes: class-cluster initializers resolve by name (`initWithString:` lives on the placeholder
  `+alloc` returns), `-[NSData bytes]` declared `^v`, CFBoolean → boolean, `test_ledger.py` isolation,
  `generated/facts.json` regenerated (`testing.rs` is part of the observe facts' fingerprint).
- 1df3937d: CI round 2 fixes: NSAttributedString stays a handle in the type layer (element props keep the string),
  the async test drains a completion that beats `cancel`, `no_construct` (NSPasteboard) and `runtime_missing`
  (5 NSTypesetter selectors) overrides, the probe loads AppKit first.
- 5e23e3ef: the probe output (`metadata/probe/appkit/types-26.5.json`, macOS 26.6.2) turns 374 declared read-only
  properties into getters.
- 8c6bec5b: ledger import of run 39572868685 and coverage.

### CI (`ci/n4-appkit`)

| Head | macos-appkit | js | linux-gtk | linux-qt | windows-winui |
|---|---|---|---|---|---|
| 94a79334 | failure 39570454889: `types` 53 passed before NSData `bytes` aborted the binary; `types_core` 3 failed; `counter` (types e2e) aborted; metadata job: `facts.json` fingerprints | failure 39570455102 (ledger; `test_ledger.py` saw the type facts) | success 39570455008 | success 39570455050 | success 39570455026 |
| 232aa359 | failure 39571473898: `types` 119/126 (NSAttributedString, item-provider cancel race, NSPasteboard construct, NSTypesetter members), `types_core` 6/8; backend 21/21, generated 570/570, counter 2/2 (incl. `appkit_types_e2e`) | failure 39571473860 (ledger: 126 type facts not in the ledger, expected) | success 39571473838 | success 39571473908 | success 39571473759 |
| 1df3937d | **success 39572277462**: types 126/126, types_core 8/8, backend 21/21, generated 570/570, counter 2/2; probe artifact | failure 39572277590 (ledger, expected) | success 39572277387 | success 39572277523 | success 39572277570 |
| 5e23e3ef | **success 39572868685**: types 126/126, types_core 8/8, backend 21/21, generated 570/570, counter 2/2; probe output byte-identical to the committed one | failure 39572868653 (ledger, expected) | cancelled 39572868589 (superseded by the next push) | failure 39572868457 (the toolchain install step, superseded) | success 39572868488 |
| **8c6bec5b** | **success 39573281044**: unit 4 + 18, backend 21/21, generated 570/570, **types 126/126**, **types_core 8/8**, counter 2/2 (`counter_e2e`, `appkit_types_e2e`) | **success 39573280936** | **success 39573281193** | **success 39573280987** | **success 39573281163** |

The fixture on the real host (`appkit_types_e2e`): `new NSURL(s).host` → "example.com", `new NSUndoManager({ levelsOfUndo: 5 })`
read back through KVC, `NSColor`/`NSFont` factories set as `text-color`/`font` (checked natively: red 0.8, 15 pt), and
`NSItemProvider.loadDataRepresentationForTypeIdentifier()` resolving from its background queue with the registered
string's bytes.

### Gates (local, at 8c6bec5b)

| Gate | Result |
|---|---|
| `tools/dev/cargo-slot cargo check --workspace --all-targets` | rc 0 (only pre-existing warnings outside the lane) |
| `cargo check -p svelters-apple -p svelters-appkit --all-targets --target aarch64-apple-darwin` | rc 0, no warnings |
| counter-mac `types.rs` / `types_core.rs` type-checked as test targets of svelters-appkit for `aarch64-apple-darwin` | rc 0 |
| `corepack pnpm -r build` | rc 0 |
| `corepack pnpm test` (incl. `test:meta`: pytest 148 passed / 1 skipped, every `--check`, `ledger.py check`, `coverage.py --check`) | rc 0 |
| `ledger.py check` | appkit 205 manual facts verified (79 + 126 type facts) + 491 inferred; gtk, qt, winui unchanged |
| `python3 tools/docs/build_ir.py` then `corepack pnpm docs:test` | unittest 11 (1 skipped) OK, site tests 17/17 |
| `@svelters/types` node tests | 22/22 (19 + 3 AppKit: tree shaking, signature closure, every module loads), strict `tsc` on GTK + Qt + AppKit typings |
| `types_objc2_ingest.py --check` (needs the cargo registry; not in CI) | current |

### Classification (`metadata/appkit/types/report.txt`)

- All classes of objc2-foundation + objc2-app-kit 0.3.2 (587): value 67, object 263, statics 0, primitive 18 (cross
  as JS values), widget 95, excluded 144; 10 structs, 5 of them shapes the core converts.
- Bound scope: **73 classes** from 24 roots (Foundation 29: object 21, value 8; AppKit 44: object 35, value 9).
- Members of bound types: **Foundation 828 / 903 bound (91%)**, **AppKit 958 / 1099 (87%)**; 1083 methods (440
  statics, 81 initializers, 50 throwing), 703 properties (374 from the probe), 23 async. Excluded by rule: 173
  deprecated, 28 core (NSObject/KVC/KVO/coding machinery), 13 unsafe. Hidden value mutators: 14. Blocked: 216, mostly
  pointer out-parameters (116), non-completion blocks (24), `NSCoder` (16), widget slots (9), CoreGraphics types.
- Naming: 0 collisions, 0 renames. Overrides: 24 roots, 10 curated, 4 `class:` entries (2 change the verdict),
  6 `string_ctor`, 1 `no_construct`, 5 `runtime_missing` selectors, one exclusion regex.
- Curated (ledger-tested: construct, props, methods, statics, lifetime, members, async where present): NSColor 124
  bound, NSImage 52, NSFont 58, NSAttributedString 42, NSURL 63, NSFileManager 52, NSPasteboard 35, NSUserDefaults 35,
  NSUndoManager 34, NSItemProvider 14. Ledger: 126 type facts: 73 `members` (one per bound type) and 53 curated groups
  (construct, props, methods, statics, lifetime × 10; async × 3: NSFileManager and NSPasteboard descriptors, NSItemProvider
  completing, rejecting, cancelling and dying with its owner on the runner).

### Build time and size

- Release rebuild of svelters-apple + svelters-appkit (`aarch64-apple-darwin`, no SDK, sccache off): 6.1 s → 7.5-8.5 s.
- rlibs: `libsvelters_appkit` 1.33 → 1.74 MB, `libsvelters_apple` 0.59 → 0.75 MB. The linked app's growth was not
  measured (linking needs the SDK); the rlib growth bounds it.
- The generated-objc2 alternative: objc2-foundation + objc2-app-kit with the bound scope's 78 header features build in
  30.5 s clean against 13.6 s today, before any thunk (see Design decisions).
- JS: `packages/types/appkit` 732 KB (78 files); a bundle keeps only what it imports (shake test).
- Hand-written core: `types.rs` 411 lines, `types_table.rs` 66, `blocks.rs` 115, bridge +181 net, backend +86.

### ABI

No change. The AppKit backend implements the v2.5 members and `cancel`; conventions in
`plans/architecture/backend-abi.md` ("AppKit non-GUI types (N4, no ABI change)").

### Shared files touched outside the lane (all additive)

- `tools/metadata/common/types_pkg.py`: `./appkit/` export group and `./manifest-appkit`.
- `packages/types/vite.js`: the `svelters:appkit/<fw>` alias in `typesAliases()`; `packages/types/package.json`
  (generated exports); `packages/types/test/types/tsconfig.json` (AppKit typings).
- `packages/testhost/src/index.js`: `callStatic(kind, "init…:")` answers a fresh handle, like `new`.
- `tools/metadata/ledger.py`: `appkit_type_facts()` joins the AppKit manifest.
- `.github/workflows/macos-appkit.yml`: `counter-mac types` and `types_core` steps, the types probe step.
- `docs/site/astro.config.mjs`: sidebar group "Non-UI types (AppKit)".
- `plans/architecture/backend-abi.md`, `plans/research/NON-GUI-TYPES.md`: new sections.
- `crates/svelters-appkit/src/testing.rs`: `invoke_callback` recording; this re-fingerprinted the 344 observe facts
  (`gen_backend.py` hashes the file), re-verified by the import of run 39572868685.

### Open items

1. **Pointer out-parameters** (116 blocked members, e.g. `-[NSColor getRed:green:blue:alpha:]`) need an out-slot path in
   `invoke_ex` (the Qt/GTK `{ret, outs}` shape).
2. **Non-completion blocks** (`enumerate…usingBlock:`, `NSColor colorWithName:dynamicProvider:`, drawing handlers) need
   call-scope callbacks with stop pointers and return values: not built.
3. **String constants** (`NSPasteboardTypeString`, `NSFontAttributeName`, ...) are not exported: their values are only
   known at run time; a probe could record them like the read-only properties.
4. **Notifications as `on()` signals** for type objects (NSUndoManager's did-undo, NSUserDefaults' did-change) are not
   wired; no type class has observe facts, so type getters are plain reads.
5. **Widget slots** of type members (9) stay blocked (the NT1 `k: widget` element-handle form is not ported).
6. **Async**: only `(e)`/`(oe)` shapes occur in the bound scope; the other seven shapes are built but untested on CI.
   Async statics have no bound member to test (`+[NSFileVersion getNonlocalVersions…]` needs iCloud).
7. A failed argument conversion inside `call_static(init…)` leaks the `+alloc`ed object (an uninitialized instance).
8. The probe file name follows the runner's default SDK (26.5); the runtime macOS (26.6.2) is what it reflects.
9. Linked binary size not measured (needs a Mac or the SDK).

### Conflict risks

- **N4-WinUI**: `tools/metadata/common/types_pkg.py` (`HEAD`/`GROUPS` lines), `packages/types/vite.js` (`typesAliases`),
  `packages/types/package.json` (generated), `packages/types/test/types/tsconfig.json`, `docs/site/astro.config.mjs`,
  `plans/architecture/backend-abi.md`: both lanes add a line next to the gtk/qt ones. `ledger.py` if WinUI adds type facts.
- **N3**: `packages/types/src/core.js` is untouched here.
- **Anyone regenerating AppKit facts**: `crates/svelters-appkit/src/generated/facts.json` and `metadata/verified/appkit.json`.
