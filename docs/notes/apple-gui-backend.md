# Apple GUI backends: AppKit, UIKit, SwiftUI (research notes)

Status: research complete (desk research from Linux, 2026-10-16/17). Experiments ran against the macOS 26.1 SDK from a GitHub mirror (research aid only, see §3.5), objc2 HEAD (Xcode 27.1 bindings), and WebKitGTK's JavaScriptCore. Items marked *unverified* or *estimate* were not confirmed.

## 0. Summary

- **Backends:**
  - macOS: AppKit.
  - iOS/iPadOS: UIKit (scene lifecycle is mandatory from the iOS 27 SDK).
  - Mac Catalyst/visionOS: later bonus targets of the UIKit backend.
  - SwiftUI: not a backend. It needs an inverted observable-model design (Expo UI does this with ~75 hand-written components plus 66 modifier adapters), offers no codegen or dynamic dev mode, and is only worth using for hosted leaf components after M4.
- **Language:** Rust + `objc2` + one small ObjC helper file for `NSInvocation`/delegate trampolines. One shared `svelters-apple` crate holds run loop, handles, the dynamic bridge and accessibility; thin AppKit and UIKit crates sit on top.
- **Dev mode is fully dynamic** (the ObjC runtime is our GObject-introspection): `objc_getClass`, KVC for props, `NSInvocation` for calls, a runtime-created target class for actions, and runtime delegate classes for sync handlers. Release uses typed objc2 calls for the tree-shaken roots.
- **Event loop:**
  - `wake` = a CFRunLoopSource signal + `CFRunLoopWakeUp`, in common modes;
  - `schedule_wake` = one re-armed CFRunLoopTimer with tolerance;
  - `request_frame` = `CADisplayLink`;
  - a `BeforeWaiting` observer ordered before Core Animation's commit;
  - async sheets only, with `runModal*` marked `blocking_nested_loop`.
- **Metadata:** ObjC headers are about as good as GIR: properties with `copy`/`weak`, near-total nullability, availability, main-actor, `@optional` protocol methods with return types. About half of the AppKit/UIKit delegate methods return values, and those are our `call_sync` handlers. Linux clang 22 parses the SDK cleanly. What needs annotation: container roles, target-action event names, delegate→signal mapping, the binding notify source, and VC-vs-view.
- **iOS:** interpreter only; any JIT plan is macOS/desktop-only. Bundled JS is fine; hot reload is dev-only.
- **Testing:** Linux can `cargo check` the Apple backend with no SDK, and can even link Mach-O binaries if an SDK is present (licence issue). Everything GUI runs on GitHub macOS runners and the iOS simulator.
- **Effort:** P-style prototype ~2.5–3.5 weeks for macOS, +1.5–2 weeks for iOS.
- **JavaScriptCore (§8):**
  - No JIT on iOS for our process. Its LLInt is ~1.6–1.8× QuickJS-NG (measured). JIT works on macOS (`allow-jit` when hardened).
  - Microtasks auto-drain at the end of each top-level API call (measured), so `run_jobs` becomes a no-op and dispatch must be batched.
  - There's no public module loader, interrupt or heap limit. Safari Web Inspector is the big plus.
  - Verdict: an optional engine after P5/M2, ~1.5–2 weeks, prototypable on this Linux box today.

## 1. Candidate APIs

### 1.1 AppKit (macOS)

- **Tree.** `NSView` is a retained, mutable tree (`addSubview:`, `addSubview:positioned:relativeTo:`, `removeFromSuperview`), which is the same shape as GTK/Qt. The window is `NSWindow`, not a view. Its single `contentView` (or `contentViewController`) is a named slot.
- **Controls.** `NSControl` subclasses (`NSButton`, `NSTextField`, `NSSlider`, `NSPopUpButton`, `NSSegmentedControl`, …) report through **target-action**: a weak `target` plus a `SEL action`, with one action per control. Which user gesture fires it is per-class (`sendActionOn:`, `continuous`). There's no named-event catalogue: the "event names" (`click`, `change`) are ours to annotate.
- **Delegates.** Richer behaviour goes through a single **weak** `delegate` per object (`NSWindowDelegate`, `NSTextFieldDelegate`/`NSControlTextEditingDelegate`, `NSSplitViewDelegate`, `NSTableViewDelegate`). Many delegate methods return values (`windowShouldClose:` → `BOOL`), which makes them our `call_sync` category. Data sources (`NSTableViewDataSource`, `NSOutlineViewDataSource`) are pull-model, i.e. M5 material.
- **NSNotification.** Many delegate callbacks also go out as notifications (`NSWindowDidResizeNotification`, `NSControlTextDidChangeNotification`). Notifications are void and multi-observer, so they're convenient for queued events: the backend can observe them without taking the single delegate slot.
- **KVO** works for most AppKit properties that are set programmatically. But KVO does **not** reliably fire for user-driven changes on controls (typing into `NSTextField` doesn't KVO-notify `stringValue`; it's delivered via `controlTextDidChange:`). So KVO isn't a general "notify" signal, and two-way binding uses action/delegate/notification per class (annotated). Cocoa Bindings (`bind:toObject:withKeyPath:options:`) exist but are legacy and macOS-only; we shouldn't build on them.
- **Layout.** Auto Layout (constraints, `NSLayoutAnchor`), `NSStackView`, `NSGridView` (10.12+), `NSSplitView`, `NSScrollView` (`documentView`) and `NSBox` (`contentView`). There are no layout managers à la Qt: a "box" is `NSStackView`.
- **Main-thread rule.** All of AppKit is main-thread-only. `NS_SWIFT_UI_ACTOR` on `NSResponder` makes this machine-readable (§3.2). `[NSApp run]` must run on the process's first thread. Our host already runs QuickJS on the toolkit thread, so this fits PLAN rule 6 directly.

### 1.2 UIKit (iOS/iPadOS, also Mac Catalyst and visionOS)

- **Tree.** `UIView` is retained and mutable (`addSubview:`, `insertSubview:atIndex:/belowSubview:`, `removeFromSuperview`). `UIStackView` is the box. Unlike `NSStackView`, its `removeArrangedSubview:` **does not remove the view from `subviews`**: the view stays visible and must also get `removeFromSuperview`. That's exactly the kind of fact the generated slot test catches (`on_remove` + `effects`).
- **Controls.** `UIControl` has **multi-target, multi-event** target-action: `addTarget:action:forControlEvents:` with `UIControlEventTouchUpInside`, `ValueChanged`, `EditingChanged`, `PrimaryActionTriggered`, … Since iOS 14 there are also block-based `UIAction`s (`addAction:forControlEvents:`, `+buttonWithType:primaryAction:`). `UIControlEvents` is an options enum in the header, so here the event catalogue **is** machine-readable (a big improvement over AppKit). The mapping to JS names (`click` = `PrimaryActionTriggered`/`TouchUpInside`) is still annotated.
- **View controllers.** UIKit is built around **view-controller containment**: `UINavigationController`, `UITabBarController`, `UISplitViewController` and `UIPageViewController` hold *view controllers*, not views. A Svelte tree that wants `<NavigationStack>` therefore produces VC-typed nodes (§5).
- **Lifecycle.** `UIApplicationMain()` never returns and owns the main thread. `UIApplicationDelegate` handles process events. **Scenes** (`UIWindowScene`, `UISceneDelegate`, iOS 13+) own the windows. As of the iOS 27 SDK the scene lifecycle is **mandatory**: an app built with the latest SDK and no scene manifest fails at launch (TN3187; there are Expo/Flutter breakage reports from the Xcode 27 betas). So our iOS `run()` must register a scene delegate class (at runtime via the ObjC runtime, or a tiny ObjC/Swift file) and put `UIApplicationSceneManifest` in `Info.plist`. A Svelte `<Window>` maps to a `UIWindow` attached to a scene. On iPhone there's one, created by the system, not by JS. Multi-window (iPad, Catalyst, visionOS) means "JS asks for a scene activation, and the system calls back".
- **Main thread.** The rule is the same as AppKit, with `NS_SWIFT_UI_ACTOR` on `UIResponder`/`UIView`.

### 1.3 SwiftUI

SwiftUI is a value-typed description (`struct … : View { var body: some View }`). It is re-evaluated from state by SwiftUI's own diffing, and its "nodes" have no identity we can retain or mutate. To drive it from Svelte you must invert it: keep a **retained model tree of reference-typed observable objects** (one per JS node, holding props and an ordered child list) and write a fixed set of SwiftUI `View` structs that render a model node by switching on its kind.

**Prior art: Expo UI (`@expo/ui/swift-ui`, `expo/expo` `packages/expo-ui/ios`)** does exactly this, and it ships:

```swift
public final class VStackViewProps: UIBaseViewProps {      // class: ObservableObject, one per JS node
  @Field var spacing: Double?
  @Field var alignment: HorizontalAlignmentOptions?
}
public struct VStackView: ExpoSwiftUI.View {
  @ObservedObject public var props: VStackViewProps
  public var body: some View {
    VStack(alignment: props.alignment?.toHorizontalAlignment() ?? .center,
           spacing: props.spacing.map { CGFloat($0) }) { Children() }
  }
}
```

- About 75 component files plus **66 hand-written modifier files** (`Modifiers/`). Modifiers are passed as a serialized `modifiers` array prop and applied in order, because SwiftUI's styling surface is modifiers, not properties.
- Two-way state needed a special `ObservableState` object and synchronous "worklet" callbacks (`SyncToggle`), which is the echo/latency problem from our signals note §1.6.
- The SwiftUI subtree is embedded in UIKit through `UIHostingController` (`<Host>`).

**Why SwiftUI is a poor primary target for us**

1. **No codegen leverage.**
   - The SwiftUI API is generic-heavy Swift: result builders, opaque `some View`, `ViewModifier`s, `Binding<T>`, environment keys.
   - It's only available as `.swiftinterface` text (1.5 MB for macOS 26.1), with no ObjC runtime and no reflection.
   - Every component and modifier is a hand-written Swift adapter that must be **compiled**. That breaks PLAN rule 5 for any new kind and rules out a dynamic dev mode.
