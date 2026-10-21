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

(filled in at the end)
