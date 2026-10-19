# Android backend: Views vs Compose, JNI bridge, metadata (desk research)

Status: research note, 2026-10-18, written on Linux. No Android SDK/NDK/emulator was installed;
the SDK platform package (`platform-37.2_r01.zip`, 68 MB, the current stable API level) was
downloaded to the session scratchpad and parsed, and small Rust probes were type-checked.

Marks: **[verified]** = checked here (file parsed, AOSP source read, probe compiled).
**[docs]** = Android documentation. **[unverified]** = memory or secondary sources; confirm on an
emulator.

Scripts (in git):

- `docs/notes/scripts/mobile/android_api_probe.py`: parses an unpacked SDK platform
  (`android-stubs-src.jar`, `data/api-versions.xml`, `data/res/values/attrs.xml`,
  `data/annotations.zip`) and prints property/listener/attribute statistics. No JDK needed.
  `python3 -I android_api_probe.py <platform-dir> [classes…]`.
- `docs/notes/scripts/mobile/android_probe/`: the native half of the loop (eventfd + timerfd on
  the main `ALooper`, `AChoreographer_postFrameCallback64`, a JNI entry point). **Type-checks
  for `aarch64-linux-android` on Linux without the NDK** **[verified]** (Rust 1.97.0, `jni-sys`
  0.4, `ndk-sys` 0.6; linking needs the NDK).

## 0. Summary and recommendation

- **UI stack: classic `android.view` Views for the backend, with Jetpack Compose as a hosted
  subtree mode later.** The decision is closer than it was a year ago, because **at Google I/O
  2026 Google declared Android UI "Compose first"**: the View components that Compose replaces
  are in maintenance mode ("We have no plans to deprecate or remove View components", critical
  bug fixes only), and **Material Components for Android (Views) is in maintenance mode** with
  "no more planned feature releases" **[docs: Android Developers Blog 2026-05; MDC-Android
  README]**. Still:
  - Views are a retained, mutable, imperative tree with identity: the same shape as GTK/Qt/
    AppKit/WinUI and as our `create/insert/set/listen` ABI. Every JS-native-UI framework that
    renders platform widgets on Android (React Native Fabric, NativeScript, Lynx) mounts Views.
  - Views can be driven **fully dynamically** through JNI reflection (our GI equivalent), so the
    dev mode needs no per-kind compiled code. Compose cannot: composables are compiled Kotlin
    functions with compiler-plugin calling conventions.
  - Compose is itself hosted in a View (`ComposeView`), so a Views backend is never a dead end:
    Compose kinds can be added as a *subtree host* element (§1.3), exactly like SwiftUI leaves
    in UIKit. That is where new Material 3 components will have to come from.
- **Bridge: Rust + JNI (`jni` 0.22) + one small embedded Java helper**, loaded from bytes with
  `InMemoryDexClassLoader` (the Slint pattern), for listener proxies and the Activity glue.
  The app is a **thin `ComponentActivity` subclass** (androidx.activity) built by Gradle, with
  the Rust `cdylib` produced by `cargo-ndk`. Not NativeActivity/GameActivity: those exist for
  apps that render into a surface themselves.
- **Loop:** Java owns the main `Looper` (the process starts in `ActivityThread.main`). The
  backend attaches to it: `wake` = an eventfd on `ALooper_addFd`, `schedule_wake` = a timerfd on
  the same looper, `request_frame` = `AChoreographer_postFrameCallback64` (API 29). `run()`
  cannot block; the host's order inverts (`ios-uikit-backend.md` §2.2, decision A2).
- **Change notification:** Views have no generic property notification. Each bindable property
  has its own listener interface (`TextWatcher`, `OnCheckedChangeListener`,
  `OnSeekBarChangeListener`, …), most are **single-slot setters**, and unlike UIKit **most fire
  on programmatic sets too**: `TextView.setText` notifies `TextWatcher`s even for identical text,
  `CompoundButton.setChecked` notifies only on change, `SeekBar` reports clamped values with a
  `fromUser` flag **[verified in AOSP source]**. The observability facts (§3.3) share the iOS
  schema.
- **Metadata:** the SDK platform package is excellent and Linux-native: Java stubs with
  nullability, `api-versions.xml` (since/deprecated/removed per class/method/field: GIR `since`
  for free), `attrs.xml` (2189 styleable attributes with formats and enum values),
  `annotations.zip` (`@IntDef` enums, `@ColorInt`, ranges). Licence-clean on Linux (AOSP,
  Apache-2.0 sources), unlike Apple.
- **JS engine:** QuickJS-NG through the NDK (needs rquickjs `bindgen` or vendored bindings;
  none ship for Android **[verified]**). JIT is allowed on Android, so V8 is a real option there
  later; Hermes is RN-coupled.
- **Testing:** fully Linux: build with the NDK, run on the emulator on GitHub's Ubuntu runners
  with KVM (`reactivecircus/android-emulator-runner`), in-process `--selftest` via `adb shell am
  start` + logcat first, UI Automator later.
- **Effort:** ~3–4 agent-lane weeks for the slice (new crate, JNI bridge, dex helper, Gradle
  template, metadata pipeline, emulator CI). More than iOS because nothing like `svelters-apple`
  exists yet.

## 1. UI stack choice

### 1.1 The options

