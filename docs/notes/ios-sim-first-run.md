# iOS: first run on a Mac and the simulator (2026-10-21)

First execution of anything iOS in this project. Before this, everything iOS (`ios-uikit-backend.md`,
`scripts/mobile/ios_probe/`) had been written and type-checked on Linux only. This note records what
builds, what runs on the simulator, which of the note's **[unverified]** claims hold, and how the
results bear on decisions I1–I9.

## Environment

| | |
| --- | --- |
| Host | Intel Mac (x86_64), macOS 15.7.5 (Darwin 24.6.0) |
| Xcode | 26.3 (17C529); SDKs `iPhoneSimulator26.2.sdk`, `iPhoneOS26.2.sdk` |
| Simulator runtime | iOS 26.3 (23D8133); device used: **iPhone 17** (402×874 pt @3x) |
| Rust | 1.97.0 (rust-toolchain.toml); added targets `x86_64-apple-ios`, `aarch64-apple-ios`, `aarch64-apple-ios-sim` via rustup (nothing else installed) |
| Crates | objc2 0.6.5, objc2-ui-kit / -foundation / -quartz-core / -core-foundation 0.3.2, rquickjs 0.14.0 |

On an Intel Mac the simulator runs **`x86_64-apple-ios`** binaries; `aarch64-apple-ios-sim` is for
Apple silicon only (built here, not run).

## Builds

| What | x86_64-apple-ios (sim) | aarch64-apple-ios (device) | aarch64-apple-ios-sim |
| --- | --- | --- | --- |
| `ios_probe` as it was written on Linux, unchanged | **builds and links** | builds and links | builds and links |
| `svelters-apple` (with and without `exceptions`, which compiles C) | builds | builds | builds |
| `svelters-host` (QuickJS via rquickjs 0.14) | **fails**: `rquickjs-sys` has no `bindings/x86_64-apple-ios.rs` | fails (same) | not tried (same cause) |
| `svelters-host --features rquickjs/bindgen`, `LIBCLANG_PATH` = Xcode's toolchain `usr/lib` | **builds** (1 m 26 s) | **builds** (1 m 04 s) | not tried |

- QuickJS's C sources compile for iOS with Apple clang through `cc` with no extra settings.
- **The bindgen output for `x86_64-apple-ios` is byte-identical to the shipped
  `x86_64-apple-darwin.rs`**, which is itself byte-identical to `aarch64-apple-darwin.rs`. So copying
  the darwin bindings for the iOS targets is correct for rquickjs-sys 0.14.0 (verified for x86_64-ios;
  aarch64-ios built with bindgen but its file was not diffed).
- Mach-O details: without `IPHONEOS_DEPLOYMENT_TARGET`, rustc's x86_64/aarch64 iOS binaries carry
  `LC_VERSION_MIN_IPHONEOS 10.0` (old format); `aarch64-apple-ios-sim` gets `LC_BUILD_VERSION`
  platform IOSSIMULATOR minos 14.0. The `aarch64-apple-ios-sim` binary is **ad-hoc linker-signed**
  (`flags=adhoc,linker-signed`), the x86_64-ios and device binaries are **unsigned**. The run script
  sets `IPHONEOS_DEPLOYMENT_TARGET=17.0` and signs the .app ad hoc (`codesign -s -`); launching an
  unsigned bundle was not tried.
- Packaging needs no Xcode project: a directory `ios_probe.app/{ios_probe, Info.plist}` with the scene
  manifest installs and launches with `simctl install` / `simctl launch --console`.

## Claims checked on the simulator

The probe now has a `--selftest` mode (`docs/notes/scripts/mobile/ios_probe/src/main.rs`) that runs
in-process after `sceneDidBecomeActive:`, drives each check from a `svelters-apple` `RunLoop` timer,
prints one `SV {json}` line per claim and exits. Target-action, notifications and display-link ticks
go through **`svelters_apple::objc::target::SveltersTarget`**, and timers/wake through
**`svelters_apple::objc::runloop::RunLoop`**, so the shared crate is exercised on iOS as it stands.
Three runs: run 1 (fresh boot), run 2 (+`SV_PREMAIN=1`), run 3 (`--no-launch-screen`). Results were
the same across runs except for the timing noted.

