# iOS backend: UIKit over the shared Apple crate (desk research)

Status: research note, 2026-10-18, written on Linux. It builds on `apple-gui-backend.md` (which
already covers the UIKit API shape, SwiftUI, App Store rules, JavaScriptCore and metadata) and on
the MAC1/FA1 AppKit slice (`mac1-appkit.md`, `fa1-appkit-ci.md`). It does not repeat them; it
records the **delta** for a UIKit backend, the per-property change-notification facts the
reactivity design (`plans/research/REACTIVITY.md`, in progress) needs, and a first slice.

Marks: **[verified]** = checked here (probe compiled, file inspected). **[docs]** = from Apple's
documentation or headers. **[unverified]** = from memory or secondary sources; confirm on a
simulator. Nothing in this note has run on iOS. **Update 2026-10-21:** the probe ran on the iOS 26.3
simulator; per-claim results are in `ios-sim-first-run.md`.

Probe: `docs/notes/scripts/mobile/ios_probe/` (a scene delegate, a KVO observer, target-action on
every slice control, `UIStackView`, `CADisplayLink` in common modes, `UIApplication::main`).
It **type-checks for `aarch64-apple-ios` and `aarch64-apple-ios-sim` on Linux with no SDK**
**[verified]**, objc2 0.6.5 / objc2-ui-kit 0.3.2, Rust 1.97.0.

## 0. Summary and recommendation

- **Backend: UIKit, Rust + objc2, as a third crate `svelters-uikit` over `svelters-apple`.**
  The AppKit slice already put the run loop (CFRunLoop wake source, re-armed timer, frame
  fallback), handle table, KVC/`NSInvocation` bridge, `SveltersTarget` and the naming rules in
  `svelters-apple`; all of it is platform-neutral Foundation/CF/objc code and applies to iOS
  unchanged. The UIKit crate adds the app/scene lifecycle, the UIKit kind tables and the
  UIControl event plumbing. Expected reuse: ~60% of backend code, ~90% of the metadata pipeline.
- **The big deltas from AppKit** are not widgets, they are ownership of the process:
  1. `UIApplicationMain` owns `main` and never returns, and **windows only exist inside scenes**
     the system connects later. The host's "load the bundle, then `run()`" order must invert
     (§2.2): this is the one shared-code change iOS forces, and Android forces the same one.
  2. UIKit has **multi-target, typed control events** (`UIControlEvents` is a 21-member enum in
     the header), so the event catalogue is machine-readable, which AppKit's single `action` is not.
  3. **Layout is safe-area/keyboard aware**: the window root must pin content to the
     `safeAreaLayoutGuide` and `keyboardLayoutGuide` (§2.5).
- **Change notification (§3):** UIKit "generally does not support KVO" **[docs]**. User edits
  arrive through `UIControl` events (`ValueChanged`, `EditingChanged`), notifications
  (`UITextFieldTextDidChangeNotification`) or delegates; **programmatic sets send no control
  events** **[docs: `UISwitch.setOn(_:animated:)`]**, so the target-action path has no echo, while
  KVO (which does fire on programmatic `setText:`) needs the existing echo guard. Per-property
  facts must say *which mechanism*, *whether programmatic sets fire it*, and *whether it is
  documented or merely works*.
- **JS engine: QuickJS-NG, interpreter-only (as everywhere on iOS).** rquickjs ships no
  pregenerated bindings for any iOS target **[verified: rquickjs-sys 0.14.0 `bindings/`]**, so
  the build needs its `bindgen` feature against the iOS SDK, or a vendored bindings file (§4).
  JSC stays the optional engine from `apple-gui-backend.md` §8.
- **Build/test:** Linux CI can `cargo check` the backend for iOS with no SDK. Linking, the
  simulator and metadata ingestion happen on the GitHub macOS runner (`xcrun simctl`), with an
  in-process `--selftest` app as the first test vehicle and XCUITest later (§6).
- **Effort:** ~2–2.5 agent-lane weeks for the slice below on top of MAC1 (the earlier
  "+1.5–2 weeks" estimate plus the host-order change and the simulator harness).

## 1. What carries over from the AppKit slice, and what changes

