# MAC1: the AppKit backend (lane notes)

Status: slice written and cross-built on Linux (2026-10-17); nothing has run on a Mac yet. Mac steps and the unverified checklist: `docs/notes/macos-setup.md`. Research behind it: `docs/notes/apple-gui-backend.md`.

## What exists

| Piece | Where |
| --- | --- |
| Shared Apple mechanics | `crates/svelters-apple`: `names` (naming rules), `enc` (type encodings, value conversion), `handles` (handle table), `testing` (main-thread test runner), and Apple-only `objc::{runloop, target, bridge}` |
| AppKit backend | `crates/svelters-appkit`: `tables` + `generated/tables.rs`, `lookup` (pure), `backend` (macOS: `AppKitBackend: svelters_abi::Backend`), `testing` (macOS harness) |
| Metadata | `metadata/appkit/*.json` (ingested), `metadata/overrides/appkit/*.yaml` (annotations), `metadata/style/appkit.yaml` (`dialect: none`, not yet consumed by `gen_props.py`) |
| Tools | `tools/metadata/appkit/{ingest.py, gen_backend.py, names.py, override.schema.json, test_names.py}`, `tools/dev/mac-cross` |
| Example | `examples/counter-mac` (runner binary; tests `generated`, `backend`, `counter`), `fixtures/counter-mac`, `fixtures/gallery-appkit` |

Kinds: `window` (toplevel), `view`, `stack-view`, `text-field`, `secure-text-field`, `button`, `slider`, `progress-indicator`, `box`. Base classes NSControl/NSView carry inherited props, the `action` signal and the `subviews` slot.

## Naming (user rules)

- Tag = class without `NS`, kebab-case. Props = ObjC property names in kebab-case (`string-value`). Methods = selector keywords joined, kebab-case (`make-key-and-order-front`). Signals: `action` (target-action, named after the `action` property), notifications by name without `NS`/`Notification` (`control-text-did-change`), delegate methods by selector (`window-should-close`). The generator rejects any other spelling.
- Rust (`names.rs`) and Python (`names.py`) implement the same rules and share a test table.
- No portability layer: AppKit fixtures say `onaction`, not `onclick`. One shared-code change: the renderer's `TOPLEVELS.window` lists `make-key-and-order-front` as a show method (the existing "first resolvable name wins" mechanism; GTK/Qt still resolve `present`).

## Decisions and deviations from the research note

1. **No ObjC helper file.** `NSInvocation` is driven from Rust with typed `msg_send!` (its own API has fixed signatures), so neither Apple crate needs a C compiler and both `cargo check --target aarch64-apple-darwin` without an SDK.
2. **Static `define_class!` classes instead of runtime-built delegate classes.** `SveltersWindowDelegate` always implements `windowShouldClose:`/`windowWillClose:` (needed for the close cascade anyway), so the `respondsToSelector:` caching problem does not arise for the slice. Runtime-generated delegates come back with the first delegate whose optional methods change behaviour by existing (table/outline views).
3. **Notifications instead of delegates** for text fields (`NSControlTextDidChangeNotification` on the default center): the delegate slot stays free.
4. **Dynamic dev mode**: props and methods not in the tables resolve at run time (`class_copyPropertyList`, then accessor methods for protocol properties like `identifier`, then `instance_methods` matched by JS name), ids from `1 << 20`. Signals are table-only.
5. **Owning method families** (`alloc/new/copy/init...`) are refused by dynamic `call`; objects are made with `create`.
6. **One-child slots replace** (window/box `content-view`): the old child is unlinked, not destroyed (as GTK's `set_child`); removing installs a fresh `NSView` (content views must not be nil).
7. **Release is deferred**: natives of destroyed/closed handles go to a graveyard drained after each checkpoint at the run loop's top level ("never delete inside signal emission").
8. **Borrowed handles** hold `objc2::rc::Weak`; a dead one is reported `owner` lazily, on the next op (AppKit has no general dealloc notification). Identity of owned objects is a pointer map (safe: the table holds them strongly); borrowed ones are matched by `Weak::load`.
9. **Close semantics** follow ABI v2.1: listened `window-should-close` runs JS through `call_sync`; `true` (preventDefault) vetoes, null allows, busy vetoes. `windowWillClose:` reports the window `closed` and everything inserted below it `parent`, then wakes the loop so the host can quit.
10. **Transient toplevels (X7)** are refused for now (`insert` of a window).
11. **Descriptors** for scalar props (bool/int/float/string), so the renderer coerces static attributes, and for struct props as well-known RECORDs (`SVL_T_RECT/SIZE/POINT/INSETS`, with `describe_type`): the QuickJS host refuses a plain JS object for a slot without a descriptor (found by the host's op-log fixture test). Enums and object handles convert dynamically. String props stringify numbers (`string-value={count}`), null is "". The scripted `counter-mac` fixture avoids object values because the log backend has no descriptors; GTK's op-log replay skips `*-mac` fixtures (Qt skips them by itself: unknown kinds).
12. **Frames**: `-[NSView displayLinkWithTarget:selector:]` on the content view of a live window (macOS 14+), paused between requests, invalidated with its window; otherwise a 16 ms CF timer.
13. **Tests run on thread 0** with `harness = false` and `svelters_apple::testing::run_cases` (the libtest CLI subset nextest needs). On Linux the binaries list zero cases.

## Metadata and licence

- `ingest.py` parsed the macOS 26.1 SDK (GitHub mirror, kept in the session scratchpad, never committed) with Linux clang 22 and libclang's Python bindings. The user directed this run; the research note recommends re-running ingestion on a Mac for the licence-clean path, which `macos-setup.md` §3 describes.
- **Committed SDK-derived data**: `metadata/appkit/*.json`, 10 classes + `_enums.json` (40 enums), 174 KiB: property/method names, neutral types, the native type spelling, access, memory attribute, macOS availability versions, enum member values. No header text and no doc comments (links to developer.apple.com instead). Deprecated/unavailable members are dropped. Methods are only the ones overrides name.
- Ingestion is deterministic (`ingest.py --check` passes on rerun); `gen_backend.py --check` likewise.

## Verified on Linux

- `cargo check -p svelters-apple -p svelters-appkit --target aarch64-apple-darwin` with no SDK.
- With the SDK via `tools/dev/mac-cross`: `cargo check -p counter-mac --all-targets` and `cargo build -p counter-mac --bins --tests` link real `Mach-O 64-bit arm64` executables (lld against the SDK's `.tbd` stubs).
- Linux unit tests: names (incl. Rust/Python agreement), encodings and value conversion (insets reorder, enum/flag names, struct bytes), handle table, generated-table invariants and lookup.
- The counter-mac fixture's op log through the Node TestHost (`fixtures/counter-mac/expected.oplog`); gallery builds; renderer tests and existing fixtures pass with the TOPLEVELS change.

## Open questions

- Should the renderer's `LABEL_KINDS` fold text children into a per-toolkit prop (AppKit `button` -> `title`, `text-field` -> `string-value`)? Today AppKit fixtures set them as attributes. With the compiler frontend (ST1) this probably belongs in its semantics tables.
- Checkbox binding (`bind:state`) and `bind:double-value`: annotated in overrides (`bindings:`) but not fed into `gen_bindings.py` yet.
- iOS/UIKit: the shared crate is ready for it (run loop, bridge, handles), nothing UIKit-specific was written.