| Claim (ios-uikit-backend.md §) | Result | Detail |
| --- | --- | --- |
| Scene manifest → `SveltersSceneDelegate` gets `scene:willConnectToSession:options:` (§2.2) | **verified** | order: `didFinishLaunching` → `willConnect` → `sceneDidBecomeActive` |
| `UIApplication::main(None, Some("SveltersAppDelegate"))` with a Rust `define_class!` app delegate | **verified** | |
| **I1 option A**: a sceneless `UIWindow` + VC + whole UI built in `didFinishLaunching`, adopted in `willConnect` via `setWindowScene:` + `makeKeyAndVisible` | **verified** | `windowScene` nil before, set after; window key, visible, renders (screenshot) |
| `UIApplicationMain` never returns; callbacks run inside it (§2.2) | **verified** | `+[NSThread callStackSymbols]` in `willConnect`: `UIApplicationMain` ← `objc2_ui_kit::UIApplication::main` ← `ios_probe::main`; `main` is `-> !` |
| Creating `UIView`/`UILabel` **before** `UIApplicationMain` (§2.2 "undefined territory") | **works (info)** | no crash, `setText:` works. Still don't rely on it: it's not documented |
| Window shown, content pinned to `safeAreaLayoutGuide` (§2.4, §2.5) | **verified** | screenshot; safe area top 62 / bottom 34 on iPhone 17 |
| Without `UILaunchScreen` the app runs letterboxed (§2.2) | **verified** | screen/window 320×480 pt instead of 402×874 (run 3 screenshot is letterboxed). Always emit `UILaunchScreen` |
| `UISwitch setOn:` / `setOn:animated:` send no `ValueChanged` (§3.2) | **verified** | 0 actions; **KVO `on` does fire** (1) |
| `UISlider setValue:` / `setValue:animated:` send no `ValueChanged` | **verified** | 0 actions; KVO `value` fires |
| `UISlider.value = 500` (max 1) clamps silently (§3.1 rule 3) | **verified** | reads back 1, no action; KVO `value` fires once |
| `UIProgressView.progress = 1.5` clamps to 1 | **verified** | |
| `UITextField setText:` sends neither `EditingChanged` nor `UITextFieldTextDidChangeNotification` | **verified** | KVO `text` fires for `setText:` |
| Simulated typing (`becomeFirstResponder` + `insertText:`) fires `EditingChanged` + `TextDidChange` | **verified** | both fire **synchronously**, once each; **KVO `text` does not fire** for typed input (confirms "KVO only for programmatic sets"). Good selftest vehicle for `bind:text` |
| KVO fires for programmatic `UILabel.text`, `UIProgressView.progress`, `UIView.hidden`/`alpha`, `UIStackView.spacing` (§3.2 "display-only") | **verified** (undocumented behaviour) | one notification per set; `setProgress:animated:` also fires |
| `sendActionsForControlEvents:` reaches `SveltersTarget -fire:` (button `PrimaryActionTriggered`, switch/slider `ValueChanged`) | **verified** | one-argument `fire:` is accepted by UIKit |
| **New:** `UIButton` `sendActions(TouchUpInside)` also fires the `PrimaryActionTriggered` target | **observed** | UIKit maps touch-up-inside to the primary action on buttons. A backend that registers both events for one JS handler would double-fire: register one per JS signal |
| `removeArrangedSubview:` leaves the view in `subviews` (§2.5) | **verified** | `superview` non-nil until `removeFromSuperview` |
| KVC through `svelters_apple::objc::bridge::kvc_set/kvc_get`: `NSNumber` → `UISlider.maximumValue`; `NSValue` with `{NSDirectionalEdgeInsets=dddd}` → `UIStackView.directionalLayoutMargins` (§1 table) | **verified** | plain `NSValue` with the right `objCType` works through KVC; `kvc_get` returns an `NSValue` (`NSDirectionalEdgeInsets: {1, 2, 3, 4}`) |
| CFRunLoop timer (`RunLoop::schedule_wake`) fires under UIKit's loop (§2.3) | **verified** | armed 300 ms, fired at 307–321 ms |
| `Waker::wake` from a background thread runs the version-0 source on the main thread | **verified** | |
| Wake source, CFRunLoop timer and `CADisplayLink` all fire in **`UITrackingRunLoopMode`** (common modes) | **verified** | nested `CFRunLoopRunInMode(UITrackingRunLoopMode, 0.3 s)`: wake +1, timer +1, ticks +18 |
| `CADisplayLink` (`+displayLinkWithTarget:selector:`, common modes) ticks into `SveltersTarget -tick:`; pausing stops it | **verified** | settled rate **59–60 Hz** (`UIScreen.maximumFramesPerSecond` = 60 on the simulator); paused link: 0 ticks over 300 ms |
| Simulator process reads the Mac's file system (§4 dev `--watch` loop) | **verified** | read `/tmp/svios/hostfile.txt` from inside the app. `cwd` is `/`, `HOME` is the app's data container under `~/Library/Developer/CoreSimulator/Devices/<udid>/data/Containers/Data/Application/<uuid>` |
| Arm64 simulator needs a code signature (§6.2) | not tested | Intel Mac; the arm64-sim binary is linker-signed anyway |
| `EditingChanged` during IME marked text; per-class `ValueChanged` for stepper/segmented/date picker; scroll view delegate on `setContentOffset:` | not tested | not in the probe's UI |
| Watchdog (`0x8badf00d`) launch budget | not tested | see the stall below |
| Scene disconnect/reconnect, background/foreground | not tested | |