| Piece (crate / file) | iOS status |
| --- | --- |
| `svelters-apple::objc::runloop` (CFRunLoop source + timers in common modes) | **as is**. CoreFoundation is identical on iOS. `UITrackingRunLoopMode` (scroll/slider tracking) is in common modes, so checkpoints keep running while the user scrolls |
| `svelters-apple::objc::target` (`SveltersTarget` with `fire:`, `notified:`, `tick:`) | **as is** for UIControl target-action (`addTarget:action:forControlEvents:` takes the same `fire:` selector) and notifications. A UIControl action may be sent with `(sender, UIEvent)`: declare `fire:forEvent:` too, or keep `fire:` (UIKit accepts 0, 1 or 2 args) **[docs]** |
| `svelters-apple::objc::bridge` (KVC set/get, `NSInvocation`, class/property lookup) | **as is**. KVC boxing covers `CGRect`/`UIEdgeInsets`/`NSDirectionalEdgeInsets` via `NSValue` (UIKit adds `+valueWithUIEdgeInsets:`; plain `NSValue` with the right `objCType` works through KVC) **[unverified]** |
| `svelters-apple::{handles, enc, names}` | **as is**. Naming: tag = class without `UI`, kebab-case (`stack-view`, `text-field`) under the N1 rules |
| `svelters-appkit::backend` (window delegate, close cascade, frames via `-[NSView displayLinkWithTarget:]`) | **rewritten** for UIKit: no window close (scenes), `+[CADisplayLink displayLinkWithTarget:selector:]` (iOS 3.1+), scene delegate instead of app run/stop |
| `tools/metadata/appkit/{ingest.py, gen_backend.py, names.py}` | **parameterised**: `--sdk iphoneos`, prefix `UI`, `-target arm64-apple-ios17.0`; same JSON shape (§5) |
| `.github/workflows/macos-appkit.yml` | **cloned** as `ios-uikit.yml` with a simulator job (§6) |

New for iOS only: scene lifecycle, view-controller-backed window root, safe area/keyboard
guides, UIControl event catalogue, `UIAction` closures (iOS 14), trait collection changes
(dark mode, Dynamic Type), app bundle packaging.

### 1.1 objc2-ui-kit maturity

- `objc2-ui-kit` 0.3.2 (crates.io, 2025-10-04, ~51 M downloads) has 441 generated modules;
  every `UIResponder` subclass is `MainThreadOnly` and constructors take a `MainThreadMarker`
  **[verified]**. The probe used `UIStackView`, `UIButton` (`buttonWithType`,
  `setTitle_forState`), `UISwitch` (`isOn`/`setOn`), `UISlider`, `UITextField`, `UILabel`,
  `UIProgressView`, `UIWindow::initWithWindowScene`, `UIViewController`, `UIControl`
  target-action, `setAccessibilityIdentifier`, NSObject KVO registration and
  `CADisplayLink::addToRunLoop_forMode` without any hand-written `extern` **[verified]**.
- Gotchas found by the probe: `UIStackView::setSpacing` (any `CGFloat` API) needs the
  `objc2-core-foundation` cargo feature; `define_class!` needs `unsafe impl NSObjectProtocol`
  for KVO observer classes; `init` of a class with ivars is `msg_send![super(this), init]`.
- The published crate is a year old. objc2 HEAD tracks newer Xcode SDKs (the Apple note saw
  Xcode 27.1 bindings at HEAD). Any iOS 27-only API means a git dependency or our own `extern`.
  Pin like MAC1.
- `UIApplication::main(None, None, mtm)` wraps `UIApplicationMain` and is `-> !` **[verified:
  compiles]**.

## 2. Platform deltas

### 2.1 Main thread

- Same rule as AppKit (`NS_SWIFT_UI_ACTOR` on `UIResponder`/`UIView`), already encoded by objc2
  as `MainThreadOnly`. The host runs QuickJS on the main thread, which is required anyway.
- Main-thread watchdog: iOS kills an app that blocks the main thread too long at launch or on
  resume (the "watchdog" termination, `0x8badf00d`) **[docs, figures unverified: on the order of
  ~20 s for launch]**. A first `checkpoint` that mounts a large app synchronously in
  `scene:willConnectToSession:` must stay well under that; the existing per-checkpoint job budget
  is the tool.

### 2.2 App lifecycle: who owns `main`

`UIApplicationMain(argc, argv, nil, @"SveltersAppDelegate")` creates `UIApplication`, loads the
Info.plist scene manifest, runs the main run loop and **never returns**. Windows need a
`UIWindowScene`, which the system connects later via the scene delegate's
`scene:willConnectToSession:options:`. As of the iOS 27 SDK the scene lifecycle is mandatory
(TN3187, `apple-gui-backend.md` §1.2).

Today the host does: `Host::new(backend)` → `host.load_bundle()` (JS mounts, creating windows) →
`host.run()` (the backend's loop). On iOS that order is impossible: UIKit objects must not be
created before `UIApplicationMain` has initialised UIKit **[unverified: creating a `UIView`
before `UIApplicationMain` is undefined territory; do not rely on it]**.

| Option | How | Verdict |
| --- | --- | --- |
| A. **Run first, mount from a callback** | `backend.run(host)` calls `UIApplicationMain`; `application:didFinishLaunchingWithOptions:` calls a new host hook (`host->started`, or a closure passed to `Host::run_with`) that loads the bundle. A `<window>` created by JS before any scene exists is a `UIWindow` with no scene; the first `scene:willConnectToSession:` assigns `window.windowScene = scene` (the property is read-write, iOS 13) and makes it key | **recommended**. Same hook serves Android (where Java owns the loop entirely, `android-backend.md` §2.2) |
| B. Mount inside `willConnect` only | load the bundle when the first scene connects | ties the JS lifetime to one scene; breaks multi-window iPad and scene reconnection |
| C. Keep the order, create UIKit objects before `UIApplicationMain` | — | no |

Lifecycle facts JS must see:

- **No quit.** iOS apps don't terminate themselves; `quit` is a no-op (exit is rejected in App
  Review **[docs: HIG]**). `onRequestClose` never fires for the main window.
