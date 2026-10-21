# Running the AppKit backend on a Mac (MAC1)

Status: written on Linux (2026-10-17). None of this has run on a Mac yet; the point of these steps is to find out what breaks. Background: `docs/notes/mac1-appkit.md`.

**Minimum macOS: 11** (plans/research/REACTIVITY.md decision 13.10; `observe` on borrowed objects relies on the
macOS 10.13 relaxed KVO unregistration). The macos-appkit CI job and `tools/dev/mac-cross` build with
`MACOSX_DEPLOYMENT_TARGET=11.0`; set the same when building a release by hand. Newer APIs are checked at run time
(`CADisplayLink` needs 14 and falls back to a timer).

## 1. One-time setup

1. **Command Line Tools** (clang, the macOS SDK, `screencapture`). Full Xcode also works.

   ```sh
   xcode-select --install          # skip if `xcode-select -p` already prints a path
   xcrun --show-sdk-path           # must print .../MacOSX*.sdk
   ```

2. **Rust.** The repo pins its toolchain (`rust-toolchain.toml`, 1.97.0); rustup installs it on first use.

   ```sh
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
   source "$HOME/.cargo/env"
   cargo install cargo-nextest --locked     # optional; `cargo test` works too
   ```

3. **Node 22+ and pnpm 10.**

   ```sh
   brew install node@22        # or any Node >= 22
   corepack enable             # provides the pinned pnpm@10.33.0 from package.json
   pnpm --version
   ```

4. **The repository, on the MAC1 branch.** The Svelte fork is vendored as a tarball (`vendor/svelte/*.tgz`, see `vendor/svelte/PIN`), so there is no separate fork checkout: `pnpm install` unpacks it.

   ```sh
   git clone <your svelters remote> svelters && cd svelters
   git checkout <the MAC1 branch, or master after the merge>
   pnpm install
   pnpm -r --filter '!@svelters/docs' build      # builds every fixture's dist/app.js
   ```

   Only two bundles are needed here; building just them is quicker:

   ```sh
   pnpm --filter @svelters-fixtures/counter-mac --filter @svelters-fixtures/gallery-appkit build
   ```

## 2. Build and run

```sh
cargo run -p counter-mac                                  # the counter: click Increment
cargo run -p counter-mac -- --fixture gallery-appkit      # every kind of the slice
cargo run -p counter-mac -- --screenshot /tmp/counter.png --exit --report /tmp/counter.json
cargo run -p counter-mac -- --watch                       # dev loop; in a 2nd terminal:
pnpm watch fixtures/counter-mac                           #   then edit fixtures/counter-mac/src/App.svelte
```

What to look at:

- **Counter**: a window titled "Counter" with "Count: 0" above an "Increment" button, 20 pt margins. Each click adds 1. Closing the window ends the process (the host quits after the last window) and prints a JSON report with `"ok": true`. Cmd+Q also quits.
- **Gallery**: typing in the field updates "You typed: ..."; the button counts clicks; the checkbox title follows its state; dragging the slider moves the progress bar and the "Slider: N" text in the box.
- **`--watch`**: saving `App.svelte` rebuilds and the window is replaced in place (`[counter-mac] reload ok=true`).
- **`--screenshot`**: the first time, macOS asks for Screen Recording permission for the terminal; grant it and rerun. The PNG is the window only.

## 3. Tests

All macOS tests run their cases in order on the main thread (no libtest harness; AppKit needs thread 0). They open real windows briefly.

```sh
cargo test -p svelters-apple -p svelters-appkit           # pure unit tests (also run on Linux)
cargo test -p counter-mac --test backend                  # hand-written: lifetime, bridge, run loop, close cascade
cargo test -p counter-mac --test generated                # one test per annotated fact (76 cases)
cargo test -p counter-mac --test counter                  # Svelte bundle in QuickJS on AppKit, 3 clicks
cargo nextest run -p counter-mac                          # same, through nextest (one process per case)
```