### Other observations

- **Main-thread stall after the first `becomeFirstResponder`**: in run 2 the 1 s display-link window
  after focusing the text field took **4.3 s with one tick**; in runs 1/3 it was ~1 s with 8–27 ticks
  (rates of 8–26 Hz right after focus). The keyboard/text-input system initialises on first focus and
  blocks the main thread for up to seconds on this Intel simulator. A hardware keyboard is attached by
  default, so no on-screen keyboard appeared. Selftests must not assume frame timing right after a
  first focus; launch code should not focus a field synchronously.
- A one-off console line `NSMapGet: map table argument is NULL` appears in the same window (from
  UIKit, not from our code as far as I can tell). Harmless so far.
- **`std::process::exit` from inside `UIApplicationMain` aborts**: exit runs Rust TLS destructors on
  the main thread; dropping `thread_local!` values that hold UIKit objects/`Rc`s touched other TLS and
  panicked ("cannot access a Thread Local Storage value during or after destruction", then "thread
  local panicked on drop, aborting"). The probe flushes stdout and calls `_exit(0)`. Any selftest/test
  runner on iOS (`svelters_apple::testing` driven from `didFinishLaunching`) should do the same, or
  keep its state out of `thread_local!`s with destructors.
- `simctl launch --console` relays the app's stdout/stderr (Rust's line-buffered `println!` included)
  until the process exits. That's enough for a JSON-lines selftest report without XCUITest.
- Simulator boot (`simctl boot` + `bootstatus -b`) of a shut-down iPhone 17 took well under a minute
  here; `install`+`launch` take a few seconds.

## Fixes and changes made

- **The original probe needed no fix** to build and link for all three iOS targets.
- `ios_probe` extended (not fixed): app delegate (`SveltersAppDelegate`), I1 option-A mounting, safe-area
  pinning, `--selftest` checks above, `SV_PREMAIN` and `SV_HOSTFILE` switches, `_exit`. New
  dependencies: `svelters-apple` (path) and `objc2-core-foundation` (`CFCGTypes`, for `CGRect`).
  Added `ios_probe/.gitignore`: the repo's `.cargo/config.toml` sets
  `build-dir = "{workspace-root}/target/build"`, and the probe is its own workspace, so its
  intermediates land in `ios_probe/target/`.