2. **Two diffing engines.** Svelte computes fine-grained ops, which we turn into model mutations, which make SwiftUI re-diff a body. Identity has to go through `ForEach` with stable IDs (our handles), or SwiftUI animations and state break.
3. **Imperative escape hatches are missing**: focus, scroll position, first responder, text selection, measuring. Each needs its own `@FocusState`/`ScrollViewReader` plumbing.
4. **Sync handlers.** SwiftUI has almost no veto/return-value callbacks. `onSubmit`, `onChange` and `.onKeyPress` (with `KeyPress.Result`, macOS 14+/iOS 17+) are the exceptions. It's simpler than AppKit here, but also less capable.
5. **Accessibility is good** (SwiftUI derives it from semantics) and **fidelity is "most modern look by default"** (Liquid Glass and similar). That's SwiftUI's real advantage, and it's reachable from UIKit/AppKit anyway, by hosting specific SwiftUI-only components inside a `UIHostingController`/`NSHostingView` leaf.

**Verdict.** SwiftUI is not a backend. At most it's a **leaf component family**: a few kinds (`Gauge`, `Chart`, a SwiftUI-only picker style) wrapped as `NSHostingView`/`UIHostingController` children inside the AppKit/UIKit backend, each with a small hand-written Swift adapter and an observable props object, Expo-style. Consider it after M4.

### 1.4 Mac Catalyst

- Catalyst runs UIKit on macOS. The headers live in the macOS SDK (`System/iOSSupport`), and `.tbd` targets include `arm64-maccatalyst`.
- Pros:
  - one UIKit backend covers iOS, iPadOS, macOS and visionOS;
  - "Optimize for Mac" idiom gives native-sized controls.
- Cons:
  - the macOS result is visibly "iPad app on Mac" in places: menus, window chrome, toolbar and split views look different and need Catalyst-specific API (`UITitlebar`, `NSToolbar` bridging, `UIMenuBuilder`);
  - many AppKit-only things are missing (`NSOutlineView`, rich `NSTableView`, `NSTextView`-class text);
  - AppKit access needs a bundle-plugin trick.
- Desktop apps are the project's core (GTK/Qt first), so a Catalyst-only macOS story would be the weakest of our desktop backends.
- **Verdict:** Catalyst is a cheap *bonus target* once the UIKit backend exists (mostly a build target and availability filtering, `API_UNAVAILABLE(macCatalyst)`). It's not the macOS backend.

### 1.5 Event-loop integration with the host checkpoint model

Both AppKit and UIKit sit on **CFRunLoop**. The main run loop runs in **modes**: `kCFRunLoopDefaultMode`, `NSEventTrackingRunLoopMode`/`UITrackingRunLoopMode` (during menu tracking, live resize, slider drags and scroll tracking), and `NSModalPanelRunLoopMode` (`runModalForWindow:`). `kCFRunLoopCommonModes` is the set that covers all of them. Every source we add must go into **common modes**, or JS freezes while the user drags a scrollbar.

| ABI member | AppKit / UIKit implementation |
| --- | --- |
| `run(host)` | macOS: `[NSApplication sharedApplication]`, set our delegate, `[NSApp run]` (or `finishLaunching` + our own `nextEventMatchingMask:` loop if we ever need pump mode). iOS: `UIApplicationMain(argc, argv, nil, "SveltersAppDelegate")`, which never returns. `run()` "returns" by `exit()` after `quit`, and iOS apps don't quit themselves at all. |
| `wake()` (thread-safe) | A version-0 `CFRunLoopSource` added to the main run loop in `kCFRunLoopCommonModes`. `wake` = `CFRunLoopSourceSignal(src); CFRunLoopWakeUp(mainLoop)`, both documented thread-safe. The `perform` callback calls `host->checkpoint`. Alternative: `dispatch_async(dispatch_get_main_queue(), …)`. The main queue is drained by the main run loop in common modes, but it allocates a block per wake and can't coalesce, so the signalled source is better. |
| `schedule_wake(ms)` (ABI v2) | One `CFRunLoopTimer` in common modes, re-armed with `CFRunLoopTimerSetNextFireDate` (cancel = a far-future date), with `CFRunLoopTimerSetTolerance` ≈ 10% for the `CoarseTimer` case (App Nap/power friendly). Main thread only, matching the ABI comment. `NSTimer` is the same object (toll-free bridged). A `dispatch_source` timer on the main queue also works, but CFRunLoopTimer keeps everything in one mechanism. |
| `request_frame` (planned) | iOS, and macOS 14+: `CADisplayLink` (`[view displayLinkWithTarget:selector:]` on macOS, `+displayLinkWithTarget:selector:` on iOS), added to the main run loop in common modes and paused when no frame is requested. Older macOS: `CVDisplayLink` (callback on a background thread → `wake`). |
| "about to block" hook | `CFRunLoopObserver` on `kCFRunLoopBeforeWaiting`. This is where Core Animation commits the implicit `CATransaction` (observer order 2000000). A checkpoint flush observer with a **lower order** (e.g. 1999000) lets all ops issued in a run-loop turn land in the same CA commit, i.e. the same frame. It's the CF analogue of Qt's `aboutToBlock`/GLib `prepare` and the right place to drain microtasks if a foreign runtime is attached (node-deno note §2(a)). |
| foreign fd (node-deno note) | `CFFileDescriptor` (kqueue fd from libuv) + `CFFileDescriptorEnableCallBacks`, as a `CFRunLoopSource` in common modes. |

**Modal sessions and nested run loops**

- `-[NSApp runModalForWindow:]`, `-[NSAlert runModal]` and `NSSavePanel runModal` spin a **nested** run loop in `NSModalPanelRunLoopMode`. Our common-mode source and timer keep firing, so `checkpoint` re-enters from within a JS-initiated `call` (backend-abi "Note from P2": the host defers it). As on Qt, JS-facing APIs must use the **async** forms instead:
  - `-[NSWindow beginSheet:completionHandler:]`
  - `-[NSAlert beginSheetModalForWindow:completionHandler:]`
  - `-[NSSavePanel beginWithCompletionHandler:]`

  Mark `runModal*` as `unsupported_reason: blocking_nested_loop`.
- **Event tracking** (menus, `NSSlider` drag, live resize, `NSScrollView` elastic scrolling) also runs nested loops inside `sendEvent:`. Because they're entered from native event dispatch, not from JS, `checkpoint` runs normally there **if** our sources are in common modes. This is the single most important AppKit integration detail.
- **iOS has no blocking modals at all.** All presentation is async (`presentViewController:animated:completion:`, `UIAlertController`), and tracking mode only affects which run-loop mode is active. Common modes cover it.

**Sync handlers.** Delegate methods and `respondsToSelector:`-gated callbacks are synchronous calls on the main thread, which is our `call_sync` path with no new mechanism. Target-action and notifications become `post_event`.

### 1.6 Accessibility mapping

- AppKit:
  - The **`NSAccessibility` protocol** (10.10+) is adopted by `NSView`, `NSWindow` and the controls, which provide roles automatically (`NSAccessibilityButtonRole`, …).
  - We set properties through the same protocol: `accessibilityLabel`, `accessibilityRole`, `accessibilityValue`, `accessibilityHelp`, `accessibilityIdentifier`, `isAccessibilityElement`.
  - Custom elements are `NSAccessibilityElement` objects.
  - Role-specific protocols (`NSAccessibilityButton`, `NSAccessibilitySlider`, …) declare the required actions (`accessibilityPerformPress`).
- UIKit: an **informal protocol on `NSObject`** (`UIAccessibility`):
  - `isAccessibilityElement`, `accessibilityLabel`, `accessibilityHint`, `accessibilityValue`;
  - `accessibilityTraits` (an options mask: button, header, selected, …), and `accessibilityIdentifier` (in `UIAccessibilityIdentification`);
  - custom actions (`UIAccessibilityCustomAction`) and containers (`accessibilityElements`).
- Both are **plain properties with KVC-compliant setters**. The `semantics: {role, actions}` block in our schema maps to:
  - AppKit: `accessibilityRole` (a string constant) plus the role protocol's `perform…` actions;
  - UIKit: `accessibilityTraits` bits plus `accessibilityActivate`/custom actions.

  The ARIA-ish props from Svelte (`aria-label` → `accessibilityLabel`, `role` → role/traits) are a small hand-written table per platform, not per widget.
- `accessibilityIdentifier` is the hook XCUITest uses (§6). The backend should set it from a `testid` prop or the handle number in test builds.

### 1.7 Lifetime and ownership

- **Retain counts, not parent ownership.**
  - A superview **retains** its subviews: `addSubview:` +1 and `removeFromSuperview` −1. A window retains its `contentView`, a stack view its arranged subviews, a VC its child VCs and its `view`.
  - Removing a view from its superview does **not** destroy it while someone else (our handle table) holds a reference. That's the opposite of Qt (`delete` children) and similar to GObject with our toggle ref, but simpler: no floating refs and no toggle refs.
- **Backend rule.**
  - The handle table holds one strong reference (`objc_retain` / objc2 `Retained<T>`) per handle. `destroy(h)` releases it.
  - `on_destroyed` is rarely needed, because natives don't free underneath us. The exception is `NSWindow` with `releasedWhenClosed = YES` (the default for programmatically created windows under manual retain-release; under ARC/objc2 set it to `NO` explicitly). The window's "closed" state is reported as an event, not a destroy.
- **Weak back-references we must hold.**
  - `delegate`, `target`, `dataSource` are **weak** (headers: `@property (weak)`). The backend's per-handle proxy (the target for actions, the delegate for sync handlers) must be retained by the handle entry, or it's deallocated immediately and events silently stop.
  - The 2021 GTK lesson ("disconnect-safe handlers") translates to this: on `destroy(h)`, first set `delegate = nil` and `target = nil` (or `removeTarget:action:forControlEvents:`), then release the proxy.