Run one case: `cargo test -p counter-mac --test generated -- slot_nsstackview_arranged_subviews`.

Optional, re-ingest the metadata from your Xcode (CI's `appkit-metadata` job does this on every push with Xcode 26.1.1 / macOS 26.1 SDK; with a different SDK version a diff is expected and interesting):

```sh
python3 -m venv /tmp/mac1-venv && /tmp/mac1-venv/bin/pip install clang==21.1.7 pyyaml jsonschema
/tmp/mac1-venv/bin/python -I tools/metadata/appkit/ingest.py   # SDK and libclang via xcrun
/tmp/mac1-venv/bin/python -I tools/metadata/appkit/gen_backend.py
git diff --stat metadata/appkit crates/svelters-appkit/src/generated
```

## 4. What to paste back

1. `sw_vers`, `uname -m`, `xcrun --show-sdk-version`, `rustc --version`.
2. Any build error, in full (the first error is enough when there are many).
3. The **whole output** of the three `cargo test -p counter-mac --test ...` commands. A failing case prints its panic message; `RUST_BACKTRACE=1` helps for crashes. If a test binary dies without a summary (abort, `SIGSEGV`, "uncaught exception"), paste the last 40 lines and say which case it was on (`test <name> ...` is printed before each case runs).
4. For `cargo run -p counter-mac`: does the window look right (a screenshot or `/tmp/counter.png` is ideal), do clicks count, does closing the window end the process, and the JSON report it prints.
5. The gallery: which controls do nothing or look wrong.
6. If you re-ingested: `git diff --stat` of the metadata.

## 5. Not verified on Linux (checklist)

Everything below compiled and linked for `aarch64-apple-darwin` on Linux, but has never executed:

- [ ] `AppKitBackend::new` on the main thread; the regular activation policy and the minimal app menu (Cmd+Q -> `stop:`).
- [ ] `define_class!` classes register (`SveltersTarget`, `SveltersWindowDelegate`) and objc2's debug-mode encoding checks accept every `msg_send!` (a mismatch panics with "invalid message send").
- [ ] KVC set/get for every prop type: BOOL, NSInteger enums by name, NS_OPTIONS (`style-mask`), CGFloat, NSString, `frame` (NSRect via NSValue), `edge-insets` (NSEdgeInsets field order).
- [ ] `NSInvocation` calls from Rust (`make-key-and-order-front` with a nil sender, `size-to-fit`, the setter-only `button-type` and `content-size` props).
- [ ] Target-action: `performClick:` reaches `-[SveltersTarget fire:]`; `listen off` clears target/action.
- [ ] `NSControlTextDidChangeNotification` observation for typing (only simulated with `postNotificationName:` in the generated tests).
- [ ] `windowShouldClose:` through `call_sync` (veto with `e.preventDefault()`), `windowWillClose:` cascade (`closed`, then `parent`), and the host quitting after the last window.
- [ ] `NSStackView removeArrangedSubview:` also removing from `subviews` (the generated slot test checks `superview == nil`).
- [ ] CFRunLoop wake source, `schedule_wake` timer (repeating timer re-armed by fire date), cross-thread `wake`, all in common modes (try dragging the slider while a JS timer runs).
- [ ] `CADisplayLink` from `-[NSView displayLinkWithTarget:selector:]` (macOS 14+); the 16 ms timer fallback on older macOS.
- [ ] `[NSApp run]` returning after `stop:` + the posted application-defined event.
- [ ] Dynamic props/methods not in the tables (`identifier`, `size-to-fit`) found through the runtime.
- [ ] Borrowed handles (`content-view` getter) and their weak references.
- [ ] objc2's `exception` feature (enabled by counter-mac) turning ObjC exceptions into errors instead of aborts.
- [ ] The renderer picking `make-key-and-order-front` for `<window>` through the QuickJS host's `methodId`.