- New `tools/dev/ios-sim-run`: build for the host's simulator target → `Name.app` with the scene
  manifest + `UILaunchScreen` (`--no-launch-screen` to omit) → ad-hoc sign → pick/boot a device by name
  or UDID → `simctl install` → `simctl launch --console` (app env via `--env K=V` → `SIMCTL_CHILD_K`) →
  `--wait REGEX` → `--screenshot` → terminate (and `--shutdown` if it booted the device). Works for
  any Rust binary that registers `SveltersSceneDelegate` (`--manifest-path`, `--bin`, or `--exe` for a
  prebuilt one; `--resources DIR` to add a JS bundle). Example:

  ```sh
  tools/dev/ios-sim-run --env SV_HOSTFILE=/tmp/x --wait SV-DONE --screenshot /tmp/probe.png -- --selftest
  ```
- No changes to `svelters-apple`, `svelters-host`, AppKit code or metadata.

## Bearing on decisions I1–I9

| # | Evidence from this run |
| --- | --- |
| I1 run-first + `started` hook | **Option A works as designed**: `didFinishLaunching` is the hook point, a full sceneless UI built there is adopted by the first scene with `setWindowScene:`. The `svelters-apple` `RunLoop` can be created before `UIApplicationMain` (it was, in `main`) and its timers/wake source then run under UIKit's loop, so the host's wake/timer plumbing needs no iOS change, only the start order |
| I2 window semantics | one implicit window adopting the first scene works; `quit` path: in-app exit must be `_exit` (selftest only) |
| I3 safe area + keyboard guide | safe-area pinning verified (t62/b34 on iPhone 17); keyboard guide not tested (hardware keyboard in the simulator hides the on-screen one). Must emit `UILaunchScreen`, otherwise 320×480 letterbox |
| I4 Dynamic Type | not tested |
| I5 observability facts schema | facts measured: control events `programmatic: never` (switch, slider, text field incl. its notification); KVO `programmatic: always` for label/progress/view/stack/switch/slider/text-field props, `user: false` for text-field typing; clamping silent on slider and progress view; `TouchUpInside` → primary action aliasing on `UIButton`. The schema fits these; KVO facts should stay `documented: false` with a simulator test, as proposed |
| I6 event naming | `primary-action-triggered` on buttons is the right single event (touch-up-inside aliases to it) |
| I7 QuickJS bindings | bindgen against the iOS SDK works on a Mac with Xcode's libclang; the generated file equals the shipped darwin bindings. Cheapest path: vendor/upstream copies of the darwin file as `*-apple-ios*.rs` (or build with `rquickjs/bindgen` on macOS CI; costs ~1 min per target) |
| I8 selftest first | **confirmed practical**: an in-process selftest with `sendActionsForControlEvents:` / `insertText:` + JSON lines over `simctl launch --console` works with no Xcode project. Use `_exit`; avoid frame-timing assertions right after first focus |
| I9 minimum iOS 17 | nothing here contradicts it; the run script sets `IPHONEOS_DEPLOYMENT_TARGET=17.0` (rustc's default is 10.0 on x86_64/aarch64-ios) |

## Open problems / next steps

- Run the selftest on an Apple-silicon Mac / GitHub macOS runner (`aarch64-apple-ios-sim`, signature
  question) and wire `tools/dev/ios-sim-run` into an `ios-uikit.yml` job.
- Upstream iOS bindings to rquickjs (or a `[patch]`); until then `rquickjs/bindgen` on macOS with
  `LIBCLANG_PATH=$(xcode-select -p)/Toolchains/XcodeDefault.xctoolchain/usr/lib`.
- Try a full host (QuickJS running a bundle) inside the app: `svelters-host` builds for iOS but was
  not linked into an app or run.
- Not covered: IME marked text, keyboard layout guide with the software keyboard
  (`simctl` can't toggle it; `defaults write com.apple.iphonesimulator ConnectHardwareKeyboard -bool
  false` before boot is the usual trick), scene disconnect, Dynamic Type, more UIControl classes.
