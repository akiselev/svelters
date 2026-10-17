# Custom and third-party native widgets (research notes)

Lane: custom-widgets research (2026-10-17). Research and design only; no implementation.
Proposal: `plans/research/CUSTOM-WIDGETS.md`. Probe scripts: `docs/notes/scripts/cw_*`.

## 0. What the code does today (the baseline a pack must plug into)

- **Kind ids are dense table indices.** GTK `KINDS[id-1]` (`crates/svelters-gtk/src/generated/tables.rs`),
  props/signals/methods/slots/attrs likewise. `lookup::kind_by_name` matches tag, GType name or
  GIR name. A second table cannot be appended without an id range.
- **Ad-hoc id partitions already exist.** GTK: `vlist::KIND_VLIST`, `menus::*`, `extras::*`
  kinds checked before the generated table in `b_resolve`; methods past `METHODS.len()` are
  typelib methods (X1). Qt: method ids from `1 << 20` are universal getters. So the backends
  already route by id range; a pack is one more range.
- **GTK creates by GType name.** `create` calls `types::ensure_registered(gtype)` (the
  generated `*_get_type()` externs) then `glib::Type::from_name` + `Object::with_type`. Props and
  signals are set/connected through GObject by name; methods fall back to GIRepository. So any
  GObject type that is *registered* is reachable with only a table row: this is the existing
  escape hatch for libadwaita and for app-written GObjects.
- **Qt is fully generated.** `svelters-qt-build` runs `tools/metadata/qt/gen_cpp.py` from a
  leaf crate's `build.rs`, compiles one `svl_qt_backend.cpp` with `cc`, links Qt via pkg-config.
  `Config` has `metadata_dir`/`overrides_dir` but only one of each, and no extra
  include/lib/source hooks. There is no QMetaObject-reflective path for props or signals
  (only `metaObject()->className()` for styling and diagnostics).
- **Tags.** `element.tag` in `metadata/<tk>/*.json` (today `button`, `headerbar`); N1 renames
  to native kebab-case (`header-bar`, `main-window`). Libadwaita gets `ad-`.
- **Distribution (CLI research).** Prebuilt runners per toolkit/platform; `add native` ejects
  a leaf crate and the runner becomes `runner = "path:…"`. A pack with native code therefore
  either forces a custom runner build or must be loadable into a prebuilt runner.


## 1. libadwaita (probe: `scripts/cw_adw_gir.py`, Adw 1.9.0 installed, GTK 4.22.4)

- **Size.** `Adw-1.gir` has 90 classes, 1 interface, 25 enums and 1 flags type. 62 classes are
  `Gtk.Widget` subclasses, once parents are followed into `Gtk-4.0.gir`: rows sit under
  `Gtk.ListBoxRow` and windows under `Gtk.Window`. The other 28 are GObjects that still want
  kinds: `Breakpoint`, `Toast`, `Toggle`, `TabPage`, `ViewStackPage`, `SidebarItem/Section`,
  `ShortcutsItem/Section`, the animation classes, `StyleManager` and `Application`. The
  existing GTK ingestion (`tools/metadata/gtk/ingest.py`) hardcodes `"Gtk"` in about 10 places.
  Taking the namespace as a parameter is the only change it needs. Its GIR loader already loads
  transitive includes.
- **`since` is on every class** (1.0: 47, 1.2: 5, 1.3: 3, 1.4: 10, 1.5: 4, 1.6: 7, 1.7: 5,
  1.8: 4, 1.9: 5). Six classes are deprecated (`Flap`, `Leaflet`, `Squeezer`,
  `ViewSwitcherTitle`, `*Window` dialogs, `MessageDialog`). At runtime,
  `adw_get_{major,minor,micro}_version()` gives the loaded library's version.
- **Tags under the rule.** Strip `Adw`, kebab-case the rest and prefix `ad-`:
  `ad-header-bar`, `ad-toolbar-view`, `ad-navigation-view`, `ad-toast-overlay`, `ad-breakpoint`,
  `ad-action-row`. None collides with a core tag because of the prefix. The core `header-bar`
  (N1) and `ad-header-bar` coexist.
