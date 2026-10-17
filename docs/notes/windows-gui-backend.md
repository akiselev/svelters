# Native Windows backend: API choice, metadata, ABI fit (desk research)

Status: research note, 2026-10-17. Written on Linux. Facts marked **[verified]** were checked
here against downloaded metadata or by running code under Wine. Everything else comes from
documentation and prior knowledge, and needs a Windows machine to confirm.

Scripts:

- `docs/notes/scripts/winmd_dump/`: a Rust dumper over `windows-metadata` 0.100.
- `docs/notes/scripts/win32_probe.{c,rc,manifest}`: a Win32 + comctl32 v6 feasibility probe.
- `docs/notes/scripts/winui_proto_filter.txt`: the windows-bindgen member-level filter for the P set.

Downloaded packages and generated output are outside git, in the session scratchpad
(`.../scratchpad/windows/{dl,gen}`). They can be re-downloaded from NuGet (see §6).

## 0. TL;DR

- **Primary: WinUI 3 (Windows App SDK), with the backend written in Rust over windows-rs
  bindings generated from `Microsoft.UI.Xaml.winmd`.**
  - It's the only Microsoft-backed native toolkit with Fluent styling, UIA built in, a retained
    tree, layout, and real virtualization.
  - Its winmd is the richest metadata source of any toolkit we target. It encodes:
    - every DependencyProperty: 1852 `XxxProperty` statics across 900 classes;
    - typed events: 322 events, 187 of them `TypedEventHandler<Sender, Args>`;
    - collections such as `Children: UIElementCollection : IVector<UIElement>`;
    - the default child slot (`[ContentProperty(Name=...)]` on 74 classes, plus inheritance);
    - attached properties (`Grid.GetRow/SetRow`);
    - virtual overrides (`[Overridable]` interfaces).