- **Autorelease pools.** Every ABI entry point and every checkpoint must be wrapped in `@autoreleasepool` (objc2: `autoreleasepool(|_| …)`). Otherwise autoreleased temporaries created while JS runs pile up until the run-loop turn ends. AppKit/UIKit already wrap each event dispatch, but `checkpoint` called from our CFRunLoopSource needs its own pool.
- **From Rust.**
  - `objc2::rc::Retained<T>` is the ARC equivalent (retain on clone, release on drop) and `Weak<T>` wraps `objc_storeWeak`.
  - Method families (`new`/`alloc`/`init`/`copy` return +1) are encoded per method by header-translator, so generated code gets ownership right.
  - In a dynamic dev mode, derive ownership from the selector name (Cocoa naming convention, same as ARC). We must follow it for `msg_send` results we hold.
- **`UIViewController.view` is lazy.** Accessing it loads it (`loadView`), so a VC's handle and its view's handle are different objects with different lifetimes (§5).

## 2. iOS-specific constraints

### 2.1 No JIT

- Third-party iOS apps cannot map writable+executable memory. `MAP_JIT` needs the Apple-only `dynamic-codesigning` entitlement, and the EU BrowserEngineKit exception is for browser engines only.
- QuickJS-NG is a pure interpreter, so it's unaffected.
- **Any JIT plan is macOS/Linux/Windows-only.** That includes the Cranelift-for-QuickJS idea (`docs/notes/quickjs-jit-cranelift.md`) and JSC's own JIT (§8.1). On macOS a hardened/notarized app additionally needs `com.apple.security.cs.allow-jit`.
- The engine must keep a working interpreter path on every platform, and JIT can only ever be an opt-in tier.

### 2.2 App Store rules on downloaded/interpreted code

- **Guideline 2.5.2** (verbatim): apps "may not download, install, or execute code which introduces or changes features or functionality of the app".
- **Guideline 4.7** allows HTML5/JavaScript mini apps and games that aren't embedded in the binary, but **4.7.2**: "Your app may not extend or expose native platform APIs or technologies to the software without prior permission from Apple."
- The **Apple Developer Program License Agreement's interpreted-code clause** (quoted from memory; check the current DPLA §3.3 before relying on the wording) allows downloaded interpreted code only if it doesn't change the app's primary purpose, doesn't create a store for other code, and doesn't bypass signing/sandbox. This clause is what Expo Updates/CodePush rely on for JS-bundle updates.
- For us:
  - **A bundled JS bundle is fine**: React Native, NativeScript and Lynx all ship this way.
  - **Hot reload is dev-only**: the M2 Vite-watch reload must be compiled out (or gated behind a debug build/entitlement-free dev flag) in App Store builds.
  - **OTA bundle updates** are grey but common (Expo Updates). They're not a goal, and if offered they must be bug-fix scope.
  - The dynamic ObjC bridge (§4) is **not** a problem in itself: the code being run is our signed binary and the bundled JS. But a JS bundle downloaded at runtime that can reach arbitrary native APIs through it is exactly the 4.7.2 case, so the release profile should use the generated (closed-world) bindings.

### 2.3 Static linking, bitcode, app size

- **Bitcode is gone**: deprecated in Xcode 14 and no longer accepted by the App Store. Nothing to do.
- **Static linking.**
  - Rust produces a `staticlib` (or the Rust binary *is* the app executable, as winit/tao/Bevy do) for `aarch64-apple-ios` / `aarch64-apple-ios-sim` / `x86_64-apple-ios`.
  - Dynamic frameworks inside the `.app` are allowed but bring no benefit for us.
  - `dlopen` of anything not inside the signed bundle is forbidden, and GTK-style dynamic GI loading has no iOS analogue anyway (the ObjC runtime is always there; §4).
- **Size.** The QuickJS-NG `qjs` binary built here is 1.5 MB (x86-64, includes libc helpers), and the engine alone is ~1 MB arm64 *estimate*. Generated typed bindings for the tree-shaken set (M6) are small. Using objc2's framework crates with only the needed cargo features adds little, because they're `extern` declarations. Expect a hello-world app well under 10 MB, versus ~5–8 MB for a Hermes RN hello world (*estimate*).

### 2.4 How the others do it on iOS