- **Data alone is enough to reach it in dev.** GTK `create` is `glib::Type::from_name` +
  `Object::with_type`, props and signals go by name, and methods fall back to GIRepository (X1).
  So once the type is registered, an Adw kind is a table row. Registration without linking:
  `gi_repository_require("Adw","1")` loads `libadwaita-1.so.0` from the typelib's
  `shared-library` attribute. `gi_registered_type_info_get_g_type` then calls `adw_*_get_type`,
  and `adw_init` is invocable as a GI function. **A libadwaita pack needs no compiled code in
  dev.** The only per-kind code today is the generated typed slot ops (`generated/slots.rs`,
  e.g. `downcast_ref::<gtk::ActionBar>().pack_start`). A pack's slot ops would have to go
  through the dynamic path: a property set (`content`, `title-widget`) or a GI method call
  (`pack_start`, `add_top_bar`, `add_toast`, `add_breakpoint`). The X1 method invoker already
  accepts object arguments.
- **Init.** libadwaita needs `adw_init()` after GTK init. That call loads the Adwaita
  stylesheet and creates the `AdwStyleManager`. `AdwApplication` only calls `adw_init` in
  `startup` and auto-loads `style.css` from the resource base path. We keep our
  `gtk::Application` (`shared_app()`, `crates/svelters-gtk/src/backend.rs`) and have the pack's
  init call `adw_init()`. **So a pack needs an init hook.**
- **Conflict with the X6 scheme op.** `style_scheme` sets
  `GtkSettings:gtk-application-prefer-dark-theme`
  (`crates/svelters-gtk/src/backend/style.rs:274`). libadwaita warns that this is unsupported
  and expects `AdwStyleManager:color-scheme` (`FORCE_DARK`/`FORCE_LIGHT`/`DEFAULT`) instead.
  The per-provider `prefers-color-scheme` our sheets use is still fine. **So a pack must be able
  to take over one style op (SCHEME).** It should also map `--sv-*` tokens onto Adwaita's named
  colours: `@accent_bg_color`, and the CSS variables `--accent-bg-color` from 1.6.
  StyleManager also exposes `accent-color`, `high-contrast` and `dark` (read-only), which are
  natural `bind:`-able app-level state.
- **Adaptive widgets.**
  - `AdwBreakpoint` is a GObject with a boxed `condition` (`adw_breakpoint_condition_parse("max-width: 500sp")`), `apply`/`unapply` signals and `add_setter(object, prop, GValue)`. It is added with `add_breakpoint` to `AdwBreakpointBin`, `AdwWindow`, `AdwApplicationWindow` or `AdwDialog`. Those owners have a `current-breakpoint` property.
    - **Option A (Svelte-native).** `<ad-breakpoint condition=… onapply onunapply>` in a `breakpoints` slot drives `$state`, and Svelte re-renders.
      - Cost: native setters are applied inside size allocation, and an async JS handler lands one checkpoint later, which can show a one-frame flicker.
      - A `call_sync` handler (X2) runs JS synchronously, but the resulting ops apply inside allocation. Whether GTK tolerates that needs a probe.
    - **Option B (native setters).** `<ad-breakpoint-setter target={node} property="…" value={…}>` children map to `add_setter`. This is flicker-free and static.
    - **Recommendation.** Ship both, with B documented as the flicker-free path.
  - `AdwNavigationView`:
    - `add(page)` pools a page, and `push`/`push_by_tag`/`pop`/`replace` move between pages.
    - Native back gestures pop natively, so the visible page must be bindable: a `visible-page-tag` prop with notify gives `bind:` echo handling exactly like `bind:value`.
    - Pages (`ad-navigation-page`, a widget) are children in a `pages` slot.
    - `pushed`, `popped`, `get-next-page` and `replaced` are signals. `get-next-page` has a return value, so it is sync-required.
  - `AdwToastOverlay` + `AdwToast`:
    - The toast is a non-widget object (like X4 actions) and is fire-and-forget (`add_toast`, `dismiss`).
    - Two shapes:
      - a declarative `<ad-toast>` child in a `toasts` slot (insert = `add_toast`, remove = `dismiss`, `dismissed` event);
      - a helper `toast(overlay, {title, timeout, buttonLabel, onbutton})` in the pack's JS.
  - `AdwDialog` is a widget presented over a parent with `adw_dialog_present(dialog, parent)`.
    - It is not a `GtkWindow`, so the X7 toplevel rules (transient, modal via `open`) need a per-kind "presenter" annotation, not the window path.