- **Scenes come and go.** The system may *disconnect* a backgrounded scene to reclaim memory
  (`sceneDidDisconnect:`) and reconnect it later with a state-restoration activity. On iPhone,
  with one scene, this is rare but legal. Recommendation: on disconnect, the window handle stays
  alive (JS state survives, the `UIWindow` is kept by the handle table) and is re-attached on the
  next connect; report `scene-did-disconnect`/`scene-will-enter-foreground` etc. as signals of an
  app-level kind.
- **Foreground/background**: `UISceneDidEnterBackgroundNotification` and friends; timers keep
  firing until suspension, then nothing runs. `schedule_wake` simply stops; there is no catch-up
  storm because the timer is one-shot and re-armed.
- **Multi-window (iPad, visionOS, Catalyst)**: JS asks for a scene
  (`-[UIApplication activateSceneSessionForRequest:errorHandler:]`, iOS 17) and the system calls
  back. A second `<window>` therefore mounts *detached* and attaches when its scene connects.
  Out of the first slice.

Info.plist needs, for a full-screen modern app: `UIApplicationSceneManifest`
(`UISceneConfigurations` → `UIWindowSceneSessionRoleApplication` → `UISceneDelegateClassName =
SveltersSceneDelegate`), **and** `UILaunchScreen` (an empty dict is enough); without a launch
screen the app runs in the letterboxed legacy screen-size mode **[docs: UILaunchScreen; behaviour
unverified on iOS 27]**.

### 2.3 Run loop, timers, frames

Unchanged from `apple-gui-backend.md` §1.5 and the AppKit implementation:

- `wake`: version-0 `CFRunLoopSource` in common modes + `CFRunLoopWakeUp` (thread-safe).
- `schedule_wake`: the re-armed repeating `CFRunLoopTimer` with 10% tolerance from 20 ms.
- `request_frame`: `+[CADisplayLink displayLinkWithTarget:selector:]` added to
  `NSRunLoopCommonModes` (the real constant, FA1's lesson), paused between requests.
  On ProMotion iPhones the link runs at 60 Hz unless `preferredFrameRateRange` asks for more and
  the Info.plist has `CADisableMinimumFrameDurationOnPhone = YES` **[docs]**.
- The `BeforeWaiting` observer ordered before Core Animation's commit (order < 2000000) works the
  same on iOS.
- No nested modal loops exist on iOS: every presentation is async (`presentViewController:
  animated:completion:`, `UIAlertController`). Nothing to mark `blocking_nested_loop`.

### 2.4 The window root: a view controller, always

On iOS a `UIWindow` shows `rootViewController.view`; setting subviews on the window directly is
unsupported for rotation, safe areas, status bar and trait propagation. So:

- `window` kind = `UIWindow` + a backend-owned `SveltersViewController` (handle-internal, not a
  separate JS node). The window's default slot puts its one child into the VC's `view`.
- `UINavigationController`/`UITabBarController` children are VC kinds (`apple-gui-backend.md`
  §5.3): out of the first slice, but the window's `content` slot should already route a VC-kind
  child to `rootViewController` instead of a subview.

### 2.5 Safe area, keyboard, Auto Layout vs UIStackView

- **Safe area.** Edge-to-edge is the default on iOS; the status bar, notch/Dynamic Island and
  home indicator overlap the window. The root VC pins its single content view to
  `view.safeAreaLayoutGuide` by default; an `ignores-safe-area` attribute (per edge) pins to the
  view edges instead (backgrounds, full-bleed images). The safe area changes at runtime
  (rotation, call banner): `viewSafeAreaInsetsDidChange` on our VC → `safe-area-insets` signal.
- **Keyboard.** iOS 15+ `UIView.keyboardLayoutGuide` **[docs]**: pin the content's bottom to
  `view.keyboardLayoutGuide.topAnchor` instead of the safe-area bottom, and the content shrinks
  above the keyboard with the system animation, no notification handling. Text fields deeper in a
  `UIScrollView` still need scroll-to-visible (later slice). Dismissal: `UIScrollView.
  keyboardDismissMode`, or `endEditing:` as a method.
- **Auto Layout vs UIStackView.** Same decision as AppKit: `UIStackView` for `box`, Auto Layout
  only inside the backend (root pinning, per-child hugging/compression as `set_child_attr`).
  The `removeArrangedSubview:` quirk (child stays in `subviews`) is real and is exercised in the
  probe; the slot annotation emits both calls and the generated test asserts `superview == nil`.
  If profiling of big trees ever says so, the taffy/Yoga fallback from the Apple note §5.1 stands.
- **Dynamic Type.** Labels/fields/buttons follow the user's text size only with
  `adjustsFontForContentSizeCategory = YES` and a `preferredFontForTextStyle:` font
  **[docs]**. Recommendation: the backend sets both by default for text kinds (a native default,
  overridable by a `font` style), because nobody remembers to and App Review/accessibility
  audits flag it.

### 2.6 Accessibility

Unchanged from the Apple note §1.6 (`accessibilityLabel`, `accessibilityTraits`,
`accessibilityIdentifier`). The slice sets `accessibilityIdentifier` from `testid` (and from the
handle number in `--selftest` builds) so XCUITest and the selftest can address views. Custom
kinds (canvas) need `UIAccessibilityElement` containers later. VoiceOver state is observable via
`UIAccessibilityVoiceOverStatusDidChangeNotification`.

## 3. Change notification and reactivity facts

### 3.1 What UIKit offers

| Mechanism | Fires for | Multiplicity | Use for |
| --- | --- | --- | --- |
| **UIControl events** (`addTarget:action:forControlEvents:`, `addAction:forControlEvents:` iOS 14) | **user interaction only**; programmatic setters send no actions ("Setting the switch to either position doesn't result in an action message being sent") **[docs]**. Code can force them with `sendActionsForControlEvents:` | multi-target | value bindings of every UIControl (`value-changed`, `editing-changed`) |
| **NSNotificationCenter** (`UITextFieldTextDidChangeNotification`, `UITextViewTextDidChangeNotification`, keyboard, scene, trait/content-size, VoiceOver) | mostly user edits; scene/keyboard/system state | multi-observer, leaves the delegate free | text views (not UIControls), app/scene/system state |
| **Delegates** (`UITextFieldDelegate`, `UIScrollViewDelegate`, `UINavigationControllerDelegate`, `UITabBarControllerDelegate`, …) | user-driven state changes, often sync with a BOOL veto | one weak slot, backend-owned | `call_sync` vetoes, scroll offset, nav-stack changes made by the swipe-back gesture |
| **KVO** (`addObserver:forKeyPath:options:context:`) | **programmatic** KVC-compliant setter calls (synthesised `@property` setters notify automatically); not user edits that bypass the setter | multi-observer | props set from elsewhere in native code; a fallback when nothing else exists. "Although the classes of the UIKit framework generally do not support KVO, you can still implement it in the custom objects of your application" **[docs, Cocoa Core Competencies]** |
| **Trait changes** (`registerForTraitChanges:withTarget:action:` iOS 17; `traitCollectionDidChange:` before) | dark mode, size class, Dynamic Type, layout direction | per view/VC | `--sv-*` token re-resolution, `prefers-color-scheme` |
| **Overrides** (`layoutSubviews`, `safeAreaInsetsDidChange`, `didMoveToWindow`) | geometry, attachment | subclass only | bounds/size observation needs our own subclass (or KVO on `bounds` on the layer, unreliable) |
| UIKit observation tracking (iOS 26 `updateProperties()`, `@Observable`) | UIKit re-running *app* code when Swift `@Observable` models change | — | not relevant to us: it observes models, not UIKit properties |

Conclusions for the reactivity ABI:

1. There is **no generic "notify" on iOS**. The backend maps a `(handle, prop)` subscription to
   one of the mechanisms above via per-property metadata. KVO is only the fallback for
   properties with no user path and no control event, and it is never "documented compliant"
   for UIKit classes: every KVO-backed fact must be verified by a generated test on the simulator
   (set programmatically → observer fired), and be marked `documented: false`.
2. **Echo is per mechanism, not per toolkit.** Control events and text notifications do not fire
   for our own sets, so a `bind:value` on `UISlider` needs no suppression (signals note §1.6 rule
   5, for free). KVO does fire for our own sets: rule 1's suppress flag applies.
3. **Clamping is silent.** `UISlider.value = 500` with `maximumValue = 1` stores 1 and sends
   nothing **[unverified]**, so rule 2 (read back after set, post one synthetic change) is the
   only way `$bindable` converges. Same for `UIStepper`, `UIProgressView.progress` (clamped to
   0…1).
4. **Continuous vs final.** `UISlider` sends `ValueChanged` continuously while dragging when
   `isContinuous` (default YES); `UITextField` sends `EditingChanged` per keystroke **including
   during IME marked-text composition** (check `markedTextRange != nil` before committing to JS
   state, as Svelte's `compositionend` would) **[unverified]**.

### 3.2 Per-property change notification (first-slice kinds plus the ones that will bite next)

| Kind.prop | User change arrives via | Programmatic set fires it? | Notes / observability fact |
| --- | --- | --- | --- |
| `text-field.text` | `UIControlEventEditingChanged` (target-action) and `UITextFieldTextDidChangeNotification` | no (neither) **[unverified, widely reported]**; KVO on `text` fires for `setText:` only | bind via `editing-changed`; `editing-did-end`/`primary-action-triggered` (Return) for "change". Sync veto: delegate `textField:shouldChangeCharactersInRange:replacementString:` → `call_sync` |
| `switch.on` (`isOn`) | `ValueChanged` | **no [docs: setOn:animated:]** | no echo guard needed |
| `slider.value` | `ValueChanged`, continuous by default; `TouchUpInside`/`TouchUpOutside` mark the end | no | clamp to min/max silently → read back |
| `stepper.value`, `segmented-control.selected-segment-index`, `date-picker.date`, `page-control.current-page`, `color-well.selected-color` | `ValueChanged` | no **[unverified per class]** | one rule for all UIControls: `ValueChanged`, user-only |
| `button` (no value) | `PrimaryActionTriggered` (iOS 9+; the "click") | — | `TouchUpInside` is the older equivalent; prefer primary action, which also covers keyboard/pointer/accessibility activation |
| `label.text`, `progress-view.progress`, `view.hidden`, `view.alpha` | no user path | KVO fires (synthesised setters) **[unverified]** | display-only: observation only matters if native code changes them; mark `observe: kvo, documented: false` |
| `text-view.text` (not a UIControl) | `UITextViewTextDidChangeNotification` / delegate `textViewDidChange:` | no | notification keeps the delegate free |
| `scroll-view.content-offset` | delegate `scrollViewDidScroll:` (every frame while scrolling) | yes (the delegate also fires for `setContentOffset:`) **[unverified]** | KVO on `contentOffset` works in practice but is undocumented; the delegate is the documented path. Throttle to frames |
| `navigation-controller.view-controllers` (the stack) | delegate `navigationController:didShowViewController:animated:` after the **interactive swipe-back pop** | yes (also after our `setViewControllers:`) | the case where the user changes a JS-owned list; must flow back as a structural change, not a prop |
| `tab-bar-controller.selected-index` | delegate `tabBarController:didSelectViewController:`; `shouldSelectViewController:` = sync veto | no | |
| `window` geometry / safe area | `viewSafeAreaInsetsDidChange`, `viewWillTransitionToSize:withTransitionCoordinator:` on our VC | — | backend-owned VC subclass makes these free |
| trait (dark mode, text size) | `registerForTraitChanges:` (iOS 17) / `UIContentSizeCategoryDidChangeNotification` | — | app-level signal, re-resolves style tokens |

### 3.3 What a per-property observability fact needs to encode (proposal for REACTIVITY.md)

Same shape as the Android note's §3.3 so one schema serves both:

```yaml
UISlider:
  props:
    value:
      observe:
        via: control_event          # control_event | notification | delegate | kvo | override | none
        source: UIControlEventValueChanged
        user: true                  # fires for user edits
        programmatic: never         # never | always | on_change | flagged (payload says who)
        slot: multi                 # multi | single (backend-owned delegate/listener slot)
        timing: continuous          # sync_in_setter | continuous | end_of_gesture | deferred
        payload: none               # value | none (backend reads the prop back)
        clamps: true                # setter may store a different value -> read back
        documented: true            # false for KVO on UIKit classes
        end_event: [UIControlEventTouchUpInside, UIControlEventTouchUpOutside]
```

`programmatic` decides echo handling (never → no guard; always/on_change → suppress flag);
`documented: false` forces a ledger test before the fact counts as verified; `slot: single` tells
the backend it must own and multiplex the delegate.

## 4. JS engine on iOS

- **QuickJS-NG, interpreter only.** No JIT is possible for third-party iOS apps (Apple note §2.1);
  QuickJS never JITs, so nothing changes. Lynx ships a QuickJS fork (PrimJS) on the iOS main
  thread **[docs: lynxjs.org]**.
- **Build.** `rquickjs-sys` 0.14.0 ships pregenerated bindings for 20 targets, none of them
  `*-apple-ios*` **[verified]**; without the `bindgen` feature its build script warns and the
  build fails. Options: enable `bindgen` (needs libclang + the iOS SDK sysroot: fine on the macOS
  runner, licence-blocked on Linux), or vendor a bindings file for `aarch64-apple-ios` generated
  once on a Mac (the C ABI is LP64, like `aarch64-apple-darwin`, which *is* shipped: copying it
  is likely correct but must be checked). Recommendation: generate on the runner, commit under
  the host crate, and upstream it to rquickjs. The QuickJS C sources compile for iOS with Apple
  clang through `cc` (as on macOS).
- **App Store rules** (Apple note §2.2, unchanged): bundled JS is fine; **hot reload is dev-only**
  and must be compiled out of release builds; OTA bundle updates are grey and not a goal; release
  builds should use the generated closed-world bindings, not the dynamic bridge reachable from
  downloaded JS (4.7.2).
- **Dev loop on the simulator.** The simulator process sees the Mac's file system, so the M2
  `--watch` loop on a bundle path is expected to work unchanged **[unverified]**. On a device it
  needs the network dev module loader from `plans/research/QUICKJS-RUNTIME.md` (Vite dev server
  over Wi-Fi/USB).
- **JavaScriptCore**: still the optional engine (Apple note §8): ~1.7× QuickJS on iOS (LLInt
  only), Safari Web Inspector on devices. Nothing new.

## 5. Metadata

Reuse `tools/metadata/appkit/ingest.py` + `gen_backend.py` with a framework parameter:

| Step | AppKit (today) | UIKit |
| --- | --- | --- |
| SDK | `xcrun --sdk macosx --show-sdk-path` | `xcrun --sdk iphoneos --show-sdk-path` (iOS, not the Catalyst copy in the macOS SDK: Catalyst marks iOS-only API unavailable) |
| clang target | `arm64-apple-macos…` | `arm64-apple-ios17.0` |
| class prefix / naming | `NS` | `UI` (N1 rules otherwise identical) |
| availability | `macos(x)` | `ios(x)` (also record `macCatalyst`/`visionos` for later targets) |
| output | `metadata/appkit/*.json` | `metadata/uikit/*.json`, `_enums.json` with `sdk: iPhoneOS27.x` |
| CI freshness | `appkit-metadata` job | `uikit-metadata` job, same pattern |

Additions the UIKit ingest needs (headers have them, AppKit didn't use them):

- **`UIControlEvents` as the signal catalogue.** For each `UIControl` subclass the overrides pick
  the events that apply (`UISwitch`: `value-changed`; `UITextField`: `editing-did-begin`,
  `editing-changed`, `editing-did-end`, `editing-did-end-on-exit`, `primary-action-triggered`).
  Signals use the enum member name in kebab-case (`value-changed`), with `via: control_event`.
  The 21 members are in the header **[verified in objc2-ui-kit]**.
- **Notification names** (`UITextFieldTextDidChangeNotification` → `text-field-text-did-change`
  under the MAC1 rule) with the class they're posted by.
- **Observe facts** (§3.3) are override data, defaulting by rule: every UIControl `value`-ish
  prop gets `via: control_event, source: ValueChanged, programmatic: never` unless overridden.
- Q7 of NON-GUI-TYPES (objc2 generated sources as primary ingest, libclang supplementing) applies
  unchanged; UIKit is in `objc2-generated` like AppKit.

## 6. Build, run and test

### 6.1 From Linux

| What | Works on Linux? |
| --- | --- |
| `cargo check --target aarch64-apple-ios` / `aarch64-apple-ios-sim` of the backend | **yes, no SDK** **[verified by the probe; targets added to toolchain 1.97.0 via rustup]** |
| Link a Mach-O for iOS | only with an iOS SDK on disk (licence: Apple hardware). Same answer as `tools/dev/mac-cross`; do it on the runner |
| Package an `.app` | trivial (directory + Info.plist + binary + bundled JS), but useless without a simulator |
| Simulator / device | no |
| Metadata ingest | no (licence); consume committed JSON |

Add a Linux CI step: `cargo check -p svelters-uikit --target aarch64-apple-ios-sim` (cheap,
licence-clean guard, as recommended for AppKit).

### 6.2 On the GitHub macOS runner

1. **Build**: `rustup target add aarch64-apple-ios-sim`; `cargo build -p counter-ios --target
   aarch64-apple-ios-sim` (Apple's linker via `xcrun`; arm64 simulator binaries get an ad-hoc
   linker signature automatically **[unverified for Rust-driven links; `codesign -s -` fixes it
   if not]**).
2. **Bundle**: a small script (`tools/dev/ios-bundle`) writes `Counter.app/{counter-ios,
   Info.plist, app.js}`. No Xcode project needed for the first slice.
3. **Simulator**: `xcrun simctl create sv "iPhone 16" <runtime>` (pick the newest installed
   runtime from `simctl list -j`), `boot`, `bootstatus -b`, `install booted Counter.app`,
   `launch --console-pty booted dev.svelters.counter --selftest` (stdout carries the JSON
   report), `simctl io booted screenshot counter.png`, `shutdown`.
4. **Tests**:
   - **First: an in-process selftest app**, like WIN1's `--selftest`: the app runs the fixture
     script, drives events by calling the same entry points a tap would
     (`sendActionsForControlEvents:`), checks the op log and native read-backs, prints JSON, and
     exits. No XCUITest, no Xcode project.
   - **Generated per-annotation tests** (create/insert/read back/remove, observe facts) need a
     live `UIApplication`. Run the existing `harness = false` test binaries inside a minimal
     app bundle (the `svelters_apple::testing::run_cases` runner started from
     `application:didFinishLaunching…`), which is what `cargo-dinghy` does for iOS. Whether
     UIKit views can be created in a bare `simctl spawn` command-line process is **unverified**;
     don't depend on it.
   - **XCUITest** (real taps by `accessibilityIdentifier`) only after the slice; it needs an Xcode
     project (`xcodegen`) and `xcodebuild test`.
5. **Signing**: none for the simulator. Devices need a Developer Program team, a provisioning
   profile and `codesign`; `rcodesign` can sign from Linux. Not in CI; TestFlight before releases.

Runner cost: macOS minutes bill 10× Linux for private repos (free for public). The simulator
job adds ~2–4 minutes of boot **[estimate]**.

## 7. Styling (brief)

UIKit has no stylesheet language, like AppKit (`metadata/style/appkit.yaml`, `dialect: none`):
every portable style property is a **setter**.

| Portable | UIKit setter |
| --- | --- |
| `color` | `textColor` (label/field), `tintColor` (controls, buttons with configuration) |
| `background-color` | `backgroundColor`; for `UIButton` the configuration's `baseBackgroundColor` |
| `border-radius`, `border-*` | `layer.cornerRadius` (+ `clipsToBounds`), `layer.borderWidth/borderColor` (a key path setter, `layer.` prefix) |
| `padding` | `directionalLayoutMargins` + `layoutMarginsRelativeArrangement` (stack), `UIButton.Configuration.contentInsets` |
| `gap`, `flex-direction`, `align-items` | `UIStackView.spacing/axis/alignment/distribution` |
| `font-*` | `font` (`UIFont` from family/size/weight; text styles for Dynamic Type) |
| `opacity`, visibility | `alpha`, `hidden` |

- `:disabled`/`:active` pseudo-classes: UIKit renders state itself (`isEnabled`, highlighted);
  per-state colours exist only on `UIButton` (`setTitleColor:forState:`, configuration update
  handler). Map the few that exist, report the rest as unsupported diagnostics.
- **`UIAppearance`** proxies (`[UISwitch appearance].onTintColor = …`,
  `appearanceWhenContainedInInstancesOfClasses:`) are the closest thing to type selectors, but
  they apply only when a view enters a window and are app-global; keep to setter rules
  (`setter_selectors: [type, class, id]` as AppKit), possibly using appearance for `@native`
  global rules later.
- Colours should be **dynamic** (`UIColor colorWithDynamicProvider:`) when they come from
  `--sv-*` tokens with a dark variant, so dark mode switches without JS.

## 8. Proposed first slice: UIKIT1 (mirrors MAC1/WIN1)

| Element (tag) | Class | Props (first cut) | Signals | Slot |
| --- | --- | --- | --- | --- |
| `window` | `UIWindow` + backend `SveltersViewController` | `background-color`; attr `ignores-safe-area` | `safe-area-insets-did-change`, scene `did-enter-background`/`will-enter-foreground` (app-level) | `content` (one, pinned to safe area + keyboard guide) |
| `view` | `UIView` | `background-color`, `alpha`, `hidden`, `accessibility-identifier` | — | none (leaf or annotated later) |
| `stack-view` | `UIStackView` | `axis`, `spacing`, `alignment`, `distribution`, `directional-layout-margins`, `layout-margins-relative-arrangement` | — | `arranged` (insert at index; remove = remove arranged + from superview) |
| `label` | `UILabel` | `text`, `number-of-lines`, `text-alignment`, `text-color`, `font` | — | — |
| `button` | `UIButton` (`UIButtonTypeSystem`, or `+buttonWithConfiguration:primaryAction:` iOS 15) | `title` (as `setTitle:forState:` normal), `enabled` | `primary-action-triggered` | — |
| `switch` | `UISwitch` | `on`, `on-tint-color`, `enabled` | `value-changed` (bind:on) | — |
| `slider` | `UISlider` | `value`, `minimum-value`, `maximum-value`, `continuous` | `value-changed` (bind:value), `touch-up-inside` | — |
| `text-field` | `UITextField` | `text`, `placeholder`, `border-style`, `secure-text-entry`, `keyboard-type`, `return-key-type` | `editing-changed` (bind:text), `editing-did-end`, `primary-action-triggered`; sync `should-change-characters` (`call_sync`, veto) | — |
| `progress-view` | `UIProgressView` | `progress` (0…1), `progress-tint-color` | — | — |

Deliverables, in order:

1. `crates/svelters-uikit`: scene/app delegate classes (`define_class!`), the run-first host hook
   (§2.2, shared with Android), window root VC with safe-area/keyboard pinning, UIControl events
   through `SveltersTarget`, KVC props via `svelters-apple`, dynamic dev mode as MAC1.
2. `tools/metadata/appkit` parameterised for UIKit; `metadata/uikit/*.json` from the runner;
   overrides with `observe` facts (§3.3) for every bindable prop above.
3. `examples/counter-ios` (+ `fixtures/counter-ios`) with `--selftest`; generated tests in an app
   bundle on the simulator; screenshot artifact.
4. `.github/workflows/ios-uikit.yml`: `uikit-metadata` freshness job + simulator job; Linux
   `cargo check` step for `aarch64-apple-ios-sim`.

Acceptance: counter fixture renders on the simulator (screenshot), `bind:value` on slider and
`bind:text` on the text field round-trip in the selftest (user event simulated with
`sendActionsForControlEvents:` / `insertText:`), clamped slider set converges, removal from the
stack leaves no subview, generated tests green.

## 9. Risks

- **Host order inversion** (§2.2) touches `svelters-host` and every runner. Small code, but a
  shared-API change: do it once for iOS and Android together.
- **No Apple device in the loop**: every UIKit fact needs the simulator on a macOS runner;
  annotation-loop latency is minutes. The simulator is not a device either: keyboard (hardware
  keyboard connected by default in the simulator, so no on-screen keyboard unless toggled),
  ProMotion, haptics, memory pressure and watchdog timing differ.
- **KVO facts are undocumented** for UIKit (§3.1). Anything that depends on them can break in an
  iOS update; keep them a fallback and covered by ledger tests on every new Xcode.
- **Scene disconnect/reconnect** and state restoration are not exercised by any test we can run
  cheaply.
- **rquickjs bindings for iOS** must be generated and maintained by us until upstreamed.
- **objc2-ui-kit lag** behind the newest SDK (0.3.2 is a year old); newest-API kinds need objc2
  HEAD or our own declarations.
- **App Store review** (dynamic bridge + interpreter): fine with bundled JS (NativeScript, RN,
  Lynx precedents); dev reload must not ship.

## 10. Decisions needed

| # | Question | Recommendation |
| --- | --- | --- |
| I1 | Host order on platforms that own `main`: add a `started` hook (`Host::run_with(on_ready)`) so the bundle loads after the toolkit is up? | **Yes**, shared with Android (A2). Desktop runners keep the current order |
| I2 | `<window>` semantics on iOS: one implicit window bound to the first scene; JS-created windows wait for a scene; `quit` is a no-op? | **Yes** for the slice; multi-scene later |
| I3 | Default content pinning: safe area + keyboard guide, with an `ignores-safe-area` escape? | **Yes** |
| I4 | Dynamic Type on by default for text kinds? | **Yes** (native default, overridable) |
| I5 | Observability facts schema (§3.3) as the shared shape with Android, fed into REACTIVITY.md? | **Yes**; `documented: false` facts need a ledger test |
| I6 | Event naming: UIControlEvents member names (`value-changed`, `primary-action-triggered`) with no portability aliases (as MAC1's `action`)? | **Yes**, consistent with the N1 "native names" rule |
| I7 | QuickJS bindings for iOS: generate on the runner and vendor, upstream to rquickjs? | **Yes** |
| I8 | First test vehicle: in-process `--selftest` app on the simulator, XCUITest later? | **Yes** |
| I9 | Minimum iOS version | **iOS 17** (trait registration API, `keyboardLayoutGuide`, `UIAction`, scene APIs; covers ~all active devices **[estimate]**) |

## 11. Sources

- UIKit and KVO ("Although the classes of the UIKit framework generally do not support KVO…",
  Cocoa Core Competencies, quoted): https://aplus.rs/2020/how-when-use-kvo-in-swift-uikit/ ;
  https://www.hackingwithswift.com/example-code/language/what-is-key-value-observing
- `UISwitch.setOn(_:animated:)` ("doesn't result in an action message being sent"):
  https://developer.apple.com/documentation/uikit/uiswitch/seton(_:animated:) ;
  programmatic changes send no events: https://developer.apple.com/forums/thread/726544
- UIKit observation tracking (iOS 26): https://nilcoalescing.com/blog/AutomaticPropertyObservationInUIKitWithObservable ;
  https://steipete.me/posts/2025/automatic-observation-tracking-uikit-appkit
- Scene lifecycle mandatory with the iOS 27 SDK (TN3187): https://developer.apple.com/forums/thread/813300
- `keyboardLayoutGuide`: https://developer.apple.com/documentation/uikit/uiview/keyboardlayoutguide
- `UILaunchScreen`: https://developer.apple.com/documentation/bundleresources/information-property-list/uilaunchscreen
- `CADisplayLink` / `CADisableMinimumFrameDurationOnPhone`: https://developer.apple.com/documentation/quartzcore/optimizing-promotion-refresh-rates-for-iphone-13-pro-and-ipad-pro
- App Store Review Guidelines 2.5.2, 4.7: https://developer.apple.com/app-store/review/guidelines/
- Lynx (PrimJS, a QuickJS fork, on the main thread): https://lynxjs.org/guide/scripting-runtime/
- cargo-dinghy (cargo test on iOS simulators/devices): https://github.com/sonos/dinghy
- objc2 / objc2-ui-kit 0.3.2: https://github.com/madsmtm/objc2 , https://crates.io/crates/objc2-ui-kit
- rquickjs-sys 0.14.0 bundled bindings list: local registry source (`rquickjs-sys-0.14.0/src/bindings/`)
- GitHub macOS runners and simulators: https://github.com/actions/runner-images