| Framework | JS engine on iOS | How native views are driven | Notes for us |
| --- | --- | --- | --- |
| **React Native (New Architecture, Fabric)** | Hermes (bytecode, interpreter on iOS); JSC removed from core in 0.79–0.81 | JS → C++ shadow tree (Yoga layout in C++) → **mounting layer on the main thread** creates and recycles `UIView`s per component (`RCTComponentViewProtocol`), with view recycling pools. Layout is computed by Yoga, *not* Auto Layout: frames are set directly | The closest analogue. Note the decision to **bypass Auto Layout** for performance and cross-platform consistency. JSI = synchronous C++ calls, like our ABI |
| **NativeScript** | V8 `--jitless` | JS calls ObjC directly through metadata + libffi (§3.4). Its UI layer (`@nativescript/core`) is a JS widget library over UIKit, with its own layout (FlexboxLayout, StackLayout) done in JS/native measure passes | proof that fully dynamic ObjC passes App Review |
| **Lynx (ByteDance)** | main thread: **PrimJS, a QuickJS fork**; background thread: JSC on iOS by default | `LynxView` (a `UIView` subclass) hosts an element tree that the engine lays out (its own CSS/flex) and renders to native `UIView`s. "Main-thread scripting" runs JS synchronously on the UI thread for gestures | validates **QuickJS on the iOS main thread** in a shipping product. Its dual-thread split is the opposite of ours (we're single-threaded by design) |
| **Expo UI (`@expo/ui/swift-ui`)** | Hermes (RN) | observable props objects per node, rendered by hand-written SwiftUI `View`s, hosted in `UIHostingController` (§1.3) | the SwiftUI-from-JS template |
| **Tauri mobile (v2)** | none of its own; the UI is a `WKWebView` | Rust core + WebView. Native views only via plugins (Swift) | not comparable: no native widget tree. Its useful part is the **Rust-on-iOS packaging** (`cargo-mobile2`, Xcode project generation, Swift plugin bridge) |
| **Valdi (Snap, open-sourced 2025)** *(from memory, unverified details)* | own runtime with a JS engine (QuickJS-family) | TypeScript components compiled ahead; a C++ layout engine (flexbox), with native views and view recycling | another "QuickJS + native views + own layout" data point |

Common pattern: **every** successful JS-native-UI framework on iOS does its own layout (Yoga or flexbox in C++) and sets frames, rather than generating Auto Layout constraints. That shapes §5.

## 3. Metadata sources for codegen

### 3.1 What exists, and what we actually ran on Linux

| Source | Where | Gives | Tried on Linux |
| --- | --- | --- | --- |
| ObjC headers (`*.framework/Headers/*.h`) | Xcode SDK; mirror tarball `joseluisq/macosx-sdks` release `26.1` (87 MB `.tar.xz`) | classes, categories, protocols (`@optional`), `@property` attributes, selectors, nullability, availability, `NS_SWIFT_NAME`, `NS_SWIFT_UI_ACTOR`, `NS_DESIGNATED_INITIALIZER`, enums/options with values, header doc comments | **yes**: stock Arch `clang 22.1.3` parses `#import <AppKit/AppKit.h>` with `-target arm64-apple-macos15 -isysroot MacOSX26.1.sdk` with zero errors (17 s with `-fmodules`, 6 s through libclang without) |
| UIKit headers | **inside the macOS SDK**: `System/iOSSupport/System/Library/Frameworks/UIKit.framework` (Mac Catalyst copy, 442 headers); full iOS SDKs mirrored at `xybp888/iOS-SDKs` (iPhoneOS 9.3 … 27.0) | same as above, for UIKit | **yes**: `-target arm64-apple-ios17.0-macabi -iframework …/iOSSupport/…` parses cleanly. Catalyst headers mark iOS-only API `API_UNAVAILABLE(macCatalyst)`, so for the iOS target the real iPhoneOS SDK is the authoritative input |
| API notes (`AppKit.apinotes`, `UIKit.apinotes`, `Foundation.apinotes`) | next to the headers, YAML | Swift renames (3655 `SwiftName` entries in AppKit), `SwiftPrivate`, `SwiftImportAsAccessors`, nullability fixes (38 + 7 `NullabilityOfRet`), availability overrides | read as YAML. clang applies them automatically with `-fapinotes-modules` |
| `.swiftinterface` | `SwiftUI.framework/Modules/SwiftUI.swiftmodule/arm64e-apple-macos.swiftinterface` (1.5 MB text), also `AppKit.swiftmodule/*.swiftinterface` (Swift overlays) | the **whole SwiftUI API** as Swift source text (structs, modifiers, result builders, `@available`) | readable; parsing needs `swift-syntax` (Swift toolchain on Linux works) or a regex pass. Only useful for the SwiftUI option (§1.3) |
| `.tbd` (TAPI text stubs) | `AppKit.framework/AppKit.tbd` | exported symbols and ObjC class names per target triple (`arm64-macos`, `arm64-maccatalyst`, …), install name | trivially readable YAML. Only gives *existence* (link check, "is class X exported on target T"), no signatures |
| objc2 `header-translator` | `madsmtm/objc2` `crates/header-translator` (20.8 kLoC Rust, libclang via `clang-rs`) | in-memory model of everything above plus per-framework `translation-config.toml` fixes | read source; not built (it wants an SDK path via `DEVELOPER_DIR`, which the mirror tarball satisfies) |
| objc2 generated crates | `madsmtm/objc2-generated` (AppKit: 302 files, 5.1 MB; UIKit: 456 files, 5.5 MB), also published as `objc2-app-kit` / `objc2-ui-kit` on crates.io; generated from **Xcode 27.1** SDKs at HEAD | typed Rust: selector, method family, `MainThreadMarker`, `Option<>` nullability, weak-property doc, `#[optional]` delegate methods with return types | inspected (see 3.3) |
| NativeScript metadata generator | `NativeScript/ios` repo, `metadata-generator/` (C++ on libclang) | a binary metadata blob linked into the app, read by the runtime to call ObjC dynamically from JS | see 3.4 |

Probe script: `docs/notes/scripts/apple_objc_dump.py` (libclang Python bindings, `pip install clang`, run with `python -I`). Outputs went to the scratchpad (`apple/out/appkit.json`, `uikit.json`).

Whole-framework counts from the probe (macOS 26.1 SDK, own declarations only, accessors counted as methods):

| Framework | classes | protocols | properties | methods |
| --- | --- | --- | --- | --- |
| AppKit | 313 | 147 | 3112 | 9408 |
| UIKit (Catalyst copy) | 524 | 210 | 3230 | 8621 |

Two libclang gotchas worth recording for the M3 generator:

- Pass `CXTranslationUnit_IncludeAttributedTypes` (0x1000). Without it libclang strips `_Nullable`/`_Nonnull` sugar, and every type reports nullability "invalid", including the implicit `NS_ASSUME_NONNULL` ones.
- Pass `-resource-dir $(clang -print-resource-dir)` when using the PyPI bindings, or `CoreFoundation.h` fails on `<stdarg.h>`.

### 3.2 What headers give us (sample: real output)

```text
NSStackView < NSView
  P delegate          id<NSStackViewDelegate> _Nullable    [weak]
  P arrangedSubviews  NSArray<__kindof NSView *> *  nonnull [readonly, copy]
  M -addArrangedSubview:(NSView * _Nonnull)              API_AVAILABLE(macos(10.11))
  M -insertArrangedSubview:atIndex:(NSView*, NSInteger)
  M -removeArrangedSubview:(NSView*)
NSControl < NSView
  P target  id _Nullable [weak]        P action SEL _Nullable
NSWindow < NSResponder
  P contentView  __kindof NSView * _Nullable [strong]
  P delegate     id<NSWindowDelegate> _Nullable [weak]
  P releasedWhenClosed BOOL [getter=isReleasedWhenClosed]
UIControl < UIView
  M -addTarget:action:forControlEvents:(id _Nullable, SEL, UIControlEvents)
  M -addAction:forControlEvents:(UIAction*, UIControlEvents)   API_AVAILABLE(ios(14.0))
UIViewController < UIResponder
  P view  UIView * _Null_unspecified [strong]
  P parentViewController UIViewController * _Nullable [readonly, weak]
  M -addChildViewController: / -willMoveToParentViewController: / -didMoveToParentViewController:
```

Compared to GIR, ObjC headers are *better* than Qt's sources and roughly at GIR's level:

- **Properties** are first-class (`@property`) with getter/setter names, `copy`/`strong`/`weak`/`assign`, `readonly`, `class`. `weak` matters a lot: every `delegate`, `target` and `dataSource` is weak, so the backend must keep its proxy objects alive itself (§1.7).
- **Nullability** is near-total in AppKit/UIKit (audited regions), with a few `_Null_unspecified` holdouts (`UIViewController.view`).
- **Availability** per platform and version (`API_AVAILABLE(macos(10.11))`, `API_UNAVAILABLE(macCatalyst)`), visible as `AvailabilityAttr` in the AST.
- **Main-thread rule** is machine-readable: `NS_SWIFT_UI_ACTOR` (`__attribute__((swift_attr("@UIActor")))`) sits on `NSResponder`, `UIResponder`, `UIView` and on individual methods, and is inherited. objc2 turns it into `MainThreadOnly` + a `MainThreadMarker` argument on constructors. The 71 AppKit headers that use it cover the whole view hierarchy.
- **Protocols** list `@optional` methods with full signatures and return types. Counted over objc2's generated traits: AppKit has 62 `*Delegate`/`*DataSource` protocols with 536 methods, of which **308 return a value** (109 `BOOL`). UIKit: 92 protocols, 659 methods, 313 non-void (112 `BOOL`). These are our `call_sync` handlers (`windowShouldClose:` → BOOL, `textField:shouldChangeCharactersInRange:replacementString:` → BOOL, `splitView:constrainMinCoordinate:ofSubviewAt:` → CGFloat).
- **Swift names** (`NS_SWIFT_NAME` and apinotes `SwiftName`) give a ready-made idiomatic name for `.d.ts` (`addArrangedSubview(_:)`), better than mangling selectors ourselves.
- **Header doc comments** survive in the AST; objc2 keeps them. Useful as provenance text for agents.

### 3.3 objc2: how it works, and what we can reuse

- `header-translator` loads each framework umbrella header with libclang for every target triple (macOS, Catalyst, iOS, tvOS, watchOS, visionOS), builds a model (`Stmt` enum: classes, protocols, categories, enums, typedefs, functions), merges across targets into `#[cfg(target_os)]`, and emits Rust with `extern_class!`, `extern_protocol!`, `extern_methods!`. Per-framework `translation-config.toml` files hold hand fixes (AppKit 574 lines, UIKit 279): skips, renames, safety, `requires-mainthreadonly` for protocols, bridged CF types.
- The model has exactly what we need: memory management (`Weak`, `Copy`, `Retain`, `UnsafeUnretained`), availability, `mainthreadonly` (from `UIActor`), nullability, method family (`new`/`init`/`copy` → ownership of returned objects), `NS_RETURNS_RETAINED`.
- It does **not** emit a neutral data file. Its output is Rust source. Reuse options:
  1. **Depend on `objc2-app-kit` / `objc2-ui-kit` for the hand-written parts of a Rust backend** (window, run loop, delegates). Recommended, whatever else we do.
  2. **Fork/patch header-translator to dump JSON** from its model (one extra emitter over `Stmt`). This yields our `svelters-element-kind/v0` input with objc2's years of edge-case fixes included. Medium effort, and MIT/Apache/Zlib licensed. Upstream might accept a `--emit json` mode.
  3. **Our own libclang pass** (the probe above, ~200 lines Python). It's enough for P and early M3 because our needs are narrower than objc2's: only view/control/delegate classes, and the dynamic runtime covers the rest.
- Recommendation: (3) for P, keeping field names compatible with objc2's model, then evaluate (2) at M3 when the Rust port of the generators happens anyway.

### 3.4 NativeScript's iOS metadata generator

`NativeScript/ios` `metadata-generator/` (Apache-2.0, 12.1 kLoC C++, last commit 2026-10-13):

- It's a **clang LibTooling** program (C++ AST, not libclang). It reads an umbrella header of all SDK modules plus app headers and produces three outputs:
  - `--output-bin`: a compact **binary metadata blob** (hashtables of `InterfaceMeta`/`ProtocolMeta`/`MethodMeta`/`PropertyMeta`/`StructMeta`, with ObjC type encodings). The runtime finds it in a Mach-O section `__DATA,__TNSMetadata` linked into the app executable, or in a prebuilt `NativeScriptDefaultMetadata.framework` (an "iOS SDK snapshot").
  - `--output-yaml`: the same model as YAML, a human-readable dump. This is the most directly reusable artifact for us as a cross-check.
  - `--output-typescript`: `.d.ts` for every ObjC API, optionally enriched from the Apple docset.
- The runtime (V8 in `--jitless` mode on iOS) resolves `UIButton.alloc().init()` through the metadata to a selector plus signature, and calls through **libffi** (vendored submodule) and the ObjC runtime. JS subclasses and delegates are implemented by creating ObjC classes at runtime and forwarding to JS.
- Relevance: NativeScript proves that **fully dynamic ObjC from JS is shippable on the App Store** (see §2) and that a metadata blob of the *whole* SDK is acceptable in size. It also illustrates what we deliberately avoid: per-object JS proxies over raw native objects. Our "No raw toolkit pointer crosses into JavaScript" rule keeps us at the element-kind level.

### 3.5 Getting headers legally on Linux

- The SDKs are covered by the **Xcode and Apple SDKs Agreement**, which licenses them for developing apps for Apple platforms and restricts use to Apple-branded computers. Running the SDK headers through clang on a Linux box is outside the letter of that licence. The GitHub mirrors (`joseluisq/macosx-sdks`, `phracker/MacOSX-SDKs`, `xybp888/iOS-SDKs`) redistribute the SDK without permission. **osxcross** doesn't redistribute anything: its users package the SDK themselves from a downloaded `Xcode.xip`, under the same licence.
- What the ecosystem does:
  - objc2: "We do not redistribute SDKs, to hopefully avoid a license violation." It runs header-translator on a Mac but **publishes the generated bindings**, including header doc comments, on crates.io.
  - NativeScript publishes `.d.ts` and an SDK-snapshot metadata framework.
  - Rust's `aarch64-apple-darwin` cross builds from Linux exist, but the Rust project ships no SDK.
- **Our policy (recommendation):**
  1. Never commit SDK files.
  2. Generate the element-kind metadata on a **macOS CI runner** with a real Xcode (licensed use), and check in only the derived facts (names, signatures, attributes, availability): `metadata/apple/*.json`, as for GTK/Qt.
  3. Linux developers and Linux CI consume the checked-in JSON and never need headers.
  4. The SDK mirror tarball used for this desk research stays in the scratchpad, as a research aid only.
  5. Keep header doc comments out of checked-in metadata, unlike objc2. Link to Apple's documentation URL instead (objc2's pattern `developer.apple.com/documentation/appkit/nsstackview?language=objc`).

### 3.6 What metadata lacks (annotation work)

Upstream gives types. These are hand-annotated per kind, in the same `metadata/overrides/apple/<Kind>.yaml` format as `signals-and-slots-semantics.md` §2.5:

| Fact | Why headers can't tell | Example annotation |
| --- | --- | --- |
| **Container role** | `addSubview:` exists on every `NSView`, yet most views aren't meant to hold children. `addArrangedSubview:`, `contentView`, `documentView`, `setView:` (`NSTabViewItem`), `viewControllers` are all just methods/properties | `NSStackView`: `default_slot: arranged`, `ops: {insert: insertArrangedSubview:atIndex:, remove: [removeArrangedSubview:, removeFromSuperview]}`. `NSScrollView`: `slot document {set: setDocumentView:}`. `NSBox`: `slot content {set: setContentView:}`. `NSWindow`: `slot content {set: setContentView:}`, `slot toolbar {accepts: NSToolbar}` |
| **UIStackView remove quirk** | `removeArrangedSubview:` keeps the subview | `effects: {child: still_subview}` → the generator emits `removeFromSuperview` too; a test asserts `child.superview == nil` |
| **Target-action event names** | `action` is one opaque `SEL`; when it fires is per class (`NSButton`: on click; `NSTextField`: on Enter, or on end-editing; `NSSlider`: continuous or not) | `NSButton: signals: [{name: click, via: target_action}]`. `NSTextField: [{name: activate, via: target_action}, {name: input, via: notification, native: NSControlTextDidChangeNotification}]` |
| **UIControlEvents → JS events** | the enum is in the header, but which event is "the" click is convention | `UIButton: click = UIControlEventPrimaryActionTriggered`. `UITextField: input = EditingChanged, change = EditingDidEnd` |
| **Delegate methods → signals, sync and result** | the header gives `BOOL windowShouldClose:`, but not that YES means "allow" (veto inverted vs GTK's `close-request`) | `NSWindowDelegate.windowShouldClose: → signal close-request, delivery: sync, result: {preventDefault: NO, default: YES}` |
| **Delegate multiplexing** | one weak `delegate` slot per object, and the app can't also set its own | the backend owns the delegate; JS-visible signals are the annotated subset |
| **Two-way binding source** | KVO doesn't fire for user edits | `NSTextField.stringValue: notify: {notification: NSControlTextDidChangeNotification}`. `NSSlider.doubleValue: notify: {target_action}` |
| **Blocking methods** | `runModal`, `runModalForWindow:` look like ordinary methods | `unsupported_reason: blocking_nested_loop` |
| **Lazy/default children** | `NSScrollView.contentView` (clip view) and `UIViewController.view` exist before anyone sets them | `default.kind: builtin` |
| **VC vs view** | `UINavigationController` children are VCs | `content: view_controller` (a new `content` value for §2.5) |

## 4. Dynamic vs generated; backend implementation language

### 4.1 The ObjC runtime is our GObject-introspection

Everything the dev profile needs is available at runtime, with no codegen and no per-project build:

| Need | Runtime facility |
| --- | --- |
| `create(kind)` | `objc_getClass("NSButton")` → `[[cls alloc] init]` (or an annotated convenience constructor) |
| `set_prop`/`get_prop` | **KVC**: `setValue:forKey:` / `valueForKey:`. It does the boxing for us (`NSNumber` → `BOOL`/`CGFloat`/`NSInteger`, `NSValue` → `NSRect`/`NSEdgeInsets`) and finds `setTitle:`/`isEnabled` by convention. Property metadata from the runtime: `class_copyPropertyList` / `property_getAttributes` (`T@"NSString",C,N,V_title`: type, copy/weak, custom getter) |
| `call(method)` | `NSInvocation` with `[obj methodSignatureForSelector:]`, which handles every ABI case, including struct returns on x86-64. Or libffi + `method_getTypeEncoding`. objc2 has `msg_send!` only for statically known signatures, so a dynamic Rust path needs libffi or an ObjC helper |
| events (target-action) | **one** generic target class per backend (`SveltersTarget`, with `-fire:` retained per handle), registered at startup with `objc_allocateClassPair` + `class_addMethod` (objc2 `define_class!` does this statically). It posts `(handle, signal)` |
| delegates / sync handlers | a per-handle delegate object of a runtime-created class that **adds only the selectors JS listens to** (`class_addMethod` with an IMP that marshals via the protocol's method types from `protocol_getMethodDescription`). Or an `NSProxy` with `respondsToSelector:` + `forwardInvocation:`. **Gotcha:** many Cocoa classes cache `respondsToSelector:` answers when the delegate is *assigned* (`NSTableView`, `UITableView`, `UIScrollView`). After `listen(on)` adds a selector, the backend must re-assign `delegate` (set nil, then the proxy). That's an annotation-free rule, applied always |
| notifications | `[NSNotificationCenter addObserver:selector:name:object:]` with the same generic target |
| class/protocol discovery | `objc_copyClassList`, `class_getSuperclass`, `class_copyProtocolList`, `protocol_copyMethodDescriptionList` |

So dev builds can be **fully dynamic**, exactly as planned for GTK ("dynamic GObject property/signal access plus GIRepository/libffi method invocation"). The checked-in metadata JSON supplies the semantics the runtime lacks (§3.6), and the runtime supplies the mechanics. Release builds (M6) generate typed `objc_msgSend` calls (objc2 `msg_send!` or the objc2 framework-crate methods) for the tree-shaken roots.

Where the runtime is weaker than GI: there's **no nullability and no generics** at runtime (`@` is just "object"), and type encodings lose `NSInteger` vs enum names. So the runtime path validates against metadata, and doesn't replace it.

### 4.2 Which language for the backend?

| Option | For | Against |
| --- | --- | --- |
| **Rust + objc2** (`objc2`, `objc2-foundation`, `objc2-app-kit`, `objc2-ui-kit`, `block2`, `dispatch2`) | same language as the host and GTK backend. `MainThreadMarker`, `Retained`/`Weak` and `define_class!` give type-checked main-thread and ownership discipline. Generated from Xcode 27.1 SDKs and actively maintained. **Cross-compiles and links on Linux** (below). Release thin bindings = calling objc2 methods, with no new codegen target | the dynamic `call` path needs libffi (or a 50-line ObjC `NSInvocation` helper compiled by `cc`). The objc2 API surface churns between 0.x releases. Generic delegate IMPs (variadic signatures) are awkward in Rust |
| ObjC (or ObjC++) behind the C ABI | `NSInvocation`, KVC, `class_addMethod` and blocks are native. The simplest dynamic code. Apple sample code applies directly | a third backend language (after Rust and C++). Manual or ARC memory reasoning. Codegen would emit ObjC |
| Swift behind `@_cdecl` | needed for SwiftUI leaves anyway | no dynamic runtime story beyond ObjC's. Swift ABI/toolchain coupling to Xcode. Swift on Linux can't build AppKit/UIKit code. The largest build complexity |

**Recommendation: Rust + objc2 for the backend, plus one small ObjC file** (`svelters_objc_helpers.m`, compiled with `cc`: `NSInvocation` dispatch, a generic delegate trampoline, and the `@autoreleasepool` boundary helper). That's the objc2 equivalent of the GTK backend's libffi use. Swift appears only for optional SwiftUI leaf components (§1.3), never on the critical path.

**Cross-build checks run here** (Linux, no Mac):

- `clang 22.1.3 -target arm64-apple-macos13 -isysroot MacOSX26.1.sdk -fobjc-arc -fuse-ld=lld -framework AppKit hello.m` (an NSWindow + NSStackView + NSButton with target-action) produced a linked `Mach-O 64-bit arm64 executable` in 4 s. `ld64.lld` resolves AppKit symbols against the SDK's `.tbd` stubs, so this is a real link check: an undefined or misspelled selector-free C symbol fails.
- `cargo build --target aarch64-apple-darwin` of a probe using `objc2 0.6`, `objc2-foundation 0.3`, `objc2-app-kit 0.3.2` (typed `NSStackView::new(mtm)`, `addArrangedSubview`, plus a dynamic `AnyClass::get("NSButton")` + KVC `setValue:forKey:`) **built and linked** with `linker = "clang"` + `-isysroot` + `-fuse-ld=lld`. It's a `Mach-O 64-bit arm64 executable` with `NOUNDEFS`. The Rust std targets `aarch64-apple-darwin` and `aarch64-apple-ios` were added to the pinned 1.97.0 toolchain for this.
- Not runnable here, obviously. Signing for distribution can also happen on Linux (`rcodesign` from `apple-codesign` signs and notarizes), but running needs Apple hardware.

## 5. Slots and layout

### 5.1 The generic `box`

- **macOS:** `NSStackView` (`orientation`, `spacing`, `alignment`, `distribution`, `edgeInsets`). Per-child attributes go through `set_child_attr`: `setCustomSpacing:afterView:`, `setVisibilityPriority:forView:`, and gravity (`addView:inGravity:`, an **ordered-per-key** slot like QMainWindow toolbar areas, `key: gravity`). Hugging/compression priorities are per-*view* Auto Layout properties (`setContentHuggingPriority:forOrientation:`) that act as the "expand" flag. Set them via `set_child_attr(parent, child, expand)` → the child's hugging priority along the stack axis.
- **iOS:** `UIStackView` (`axis`, `spacing`, `alignment`, `distribution`, `layoutMargins`), with the remove quirk (§3.6) and `setCustomSpacing:afterView:`.
- Both are Auto Layout under the hood. For P-scale UIs (tens to hundreds of views) stack views are fine and give native behaviour (RTL, Dynamic Type, safe areas, baseline alignment) for free.

**The layout decision (recommendation).**

- P/M2–M4: **stack views + Auto Layout** for `box`/`grid` (`NSGridView` on macOS; nested stacks on iOS).
- Expose raw constraints only as a later escape hatch: an `sv-constraint` element or a `layout` prop with VFL. Don't build a constraint DSL.
- If profiling shows Auto Layout cost on large lists, adopt RN's answer: a Yoga/taffy flex layout in Rust that sets `frame`s directly. `taffy` is pure Rust and already relevant for egui. That decision belongs to M5 (virtualized lists use cell reuse anyway: `NSTableView`/`UICollectionView`).

### 5.2 Named slots

| Kind | Slots (proposed annotations) |
| --- | --- |
| `NSWindow` | `content` (one: `setContentView:`; or `contentViewController` when the child is a VC kind; route by kind), `toolbar` (one: `NSToolbar`, `setToolbar:`; toolbar items come from a delegate: `toolbar:itemForItemIdentifier:willBeInsertedIntoToolbar:`, so the backend synthesizes the delegate from child `ToolbarItem` elements), `titlebar_accessory` (many: `addTitlebarAccessoryViewController:`, attr `layoutAttribute`) |
| `NSSplitView` / `NSSplitViewController` | `panes` (many, ordered: `addArrangedSubview:` / `insertArrangedSubview:atIndex:` on 10.11+, or `addSplitViewItem:` with `NSSplitViewItem` wrapping a VC: `effects.child: wrapped:NSSplitViewItem`, attrs `canCollapse`, `holdingPriority`, `minimumThickness`) |
| `NSScrollView` | `document` (one: `setDocumentView:`); `contentView` is a builtin clip view (`default.kind: builtin`) |
| `NSTabView` / `NSTabViewController` | `pages` (many: `NSTabViewItem` wrapping a view, attrs `label`, `image`; same shape as the GtkNotebook example in §2.5.8) |
| `UINavigationController` | `stack` (many, ordered, **VC content**: `setViewControllers:animated:` with the full list; push/pop are diffs of that list). The top of the stack is "the page". Per-page `navigationItem` (title, bar buttons) is a **sub-slot on the child VC** (`item.navigationItem`) |
| `UITabBarController` | `tabs` (many, VC: `setViewControllers:animated:`; attrs from the child's `tabBarItem`: title, image, badge) |
| `UISplitViewController` | `primary`/`supplementary`/`secondary` (one each, VC: `setViewController:forColumn:`, iOS 14+) |
| `UIWindow` | `root` (one VC: `setRootViewController:`) |

### 5.3 View-controller containment vs our single tree

- UIKit keeps **two parallel trees**: VCs (containment: `addChildViewController:` → add the child's `view` into the parent's view hierarchy → `didMoveToParentViewController:`) and views. Containers like `UINavigationController` take VCs only. Our ABI has one tree of handles.
- Mapping (recommendation):
  - **VC kinds are first-class element kinds** (`UINavigationController`, `UITabBarController`, a generic `ViewController` page kind). A Svelte `<Page>` is a VC kind whose default slot is its `view`. `create(Page)` = a generic `SveltersViewController` whose `loadView` returns a container view (or a `UIStackView`) owned by the same handle.
  - **A plain view inserted into a VC-only slot is wrapped automatically**: `effects.child: wrapped:ViewController`, the same mechanism as `NSSplitViewItem`/`GtkNotebookPage`. The wrapper is an implicit handle the host learns about through the insert's `returns: handle:item` (§2.5.2), so `remove(parent, child)` can find it.
  - **A VC kind inserted under a view parent** does the full containment dance: find the nearest ancestor VC by walking up the handle tree, then `addChildViewController:`, `addSubview:`, `didMoveToParentViewController:`. Removal reverses it. That's one generic rule in the backend, not per-kind annotation.
- `insert(parent, child, before, slot)` is enough for all of this. `before` is honoured by recomputing the full `viewControllers` array and calling `setViewControllers:animated:`, which UIKit diffs and animates. Push/pop animation becomes an attribute (`animated`), defaulting to true after first mount.
- `set_child_attr` (planned M4) carries:
  - stack per-child spacing, visibility priority, hugging (expand), gravity;
  - split item thickness/collapse;
  - tab item title/image/badge;
  - grid row/column (`NSGridView`, on the GtkGrid pattern in §2.5.7).

## 6. Testing without a Mac

| Layer | Runs on Linux? | How |
| --- | --- | --- |
| Metadata ingestion (headers → JSON) | technically yes (shown here), **licence says no** (§3.5) | run on a macOS runner, check the JSON in. Linux CI only validates schema and diffs |
| Generator output (Rust/ObjC source) | yes | deterministic byte-identical rerun check, as for GTK/Qt |
| `cargo check` of the Rust backend for `aarch64-apple-darwin` / `aarch64-apple-ios` | **yes, no SDK needed** (verified) | Linux CI job |
| **Compile + link check** of the backend for `aarch64-apple-darwin` / `aarch64-apple-ios` | yes, *if* an SDK is present (shown in §4.2) | same licence problem for CI on Linux. Practical answer: do it on the macOS runner too. Locally, developers who own Macs can do it on Linux with their own SDK copy |
| Op-log fixtures against the log backend | yes | unchanged; the backend-independent part of P |
| Anything that creates an `NSView` | **no** | GNUstep is *not* a substitute: wrong APIs (no `NSStackView`/Auto Layout parity) and different behaviour. Don't go there (objc2's `gnustep = true` exists only for the runtime) |

**Recommended setup**

1. **GitHub-hosted macOS runners**: `macos-15` / `macos-26` on Apple silicon (M-series, 3 vCPU/7 GB on the standard image; larger arm64 runners are paid).
   - Free for public repositories; for private ones they bill at the macOS rate (10× Linux minutes).
   - Images include Xcode and iOS simulators.
   - Run `cargo test` for the AppKit backend there. GUI tests work because hosted macOS runners have a logged-in Aqua session: windows can be created and screenshotted (`screencapture`), and accessibility queries work once the runner process has Accessibility permission. That's the case on hosted images for XCUITest's own runner. For our own AX queries, *verify on first use*.
2. **The per-annotation verification tests (§2.5.4 recipe) on macOS** run **in-process**, the same as Qt/GTK under Xvfb: create, insert, read back via `get`, remove, check liveness with an objc2 `Weak`. No UI automation framework is needed, because these are direct API calls on the main thread inside a test binary that runs `NSApplication` with `activationPolicy = .prohibited`/`.accessory`.
3. **iOS**: build with `cargo build --target aarch64-apple-ios-sim`, package a minimal `.app` (Info.plist with a scene manifest, signed ad-hoc for the simulator), then `xcrun simctl boot` / `install` / `launch`. Screenshots: `xcrun simctl io booted screenshot`.
   - **Interaction**: an **XCUITest** target, which needs an Xcode project (generate it with `xcodegen` or `cargo-mobile2`). It queries by `accessibilityIdentifier`, which the backend sets from `testid`, so the same fixture's scripted interaction (`{"event": …}` steps) can be replayed through real taps.
   - Alternatively, in-process tests via `simctl launch --console` with an env var that tells the host to run a fixture script and print the op log, the same as the GTK/Qt fixture harness. That works without XCUITest, and **this is the one to build first**.
4. **Physical devices**: not in CI. Manual smoke tests plus TestFlight before any release.
5. **Linux CI**: schema checks, generator determinism, log-backend fixtures. Also a **`cargo check --target aarch64-apple-darwin` and `--target aarch64-apple-ios` job**. objc2's framework crates are plain Rust `extern` decls, so `cargo check` (no link) **needs no SDK at all**: verified here with `SDKROOT` unset, and both targets checked the objc2 AppKit/UIKit probe cleanly (about 3 min each from cold with `-j4`; the dependency graph pulls in many objc2 framework crates). It's the cheap, licence-clean Linux-side guard against Rust-level breakage in the Apple backend.

## 7. Recommendation

1. **Primary backend per platform.**
   - macOS: **AppKit**.
   - iOS/iPadOS: **UIKit** (scene lifecycle mandatory).
   - visionOS and Mac Catalyst come for free from the UIKit backend, later and as secondary targets.
   - SwiftUI: not a backend (§1.3).
2. **Sharing between AppKit and UIKit.** Share the **runtime mechanics**, not the widget code. One `svelters-apple` crate holds:
   - the CFRunLoop integration (`wake` source, `schedule_wake` timer, `BeforeWaiting` flush observer, `CADisplayLink`);
   - the handle table with `Retained`/`Weak` and autorelease pools;
   - the dynamic ObjC bridge (KVC, `NSInvocation` helper, generic target, runtime delegate classes with the re-assign-on-listen rule);
   - the metadata loader and accessibility mapping glue.

   Two thin crates (`svelters-appkit`, `svelters-uikit`) hold the app/window/scene lifecycle and per-kind generated tables. Metadata is per framework (`metadata/apple/appkit/*.json`, `…/uikit/*.json`) with shared Foundation/QuartzCore types. Estimated sharing: ~60% of backend code.
3. **SwiftUI verdict.** Possible only as the Expo UI-style inverted model with hand-written Swift adapters per component and modifier. There's no codegen leverage and no dynamic dev mode, and it violates PLAN rule 5 for each new kind. Use it **only for leaf components** hosted inside AppKit/UIKit, after M4.
4. **Metadata.**
   - P: the libclang probe (`apple_objc_dump.py`) extended to emit `svelters-element-kind/v0` (`toolkit: appkit|uikit`), run on a macOS runner.
   - M3: evaluate a JSON emitter in objc2's header-translator.
   - Annotations as in §3.6. A new `content: view_controller` value and a `via: target_action | notification | delegate | control_event` field on signals are the only schema additions.
5. **Prototype effort (P-style: window, stack, label, button, text field; counter + keyed-list fixtures).** Estimates for one engineer who knows Rust but not much Cocoa:

   | Piece | Effort |
   | --- | --- |
   | `svelters-apple` core: run loop + `wake` + `schedule_wake` + autorelease + handle table | 3–4 days |
   | Dynamic bridge (KVC props, target-action, one delegate: `windowShouldClose:` via `call_sync`) | 3–4 days |
   | AppKit kinds: `NSWindow` (content slot), `NSStackView`, `NSTextField` label/field (`input` via notification), `NSButton` | 2–3 days |
   | Metadata probe → v0 JSON for those 5 kinds + annotations + in-process verification tests on a macOS runner | 3–4 days |
   | CI: macOS runner, fixture harness, screenshot | 2 days |
   | **macOS subtotal** | **~2.5–3.5 weeks** |
   | UIKit on top: scene lifecycle `run`, `.app` packaging for the simulator, `UIStackView`/`UILabel`/`UIButton`/`UITextField`, simctl fixture runner | +1.5–2 weeks |

6. **Risks.**
   - **No Mac in the loop.** Every GUI fact needs a macOS runner, and annotation-loop latency goes from seconds (Xvfb) to minutes (CI round trip). Mitigation: batch annotation runs, or a self-hosted Mac mini.
   - **SDK licence.** Ingestion must run on Apple hardware to be clean (§3.5).
   - **Delegate semantics.** There's a single weak delegate slot, `respondsToSelector:` caching, and inverted veto conventions. These are the most likely source of "events silently stop" bugs, so the generated tests must cover every delegate-backed signal.
   - **Auto Layout ambiguity and performance.** Unsatisfiable constraints are logged, not thrown. The backend should turn `NSLayoutConstraint` breakage logs into dev diagnostics. Large dynamic lists need M5's cell reuse, not stack views.
   - **iOS lifecycle mismatch.** On iOS the app does not own window creation or app termination (scenes come and go, the process is suspended). Svelte `<Window>` semantics need a platform note, and `quit` is a no-op on iOS.
   - **objc2 0.x churn.** Pin versions. The M6 release bindings depend on its API.
   - **App Store review.** The dynamic bridge plus a bundled JS bundle is fine (NativeScript precedent). Runtime-downloaded JS with native reach is not (4.7.2). Keep hot reload dev-only.

## 8. JavaScriptCore (JavaScriptCore.framework) as the JS engine on Apple platforms

> **Added on request (owner topic, 2026-10-17).** This covers JSC **instead of QuickJS** behind the `JsEngine` trait (`crates/svelters-host/src/engine.rs`). It is independent of which GUI backend is used.

### 8.1 JIT availability (checked against primary sources where possible)

| Platform / packaging | JIT for a JSContext created by *our* process? | Evidence |
| --- | --- | --- |
| **iOS / iPadOS, App Store or ad-hoc** | **No: LLInt interpreter only.** JIT needs writable+executable memory, which the kernel gates on the Apple-only `dynamic-codesigning` entitlement. `WKWebView` gets JIT because its JS runs in Apple's `com.apple.WebKit.WebContent` process, which has the entitlement, not in the app process. The EU alternative-browser-engine program (BrowserEngineKit) grants JIT only to browser apps for their own engine's content extensions. It doesn't apply to an app using `JSContext` | Apple DTS on the developer forums ("iOS does not allow general apps to generate code on the fly"; the only exception is third-party browsers); Skip's `skip-script` docs (JSC "noticeably slower on iOS" because JIT is blocked without a special entitlement); the iOS security guide quoted on vm-dev. **Not tested on a device here.** |
| **macOS, not hardened** (dev builds, `cargo run`) | **Yes**, full tiering (LLInt → Baseline → DFG → FTL) | standard JSC behaviour; no entitlement is needed without the hardened runtime |
| **macOS, hardened runtime / notarized** | **Only with `com.apple.security.cs.allow-jit`.** Without it, `mmap(MAP_JIT)` fails. Apple DTS (Quinn) explicitly advises setting it "if you need to run a JavaScriptCore context inside a hardened runtime app". *Unverified:* whether JSC then falls back to the LLInt silently or aborts. Plan on setting the entitlement | Apple "Hardened Runtime" docs; forum thread 674497 |
| **Mac App Store (sandboxed)** | `allow-jit` is a hardened-runtime entitlement that is compatible with App Sandbox, and browsers ship in the MAS with it. *Believed allowed; confirm in review before relying on it* | — |
| **Linux (WebKitGTK `javascriptcoregtk-4.1` 2.52.3, installed here)** | Yes, and switchable at runtime with `JSC_useJIT=false` (env options are honoured in release builds) | measured below |

**Benchmarks (run here: x86-64 Linux, same machine, Octane kernels, higher = better).**

Engines:

- QuickJS-NG 0.16.2: the exact sources vendored by `rquickjs-sys 0.14.0`, `qjs` built Release.
- JSC: WebKitGTK 2.52.3 through a 60-line C-API runner (`apple/bench/jsc_run.c` in the scratchpad), run with `JSC_useJIT=false` (LLInt only) and with JIT on.

| Kernel | QuickJS-NG | JSC LLInt (no JIT) | LLInt / QJS | JSC JIT | JIT / QJS |
| --- | --- | --- | --- | --- | --- |
| Richards | 135 | 244 | 1.8× | 7706 | 57× |
| DeltaBlue | 106 | 172 | 1.6× | 7934 | 75× |
| RayTrace | 247 | 394 | 1.6× | 21069 | 85× |
| Splay (GC) | 1124 | 1850 | 1.6× | 13763 | 12× |
| NavierStokes | 306 | 558 | 1.8× | 11233 | 37× |

| Startup + `print(1)` | wall | max RSS | binary |
| --- | --- | --- | --- |
| QuickJS-NG `qjs` | 6 ms | 13.7 MB | 1.5 MB (static) |
| JSC LLInt | 32 ms | 36.3 MB | 37.6 MB `.so` (system framework on Apple: 0 bytes in our app) |
| JSC JIT | 27 ms | 39.6 MB | same |

Reading:

- On iOS, JSC's interpreter is about **1.6–1.8× QuickJS-NG** on compute-bound code. That's useful but not transformative.
- For a UI app the hot path is Svelte's effect graph plus ~1 µs ABI calls, and both engines are fast enough for P-scale UIs.
- The JIT gap (12–85×) only exists on macOS (and Linux), where a Cranelift-JIT QuickJS would be competing too.
- JSC costs ~25 ms and ~23 MB more at startup. On Apple, though, it adds **no binary size**, because it's a system framework (QuickJS adds ~1 MB to the app).

### 8.2 Event-loop integration and the `JsEngine` mapping

Facts measured with the C API on Linux (same C API as Apple's framework):

```text
eval #1 queues Promise.then, returns  → job already ran:       ["sync","job1"]
JSObjectCallAsFunction(f) returns     → job already ran:       [..., "call","job-from-call"]
inside an eval, before it returns     → job not yet run:       []
native callback → JSObjectCallAsFunction (nested, JS on stack) → nested job NOT run until the
outermost API call returns:           ["nested-handler"] then ["nested-handler","job-queued-in-nested"]
```

- **Microtasks are drained automatically when the outermost API entry returns.** The drain happens when JSC's API lock depth drops to 0. There is **no public way** to defer it or bound it. The `MicrotaskDelayScope` / microtask-queue symbols exist in `JavaScriptCore.tbd`, but they're C++ internals, not API.
  - Our `run_jobs(max)` becomes a no-op (`JobStatus { ran: 0, pending: false }`). The per-checkpoint job budget (`max_jobs_per_checkpoint`) cannot be enforced, so a runaway promise loop hangs inside the drain, with only the execution-time limit (8.3) to stop it.
  - Each `dispatch()` from the core is a top-level call, so **Svelte flushes after every single event**, not once per checkpoint. Fix: add `__svelters_dispatch_batch([[h, sig, args], …])` and have the JSC engine deliver all queued events in **one** top-level call. That gives one drain per checkpoint, the same as QuickJS's steps 1–2.
- **`call_sync` interaction.**
  - When a sync handler arrives while JS is on the stack (JS op → toolkit emits synchronously), JSC does not drain, which matches the signals-note rule.
  - When it arrives from the idle loop (user clicks close → `windowShouldClose:`), the `call_sync` *is* the outermost call, so effects **run before it returns**.
  - The core's existing rule still holds: ops issued in a sync scope are queued, not applied. So this is a semantic difference only in *when* effects run, not in what reaches the toolkit. Acceptable, but write it down in the `call_sync` design.
- **Timers.** JSC provides no `setTimeout`. The core's timer heap stays, with the engine binding calling `timer_*`. Under ABI v2 the backend's `schedule_wake` is a `CFRunLoopTimer`, which is exactly the planned design. There's no conflict.
- **JSC's own run-loop use.** "By default, calling `JSContextGroupCreate` will use the run loop of the thread it was called on" (`JSContextRef.h`). JSC schedules GC/heap timers (eden/full collection, incremental sweeper) and WASM/`Atomics.waitAsync` tasks as CFRunLoop timers on that run loop. Consequences:
  1. Create the context group on the **main thread** after the backend initializes, so GC work interleaves with UI events in common modes.
  2. If our process blocks the main run loop (a nested modal), GC timers still fire inside the nested loop, because CFRunLoopTimers in common modes run there. That's harmless.
  3. On Linux/WebKitGTK the same timers go through the GLib main context, which fits the GTK backend.
- **Thread affinity.**
  - Contexts in one `JSContextGroup`/`JSVirtualMachine` share a heap and a lock. The C API takes the lock internally on each call, so calls from any thread are serialized.
  - We use one thread (main) anyway, which is what PLAN rule 6 requires.
  - `wake` from host threads stays a backend concern, with no JS involved.
- **Modal/nested loops.** These are the same as QuickJS: the core already defers a checkpoint that arrives while JS runs. JSC adds the auto-drain difference above.

**Sketch of a JSC `JsEngine`** (Rust, over the C API; on Apple via `objc2-javascript-core` 0.3.2, which already binds `JSContextRef.h`/`JSObjectRef.h`, or via a 300-line hand `-sys`; on Linux via `javascriptcore-rs-sys` 2.0 or the same hand `-sys` linked to `javascriptcoregtk-4.1`):

```rust
struct JscEngine {
    group: JSContextGroupRef, ctx: JSGlobalContextRef,
    dispatch_batch: JSObjectRef,              // JSValueProtect'ed; __svelters_dispatch_batch
    timers: HashMap<u32, JSObjectRef>,        // protected callbacks, unprotect on clear/fire-once
    errors: Vec<JsError>, core: *const Core,  // JSClassDefinition private data → op_* calls
}
impl JsEngine for JscEngine {
    fn load_module(&mut self, entry: &Path) -> Result<(), JsError> {
        // classic script: Vite `formats: ['iife']` (or wrap the ESM, see 8.3); sourceURL = entry
        eval(self.ctx, &read(entry)?, entry)   // microtasks drained on return
    }
    fn dispatch(&mut self, h: u64, sig: &str, args: &[Value]) -> Result<(), JsError> {
        // core change: prefer dispatch_many(&[(h, sig, args)]) so one call = one drain
        call(self.ctx, self.dispatch_batch, &[to_js_array(h, sig, args)])
    }
    fn fire_timer(&mut self, id: u32) -> Result<(), JsError> { call(self.ctx, self.timers[&id], &[]) }
    fn run_jobs(&mut self, _max: usize) -> JobStatus { JobStatus::default() } // JSC drained already
    fn take_errors(&mut self) -> Vec<JsError> { std::mem::take(&mut self.errors) }
}
```

What doesn't fit the trait as written:

- `run_jobs` has no meaning.
- `dispatch` should be batched.
- Interrupts (`ErrorKind::Interrupted`) and the memory limit (`ErrorKind::OutOfMemory`) have no public API (8.3).
- `UnhandledRejection` needs the SPI `JSGlobalContextSetUnhandledRejectionCallback`, or a JS-side polyfill that wraps `Promise` (imperfect).

### 8.3 Practicalities

- **ES modules.** The public C API and ObjC API have **no module loader**. `JSScript`/`JSScriptEvaluate`/`JSModuleLoaderDelegate` are exported (they're in `JavaScriptCore.tbd`) but not in the SDK's public headers, so they're SPI.
  - Our bundles are already self-contained. The counter fixture's `dist/app.js` (146 KB) has **no `import`/`export` and no top-level await**. Wrapped as `"use strict"; (() => { … })();`, it parses and runs in JSC up to `globalThis.__svelters_host is not defined`, i.e. it works.
  - Cleaner: `packages/build` `lib.formats: ['iife']` (Vite/rolldown supports it) for a JSC build profile, plus a lint that the bundle has no top-level await. Source maps then need no offset fix.
- **Stack traces / source maps.**
  - `Error.stack` in JSC is `fn@url:line:col` lines (Safari format, not V8's `at fn (url:line:col)`). The `sourceURL` passed to `JSEvaluateScript` names the file.
  - The M2 source-map translator must parse both formats. It's small, but it's one more format.
- **Debugging.**
  - Apple: `JSGlobalContextSetInspectable(ctx, true)` (public; macOS 13.3+/iOS 16.4+) makes the context appear in **Safari → Develop**, including on a USB- or Wi-Fi-connected iPhone: breakpoints, console, profiler, with our source maps. This is a real advantage over QuickJS, which has no debugger.
  - Linux: WebKitGTK exports the same function plus `JSRemoteInspector*`, and the WebKitGTK remote inspector works over `WEBKIT_INSPECTOR_SERVER`.
- **Interrupts, memory limits.**
  - `JSContextGroupSetExecutionTimeLimit(group, seconds, callback, ctx)` is a watchdog that terminates runaway scripts. It's **SPI on Apple**: exported in `JavaScriptCore.tbd` and in WebKitGTK's `.so`, but absent from the public headers.
  - Calling SPI risks App Store rejection (guideline 2.5.1, public APIs only). Note that React Native's JSC executor historically avoided it.
  - There's no heap limit API at all.
  - So the core's `interrupt` flag and `memory_limit` config work only on QuickJS. On JSC: dev builds can use the SPI (or `dlsym` it), and release builds lose the 1 s sync-handler watchdog (signals note §1.5 item 7).
- **Rust bindings.**

  | Crate | Status |
  | --- | --- |
  | `objc2-javascript-core` 0.3.2 | generated from the Apple SDK headers: C API and ObjC API. Apple only |
  | `javascriptcore-rs(-sys)` 2.0.0 (tauri-apps, 28 M downloads, updated 2026-09) | binds WebKitGTK's **GObject** `JSCContext` API (`jsc_context_*`), plus whatever C API the sys crate exposes. Linux only, and the GObject API doesn't exist on Apple |
  | `rusty_jsc` 0.1.0 (wasmer, 2023) | C API, both platforms; stale |
  | `javascriptcore`/`-sys` 0.0.6 (endoli, 2024) | C API, small; stale |

  Recommendation: our own thin `svelters-jsc-sys` over the ~40 C functions we need, using the **C API only**, because it's identical on Apple and WebKitGTK. Then the same engine code runs on this Linux machine and on Apple.
- **React Native's history.**
  - RN used the system JSC on iOS and a bundled JSC on Android.
  - Facebook built **Hermes** (2019) for Android startup time and memory: precompiled bytecode means no parse at launch, and there's no JIT anyway.
  - It became the default on both platforms in RN 0.70 (2022). JSC was moved out of core into `@react-native-community/javascriptcore` in 0.79–0.81.
  - The reasons map onto our situation: iOS JSC has no JIT, so the JIT advantage is moot on mobile; startup (bytecode) and memory dominate; and one engine everywhere means one set of bugs and one debugger story (Hermes has CDP).
  - QuickJS also has bytecode precompilation (`qjsc` / `JS_WriteObject`), so a QuickJS-everywhere plan already gets RN's main Hermes benefit.
- **Prototyping on Linux.**
  - **Yes, today.** `javascriptcoregtk-4.1` 2.52.3 is installed (`webkit2gtk-4.1` package). It ships the Apple-compatible C API headers (`<JavaScriptCore/JavaScript.h>`) and exports `JSGlobalContextSetInspectable` and the SPI listed above.
  - The JIT is on by default and `JSC_useJIT=false` emulates iOS. `javascriptcoregtk-6.0` is not installed.
  - A `JscEngine` behind a cargo feature can run the existing fixture harness here, against the log backend and the GTK backend, before anyone touches a Mac.

### 8.4 Recommendation and effort

- **Don't replace QuickJS with JSC.** Make JSC an **optional engine behind `JsEngine`** (`--features engine-jsc`), worth doing **after P5/M2** for two reasons:
  1. **Safari Web Inspector debugging** on macOS/iOS devices (public API).
  2. **macOS JIT** for CPU-heavy apps, with zero binary cost.
  
  On iOS it buys only ~1.7× interpreter speed, at +25 ms startup, +23 MB RSS, a different microtask model, and no public watchdog or memory limit.
- **Effort:**
  - ~1.5–2 weeks for an engine at parity with `quickjs.rs` (564 lines): `-sys` bindings, value conversion, host-op functions via `JSObjectMakeFunctionWithCallback`, timers, error/stack capture, the `dispatch_batch` core change, an IIFE build profile, and the fixture harness green on Linux.
  - Another ~3–5 days for Apple-side bring-up (link the framework, inspector, `allow-jit` entitlement on macOS) once a Mac CI runner exists (§6).
- **Versus the Cranelift JIT idea (`docs/notes/quickjs-jit-cranelift.md`):**
  - A QuickJS JIT is **unusable on iOS** (no `MAP_JIT` for third-party apps). The same is true of JSC's JIT, so iOS is interpreter-only whichever engine we pick. On iOS the choice is QuickJS (smaller, controllable job pump, interrupts) vs JSC LLInt (~1.7× faster, inspector).
  - On macOS, JSC's JIT is free (already written, maintained by Apple), while a Cranelift JIT for QuickJS is a research project that also needs `allow-jit` under hardened runtime.
  - If "faster JS on macOS" becomes a real requirement, **JSC-on-Apple is much cheaper than a Cranelift JIT**. The Cranelift work only makes sense for Linux/Windows, and there V8 (deno_core) or JSC-via-WebKitGTK are also on the table.

## Appendix: downloaded artifacts and sources

Scratchpad (not in git): `/tmp/claude-1000/-projects-svelters/10c0a3c6-7a5c-4d9b-9688-b5af0249a13a/scratchpad/apple/`

- `dl/macsdk/MacOSX26.1.sdk` (from `github.com/joseluisq/macosx-sdks` release 26.1). It includes `System/iOSSupport` (Catalyst UIKit), `.apinotes`, `.swiftinterface` and `.tbd`.
- `objc2/` (sparse clone of `madsmtm/objc2`: header-translator and framework crate configs); `gen/` (sparse `madsmtm/objc2-generated`: AppKit, UIKit).
- `ns-ios/metadata-generator` (NativeScript); `expo/packages/expo-ui/ios` (Expo UI SwiftUI).
- `out/appkit.json`, `out/uikit.json`: probe dumps. `work/hello.m`: cross-linked AppKit binary. `rsprobe/`, `rscheck/`: objc2 cross build/check.
- `bench/`: QuickJS-NG `qjs` build, `jsc_run`, Octane kernels (`dl/octane`).

Scripts (in git): `docs/notes/scripts/apple_objc_dump.py`, `jsc_capi_probe.c`, `jsc_vs_quickjs_bench.sh`.

Environment change outside the repo: added rustup targets `aarch64-apple-darwin` and `aarch64-apple-ios` to toolchain 1.97.0.

Sources:

- App Store Review Guidelines 2.5.2, 4.7, 4.7.2: https://developer.apple.com/app-store/review/guidelines/
- Hardened Runtime / `allow-jit`: https://developer.apple.com/documentation/security/hardened-runtime ; DTS on JSC under hardened runtime: https://developer.apple.com/forums/thread/674497
- No JIT for third-party iOS apps: https://developer.apple.com/forums/thread/759238 ; https://skip.dev/docs/modules/skip-script/
- UIScene lifecycle mandatory with the iOS 27 SDK (TN3187, quoted): https://developer.apple.com/forums/thread/813300
- Lynx runtime (PrimJS = QuickJS-based; JSC on iOS for the background thread): https://lynxjs.org/guide/scripting-runtime/
- Expo UI `Host` / SwiftUI: https://docs.expo.dev/versions/latest/sdk/ui/swift-ui/host.md
- objc2: https://github.com/madsmtm/objc2 (header-translator README, "We do not redistribute SDKs"); NativeScript iOS runtime: https://github.com/NativeScript/ios
- crates.io: `objc2-app-kit`/`objc2-ui-kit`/`objc2-javascript-core` 0.3.2, `javascriptcore-rs` 2.0.0, `rusty_jsc` 0.1.0, `javascriptcore` 0.0.6.