- **Version gating.**
  - Metadata keeps `availability.since` per kind, prop, signal and method from GIR.
  - At runtime, `resolve` returns 0 with "AdwSidebar needs libadwaita 1.9; loaded 1.5" when the loaded minor is older.
  - The app states a floor in config (`[packs.adw] min = "1.4"`). The typings generator emits
    kinds above the floor as `@since` JSDoc plus an error type, so svelte-check catches them at
    build time.
  - Release thin bindings (M6) must not hard-link `get_type` symbols newer than the floor. They should use `dlsym`/GI for those, or the floor must equal the build version.
  - Flatpak pins it (GNOME 49 runtime → libadwaita 1.8).

## 2. Qt Advanced Docking System (probe: `scripts/cw_moc_json.py`)

ADS `4f4f602` (2026-09-10), version 5.1.1, LGPL-2.1. The headers were cloned into the
scratchpad and nothing was built. The probe runs `moc --output-json` (Qt 6.11 moc) on every
header with `Q_OBJECT` and libclang on the same headers.

- **moc sees signals and slots, almost no properties.**
  - The 21 QObject classes have 17 `Q_PROPERTY`s in total, 0 of them on `CDockManager` or `CDockWidget`.
  - They have 49 signals and 72 slots, 0 `Q_INVOKABLE`, and 0 meta-registered enums.
  - `DockWidgetArea` sits in a `Q_NAMESPACE` without `Q_ENUM_NS`; `SideBarLocation` uses the obsolete `Q_ENUMS`.
- **libclang sees the real API:** 591 public C++ methods.
  - On `CDockManager`: `addDockWidget(DockWidgetArea, CDockWidget*, CDockAreaWidget*, int)`, `addDockWidgetTab`, `addDockWidgetTabToArea`, `addAutoHideDockWidget(SideBarLocation, …)`, `addDockWidgetFloating`, `removeDockWidget`, `setCentralWidget(CDockWidget*)`, `saveState(int) -> QByteArray`, `restoreState(const QByteArray&, int)`, `addPerspective`, `openPerspective`, `findDockWidget(QString)`, and static `setConfigFlags`/`setConfigFlag`/`setConfigParam`.
  - On `CDockWidget`:
    - constructors `(const QString& title, QWidget* parent)` and `(CDockManager*, const QString&, QWidget*)`;
    - `setWidget(QWidget*, eInsertMode)`/`takeWidget()`, `setFeatures(DockWidgetFeatures)` and `toggleView`;
    - signals `closeRequested`, `closed`, `viewToggled(bool)`, `topLevelChanged(bool)`, `visibilityChanged(bool)` and `featuresChanged`.
- **Conclusion.** The reflection-only path (QMetaObject) would give ADS events and a few slots, but not the container API, construction or state. ADS needs **moc JSON + libclang**, plus overrides. That is the same pipeline any header-only Qt library needs.
- **Ownership facts already exist in ADS's PyQt `sip/` bindings.**
  - The annotations include `CDockWidget(const QString&, QWidget* parent /TransferThis/)`, `setWidget(QWidget* /Transfer/)`, `takeWidget() /TransferBack/` and `setToolBar(QToolBar* /Transfer/)`.
  - That is the analogue of the PySide typesystem we already ingest for Qt. When a library ships `.sip` (PyQt) or typesystem XML (PySide), they are an ownership source with `origin: sip`.