| | Android Views (`android.view`, `android.widget`) | Jetpack Compose |
| --- | --- | --- |
| Model | retained mutable tree, `ViewGroup.addView(child, index)`, setters, listeners | functions of state, re-executed (recomposed); nodes (`LayoutNode`) are internal, no public mutable tree |
| Fit with our ABI | direct: handle = View, `insert` = `addView`, `set_prop` = setter, `listen` = listener | inverted: a per-handle observable model (`mutableStateOf` props, `mutableStateListOf` children) rendered by a fixed `@Composable` per kind (Expo UI's pattern) |
| Dynamic dev mode | yes: `Class.forName`, `getMethod`, `invoke` over JNI; `java.lang.reflect.Proxy` for listeners | no: every kind is a compiled Kotlin adapter; reflection over composables is technically possible but relies on compiler-plugin internals (`Composer`, `$changed` masks) |
| Metadata | `android.jar` stubs + `api-versions.xml` + `attrs.xml` (§4) | Kotlin signatures (`api/current.txt` signature files in AndroidX, Kotlin metadata in `classes.jar`); default arguments' values are not in metadata |
| Status (2026) | **maintenance mode** for components Compose replaces; not deprecated, no removal planned **[docs]** | the only place new UI features land; Material 3 Expressive is Compose-first |
| Look | platform `android.widget` is dated (Theme.Material/DeviceDefault). Modern look needs MDC-Android (Views; M3 + Expressive theming as of 1.14, now in maintenance) **[secondary sources]** | current Material 3 |
| Two-way state | listener after the fact; programmatic sets echo (§3) | **controlled** components: `TextField(value, onValueChange)` shows nothing new unless state is written back; no echo for programmatic writes |
| Styling | setters + compiled XML styles/themes; no runtime stylesheet | `Modifier` chains + `MaterialTheme`: maps naturally onto a style vocabulary |
| Binary/startup cost | zero (framework is on the device) | Compose runtime + UI + Material in the APK (several MB **[estimate]**), first-frame cost mitigated by baseline profiles |
| Prior art driving it from JS | React Native (Fabric mounts Views), NativeScript, Lynx | Expo UI `@expo/ui/jetpack-compose` (alpha, components inside a `Host`) |

### 1.2 Why Views first

1. **Architecture fit.** Our renderer emits fine-grained ops against identities. Views take them
   1:1. Compose would add a second diffing engine (Svelte computes ops → we mutate snapshot state
   → Compose recomposes), the SwiftUI problem from `apple-gui-backend.md` §1.3.
2. **Codegen and dev mode.** PLAN rule 5 (new kinds without recompiling) holds for Views via
   reflection; for Compose every kind and every modifier needs a compiled adapter. A pre-built
   adapter catalogue (all of `material3` + `foundation` generated into one AAR) would restore
   most of the dev experience, but it is a project of its own.
3. **Imperative surface.** Focus, selection, scroll position, IME actions, measuring: plain
   methods on Views; in Compose each needs a state holder (`FocusRequester`, `ScrollState`,
   `TextFieldState`) plumbed per adapter.
4. **Maintenance mode is not removal.** The platform widgets and MDC still work, receive
   critical fixes and will run for years; RN's whole Android ecosystem depends on them.

What maintenance mode does change: **new components arrive Compose-only**. So the Android
backend should plan Compose hosting from the start, rather than treat it as a SwiftUI-style
afterthought.

### 1.3 Compose as a hosted subtree (later milestone)

- A `compose-host` element (a `ComposeView`, i.e. a View) whose descendants are **Compose
  kinds** rendered in one composition. Each Compose-kind handle is a Kotlin model object
  (`SvNode(kind, props: SnapshotStateMap, children: SnapshotStateList)`), and one generated
  `@Composable fun Render(node)` dispatches on kind to a generated adapter that calls the real
  composable (`Button(onClick = { post(node, "click") }) { Children(node) }`).
- Ops on Compose handles become snapshot writes, applied in one
  `Snapshot.withMutableSnapshot { … }` per checkpoint, so one checkpoint = one recomposition.
- Identity: children rendered in `key(handle) { Render(child) }`.
- Two-way: adapters write user values into the model first (controlled component), then post
  the event; programmatic sets never echo.
- Observation of Compose-owned state (`TextFieldState`, `LazyListState.firstVisibleItemIndex`,
  `ScrollState.value`): `snapshotFlow { state.value }` collected on the main dispatcher, or one
  global `Snapshot.registerApplyObserver` that maps changed state objects to handles.
- Requires the host Activity to provide the ViewTree owners (Lifecycle, SavedStateRegistry):
  `ComponentActivity` does, which is one reason to use it from the first slice (§2.3).
- Prior art: Expo UI's `Host` + Jetpack Compose components (alpha). Effort: comparable to the
  SwiftUI leaf work, ~3+ weeks for a useful catalogue **[estimate]**. Decision A1.

## 2. Bridge, threading, packaging

### 2.1 Rust ↔ JVM

- **JNI via `jni` 0.22.4** (214 M downloads, 2026-03) for typed calls, with `jni-sys` underneath.
  Every ABI op on the main thread already holds a `JNIEnv` (the main thread is attached: it is a
  Java thread). Local references must be scoped (`with_local_frame`) in every op, or a long
  checkpoint overflows the local reference table (512 on older runtimes; ART's is larger but
  still finite **[unverified]**).
- **Handles** hold a JNI **global reference** per View. `destroy` deletes it. The View tree keeps
  children alive (a `ViewGroup` holds its children), like UIKit's retain; removal does not
  destroy. The JVM GC frees a View only when no global ref and no parent hold it, so there is no
  `on_destroyed` underneath us except windows/Activities (§2.4).
- **Dynamic dev mode = reflection over JNI.** `FindClass`/`GetMethodID` by name and JNI
  signature (from metadata, or from `Class.getMethods()` at runtime), `Call*Method`. Props: the
  setter/getter pair (`setText`/`getText`), resolved by naming rules plus metadata. Cost: a JNI
  call is ~10–100 ns; method IDs are cached per (class, name) **[estimate]**.
- **Runtime introspection exists**: `android.view.inspector` (API 29) is public API, and the
  framework generates an `InspectionCompanion` per widget class from `@InspectableProperty` on
  its getters (TextView has 57, ProgressBar 21 **[verified: AOSP source]**). Mapping a View class
  through `StaticInspectionCompanionProvider` should enumerate its properties with XML attribute
  names, types and enum/flag mappings, at runtime **[unverified: needs an emulator]**. If it
  works it is our `g_object_class_list_properties`.
- **Listeners need Java classes.** JNI cannot implement a Java interface. Two ways:
  1. `java.lang.reflect.Proxy.newProxyInstance(loader, [OnClickListener], handler)`, where
     `handler` is one Java `InvocationHandler` class whose `invoke` is a `native` method:
     fully dynamic, any listener interface, ~µs per call, boxed args. **The dev-mode path.**
  2. A generated Java class implementing the annotated listener interfaces with `native`
     callbacks (`SvListeners implements View.OnClickListener, TextWatcher,
     CompoundButton.OnCheckedChangeListener, SeekBar.OnSeekBarChangeListener …`): typed and
     fast. **The release path** (M6 tree shaking keeps only used interfaces).
- **Where Java code lives**: compile the helper with `javac` + `d8` in `build.rs` and embed the
  `classes.dex` bytes in the `.so`; load them with `dalvik.system.InMemoryDexClassLoader`
  (API 26). That is what Slint's Android backend does (`android-build` crate, `javahelper.rs`)
  **[verified: Slint source]**. It keeps the Java side versioned with the Rust crate and works
  with any app template. (In-memory dex is unaffected by Android 14's rule that dynamically
  loaded code *files* must be read-only.)

Sketch of the helper (Java, ~100 lines in the real thing):

```java
final class SvBridge implements java.lang.reflect.InvocationHandler {
    static native Object dispatch(long token, String method, Object[] args); // -> host post_event / call_sync
    final long token;
    SvBridge(long token) { this.token = token; }
    public Object invoke(Object proxy, java.lang.reflect.Method m, Object[] args) {
        return dispatch(token, m.getName(), args);   // boolean-returning listeners = call_sync
    }
}
```

### 2.2 Threading and the loop

- **All View operations on the main ("UI") thread**: `ViewRootImpl.checkThread` throws
  `CalledFromWrongThreadException` for attached views **[docs]**. Our host already runs JS on
  the toolkit thread. Nothing runs on other threads except `wake`.
- **Java owns the loop.** The process starts in Java; `Looper.loop()` is already running when
  `Activity.onCreate` calls into Rust. So:

| ABI member | Android implementation |
| --- | --- |
| `run(host)` | **cannot block**: register the loop sources (below) and return "running" (a new non-blocking return, decision A2). The host stays alive in a process-global, owned by the Rust side |
| `wake()` (any thread) | `eventfd` registered with `ALooper_addFd(mainLooper, fd, 0, ALOOPER_EVENT_INPUT, cb, …)`; `wake` writes 1. Wakes coalesce in the counter; the callback reads it and runs `checkpoint` **[verified: compiles]** |
| `schedule_wake(ms)` | a `timerfd` (CLOCK_MONOTONIC) on the same looper, re-armed with `timerfd_settime`; cancel = disarm. Pure native, no `Handler` JNI round trip **[verified: compiles]** |
| `request_frame` | `AChoreographer_postFrameCallback64` (API 29; `postVsyncCallback` API 33 adds frame timelines) from the main thread **[docs]** |
| "about to block" | no public idle hook in the NDK. `MessageQueue.addIdleHandler` (Java) is the equivalent of CF's `BeforeWaiting`; ops are applied immediately anyway, so it is only needed for batching later |
| `quit` | `Activity.finish()` of the host Activity; the process may stay alive (Android decides). Never `System.exit` |

- **Nested loops**: none in normal use. Dialogs are async (`Dialog.show()` returns); there is no
  `runModal`. Nothing to mark `blocking_nested_loop`.
- **ANR**: blocking the main thread for 5 s on input dispatch produces an "Application Not
  Responding" dialog **[docs]**. The checkpoint job budget is again the tool.

### 2.3 Packaging

| Option | What it is | For us |
| --- | --- | --- |
| `NativeActivity` (platform) / `android-activity` crate | Activity implemented in C; app draws to a `Surface` | no View tree of ours to host, IME and AndroidX integration poor. For games/self-rendering toolkits (Slint uses it) |
| `GameActivity` (AGDK) | AppCompat-based Activity forwarding input/surface to native | same: designed for a `SurfaceView` renderer |
| **Thin `ComponentActivity` subclass + Rust `.so`** | `SveltersActivity extends ComponentActivity`; `onCreate` loads the library and calls `nativeStart(this, root)`; the backend builds Views into `root` | **recommended**: real View hierarchy, IME, accessibility, predictive back (`OnBackPressedDispatcher`), `enableEdgeToEdge()`, ViewTree owners for `ComposeView` |

- **Build:** `cargo-ndk` (4.1.2) builds the `cdylib` for `arm64-v8a` (devices) and `x86_64`
  (emulator on CI) into `jniLibs/`; a checked-in minimal Gradle project (`android/` template:
  `settings.gradle.kts`, one `app` module, AGP, `androidx.activity`) packages the APK/AAB and
  signs it. `cargo-apk` (last release 2023) and `xbuild` (2022) are unmaintained and target
  NativeActivity; avoid.
- **JS bundle** goes into `assets/` and is read through `AAssetManager` (NDK) or copied once to
  the app's files dir.
- **16 KB pages:** Google Play requires native libraries of apps targeting Android 15+ to be
  16 KB-aligned (deadline Nov 2025) **[secondary sources]**. NDK r28+ links 16 KB-aligned by
  default; with Rust pass `-C link-arg=-Wl,-z,max-page-size=16384` explicitly to be safe and
  check `readelf -l` `p_align = 0x4000` in CI.
- **Signing:** debug keystore for CI; release keys are the app author's. All Linux.

### 2.4 Activity lifecycle (the Android-specific trap)

- **Configuration changes recreate the Activity** by default (rotation, dark mode, font scale,
  window resize on foldables/desktop windowing). Our View tree was built with the old Activity
  as `Context`. Recommendation: declare `android:configChanges` for everything we can handle
  (`orientation|screenSize|screenLayout|smallestScreenSize|keyboard|keyboardHidden|density|
  uiMode|fontScale|locale|layoutDirection`), forward `onConfigurationChanged` as an app-level
  signal, and let Views re-resolve themselves (Compose apps commonly do the same).
- **Process death** (backgrounded app killed for memory) loses the JS heap; Android restores the
  Activity with a `savedInstanceState` bundle. Without app-level state persistence the app
  restarts from scratch. Out of scope for the slice; document it.
- **Edge-to-edge is mandatory**: apps targeting API 35 are drawn edge-to-edge, and from API 36
  the `windowOptOutEdgeToEdgeEnforcement` opt-out is disabled **[docs: Android 16 behaviour
  changes]**. The window root must apply system-bar/IME insets
  (`setOnApplyWindowInsetsListener`, `WindowInsets.Type.systemBars() | ime()`) as padding by
  default, with an opt-out attribute: the same contract as the iOS safe area.
- **Predictive back** is on by default for apps targeting API 36: `onBackPressed` is no longer
  called and `KEYCODE_BACK` is not dispatched **[docs]**. Back handling must be **declared ahead**
  (`OnBackPressedCallback.isEnabled`), not decided when the gesture happens. For JS this means a
  `close-request`-style veto cannot be a synchronous `call_sync`: it becomes a prop
  (`intercepts-back = {dirty}`) plus a `back` event after the gesture commits. This differs from
  every desktop backend; decision A5.
- **`<window>`**: the Activity's content root, one per Activity. Secondary windows are Dialogs
  (async) or further Activities (later).

## 3. Change notification and reactivity facts

### 3.1 What Views offer

- **No generic observation.** No property system, no notify signal, no KVO equivalent. Some
  setters call `notifyViewAccessibilityStateChangedIfNeeded`, but accessibility events are only
  produced while an accessibility service is on, so they are no hook.
- **Per-widget listener interfaces.** The probe over the platform stubs **[verified]**: in
  `android.view` + `android.widget` + `android.text`, 95 listener-like interfaces with 163
  methods, 62 of them returning a value (mostly `boolean` "consumed": `onTouch`, `onKey`,
  `onLongClick`, `onEditorAction`, `onQueryTextSubmit`). Those are `call_sync` candidates.
- **Mostly single slot.** `View` has 14 `setOn…Listener` setters (one listener each, replacing
  the previous) and only 3 add/remove pairs (`OnAttachStateChangeListener`,
  `OnLayoutChangeListener`, `OnUnhandledKeyEventListener`). `TextView.addTextChangedListener`
  is multi. The backend must own single-slot listeners and fan out, like Apple delegates.
- **Programmatic sets usually notify.** Unlike UIKit control events:
  - `TextView.setText` calls `sendBeforeTextChanged`/`sendOnTextChanged`/`sendAfterTextChanged`
    with **no equality check**, so `TextWatcher`s fire even for identical text (GTK Entry's
    behaviour) **[verified: AOSP `TextView.java`]**.
  - `CompoundButton.setChecked` notifies only when the value changes (`if (mChecked !=
    checked)`) and guards re-entrancy with `mBroadcasting` **[verified: AOSP]**.
  - `ProgressBar.setProgressInternal` clamps to `[min, max]` and returns early when unchanged;
    `SeekBar` then calls `onProgressChanged(seekBar, progress, fromUser)`, so programmatic sets
    arrive with `fromUser = false` and the *clamped* value **[verified: AOSP]**. Signals note
    rule 5 (user-intent signal) is available here as a flag.

Consequences for the reactivity ABI: the subscription (`handle`, `prop`) maps to installing the
backend's listener for the property's mechanism (once per object, shared by all props it
covers: one `OnSeekBarChangeListener` serves `progress` and the tracking events). Echo
suppression follows the signals note §1.6 rules 1 and 3 (suppress flag + compare before set),
except where a `fromUser` flag exists.

### 3.2 Per-property change notification (concrete widgets)

| Kind.prop | Mechanism (slot) | Fires on programmatic set? | Notes |
| --- | --- | --- | --- |
| `edit-text.text` / `text-view.text` | `TextWatcher` via `addTextChangedListener` (multi): `before/on/afterTextChanged` | **always, even identical** [verified] | bind on `afterTextChanged`; IME composition shows as composing spans in the `Editable` (check `BaseInputConnection.getComposingSpanStart`) [unverified]. Sync filter/veto: `InputFilter` (returns replacement text) |
| `switch.checked`, `check-box.checked`, `toggle-button.checked` | `CompoundButton.OnCheckedChangeListener` (single) | **on change only** [verified] | |
| `seek-bar.progress` | `SeekBar.OnSeekBarChangeListener` (single): `onProgressChanged(…, fromUser)`, `onStart/StopTrackingTouch` | **on change, `fromUser=false`, clamped** [verified] | no suppression needed: filter on `fromUser` |
| `progress-bar.progress` | none | — | display-only; `observe: none` |
| `radio-group.checked-radio-button-id` | `RadioGroup.OnCheckedChangeListener` (single) | yes [unverified] | |
| `spinner.selected-item-position` (`AdapterView`) | `OnItemSelectedListener` (single) | yes, **deferred to the next layout pass**, and once on first layout [unverified, a well-known quirk] | `timing: deferred` |
| `number-picker.value` | `NumberPicker.OnValueChangeListener` (single) | no [unverified] | |
| `date-picker` / `time-picker` | `OnDateChangedListener` / `OnTimeChangedListener` | yes [unverified] | |
| `view.focused` | `OnFocusChangeListener` (single); `ViewTreeObserver.OnGlobalFocusChangeListener` (multi, per window) | yes | |
| `view.scroll-x/-y` | `OnScrollChangeListener` (single, API 23) | yes | throttle to frames |
| `view` bounds (`width`, `height`, `left`, `top`) | `OnLayoutChangeListener` (multi) | after the layout pass | `timing: deferred` |
| `view.visibility`, `alpha`, `enabled` | **none** | — | programmatic-only; nothing to observe |
| window insets (safe area) | `OnApplyWindowInsetsListener` (single) | — | root-only, app-level signal |
| back | `OnBackPressedCallback` (androidx) / `OnBackInvokedCallback` (API 33) | — | declared ahead via `isEnabled` (§2.4) |
| Compose state (later) | `snapshotFlow` / `Snapshot.registerApplyObserver` | our own writes are known | §1.3 |

### 3.3 Observability facts (same schema as `ios-uikit-backend.md` §3.3)

```yaml
SeekBar:
  props:
    progress:            # inherited from ProgressBar; the observe fact lives on SeekBar
      observe:
        via: listener              # listener | watcher | none | (iOS: control_event | notification | delegate | kvo | override)
        source: android.widget.SeekBar$OnSeekBarChangeListener.onProgressChanged
        install: setOnSeekBarChangeListener
        slot: single               # backend owns it, fans out to every subscribed prop/signal
        user: true
        programmatic: flagged      # never | always | on_change | flagged
        flag_arg: fromUser
        timing: continuous         # sync_in_setter | continuous | end_of_gesture | deferred
        payload: value             # value (arg index 1) | none
        clamps: true
        documented: true
        end_event: onStopTrackingTouch
EditText:
  props:
    text:
      observe: { via: watcher, source: android.text.TextWatcher.afterTextChanged, install: addTextChangedListener,
                 slot: multi, user: true, programmatic: always, identical_set_fires: true, timing: sync_in_setter, payload: none }
```

Android-specific additions to the shared schema: `install` (the setter/adder), `flag_arg`, and
`identical_set_fires` (drives "compare before set"). `slot: single` facts let the generator emit
one backend-owned listener per interface.

## 4. Metadata ingestion

### 4.1 Sources (measured on `platform-37.2_r01.zip`)

| Source | Gives | Measured |
| --- | --- | --- |
| `android-stubs-src.jar` | Java source of the public API: classes, methods, inner listener interfaces, `@Nullable/@NonNull`, constants. No Javadoc | 4756 files; `android.view`/`widget`/`text`: 431 types parsed by the probe |
| `android.jar` | the same as bytecode (what the compiler links against) | not needed when the stubs are read |
| `data/api-versions.xml` | **since / deprecated / removed API level per class, method, field**, plus SDK extension levels | 7135 classes; e.g. `SeekBar since=1`, `SeekBar(Context,AttributeSet,int,int) since=21` |
| `data/res/values/attrs.xml` | `declare-styleable` per widget: XML attribute names, `format` (boolean, dimension, color, reference, enum/flag with values) | 299 styleables, 2189 attrs; `View` 123 attrs (69 share a setter's name), `TextView` 106 (46), `ProgressBar` 27 (16) |
| `data/annotations.zip` | external annotations: `@IntDef` value lists (enums/flags), `@ColorInt`, `@Px`, `@IntRange/@FloatRange`, `@DrawableRes`, `@CallSuper` | in view+widget: IntDef 236, IdRes 125, ColorInt 63, DrawableRes 46, FloatRange 36, Px 31, IntRange 26 |
| `data/widgets.txt` | the layout editor's widget list with superclass chains | 75 entries |
| AOSP full sources (`frameworks/base`, Apache-2.0) | Javadoc `@attr ref android.R.styleable#TextView_text` on setters (309 in `TextView.java`), `@InspectableProperty` on getters | read online for this note |
| Runtime `InspectionCompanion` (API 29) | per-class property names, attribute ids, enum/flag mappings | unverified (§2.1) |
| AndroidX / MDC AARs | `classes.jar` + Kotlin metadata; `api/current.txt` signature files in the AndroidX source tree (metalava format, includes nullability) | not probed |

Licence: the platform package is under the Android SDK License Agreement, but its content
(stubs, attrs, api-versions) is AOSP-derived and Apache-2.0 (the stub files carry the AOSP
header **[verified]**). Ingestion on Linux and committing derived facts is clean, unlike Apple.
Prefer fetching AOSP sources or the platform zip in CI over committing SDK files.

### 4.2 Classification into props, signals, methods

- **Props** = `setX(T)` with a matching `getX()`/`isX()` of the same type (View: 132, TextView:
  70, ProgressBar: 24 pairs **[verified]**). Overloads (`setText(CharSequence)` /
  `setText(int resId)` / `setText(CharSequence, BufferType)`) need the NON-GUI Q4 runtime
  dispatch or a preferred-overload rule (`CharSequence` beats `@StringRes int`).
- **Setter-only props** (`setOnClickListener`, `setTextAppearance(int)`): methods unless the
  styleable lists the attribute.
- **XML attribute ↔ setter**: `@attr ref` in AOSP Javadoc (authoritative), else name match
  against the class's styleable (roughly half match by name). Attribute formats give enum/flag
  values for free (`orientation: horizontal|vertical`).
- **Signals** = methods of listener interfaces installed by `setOn*Listener` /
  `add*Listener|Watcher|Callback`, named `<method without "on">` in kebab-case
  (`onCheckedChanged` → `checked-changed`, `afterTextChanged` → `after-text-changed`,
  `onClick` → `click`). Non-void methods = `call_sync` with the `boolean` "consumed/handled"
  result mapped to `preventDefault` (annotated per method, as AppKit delegates).
- **Children**: `ViewGroup.addView(View, int)` / `removeView` on every `ViewGroup`; container
  roles are annotation (as for `NSView`): `LinearLayout` (ordered, `layout_weight`/`gravity` as
  child attributes through `LinearLayout.LayoutParams`), `FrameLayout`, `ScrollView` (one
  child), `RecyclerView`/`ListView` (adapter: M5).
- **Child attributes** = `LayoutParams` fields per container (`layout_width`, `layout_weight`,
  `layout_gravity`, margins), from the `ViewGroup_Layout` / `LinearLayout_Layout` styleables.
  Maps onto `set_child_attr` (X3).
- **Since** from `api-versions.xml` exactly like GIR `since` + FG1's runtime version gates
  (`Build.VERSION.SDK_INT`).
- Naming (N1): tag = simple class name in kebab-case (`linear-layout`, `seek-bar`, `edit-text`).

## 5. JS engine on Android

- **QuickJS-NG through the NDK.** Plain C, builds with the NDK clang through `cc`. rquickjs-sys
  0.14.0 ships no Android bindings **[verified]**; use its `bindgen` feature with the NDK sysroot
  (works on Linux CI: the NDK is Linux-native) or vendor generated bindings
  (`aarch64-linux-android`, `x86_64-linux-android`), and upstream them. Lynx runs PrimJS
  (QuickJS fork) on Android's main thread **[docs]**.
- **JIT is allowed on Android** (V8 in apps works, e.g. Javet/J2V8 **[unverified for current
  SELinux policy]**). So V8 via deno_core-style embedding is the realistic "faster engine"
  option on Android, unlike iOS. Not for the slice.
- **Hermes**: bytecode-precompiled, interpreter, made for Android startup. Its embedding API is
  JSI (C++) and the build is RN-coupled; no Rust bindings worth using **[unverified]**. Not
  recommended; QuickJS bytecode (`JS_WriteObject`) gives the startup benefit.
- **Google Play and interpreted code**: downloading executable code (dex, JAR, `.so`) is
  forbidden, but the rule "does not apply to code that runs in a virtual machine or an
  interpreter where either provides indirect access to Android APIs (such as JavaScript in a
  webview or browser)"; interpreted code loaded at run time must not enable policy violations
  **[docs: Device and Network Abuse]**. Bundled JS is fine; dev reload stays a dev feature.
- **Dev loop**: `adb reverse tcp:5173 tcp:5173` and the network module loader from
  `plans/research/QUICKJS-RUNTIME.md`; works on emulator and USB devices.

## 6. Testing and CI

All Android tooling runs on Linux, so no licence problem and no macOS minutes.

| Layer | How |
| --- | --- |
| `cargo check --target aarch64-linux-android` | no NDK needed **[verified]** |
| Build `.so` + APK | NDK + SDK command-line tools + Gradle on `ubuntu-latest` (`android-actions/setup-android` or the preinstalled SDK on the runner image) |
| Metadata | `android_api_probe.py`-style ingest from the platform zip (cached), deterministic rerun check, as GTK/Qt |
| Emulator | `reactivecircus/android-emulator-runner@v2` on `ubuntu-latest` with KVM. Hardware acceleration works on standard 2-vCPU Linux runners since April 2024 after a udev step that opens `/dev/kvm` to the runner user **[docs: GitHub changelog, action README]**. Use an `x86_64` Google APIs image of the target API level |
| First test vehicle: `--selftest` | `adb install`, `adb shell am start -n dev.svelters.counter/.SveltersActivity --es svelters.selftest counter`, the app runs the scripted fixture in-process, prints a JSON report to logcat, `finish()`es; the job greps logcat and pulls `adb exec-out screencap -p` |
| Generated per-annotation tests | in-process inside the same app (`--es svelters.selftest generated`): create/insert/read back/remove/observe facts on the main thread, exactly the macOS pattern |
| Real input | UI Automator (`uiautomator.jar` ships in the platform) or `adb shell input tap/text`; views found by resource name: set `AccessibilityNodeInfo.setViewIdResourceName("sv:id/<testid>")` in an `AccessibilityDelegate` (Compose's `testTagsAsResourceId` does the same) **[unverified for Views]** |
| Instrumented tests (`./gradlew connectedCheck`, Espresso) | possible, not needed first |

Environment change outside the repo: rustup targets `aarch64-linux-android`, `aarch64-apple-ios`
and `aarch64-apple-ios-sim` added to toolchain 1.97.0 for the probes. Downloads (platform zip,
AOSP files) are in the session scratchpad only.

Local development: this WSL2 machine has no `/dev/kvm`, so no local emulator here; a USB device
via `adb` or CI is the loop. On a Linux desktop with KVM the emulator runs locally.

## 7. Styling (brief)

- Android's styles and themes are **compiled resources** (`res/values/styles.xml` → `R.style.*`
  via aapt2). A theme can't be created at run time; only compiled ones can be applied
  (`ContextThemeWrapper`, `setTheme` before `setContentView`, per-View `defStyleAttr` at
  construction). So like AppKit/UIKit/WinUI, the portable style layer maps to **setters**:

| Portable | View setter |
| --- | --- |
| `color` | `setTextColor(ColorStateList)` |
| `background-color`, `border-radius`, `border` | `setBackground(GradientDrawable)` with color/cornerRadius/stroke, or `setBackgroundTintList` to keep the theme's shape |
| `padding` | `setPaddingRelative` (px from dp: `TypedValue.applyDimension`) |
| `margin`, `flex-grow`, `align-self` | child attributes on `LayoutParams` (`MarginLayoutParams`, `layout_weight`, `layout_gravity`) |
| `gap`, `flex-direction` | `LinearLayout.setOrientation`; no native `gap`: per-child margins, or a transparent divider drawable with `setShowDividers(SHOW_DIVIDER_MIDDLE)` |
| `font-size`, `font-weight`, `font-family` | `setTextSize(TypedValue.COMPLEX_UNIT_SP, …)`, `setTypeface(Typeface.create(family, weight, italic))` (API 28) |
| `opacity`, `visibility`, `elevation` | `setAlpha`, `setVisibility`, `setElevation` |

- **Pseudo-classes map to `ColorStateList` states**: `:disabled` → `-android:state_enabled`,
  `:active` → `state_pressed`, `:checked` → `state_checked`, `:focus` → `state_focused`. A colour
  rule with pseudo-classes compiles into one `ColorStateList` per property: Android's native
  equivalent of CSS state selectors, and a better fit than anything UIKit has.
- **Units**: CSS `px` = Android `dp`; `sp` for text so the user's font scale applies.
- **Tokens / dark mode**: the app theme should be `Theme.Material3.DayNight` (MDC) or
  `Theme.DeviceDefault.DayNight`, with dynamic colour (Android 12+ wallpaper colours) from
  `DynamicColors`. `--sv-*` tokens resolve per `uiMode` on `onConfigurationChanged`.
- **Compose subtree**: modifiers and `MaterialTheme` take the same vocabulary (`Modifier.padding`,
  `background`, `clip(RoundedCornerShape)`), which is easier than Views.

## 8. Proposed first slice: AND1 (mirrors MAC1/WIN1)

Platform `android.widget` only (no MDC), `ComponentActivity` host, minSdk 29, target/compile
SDK 37.

| Element (tag) | Class | Props (first cut) | Signals | Slot / child attrs |
| --- | --- | --- | --- | --- |
| `window` | the Activity's content root (`FrameLayout` from `nativeStart`) | `title` (activity title), `background-color`; attr `ignores-insets` | `back` (+ `intercepts-back` prop), `configuration-changed`, `insets-changed` | `content` (one, padded by system-bar + IME insets) |
| `linear-layout` | `LinearLayout` | `orientation`, `gravity`, `padding`, `show-dividers` | — | `children` (`addView(child, index)`); child attrs `layout-weight`, `layout-gravity`, `layout-width/height` (`match-parent`/`wrap-content`/dp), margins |
| `text-view` | `TextView` | `text`, `text-size`, `text-color`, `gravity`, `max-lines` | — | — |
| `button` | `Button` | `text`, `enabled` | `click` (`OnClickListener`) | — |
| `switch` | `Switch` (platform; `SwitchMaterial` later) | `checked`, `text`, `enabled` | `checked-changed` (bind:checked) | — |
| `seek-bar` | `SeekBar` | `progress`, `min` (API 26), `max` | `progress-changed` (bind:progress via `fromUser`), `start-tracking-touch`, `stop-tracking-touch` | — |
| `edit-text` | `EditText` | `text`, `hint`, `input-type`, `ime-options`, `single-line` | `after-text-changed` (bind:text), `editor-action` (sync, returns handled) | — |
| `progress-bar` | `ProgressBar` (horizontal style needs `defStyleAttr = android.R.attr.progressBarStyleHorizontal` at construction) | `progress`, `max`, `indeterminate` | — | — |

Construction-time style is a real wrinkle: the horizontal `ProgressBar` and several widget
variants are chosen by `defStyleAttr` in the constructor, not a setter. That needs a
`construct`-time attribute (ABI v2.5 `construct` already exists) or kind aliases
(`progress-bar-horizontal`). Decision A6.

Deliverables, in order:

1. `crates/svelters-android`: JNI bridge (global-ref handle table, reflection props/methods,
   `Proxy` listeners, `InMemoryDexClassLoader` helper), looper integration (probe code), the
   window root with insets and back callback, observe facts for the slice props.
2. Host change shared with iOS: non-blocking `run` + `started` hook (A2/I1).
3. `tools/metadata/android/` ingest from the platform zip (stubs, api-versions, attrs,
   annotations) → `metadata/android/*.json` + overrides; generator for tables and tests.
4. `android/` Gradle template + `examples/counter-android` (`cargo-ndk` → `jniLibs`), `--selftest`
   intent extra, bundled `fixtures/counter-android`.
5. `.github/workflows/android.yml`: metadata determinism, `cargo check`, NDK build with the
   16 KB alignment check, emulator job (KVM) running selftest + generated tests + screenshot.

Acceptance: counter renders on the API 37 emulator (screenshot), `bind:text`/`bind:checked`/
`bind:progress` round-trip without echo loops (identical `setText` does not re-enter JS),
clamped `progress` converges, predictive back reaches JS as `back`, generated tests green.

## 9. Risks

- **Views are in maintenance mode.** Fine for years, but the modern Material look and new
  components will be Compose-only; without the Compose subtree mode (§1.3) Android apps will
  look a generation behind. Biggest strategic risk.
- **Activity lifecycle** (recreation, process death, multi-window resize) has no desktop analogue;
  `configChanges` covers most of it, process death is unsolved by design.
- **JNI correctness**: local-ref overflow in long checkpoints, global-ref leaks, exceptions
  pending after a call (every JNI call must check and clear `ExceptionCheck`, mapping Java
  exceptions to ABI errors). Mechanical but error-prone.
- **Echo on programmatic sets** is the norm (§3.1), with per-widget differences (identical-text
  `TextWatcher`, deferred `OnItemSelected`). Facts must be verified on an emulator per API
  level; behaviour changed across versions before.
- **Back/edge-to-edge semantics** differ from desktop (declared-ahead back veto, mandatory
  insets). JS APIs need a platform note.
- **Build complexity**: Rust + NDK + Gradle/AGP + Java helper. Contained by the template and
  `build.rs`, but the toolchain download (SDK + NDK ~1–2 GB) makes CI cold starts slow; cache it.
- **Platform widgets look dated**; MDC adds Gradle deps and an AppCompat/Material theme
  requirement (also maintenance mode).

## 10. Decisions needed

| # | Question | Recommendation |
| --- | --- | --- |
| A1 | Views or Compose as the Android backend | **Views** now; plan a `compose-host` subtree mode (Compose kinds with generated Kotlin adapters) as the second Android milestone, because new components are Compose-only |
| A2 | Host/loop inversion: non-blocking `run` + `started` hook | **Yes**, shared with iOS (I1) |
| A3 | App host: thin `ComponentActivity` + Gradle template + `cargo-ndk`, Java helper embedded as dex | **Yes**; not NativeActivity/GameActivity, not cargo-apk/xbuild |
| A4 | Widget set: platform `android.widget` for the slice, MDC as a widget pack later | **Yes** (MDC is also maintenance mode; Compose subtree is the modern path) |
| A5 | Back gesture semantics: declared `intercepts-back` prop + `back` event instead of a sync veto | **Yes**; document as a platform difference (also applies to iOS swipe-back via `gestureRecognizerShouldBegin` if we want parity) |
| A6 | Construction-time style variants (`defStyleAttr`): `construct` attributes or kind aliases | **Construct attributes** (`style-attr="progress-bar-style-horizontal"`), reusing ABI v2.5 `construct` |
| A7 | minSdk | **29** (Android 10: `postFrameCallback64`, `InspectionCompanion`, `Typeface.create(…, weight, …)`); 26 is possible with fallbacks |
| A8 | Configuration changes: handle in-process via `configChanges`, no Activity recreation | **Yes** |
| A9 | Observability facts schema shared with iOS (§3.3) | **Yes**, feed into REACTIVITY.md |
| A10 | Metadata source of record: platform zip (stubs + api-versions + attrs + annotations), AOSP sources for `@attr ref`; runtime `InspectionCompanion` as a cross-check | **Yes** |

## 11. Sources

- Compose-first announcement (View components in maintenance mode, "no plans to deprecate or
  remove"): https://android-developers.googleblog.com/2026/05/android-ui-development-is-compose-first.html
- MDC-Android maintenance mode: https://github.com/material-components/material-components-android (README)
- Android 16 behaviour changes (edge-to-edge opt-out disabled, predictive back default):
  https://developer.android.com/about/versions/16/behavior-changes-16
- Choreographer NDK API (`postFrameCallback64` API 29, `postVsyncCallback` API 33):
  https://developer.android.com/ndk/reference/group/choreographer
- Looper NDK API (`ALooper_addFd`): https://developer.android.com/ndk/reference/group/looper
- 16 KB page sizes: https://developer.android.com/guide/practices/page-sizes
- Google Play Device and Network Abuse policy (interpreter exemption):
  https://support.google.com/googleplay/android-developer/answer/9888379
- GitHub Actions hardware-accelerated Android emulator on standard runners:
  https://github.blog/changelog/2024-04-02-github-actions-hardware-accelerated-android-virtualization-now-available/ ;
  https://github.com/ReactiveCircus/android-emulator-runner
- Slint's Java helper (javac + d8 in `build.rs`, `InMemoryDexClassLoader`):
  https://github.com/slint-ui/slint/tree/master/internal/backends/android-activity
- Expo UI Jetpack Compose: https://docs.expo.dev/versions/latest/sdk/ui/jetpack-compose/
- AOSP sources read for listener semantics (`TextView.setText`, `CompoundButton.setChecked`,
  `ProgressBar.setProgressInternal`, `SeekBar.onProgressRefresh`, `@InspectableProperty`):
  https://android.googlesource.com/platform/frameworks/base/+/refs/heads/main/core/java/android/widget/
- SDK platform package: https://dl.google.com/android/repository/platform-37.2_r01.zip (index:
  `repository2-3.xml`)
- Lynx (PrimJS on Android): https://lynxjs.org/guide/scripting-runtime/
- crates.io: `jni` 0.22.4, `ndk-sys` 0.6.0, `android-activity` 0.6.1, `cargo-ndk` 4.1.2,
  `cargo-apk` 0.10.0 (2023), `xbuild` 0.2.0 (2022), `rquickjs` 0.14.0.