- **Fallback / "always works": Win32 + comctl32 v6.**
  - It builds and *runs* on Linux CI under Wine **[verified]**.
  - It needs no runtime.
  - But it has no layout (we'd own one, e.g. Taffy), no Fluent look, and almost no structured
    metadata. Messages are bare `u32` constants with no link to a control or parameter types,
    so nearly everything is annotation.
- Excluded:
  - **UWP XAML** (`Windows.UI.Xaml`): maintenance mode, older control set. It does have
    `XamlDirect`, the ID-keyed API that React Native Windows used, but WinUI 3 does **not**
    ship it **[verified]**.
  - **WPF/WinForms**: need a CLR in-process.
  - **Direct2D custom drawing**: duplicates the planned egui backend (M7). egui on Windows plus
    AccessKit already covers that path.
- **Loop fit is good for both primaries.** Mapping onto our ABI:
  - `run`: `Application.Start` or our own `GetMessage` loop with
    `ContentPreTranslateMessage` (XAML Islands);
  - `wake`: `DispatcherQueue.TryEnqueue` (thread-safe) or `PostMessage`;
  - `schedule_wake`: one `DispatcherQueueTimer` or `SetTimer`, both on the native loop and
    both working inside nested modal loops.
- Codegen size: windows-bindgen over the whole `Microsoft.UI.Xaml` namespace emits
  **1.28 M lines / 67 MB of Rust** **[verified]**. That's the 2021 build-time trap again.
  - Member-level filters (`Type::{get_Text, put_Text, add_Click}`) cut the P set to
    **8.6 k lines**.
  - That subset **cargo-checks for `x86_64-pc-windows-gnu` on Linux** **[verified]**.
  - So the M6 binding linker maps straight onto windows-bindgen filters.
- Prototype estimate (window, stack, label, button, text box, counter fixture, on Windows CI):
  - WinUI 3: about 2–3 agent-lane weeks.
  - Win32: about 1–1.5 weeks.

## 1. Candidate APIs

### 1.1 Win32 + Common Controls v6

- **Widget model.**
  - Retained tree of `HWND`s: every control is a child window of a window class
    (`WC_BUTTONW`, `WC_EDITW`, `WC_STATICW`, `WC_LISTVIEWW`, …; 43 `WC_*` constants in
    win32metadata **[verified]**).
  - Properties are window text, styles (`BS_*`, `ES_*`) and messages (`BM_SETCHECK`,
    `EM_SETCUEBANNER`, `LVM_*` with 155 messages, `TCM_*` with 36).
  - Events reach the **parent** as `WM_COMMAND`/`WM_NOTIFY` with notification codes
    (`BN_CLICKED`, `EN_CHANGE`).
  - There are no containers and no layout. Every child is positioned in pixels by its parent's
    `WM_SIZE`.
  - We would implement container kinds (stack, grid) ourselves on top of Taffy (Rust,
    flexbox+grid; it's the obvious pick since the host is Rust) or Yoga. Our "stack" would then
    be a plain `HWND` with our layout attached.
- **Accessibility.**
  - Standard controls get UIA through the built-in MSAA→UIA proxies (names, roles, invoke,
    value).
  - Custom containers need nothing extra because they are plain windows. Custom-drawn
    controls would need our own `IRawElementProviderSimple`.
  - Adequate, not great: no rich UIA patterns on the older controls.
- **Theming and look.**
  - Visual styles (uxtheme) give the Windows 7/10 look, not Fluent. Common controls
    don't adopt Windows 11 Fluent visuals.
  - There's no documented dark mode for comctl32. Apps use undocumented uxtheme ordinals or
    the `DarkMode_Explorer` theme class. Fonts default to the system GUI font unless we send
    `WM_SETFONT`.
  - It looks dated beside WinUI apps.
- **Deployment.** One exe plus an app manifest (comctl32 v6 + per-monitor DPI v2). No runtime,
  no MSIX. Works on everything from Windows 7 up.
- **Longevity.** Win32 will outlive everything else here, but it's frozen: no new controls,
  and Microsoft's UI investment is in WinUI. It's safe, but it won't improve.
- **Loop fit [verified under Wine].**
  - `run`: a plain `GetMessage`/`DispatchMessage` loop, using `IsDialogMessage` for tab
    navigation.
  - `wake`: `PostMessageW(hwnd, WM_APP_WAKE)` from any thread, which yields a checkpoint on the
    main thread.
  - `schedule_wake(ms)`: `SetTimer(hwnd, ID, ms)`. Re-arming the same ID replaces the
    pending timer, which matches the planned ABI v2 semantics exactly. `ms < 0` maps to
    `KillTimer`.
  - Both are window messages, so they keep firing inside nested modal loops (`MessageBox`,
    menu tracking, the move/size loop). That's the property we wanted from native timers.
  - Blocking modals (`DialogBoxParam`, `MessageBox`, `TrackPopupMenu`) need the
    `unsupported_reason: blocking_nested_loop` treatment from `backend-abi.md`.

### 1.2 WinUI 3 / Windows App SDK

- **Widget model.**
  - Retained XAML visual tree: `DependencyObject` → `UIElement` → `FrameworkElement` →
    `Control`/`Panel`.
  - Full layout system (measure/arrange, `StackPanel`, `Grid`, `RelativePanel`, `Canvas`),
    templates and styles, and virtualizing lists (`ListView`, `ItemsRepeater`, `ItemsView`).
  - Controls are activated in code through WinRT factories (`IButtonFactory.CreateInstance`),
    with no XAML markup. `XamlReader.Load(string)` remains available for snippets and
    templates.
- **Accessibility.** UIA is native: every control has an `AutomationPeer`
  (`OnCreateAutomationPeer` is an overridable virtual on `UIElement` **[verified]**), and
  `AutomationProperties.*` are attached properties. It's the best a11y story on Windows.
- **Theming and look.** Fluent 2 (WinUI 3 control styles, Mica/Acrylic backdrops), with
  light, dark and high contrast handled by `ThemeResource`. This is "native Windows 11".
- **Deployment.** No MSIX required. Two unpackaged modes:
  - **Framework-dependent.** Call `MddBootstrapInitialize2`
    (`Microsoft.WindowsAppRuntime.Bootstrap.dll`) at startup. The Windows App Runtime must be
    installed (a redistributable installer).
  - **Self-contained.**
    - Copy the runtime DLLs next to the exe. The `runtimes-framework/win-x64/native` folder of
      the WinUI package alone is 139 MB across all locales **[verified]**; trimmed, x64 only
      it's tens of MB.
    - Then call `WindowsAppRuntime_EnsureIsLoaded()`, which enables reg-free WinRT
      activation. That's exactly what
      `UndockedRegFreeWinRT-AutoInitializer.cpp` in the Foundation package does
      **[verified]**.
    - A Rust exe can make the same call. No MSBuild is needed, but we have to replicate what
      the `.targets` files do: copy the DLLs and `.pri` resources, and write the manifest.
  - Either way, add `XamlControlsResources` to `Application.Resources` or controls fall back
    to unstyled templates.
- **Longevity.**
  - It's Microsoft's stated native UI direction.
  - The SDK is still moving fast: the metapackage is at `2.5.1`, split into component
    packages (`Microsoft.WindowsAppSDK.WinUI` 2.3.9, `.Foundation` 2.3.12,
    `.InteractiveExperiences` 2.1.9, …) **[verified from NuGet]**.
  - Expect API churn at the SDK level but stability at the XAML level, since
    `Microsoft.UI.Xaml` mirrors `Windows.UI.Xaml`.
- **Loop fit.** Two hosting modes, and both keep the toolkit loop authoritative:
  1. **`Application.Start(callback)`**: WinUI owns the loop. Create the window in
     `OnLaunched`; that's an `[Overridable] IApplicationOverrides` and needs a composable
     subclass (`--implement` in windows-bindgen).
  2. **XAML Islands.**
     - Our own Win32 loop hosts `DesktopWindowXamlSource` (`Initialize(parentWindowId)`),
       after `DispatcherQueueController.CreateOnCurrentThread()` and
       `WindowsXamlManager.InitializeForCurrentThread()` **[verified API present]**.
     - The loop must call `ContentPreTranslateMessage(&msg)`, a plain C export in
       `Microsoft.UI.Dispatching.Interop.h` that is *not* in winmd **[verified]**.
     - This mode lets the Win32 and WinUI backends share the window, loop, timer and wake code,
       with no Application subclass needed.

  In either mode:
  - `wake` = `DispatcherQueue.TryEnqueue(handler)`, which is thread-safe by design.
  - `schedule_wake` = one `DispatcherQueueTimer` (`Interval`, `IsRepeating=false`,
    `Start()` re-arms) **[verified API]**. On the island path, `SetTimer` works too.
  - Nested loops matter less: `ContentDialog.ShowAsync` is async (no nested pump), which fits
    the "present + response signal" rule. `Closing` events carry `Cancel`.

- **C++/WinRT vs windows-rs.** Both are generated from the same winmd.
  - C++/WinRT (`cppwinrt.exe`) emits header-only projections and is what Microsoft's own
    samples use.
  - windows-rs (`windows-bindgen`) emits Rust and runs on Linux.

  See §3.1 for the choice.

### 1.3 UWP XAML (`Windows.UI.Xaml`)

- It's in-box (no runtime to ship) and hostable in desktop apps through system XAML Islands
  (`Windows.UI.Xaml.Hosting`, Windows 10 1903+).
- But UWP is in maintenance. The in-box control set is older: modern controls come from WinUI 2,
  which needs a framework package and is awkward unpackaged.
- One unique asset: **`XamlDirect`**
  (`Windows.UI.Xaml.Core.Direct`).
  - It's an ID-keyed middleware API:
    - `CreateInstance(XamlTypeIndex)`;
    - `Set{Int32,String,…}Property(obj, XamlPropertyIndex, v)`;
    - `AddToCollection` / `InsertIntoCollectionAt` / `RemoveFromCollectionAt`;
    - `AddEventHandler(obj, XamlEventIndex, handler, handledEventsToo)` **[verified in the
      SDK contract winmd]**.
  - That is almost exactly our `resolve`/`create`/`insert`/`set_prop`/`listen` vtable.
  - **But it does not exist in WinUI 3**: `Microsoft.UI.Xaml.winmd` 2.3.9 has no
    `Core.Direct` namespace **[verified]**.
- Not recommended. Note it as prior art for an ID-keyed design.

### 1.4 WPF / WinForms

Both are managed (.NET). A native host would have to load the CLR through
`hostfxr`/`nethost`, and the backend would be generated C#. That means two GCs and two
runtimes in one process, a .NET dependency or a large self-contained payload, and only partial
trimming/NativeAOT support for these UI stacks.

WPF did gain a Fluent theme in .NET 9, but there's no clean native-host story. **Excluded**,
as the brief suggested.

### 1.5 Direct2D / DirectComposition custom drawing

This means drawing our own controls (the egui path). Fluent look-alike and IME, text editing and
accessibility all become our problem. On a11y, AccessKit has a mature Windows UIA adapter.

The planned egui backend (M7) already gives us this on Windows through `eframe`/`winit` +
AccessKit, so a bespoke D2D toolkit isn't worth building. If we need "custom-drawn but native
windowing", it's egui.

### 1.6 Comparison

| | Win32 + comctl32 v6 | WinUI 3 | UWP XAML | WPF/WinForms | D2D custom (≈egui) |
|---|---|---|---|---|---|
| Tree / layout | HWND tree, **no layout** | XAML tree, full layout | XAML tree, full layout | full | ours |
| A11y | MSAA→UIA proxies | native UIA peers | native UIA peers | UIA | AccessKit |
| Look | Win7/10 visual styles, no dark | **Fluent 2**, dark/HC | older Fluent | Fluent theme (.NET 9) | ours |
| Runtime | none | WinAppSDK (framework or self-contained) | in-box | .NET | none |
| Direction (2026) | frozen, permanent | **strategic** | maintenance | maintenance | n/a |
| Loop / wake / timer | GetMessage / PostMessage / SetTimer | Start or Islands / TryEnqueue / DispatcherQueueTimer | CoreDispatcher | Dispatcher | winit |
| Metadata quality | poor (untyped messages) | **excellent** | excellent (+XamlDirect) | reflection | n/a |
| Linux CI | **builds + runs under Wine** | builds (cross); runs only on Windows | no | no | yes |

## 2. Metadata sources for codegen

### 2.1 Files and where they come from

All are ECMA-335 `.winmd` files. Versions are pinned below; downloaded 2026-10-16.

| File | NuGet package (version) | Size | Contents |
|---|---|---|---|
| `Windows.Win32.winmd` | `Microsoft.Windows.SDK.Win32Metadata` 71.0.30-preview | 24 MB | Win32 functions, constants, structs, COM interfaces |
| `Windows.Foundation.UniversalApiContract.winmd` (+90 contract winmds) | `Microsoft.Windows.SDK.Contracts` 10.0.28000.2705 | 6.2 MB | WinRT `Windows.*`, incl. `Windows.UI.Xaml`, `Windows.Foundation.Collections` |
| `Microsoft.UI.Xaml.winmd`, `Microsoft.UI.Text.winmd` | `Microsoft.WindowsAppSDK.WinUI` 2.3.9 (`metadata/`) | 1.6 MB | WinUI 3 |
| `Microsoft.UI.winmd`, `Microsoft.Foundation.winmd`, `Microsoft.Graphics.winmd` | `Microsoft.WindowsAppSDK.InteractiveExperiences` 2.1.9 | 0.3 MB | `DispatcherQueue`, `AppWindow`, Composition, Input |
| `Microsoft.Windows.*.winmd` | `Microsoft.WindowsAppSDK.Foundation` 2.3.12 | small | resources, lifecycle; needed transitively |
| `Microsoft.Web.WebView2.Core.winmd` | `Microsoft.Web.WebView2` 1.0.3719.77 | | needed transitively (the `WebView2` control) |

`Microsoft.UI.Xaml.winmd` cannot be resolved alone. Generating bindings needed the five
packages above plus the SDK `Windows.winmd` (bundled in windows-bindgen as `default`).

### 2.2 How windows-rs / windows-bindgen consumes them

- `windows-bindgen` 0.100 takes the inputs as `--in <files|dirs|default>`, plus:
  - `--filter` / `--filter-file`;
  - `--implement <iface>`, which generates implementation traits for overridables such as
    `IApplicationOverrides`;
  - `--minimal` (no class wrappers), `--sys` (raw ABI) and `--package`.
- **Member-level filters**:
  - Syntax: `Namespace.Type::{get_Text, put_Text, add_Click}` (`Type::{}` = name-only
    shell). Use raw accessor names inside braces; `Property.X` sugar only works outside them.
  - This is the M6 tree-shaking root set, expressed in bindgen's own language.
  - The `windows` crate itself is generated this way. Microsoft.UI.* bindings are not shipped
    in `windows`, so we would generate our own.
- Generated code depends on:
  - `windows-core`: COM/WinRT runtime, HSTRING, factory cache, `#[implement]`;
  - `windows-collections` (`IVector`), `windows-future` (async), `windows-numerics`;
  - `windows-link`: raw-dylib, **so no import libraries are needed to link**.
- Measured **[verified]**:

  | Filter | Output |
  |---|---|
  | whole `Microsoft.UI.Xaml` | 1,282,148 lines / 67 MB; bindgen took 109 s |
  | 5 requested classes, whole types incl. base chains | 21,413 lines / 1.1 MB |
  | P prototype set, member-level (`gen/proto_filter.txt`, ~30 entries) | 8,757 lines / 0.2 MB |

  - The P set `cargo check`s on Linux with `--target x86_64-pc-windows-gnu` after two fixes:
    - adding the `I*Factory`/`I*Statics` interfaces the `Type::{}` shells reference (two
      rounds, found automatically from the compile errors);
    - stripping one generated `IntoIterator` impl that calls an unfiltered `First()`.
  - Both are minor bindgen 0.100 filter rough edges that our generator would handle.
  - `windows-core` items are `cfg(windows)`, so the host target can't check the code.

### 2.3 Parsing in Python and Rust (tried)

- **Rust `windows-metadata` 0.100 [verified]:**
  - `reader::Index::new(files)` merges several winmds and resolves names across them.
  - It exposes `TypeDef`, `MethodDef` (+ `signature(generics)`, `params()`), `InterfaceImpl`,
    attributes with decoded values (`Value::TypeName`, `EnumValue`, `Utf16`) and generic
    instantiation (`IVector<UIElement>`), plus `Item::Fn`/`Item::Const` for Win32 `Apis`.
  - It does *not* expose the ECMA `Property`/`Event` tables. Properties and events are
    recovered from `SpecialName` accessor methods (`get_`/`put_`/`add_`/`remove_`), which is
    what windows-bindgen does too.
  - `docs/notes/scripts/winmd_dump/` (~250 lines) does the whole projection we need. It fits
    the M3 plan to run generators in Rust inside `build.rs`, and it builds in seconds.
- **Python `dnfile`:** not tried. Rust covered the need, and `dnfile` only gives raw tables;
  we would re-implement signature/blob decoding, custom-attribute decoding and cross-file
  resolution.
  - For a P-phase Python generator, the practical route is to run `winmd_dump` (or a JSON
    mode of it) and consume the JSON.
  - Recommendation: **Rust from day one for Windows metadata.**

### 2.4 Dump: Button, StackPanel, TextBox, ListView, NavigationView

Command:

```text
winmd_dump Microsoft.UI.Xaml.Controls.Button+ ... -- <winmds>
```

`+` walks the base chain. Full output is in the scratchpad (`dump_5.txt`, `dump_more.txt`).
Abridged, **[verified]**:

```text
Button : ButtonBase : ContentControl : Control : FrameworkElement : UIElement : DependencyObject
  @Composable(IButtonFactory)  @Static(IButtonStatics)  @Threading(ASTA) @MarshalingBehavior(Agile)
  Button          prop Flyout: FlyoutBase [rw]                 DP: FlyoutProperty
  ButtonBase      event Click: RoutedEventHandler              props ClickMode, Command, CommandParameter, IsPointerOver, IsPressed
  ContentControl  @ContentProperty(Name="Content")             prop Content: IInspectable [rw]  ContentTemplate, ContentTemplateSelector
                  virtual OnContentChanged(old, new)           (IContentControlOverrides, [Overridable])
  UIElement       event KeyDown: KeyEventHandler … ; virtual OnCreateAutomationPeer(); static KeyDownEvent: RoutedEvent …
  DependencyObject method GetValue(dp) / SetValue(dp, IInspectable) / ClearValue(dp) / RegisterPropertyChangedCallback(dp, cb) -> i64

StackPanel : Panel
  Panel           @ContentProperty(Name="Children")  prop Children: UIElementCollection [r]
                  UIElementCollection : IVector<UIElement>  (Append, InsertAt, RemoveAt, IndexOf, Move, Clear, ReplaceAll)
  StackPanel      props Orientation, Spacing: f64, Padding: Thickness, CornerRadius, BorderBrush …  (8 DPs)

TextBox : Control      @InputProperty(Name="Text")   (27 DPs)
  props Text, PlaceholderText, Header: IInspectable, IsReadOnly, MaxLength, SelectionStart …
  events TextChanged: TextChangedEventHandler
         BeforeTextChanging: TypedEventHandler<TextBox, TextBoxBeforeTextChangingEventArgs>   args: Cancel [rw], NewText
         TextChanging, SelectionChanging, Paste, CopyingToClipboard, …
  methods Select(i32,i32), SelectAll(), Undo(), …, GetLinguisticAlternativesAsync() -> IAsyncOperation<IVectorView<HSTRING>>

ListView : ListViewBase : Selector : ItemsControl
  ItemsControl    @ContentProperty(Name="Items")  ItemsSource: IInspectable, ItemTemplate: DataTemplate, ItemTemplateSelector
                  virtuals GetContainerForItemOverride, PrepareContainerForItemOverride, ClearContainerForItemOverride …
  ListViewBase    events ContainerContentChanging (args: Item, ItemIndex, ItemContainer, Phase, InRecycleQueue, Handled),
                         ChoosingItemContainer (args: ItemContainer [rw], IsContainerPrepared [rw]), ItemClick, SelectionChanged
                  prop Header: IInspectable

NavigationView : ContentControl   (37 DPs; 10 events, all TypedEventHandler<NavigationView, …Args>)
  slot-like props  Header: IInspectable, PaneHeader / PaneFooter / PaneCustomContent / ContentOverlay: UIElement,
                   AutoSuggestBox: AutoSuggestBox, MenuItems / FooterMenuItems: IVector<IInspectable> [r], MenuItemsSource
  events           SelectionChanged, ItemInvoked, BackRequested, PaneClosing (args.Cancel [rw]), Expanding, Collapsed …

Grid statics     GetRow/SetRow(FrameworkElement, i32), GetColumn/SetColumn, GetRowSpan/SetRowSpan, …  (attached props)
Window           @ContentProperty(Name="Content"); Activate(), Close(); event Closed: TypedEventHandler<IInspectable, WindowEventArgs>
AppWindow        event Closing: TypedEventHandler<AppWindow, AppWindowClosingEventArgs>  args.Cancel [rw]
```

Namespace-wide statistics for `Microsoft.UI.Xaml.*` **[verified]**:

- 900 runtime classes and 1852 DependencyProperty statics.
- 322 events declared on class interfaces. Their delegate types are:
  - `TypedEventHandler` 187;
  - `EventHandler<T>` 36;
  - `RoutedEventHandler` 23;
  - a long tail of 2–8 per bespoke handler.
- `[ContentProperty]` on 74 classes, which then inherit it. Values: `Content` 11, `Items` 8,
  `Child` 6, `Children` 4, `Inlines` 3, `Header` 3, …

### 2.5 What winmd gives us vs what needs annotation

**Given by metadata (ingest automatically):**

| Fact | Source | Maps to schema v0 |
|---|---|---|
| Kinds, base chain, interfaces, abstract vs constructible | `TypeDef.extends`, `[Composable]`/`[Activatable]` (factory present = constructible) | `bases`, `abstract`, `constructors` |
| Properties + types + read-only vs rw | `get_`/`put_` on default + versioned interfaces (`IFoo`, `IFoo2`, …) | `properties[].type/access` |
| DependencyProperty identity | `IFooStatics.get_XxxProperty` | `properties[].notify` (`RegisterPropertyChangedCallback` works for every DP) |
| Events + delegate + args type + args fields | `add_X(handler)`, the delegate's `Invoke`, the args class's props | `signals[]` |
| **Return channel through args** | settable args props: `Cancel` (`AppWindowClosingEventArgs`, `TextBoxBeforeTextChangingEventArgs`, `NavigationViewPaneClosingEventArgs`, `ContentDialogClosingEventArgs`), `Handled` (`KeyRoutedEventArgs`, `ContainerContentChanging`) | `handler_result` and the `result:` recipe (signals note §2.5.9); inferred mechanically |
| Deferrals (async veto) | `GetDeferral()` on args (`ContentDialogClosingEventArgs`) | flag `sync_required` + deferral, see §3 |
| **Default slot** | `[ContentProperty(Name=…)]`, inherited | `children.default_slot` |
| Collection slots | prop type `IVector<UIElement>` / `UIElementCollection` / `IVector<IInspectable>` | `cardinality: many` with ops `InsertAt/RemoveAt/IndexOf/Move` |
| Single-child slots | prop type `UIElement` or `IInspectable` | `cardinality: one`, ops `put_X(child)` / `put_X(null)` |
| Attached props (per-child attrs) | `Owner.GetX(elem)/SetX(elem, v)` statics + `XProperty` | `attrs[]` with `set_child_attr` |
| Virtuals | `[Overridable]` interfaces (`MeasureOverride`, `OnCreateAutomationPeer`, `SelectTemplateCore`) | generated subclasses (Qt-like) |
| Async methods | return `IAsyncOperation<T>` | promise-returning `call` |
| Versioning | `[ContractVersion]` per type and interface | `availability` |
| Threading | `[Threading(ASTA)]`, `[MarshalingBehavior(Agile)]` | backend thread checks |
| Dev-time string conversion | `IXamlType.CreateFromString` | `default` and enum parsing in dev |

**Missing; needs annotation (Windows-specific list):**

- **Which `IInspectable`/`UIElement` props are child slots vs data or references.**
  - `Header: IInspectable` takes either a string or an element. `SelectedItem: IInspectable`
    is data, not a slot. `AutoSuggestBox` is a slot. `ContentOverlay` is a slot.
  - Typing alone can't tell. This is the same `role: value | child_slot | reference` call as
    GTK/Qt, but Windows has many more `IInspectable`-typed props.
- **`Items` vs `ItemsSource`** exclusivity, which items count as their own container
  (`NavigationViewItem` inside `MenuItems`), and template requirements for `ItemsSource`
  mode.
- **Which events need sync dispatch.** Everything with a writable `Cancel`/`Handled` or a
  deferral can be found by metadata, but whether JS *must* answer synchronously is a
  semantic call. `TextChanging` fires during layout, and `BeforeTextChanging.Cancel` must be
  synchronous.
- **Echo suppression for two-way binding**: does setting `Text` raise `TextChanged`? (Yes,
  asynchronously.) Same as GTK/Qt.
- **Ownership is mostly free.** Refcounting makes parent-destroys-child impossible. Only the
  "one parent" rule needs annotation: an element in a second tree throws `E_INVALIDARG`.
- **Blocking APIs:** few. Most modals are `IAsyncOperation`, but flag
  `ContentDialog.ShowAsync`'s "only one open per XamlRoot" rule.
- **Non-metadata glue**: `ContentPreTranslateMessage`, `MddBootstrapInitialize2`,
  `WindowsAppRuntime_EnsureIsLoaded` are C exports from headers only. `IWindowNative` /
  `IDesktopWindowXamlSourceNative` HWND interop is classic COM in `.idl`/`.h` (shipped in
  the WinUI package `include/`).
- **Win32 needs annotation for nearly everything**:
  - Which messages apply to which control class, and the wParam/lParam encoding of each
    "property" (`BM_GETCHECK` → `BST_*`, `EM_SETCUEBANNER(TRUE, LPCWSTR)`).
  - Which notifications arrive via `WM_COMMAND` vs `WM_NOTIFY`, and with which `NMHDR`
    struct.
  - Window style bits that are construct-only vs `SetWindowLong` + `SetWindowPos(FRAMECHANGED)`.
  - win32metadata has the constants and structs, but messages are untyped `u32` constants
    (`BM_*` and `BN_*` sit in `UI.WindowsAndMessaging`, not with the controls).
  - Upshot: a Win32 backend is closer to hand-written than generated. Its kind count is small
    (~25 controls), so that's tolerable.

## 3. Fit with the svelters ABI and generated backend

### 3.1 Backend language: Rust over windows-rs (recommended)

The Qt backend is C++ only because Qt's API is C++. WinRT is a language-neutral ABI (COM
vtables + HSTRING), so Rust is a first-class consumer:

- One language with the host. The backend can be a normal workspace crate, with no C++
  toolchain and no `cc` leaf build.
- It cross-builds from Linux (§4). C++/WinRT requires MSVC or clang-cl plus the SDK headers,
  and `cppwinrt.exe` (Windows-only) to generate projections.
- `windows-bindgen` already turns winmd into exactly the typed calls we'd generate. Our
  generator emits:
  - the bindgen filter file (the root set);
  - a dispatch table `prop_id → fn(&IInspectable, &svl_value)` using those typed wrappers.
- The leaf-crate pattern still applies for compile-time containment. Bindings for a
  release root set go in a leaf crate whose `build.rs` runs windows-bindgen in-process
  (it's a library) on a member-filtered list. The dev build uses a fixed "fat" prototype set
  or the dynamic path below.

C++/WinRT behind the C ABI is the fallback, if windows-rs composition (`#[implement]` of
composable classes such as `Application`/`DataTemplateSelector` subclasses) proves too
painful. It's mature but not required, and Islands hosting removes the `Application`
subclass.

### 3.2 Properties: generated typed calls, with a reflective dev path

- **Release / M6:** generated typed calls (`TextBlock::SetText(&HSTRING)`). Fast and
  tree-shakable by bindgen filters.
- **Dev (no Cargo per `.svelte` edit, per Rule 5):** WinUI ships reflection.
  - `XamlControlsXamlMetaDataProvider` (`IXamlMetadataProvider`) **[verified present]** has
    `GetXamlType("Microsoft.UI.Xaml.Controls.Button")`, which returns an `IXamlType` with:
    - `ActivateInstance()`, `GetMember(name)`, `ContentProperty`, `AddToVector(instance, v)`;
    - `CreateFromString(s)`.
  - `IXamlMember` has `SetValue(instance, IInspectable)`, `GetValue`, `IsDependencyProperty`,
    `IsAttachable` and `Type`.
  - This is the XAML parser's own path, so it covers create, set-by-name, the content
    property, collection add, attached props and string→typed conversion (enums,
    `Thickness`, brushes from `"Red"`).
  - It's the WinUI analogue of GObject property-by-name. Values are boxed with
    `Windows.Foundation.PropertyValue` (`IReference<T>`).
  - Alternative: `DependencyObject.SetValue(dp, boxed)` with `dp` fetched once from the
    `XxxProperty` static (one generated getter per DP, 1852 total, or fetched through
    `IXamlMember`).
- **`ICustomPropertyProvider` is the other direction.** It lets XAML bind *to our* objects,
  for `{Binding}` in templates. It isn't for setting control props, so it's irrelevant
  unless we expose JS objects to data templates.
- **Events have no reflective API** (`IXamlMember` is properties only). Options:
  - **Routed input events** (`UIElement.KeyDownEvent`, `PointerPressedEvent`, …): call
    `UIElement.AddHandler(RoutedEvent, handler, handledEventsToo)` with a handful of generic
    delegate types. This is fully dynamic.
  - **CLR-style events**: they all follow `add_X(delegate) -> token`. 187/322 are
    `TypedEventHandler<S, A>`, whose `Invoke(sender, args)` ABI is two `IInspectable*`
    whenever both are reference types.
    - One "universal delegate" COM object can answer `QueryInterface` for any parameterized
      IID. WinRT pinterface IIDs are computed deterministically (UUIDv5/SHA-1 over the type
      signature), and windows-core already does that at compile time.
    - Combined with the vtable slot index from winmd (method order), that allows dev-time
      dynamic subscription.
    - The old Chakra/WWA JS projection worked this way: winmd read at runtime.
    - It's an M3+ option. P uses generated `add_Click` connectors.

### 3.3 Events and the sync-handler model

- Events are delivered on the UI thread, synchronously, inside the native handler. That maps
  directly to `host->call_sync` for events that have a result channel, and to
  `host->post_event` otherwise.
- **Result through args, not return values.** No WinRT event delegate returns a value. The
  result recipe is always "write `args.Cancel`/`args.Handled`/out-prop after JS returns",
  which is exactly the `event.defaultPrevented → native` model from the signals note §1.5.
  Writable args props can be detected from metadata, as listed in §2.5.
- **Deferrals** (`args.GetDeferral()`) let async JS answer later. That's a cleaner answer to
  the "handler returned a Promise" open question than GTK or Qt have: take a deferral if
  the handler returns a thenable, and `Complete()` on settle.
- Window close: `Window.Closed` (`WindowEventArgs.Handled`) and `AppWindow.Closing`
  (`Cancel`) are the `close-request` analogues.

### 3.4 Ownership

- WinRT is refcounted COM. The handle table holds one strong ref per handle. `destroy`
  releases it and removes the element from its parent if it's still attached.
- There's no "parent deletes child" (Qt) or floating-ref sinking (GTK). An element is
  removed from the tree but survives while referenced, so `on_destroyed` rarely fires (window
  closed / `XamlRoot` gone).
- Risks:
  - **Cycles through event handlers.** A closure that captures the element keeps it alive.
    WinUI uses `IReferenceTracker` for XAML↔CLR. Our delegates must hold only the
    `svl_handle` (an integer), never a COM ref, which is our existing rule anyway.
  - **Threading.** Elements are ASTA-bound, so every op must run on the UI thread. The host
    already only calls the backend on the main thread.
- Liveness for tests: `IWeakReferenceSource` → `IWeakReference::Resolve` gives the `QPointer`
  analogue.

### 3.5 Slots

| Svelte construct | WinUI mechanism | Op descriptor (signals note §2.5) |
|---|---|---|
| default children of a Panel | `Children` (`[ContentProperty]`), `UIElementCollection` | `insert: InsertAt(index, child)`, `remove: RemoveAt(IndexOf(child))`, `move: Move(old,new)` |
| default child of ContentControl / Border / Window | `Content` / `Child` | `set: put_Content(child)`, `clear: put_Content(null)`, `effects.old: unparented` |
| named single slots | `Header`, `PaneHeader`, `PaneFooter`, `PaneCustomContent`, `ContentOverlay`, `Flyout`, `TitleBar` (via `Window.SetTitleBar`) | `set: put_X(child)` |
| named many slots | `NavigationView.MenuItems` / `FooterMenuItems`, `CommandBar.PrimaryCommands`, `TabView.TabItems` | `insert: InsertAt`, `accepts: [NavigationViewItem…]` |
| per-child attrs | attached properties `Grid.Row/Column/RowSpan`, `Canvas.Left`, `RelativePanel.*`, `AutomationProperties.Name`, `ToolTipService.ToolTip` | `set_child_attr` = `Grid::SetRow(child, v)`; metadata gives `Owner.Get/SetX` |

Notes:

- `insert(…, before)` → compute `index = IndexOf(before)` and call `InsertAt`. It's all O(n),
  but fine.
- **Attached props aren't really per-parent.** `AutomationProperties.Name` and
  `ToolTipService.ToolTip` attach to *any* element. They're closer to ordinary props with a
  qualified name (`AutomationProperties.Name="Save"`) than to per-child attrs. Expose
  both: owner-qualified attrs on any element, with `set_child_attr` reserved for
  parent-owned ones (`Grid.*`, `Canvas.*`).
- **`Header: IInspectable`** takes a string *or* an element. Generate both a prop (string) and
  a slot (element) for such dual-typed content props. This applies to `Content` itself as
  well: `<Button>Click</Button>` sets text, while `<Button><Icon/></Button>` sets an element.

### 3.6 Lists and virtualization (M5)

Two ways Svelte-rendered rows fit, both pull-based like GTK `GListModel` + factory:

1. **`ItemsRepeater` + our `IElementFactory` (preferred).**
   - `ItemTemplate` is typed `IInspectable` and accepts any `IElementFactory`
     **[verified]**.
   - `GetElement(ElementFactoryGetArgs{Data, Parent})` → `call_sync` into JS, which renders
     the row component for `Data` (index or key) into a detached handle and returns the root
     `UIElement`.
   - `RecycleElement(args)` → JS destroys or pools the row.
   - `ItemsSource` is a backend-implemented `IObservableVector<IInspectable>` (or an
     `ItemsSourceView`) over the JS keyed list. `VectorChanged` comes from the Svelte keyed
     `each` diff.
   - Layout comes from `StackLayout`/`UniformGridLayout`. Selection, keyboard nav and UIA
     list patterns are on us. The newer `ItemsView` wraps ItemsRepeater with selection.
2. **`ListView` + `ContainerContentChanging`.**
   - Keep `ListView`'s selection, keyboard and UIA, and fill containers ourselves.
   - In `ChoosingItemContainer`, supply or reuse a `ListViewItem`.
   - In `ContainerContentChanging` (`Item`, `ItemIndex`, `ItemContainer`, `InRecycleQueue`,
     `Phase`, `Handled`), set `ItemContainer.Content` to the Svelte-rendered row and mark
     `Handled`.
   - No `DataTemplate` is needed. Templates otherwise require XAML strings through
     `XamlReader.Load`.

Both need a sync re-entry (`call_sync`) during layout. That's the same re-entrancy question
as the GTK factory `bind` signal, so it can share that answer.

## 4. Testing on Linux CI

| What | Linux? | Evidence |
|---|---|---|
| Parse winmd, generate metadata JSON, run generators | **yes** | `winmd_dump` (windows-metadata) **[verified]** |
| windows-bindgen generation | **yes** | all runs above **[verified]** |
| `cargo check`/`build` the Rust backend for Windows | **yes**: `rustup target add x86_64-pc-windows-gnu` + mingw-w64 linker; windows-link raw-dylib means no import libs | proto bindings checked **[verified]** |
| MSVC-ABI target (`x86_64-pc-windows-msvc`) | yes with `cargo-xwin` (downloads the MSVC CRT + SDK, license acceptance) + clang-cl/lld-link (both present here) | not run |
| Win32 + comctl32 v6 **runtime tests** | **yes, under Wine + Xvfb** | `win32_probe.exe`: comctl32 6.0 active, label/button/edit created and laid out, 2× `BM_CLICK` → `count: 2`, `EN_CHANGE`, `SetTimer` one-shot re-arm → checkpoint, cross-thread `PostMessage` wake → checkpoint on the main thread. `RESULT PASS` in ~30 s cold (Wine prefix creation dominates) **[verified, Wine 11.7]** |
| A11y fidelity under Wine | **no** | MSAA name was right (`Increment`) but the role came back `10` (CLIENT), not `43` (PUSHBUTTON) **[verified]**. UIA assertions need real Windows |
| WinUI 3 runtime under Wine | **no** (not attempted). Needs DirectComposition/DWM, CoreMessaging and the WinAppSDK runtime, which Wine doesn't provide well enough | treat as Windows-only |
| WinUI 3 / Win32 on real Windows | GitHub Actions `windows-latest` (Windows Server 2022/2025): interactive desktop session available, WinAppSDK self-contained works unpackaged | standard |

Recommended setup:

1. **Every PR (Linux):**
   - metadata ingestion and generator determinism (byte-identical output);
   - `cargo check --target x86_64-pc-windows-gnu` for the Windows backend crate;
   - Win32 backend fixture tests under `xvfb-run wine` (op-log + scripted interaction, same
     JSON fixtures as GTK/Qt).
2. **Every PR touching Windows (or nightly): a GitHub Actions `windows-latest` job** that
   builds MSVC, runs the WinUI backend fixtures and the per-annotation verification tests
   (slots read back through `Children`/`Content`, `Cancel`/`Handled` effects, weak-ref
   liveness), and takes screenshots. UIA checks run here, through
   `IUIAutomation::ElementFromHandle` or FlaUI.
3. **Dev loop:** develop on Linux against Wine (Win32) and the cross-check, with a Windows
   VM or runner for WinUI. Since `.svelte` edits never rebuild the backend (Rule 5), the
   Windows box only needs a prebuilt host + backend binary and the Vite watch output. That
   dev host could even be a Windows runner pulling the bundle.

## 5. Recommendation

- **Primary: WinUI 3**, with a Rust backend via windows-bindgen.
  - Prefer **XAML Islands hosting** (our own Win32 loop + `DesktopWindowXamlSource`) for P:
    - it avoids the composable `Application` subclass;
    - it shares the loop, `wake` (`PostMessage`) and `schedule_wake` (`SetTimer`) code with
      the Win32 backend;
    - the `ContentPreTranslateMessage` hook keeps keyboard and accelerators correct.
  - Move to `Application.Start` only if islands show gaps. Known island caveats: theming
    resources must be added manually, and multi-window focus handling needs care.
  - Ship self-contained (`WindowsAppRuntime_EnsureIsLoaded`). Offer framework-dependent
    later for smaller installers.
- **Fallback: Win32 + comctl32 v6**, with a Taffy-based layout.
  - It's the Linux-testable baseline: the whole fixture suite runs under Wine in CI.
  - It covers Windows 7 / LTSC / no-runtime deployments.
  - Its loop code is shared with the island host anyway, so a thin Win32 backend is cheap
    once the WinUI host shell exists.
- **Not pursued:** UWP XAML (maintenance; its XamlDirect is good prior art for ID-keyed
  dispatch), WPF/WinForms (CLR), and bespoke D2D (that's egui/M7).

**Prototype scope and effort (P-style: window, vertical stack, label, button, text box,
counter + text-binding fixture, op-log parity, screenshot on a Windows runner):**

| Work item | WinUI 3 (islands) | Win32 |
|---|---|---|
| Metadata ingest → schema v0 JSON for ~8 kinds (`winmd_dump` + `--json`) | 2–3 d | 1 d (constants) + 2 d manual annotation |
| Generated bindgen filter + dispatch tables (`resolve`, `create`, `set_prop`, `listen`) | 3–4 d | 2–3 d (message tables) |
| Host shell: window, loop, `wake`, `schedule_wake`, bootstrap/self-contained packaging | 3–4 d | 1–2 d (probe already covers it) |
| Layout | free (StackPanel) | 2 d (Taffy integration for stack) |
| CI: Linux cross-check + Wine (Win32) / Windows runner (WinUI) | 2 d | 1 d |
| **Total** | **~2–3 weeks** | **~1–1.5 weeks** |

**Risks:**

1. **windows-rs churn.** windows-bindgen/metadata are at 0.100 and their APIs change between
   releases (the filter syntax quirks above). Pin versions and vendor the generated bindings
   per Windows App SDK version.
2. **Windows App SDK churn and deployment complexity.** We have to replicate MSBuild
   `.targets` behaviour (DLL set, `resources.pri`, manifest) without MSBuild. Self-contained
   size is tens of MB.
3. **Codegen size.** Full namespace generation is 67 MB of Rust. We must keep member-level
   root sets from the start (M6 as a design constraint, not an afterthought) or use the
   dynamic `IXamlMetadataProvider` path in dev.
4. **Composition/subclassing from Rust** (`Application`, `DataTemplateSelector`, custom
   `Panel` with `MeasureOverride`) is the least-trodden windows-rs path. Islands + an
   `IElementFactory` implementation (a plain interface, not a composable class) sidestep most
   of it.
5. **No WinUI runtime testing on Linux.** WinUI regressions surface only on the Windows
   runner, so keep the Win32 backend green under Wine as the always-on smoke test of shared
   host and loop code.
6. **Re-entrancy during layout.** `ContainerContentChanging`/`GetElement` call JS in the
   middle of a measure pass. This needs the `call_sync` rules (still open in `backend-abi.md`)
   before M5.
7. **Win32 metadata poverty.** The Win32 backend is mostly hand-annotated message tables.
   Keep its kind set small.

## 6. Links

- win32metadata: <https://github.com/microsoft/win32metadata> (NuGet `Microsoft.Windows.SDK.Win32Metadata`)
- windows-rs (windows-bindgen, windows-metadata): <https://github.com/microsoft/windows-rs>
- Windows App SDK: <https://github.com/microsoft/WindowsAppSDK>. WinUI: <https://github.com/microsoft/microsoft-ui-xaml>
- NuGet flat container used for downloads: `https://api.nuget.org/v3-flatcontainer/<id>/<ver>/<id>.<ver>.nupkg`
  (`microsoft.windowsappsdk.winui` 2.3.9, `.interactiveexperiences` 2.1.9, `.foundation` 2.3.12,
  `microsoft.windows.sdk.contracts` 10.0.28000.2705, `microsoft.web.webview2` 1.0.3719.77)
- XAML Islands in WinAppSDK (`DesktopWindowXamlSource`):
  <https://learn.microsoft.com/windows/apps/develop/ui/xaml-islands/xaml-islands>
- Taffy (layout for the Win32 path): <https://github.com/DioxusLabs/taffy>. AccessKit: <https://github.com/AccessKit/accesskit>