- **Container semantics for kinds:**
  - `ads-dock-manager`:
    - Constructed with a parent. If the parent is a `QMainWindow`, the constructor calls `setCentralWidget(this)` itself (`DockManager.cpp:540`). Our insert into `main-window`'s central slot must therefore tolerate an already-set central widget, or create the manager parentless and insert it normally (preferred).
    - Children are `ads-dock-widget`s, with child attributes `parent:area` (left/right/top/bottom/center → `DockWidgetArea`), `parent:auto-hide` (`SideBarLocation`), `parent:floating` and `parent:tab-of={otherDock}` (`addDockWidgetTabToArea`).
    - These map onto X3 child attributes. Insert order matters because areas are created by earlier inserts. `remove` = `removeDockWidget`.
    - A `central` slot maps to `setCentralWidget`.
  - `ads-dock-widget`:
    - `title` is construct-time; there's a `setWindowTitle` alias.
    - Its one child slot is `content`, via `setWidget`/`takeWidget`.
    - Props `features` (flags), `closable`/`movable`/`floatable` (split flags), `toggle-view` (bool) and `toolbar` (slot).
    - Native close (the user clicks ×) emits `closeRequested`/`closed`. With `DockWidgetDeleteOnClose`, ADS deletes the widget, which is `SVL_GONE_CLOSED`. Otherwise it only hides: `bind:open` with echo.
  - Areas (`CDockAreaWidget`), containers and floating windows (`CFloatingDockContainer`, which is a toplevel `QDockWidget` subclass) are made by ADS, never by the app. They reach JS as **borrowed handles** (X1) through `dockAreaCreated`/`floatingWidgetCreated` signal args or getters.
  - **State.** `saveState()` returns an XML `QByteArray` and `restoreState(bytes)` reads it back. Expose both as methods (`call('save-state') -> Uint8Array`, `restore-state(bytes)`) and add a host convenience `bind:layout`? No: layout is a blob changed by the user at drag time, with no change signal (ADS has `stateRestored` but no "layout changed"). Methods plus app-side persistence are the honest API. Restore needs every dock widget to exist first, with object names = their keys. The `id` attribute already sets `objectName` (X6 `SVL_STYLE_NAME`), so `<ads-dock-widget id="files">` is the restore key, which is a good fit.
  - **Global config.** `CDockManager::setConfigFlags` must run *before* the first manager is constructed. It is a static pack setting, `[packs.ads] config-flags = [...]` (applied in the pack's init hook), or a construct-only prop on `ads-dock-manager` applied in `create` before `new`.
- **Styling.**
  - ADS sets its own QSS on the dock manager (`stylesheets/default.css`, opt out with `DisableStylesheet`). Widget-level sheets beat our application-level X6 sheet.
  - The pack should default `DisableStylesheet` off but document it. Alternatively, it can append its sheet to the X6 sheet chain so `<style>` rules for `ads--CDockWidgetTab` win by order.
  - The QSS type selector for namespaced classes is `ads--CDockWidgetTab`: moc class names with `::` → `--` in QSS.
- **Build.**
  - ADS uses CMake. It also builds fine as a plain source list (about 25 .cpp files, moc and one .qrc), and a `cc`-crate build in a pack crate's `build.rs` can drive moc/rcc like `cxx-qt-build` does.
  - Options:
    - (a) system package (`qt6-advanced-docking-system`, available in Arch AUR, Debian `libqtadvanceddocking-qt6`?), found via CMake config or pkg-config;
    - (b) a vendored git submodule built through `cmake` crate;
    - (c) vendored sources compiled with `cc` + moc.
  - Recommend (b) as the default, with (a) behind a feature flag for distros. LGPL-2.1 requires dynamic linking or relinkable objects for proprietary apps, so build it as a shared library by default.

## 3. Qt with no codegen: what QMetaObject gives (probe: `scripts/cw_qt_reflect.py`)

- **Creating by class name is the gap.**
  - `QMetaType::fromName("QPushButton*")` is invalid, since pointer metatypes are registered lazily. Value metatypes need copyable types, so widgets can never be created through QMetaType.
  - `QMetaObject::newInstance` needs `Q_INVOKABLE` constructors, and `QPushButton` has 0.
  - `QUiLoader::createWidget(name)` works for 174 classes here: the built-ins plus every *Qt Designer plugin* on `QT_PLUGIN_PATH/designer`. A library that ships a designer plugin (`QDesignerCustomWidgetInterface::createWidget`) is creatable by name with zero glue. ADS doesn't ship one.
  - Otherwise a pack must provide a factory, which is a one-line generated or hand-written function per kind.
- **After creation, everything Q_OBJECT-declared is reflective:**
  - `QMetaProperty::read/write` with QVariant (75 props on QPushButton);
  - signals through `QMetaObject::connect` to a receiver object that overrides `qt_metacall`. This is the `QSignalSpy`/PyQt technique: one generic receiver decodes `void** args` by `QMetaMethod::parameterMetaType`;
  - slots and `Q_INVOKABLE`s through `QMetaMethod::invoke`.
- **So a Qt dev-mode "reflected kind"** = factory + the meta-object, with QVariant↔`svl_value` conversion done once in the backend.
  - This covers app-authored `Q_OBJECT` widgets well, because authors write `Q_PROPERTY`/signals.
  - It covers Qt itself partially (no plain methods), and ADS poorly (§2).
  - Today's Qt backend has no such path. Adding it is backend work that every Qt pack and app widget benefits from, and it is also the Qt analogue of GTK's GObject/GI dev path.

## 4. Custom WinUI 3 components (desk research; WIN1 is building the backend now)

- **Third-party control libraries come in two very different kinds:**
  - **C++/WinRT (native) components.**
    - Shipped as `Foo.dll` + `Foo.winmd` (+ `.pri`/`.xbf` resources).
    - The winmd is the same ECMA-335 metadata as `Microsoft.UI.Xaml.winmd`. Our `winmd_dump` (`docs/notes/scripts/winmd_dump/`) and windows-bindgen read it unchanged, provided the dependency winmds are passed for name resolution.
    - Examples are Win2D's `CanvasControl` and the unofficial `XamlToolkit.WinUI.Native` ports.
  - **C#/.NET libraries, which is where most of the ecosystem lives, including the Windows Community Toolkit.**
    - The Toolkit is "C#-based and not a Windows Runtime Component, so not directly consumable from C++" ([MS Q&A](https://learn.microsoft.com/en-gb/answers/questions/1039374/datagrid-in-winui3-with-c-winrt)).
    - From a native (Rust) process they are reachable only through a **C#/WinRT-authored wrapper component** (`CsWinRTComponent`, which emits a winmd and hosts .NET through `WinRT.Host.dll`; [MS docs](https://learn.microsoft.com/en-us/windows/apps/develop/platform/csharp-winrt/create-winrt-component-winui-cswinrt)).
    - That means a .NET runtime dependency and an MSBuild step in the pack's build. This is a real cost: a WinUI pack of the Community Toolkit is a *.NET-hosting pack*, built once by the pack maintainer, not by the app.
- **Activation of non-system types:**
  - **WinUI's own types** are activated reg-free through the Windows App Runtime (`WindowsAppRuntime_EnsureIsLoaded`; `windows-gui-backend.md` §1.2).
  - **Third-party types are not registered.** Unpackaged apps need one of:
    - (a) an application manifest with `<file name="Foo.dll"><activatableClass name="Foo.Bar" threadingModel="both"/></file>` (reg-free WinRT, the MSBuild `.targets` output);
    - (b) `LoadLibrary` + `DllGetActivationFactory(HSTRING)` directly;
    - (c) windows-rs's built-in fallback, which tries a DLL named after the namespace when `RoGetActivationFactory` returns `REGDB_E_CLASSNOTREG` (hardened in [windows-rs#3365](https://github.com/microsoft/windows-rs/pull/3365)).
  - The pack's descriptor should name its DLLs and classes explicitly, and the backend should use (b). That is deterministic, needs no manifest, and works for dlopened packs.
- **Reflection for dev mode:**
  - Every XAML library compiled by the XAML compiler carries an `IXamlMetadataProvider` (`XamlTypeInfo.g.cpp`), and an app's provider aggregates "other providers".
  - WIN1's planned dev path uses `XamlControlsXamlMetaDataProvider.GetXamlType(name)` → `ActivateInstance`/`GetMember(name).SetValue`. A pack adds **its library's provider** to the backend's provider list. Then create-by-name, set-by-name, the content property and collection add all work with no generated code.
  - Events still need the "universal delegate" or generated `add_X` connectors (`windows-gui-backend.md` §3.2), like WinUI itself.
- **User-authored UserControls and templated controls.**
  - Both need the XAML compiler (MSBuild) for `.xaml` → `.xbf` + `XamlTypeInfo`. A Rust-only app cannot author them without MSBuild.
  - They can be authored in a separate C++/WinRT or C# component project, which then becomes a pack (dll + winmd + pri).
  - For most app needs, a **Svelte composite** is the answer, because it needs nothing native (§6).
- **Metadata ingestion:** run `winmd_dump` on the pack's winmd (plus WinUI winmds for resolution) to get `metadata/winui/<Kind>.json`. The content property comes from `[ContentProperty]`, and dependency properties from `XxxProperty` statics. Overrides and ledger go per pack.
- **Tags:** `CommunityToolkit.WinUI.Controls.SettingsCard` → `ct-settings-card`, where the pack chooses the prefix. Namespaces are dropped like the `Gtk`/`Q` prefixes.

## 5. Custom UIKit/AppKit components (desk research; MAC1 is building AppKit now)

- **ObjC classes are dynamic.**
  - `objc_getClass("PSPDFView")` → `alloc/init`, then KVC for props, a generic target for actions and a runtime-built delegate for protocols (`apple-gui-backend.md` §4.1). A third-party ObjC view is reachable with **no generated code**, just like AppKit itself.
  - Linking caveat: classes from a *static* library are dead-stripped unless referenced. The pack's link step must use `-ObjC` (or `-force_load`), or the pack's registration code must reference each class symbol (`[Foo class]`).
  - Frameworks loaded dynamically (`NSBundle`/`dlopen`) register their classes on load. That is fine on macOS but not allowed for downloaded code on iOS (§2.2 there). On iOS, packs are always linked into the app binary.
- **Swift classes:**
  - A Swift `NSView`/`UIView` subclass *is* an ObjC class: `NSClassFromString("MyModule.MyView")`, or a stable name with `@objc(MyView)`. The runtime can create it.
  - Its members are visible to `respondsToSelector`/KVC only if marked `@objc` (or the class is `@objcMembers`).
  - Swift-only API needs a hand-written `@objc` shim in the pack, a few lines per member. This covers generic types, structs, enums with payloads, `async` and result builders.
  - SwiftUI views need an `NSHostingView`/`UIHostingController` wrapper class with `@objc` props that feed an `ObservableObject`.
- **Ingestion: header vs swiftinterface.**
  - ObjC libraries: the libclang pipeline MAC1 already has (`apple_objc_dump.py`) works on any framework's headers.
  - Swift libraries: **ingest the generated ObjC header** (`MyModule-Swift.h`, from `swiftc -emit-objc-header`). It contains exactly the `@objc`-visible surface, which is exactly what the runtime path can reach, and it goes through the same libclang pass.
  - `.swiftinterface` or `swift-api-digester -dump-sdk` (JSON) describe the full Swift API. That is useful only to tell an author which members need `@objc`, not for codegen.
- **Packaging:**
  - An Apple pack is an SPM package or xcframework plus metadata.
  - The backend's link step (Rust leaf crate on Apple, `cc`/`swiftc` driven by `build.rs`) adds `-framework`/`-l` flags.
  - App-authored Swift views live in the app's own `native/` Swift package, compiled into the runner.

## 6. App-authored widgets

| Toolkit | How it is written | Metadata | Dev path, no codegen | What the app must do |
| --- | --- | --- | --- | --- |
| GTK (Rust) | `glib::wrapper!` + `ObjectSubclass` in the app's `native/` crate | none from GIR; **runtime introspection** gives props (`g_object_class_list_properties`: type, flags, default, range) and signals (`g_signal_list_ids`/`g_signal_query`: params, return, flags) | yes: create by GType name, props and signals by name | make sure the type is registered before `resolve` (`MyWidget::static_type()` in the pack's init). Tags come from the GType name (`MyAppFancyGauge`, minus the app's declared prefix) |
| GTK (Vala/C) | GObject in a shared lib | `valac --gir` / `g-ir-scanner` gives GIR + typelib, so **identical to libadwaita** | yes, through the typelib | ship `.gir` + `.typelib`, add to `GI_TYPELIB_PATH` |
| Qt (C++) | `Q_OBJECT` subclass in the app's `native/` leaf crate | `moc --output-json` on the app header (the same pipeline as ADS) + libclang for plain methods | partly: reflected kind (factory + QMetaObject) once the backend has the §3 path | register a factory (`svl_qt_register("FancyGauge", []{ return new FancyGauge; })`), or let the pack generator emit it from the moc JSON |
| WinUI | C++/WinRT or C# component project (MSBuild) | its winmd | yes via its `IXamlMetadataProvider` | build the component once, list the dll in the pack descriptor |
| AppKit/UIKit | ObjC or `@objc` Swift class | headers / `-Swift.h` | yes via the ObjC runtime | link it; `-ObjC` |
| any | **Svelte composite component** | its own TS props | nothing native: an ordinary `.svelte` file built from native elements | nothing. Ship as an npm package. It is a "pack" with no native layer |

Svelte composites deserve the emphasis. Most "custom widgets" in apps are compositions: a
labelled field, a card or a toolbar group. In svelters they are ordinary components and need
no ABI, no metadata and no rebuild. Native custom widgets are needed only for custom drawing
(the X8 canvas covers much of that), custom layout or native-level performance.

## 7. Prior art (from documentation; links current as of 2026-10)

| System | Unit | How the runtime learns about it | Codegen | Dev without rebuild? | Lesson for us |
| --- | --- | --- | --- | --- | --- |
| React Native new architecture ([Fabric native components](https://reactnative.dev/docs/fabric-native-components-introduction), [Codegen](https://reactnative.dev/docs/the-new-architecture/what-is-codegen)) | npm package with `ios/`+`android/` sources and a typed JS spec (`codegenNativeComponent<Props>`) | **autolinking**: the CLI scans `node_modules` and generates a registration provider compiled into the app | from the **JS/TS spec** to C++ props/event structs + native interfaces | no: adding a native component means a native rebuild (or a custom dev client) | spec→codegen keeps types in one place. Autolinking from package metadata is the right UX, but they require a rebuild. Our GTK/ObjC/WinUI reflective paths can avoid that |
| [Expo Modules API](https://docs.expo.dev/modules/module-api/) | npm package + `expo-module.config.json` + Swift/Kotlin `Module` DSL (`View(MyView.self) { Prop("x"){…}; Events("onChange") }`) | [autolinking](https://docs.expo.dev/modules/autolinking/) from the config file; `npx expo install` | none: the DSL is the registration (runtime reflection over the DSL) | needs a dev-client rebuild when native code changes | a **small declarative registration DSL in the native language** is pleasant for hand-written packs. Our equivalent is the override YAML + generated thunks |
| [Flutter plugins](https://docs.flutter.dev/packages-and-plugins/developing-packages) / [platform views](https://docs.flutter.dev/platform-integration/android/platform-views) | pub package with `flutter: plugin: platforms:` in `pubspec.yaml` | tool-generated `GeneratedPluginRegistrant` compiled into the app | [Pigeon](https://pub.dev/packages/pigeon) for typed channels | no | platform views (embedding a native view in a foreign renderer) are expensive. We don't have that problem because we *are* the native toolkit, and that's why packs are cheap for us |
| [NativeScript plugins](https://docs.nativescript.org/plugins/) + [metadata generator](https://docs.nativescript.org/guide/metadata) | npm package with `platforms/ios` (Podfile, frameworks) / `platforms/android` | at build time, the metadata generator scans **all** linked native libs, plugins included, into a blob; the runtime calls any API through it | metadata only, no per-API glue | adding a plugin → native rebuild. After that, JS calls anything | **metadata blob + reflective runtime** is our GTK/ObjC model. A pack contributes metadata; the backend does the calling |
| [Tauri v2 plugins](https://v2.tauri.app/develop/plugins/) | a crate `tauri-plugin-x` + npm `@tauri-apps/plugin-x` (guest bindings) + permission files | `Builder::plugin(x::init())` at compile time | `build.rs` generates permission schemas | no (Rust rebuild) | `tauri add x` edits `Cargo.toml`, `package.json`, `lib.rs` and capabilities in one command: the model for `svelters add <pack>`. A pack is **two packages, one crate + one npm, versioned together** |
| [Qt Designer custom widget plugins](https://doc.qt.io/qt-6/designer-creating-custom-widgets.html) | a shared lib implementing `QDesignerCustomWidgetInterface` (`name`, `includeFile`, `createWidget`, `domXml`) in `plugins/designer` | `QPluginLoader` / `QUiLoader` at runtime | none | **yes**: Designer and QUiLoader create plugin widgets by class name, and properties come through QMetaObject | proves the "dlopen + factory by name + meta-object" model works for Qt. A library that ships a designer plugin is a zero-glue Qt pack (§3) |
| GtkBuilder / Blueprint | `<object class="AdwHeaderBar">` | GType registry (`g_type_from_name`, `gtk_builder_get_type_from_name` tries `*_get_type` symbols through `dlsym`) | none | yes | GTK's own UI files reach any registered or `get_type`-exported class by name. Our GTK pack in dev is the same mechanism |

**Common pattern.** One installable unit (package manager native to the app's ecosystem) carries
native code + typed JS surface + a declarative manifest. A CLI command wires it in, and registration
is generated (autolinking) rather than hand-written. Where the host runtime is reflective
(NativeScript, Designer, GtkBuilder), adding the unit needs only metadata. Otherwise it needs a rebuild.

## 8. Design conclusions (full proposal: `plans/research/CUSTOM-WIDGETS.md`)

- Packs plug into the **backend**, not the host. The core keeps the single handle table,
  liveness, lifetime classes, the loop and style. Peer backends (host multiplexing) fail at mixed
  trees and break X1 identity.
- Ids gain a pack field: bits 22..27, below the listen flags in 28..30, above Qt's `1<<20` getters.
  Routing follows the member id for props, methods and signals, the parent's kind for insert and
  child attributes, and the kind for create.
- Data packs (GIR/typelib, ObjC runtime, XAML metadata providers, Qt designer plugins) need no
  compiled code and work in prebuilt runners. Code packs use `svl_pack_ops` + `svl_pack_services`
  (new `svelters_pack.h`). The ABI gets one additive `add_pack` member (v2.5).
- Pilot: libadwaita. Then app GTK widget → Qt QMetaObject engine + app widget → ADS → dlopen →
  WinUI/Apple.
- Probe outputs reproducible with: `python3 -I docs/notes/scripts/cw_adw_gir.py`,
  `python3 -I docs/notes/scripts/cw_moc_json.py <ads>/src`,
  `QT_QPA_PLATFORM=offscreen python3 -I docs/notes/scripts/cw_qt_reflect.py`.
