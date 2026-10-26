# Native look and feel for skelter on Windows 11 and macOS

Date: 2026-10-26. Status: research for the user; nothing is built. Question asked: "how can we make
svelters-skelter and its widget library look and behave natively on Windows and macOS? What styling
would we need, what support would we need to add?"

Evidence: svelters `683220d1` (skelter crates, `plans/research/{DESIGN,ARCH-WIDGETS,WIDGET-PARITY}.md`,
`skelter/{platform,architecture}.md`), cadatomic at the same day's `master` (`DECISIONS.md`,
`SCOPE.md`, `cadatomic-ui` theme), microsoft-ui-xaml `e43de2a` (`Common_themeresources_any.xaml`,
`ScrollBar_themeresources.xaml`, `DensityStyles/Compact.xaml`), zed `2c335b8`, and the public sources
cited inline. Numbers marked **[measure]** have no published source and need a screenshot from a real
machine.

## 1. Answer in one paragraph

**Split "native" into feel, frame and skin, and do the first two.**
- **Feel** is behaviour: keys, scrolling, focus, menus, dialogs, OS settings. It is mostly built. It
  needs a few wiring fixes and OS reads (§4.1), and it is what users notice.
- **Frame** is the window chrome and the colours and materials the OS owns: Mica, vibrancy, the
  title bar, accent, highlight, high contrast. It is cheap to add, and none of it exists yet (§4.2).
- **Skin** is per-OS control visuals: Fluent 2 controls on Windows, Aqua/Tahoe controls on macOS.
  skelter's styling infrastructure can carry it (§3), but it is two complete theme sets, one of
  which (macOS) has no published metrics and changed shape in a single release (Tahoe). Every other
  custom-drawn toolkit that tried it either dropped it or left it to the community (§2).
- **Recommendation:** feel and frame now. A per-OS skin is a design choice that reverses a user
  decision (DESIGN §7, cadatomic DECISIONS:37: one VS Code Modern look everywhere), so it goes to
  the user (§6). If wanted, it should ship as optional theme sets on the same token pipeline, not as
  a default.

## 2. What other toolkits learned

| Toolkit | Approach | Lesson |
|---|---|---|
| **Slint** 1.16 (2026) | Made Fluent the default on every platform and is deprecating its other styles: "Cupertino in particular has fallen into an uncanny valley where it looks close to native macOS but not close enough"; "We're unifying the visual style, not the behavior" ([blog](https://slint.dev/blog/default-native-style-change)) | Imitation skins cost more than they return |
| **Flutter** | Adapts behaviour per platform (bouncing scroll physics on macOS, selection gestures, toolbars). The look comes from Material unless you use the community `macos_ui`/`fluent_ui` packages | Feel is built in; skin is opt-in |
| **Zed / GPUI** | One brand look. macOS keeps the native traffic lights over a transparent full-size title bar (inset 9,9). Windows draws its own caption buttons with Segoe Fluent Icons glyphs (36 px wide, not 46) with real `HTMAXBUTTON`/`HTCLOSE` hit tests, and Mica/Mica Alt through `DWMSBT_*` | The frame is native, the content is the brand |
| **Qt Quick Controls** | The macOS style draws with the native frameworks at runtime: macOS-only, not customisable, may change between minor Qt versions, some controls fall back to Fusion ([docs](https://doc.qt.io/qt-6.9/qtquickcontrols-macos.html)) | Only real native drawing is really native, and it gives up styling |
| **Avalonia** | Fluent everywhere; native macOS looks are third-party themes over Fluent | Same split as Flutter |
| **Electron** | `backgroundMaterial: mica/acrylic/tabbed`, `vibrancy`, `titleBarOverlay` | The frame is the part apps actually adopt |
| **VS Code, Figma, Blender, Fusion 360, Onshape, JetBrains** | One cross-platform brand; OS dark mode and (sometimes) accent; native window behaviour | The pattern cadatomic already follows |

## 3. Where skelter stands

### 3.1 Decided

- **One look:** VS Code Dark/Light Modern with the affordance layer, Fluent 2 metrics where VS Code
  has no widget (DESIGN §7, ARCH-WIDGETS Q4, cadatomic DECISIONS:37). Inter is bundled "so snapshots
  stay identical on every OS" (DESIGN:91).
- **Native behaviour:** text fields, selection and keyboard follow each OS (cadatomic DECISIONS:36),
  Cmd is the primary modifier on macOS (DECISIONS:180), the macOS global menu bar, OS dialogs for
  OS-style tasks (DECISIONS:38), OS light/dark/high contrast followed (SCOPE W11).

### 3.2 Built and usable for a native look

- **Behaviour policy, one row per OS** (`skelter-widgets/src/interact/policy.rs:183-346`): focus on
  click (off on Mac), drag threshold, double-click and tooltip timing, caret blink, kinetic scroll,
  overlay/classic scrollbars, `button_order`, submenu delay, Alt to the menu bar, `auto_default`.
  The Windows and Mac rows are "from the platform guidelines until WG3w/WG3m pin it" (`:20-23`).
- **Text keys per OS** (`text/keys.rs:221-239`): Cmd and Option motions and the Emacs Ctrl-A/E/K…
  set on Mac; quire word and undo rules per platform.
- **Scroll physics per OS** (`skelter-anim/src/scroll.rs:748-772`): animated wheel on Windows, 1:1
  with OS momentum and a rubber band on macOS; DirectManipulation on Windows touchpads
  (`skelter-platform-win32/src/dmanip.rs`), gesture and momentum phases on macOS
  (`skelter-platform-macos/src/events.rs:138-167`).
- **Text input:** TSF plus `WM_IME_REQUEST` on Windows; the full `NSTextInputClient` on macOS.
- **Chrome hooks:** Windows custom frame with `WM_NCHITTEST` regions and `HTMAXBUTTON` snap layouts
  (`hit_test.rs:1-49`, `window.rs:769-851`); macOS full-size content view with a transparent title
  bar (`skelter-platform-macos/src/window.rs:178-192`); macOS menu bar through muda with the app
  menu and a default Edit menu (`menu.rs:1-93`).
- **Styling hooks:** `@media (-sv-platform: macos|windows|linux)` (`svelters-style/src/media.rs`),
  emulable through `Policy::platform`; `prefers-color-scheme`, `prefers-contrast`,
  `prefers-reduced-motion`; `:backdrop` (window inactive), `:default`, `:focus-visible`, `::part()`;
  gradients, inset and outer `box-shadow`, per-corner radii, transitions.
- **Native dialogs:** file and message boxes through rfd.
- **Accessibility:** AccessKit adapters on both OSes (untested with Narrator/NVDA/VoiceOver).

### 3.3 Defects found while surveying (fix regardless of the look decision)

| # | Defect | Where | Effect |
|---|---|---|---|
| D1 | `WM_SETTINGCHANGE` is handled only on the message-only window. Message-only windows (`HWND_MESSAGE`) receive no broadcasts | `skelter-platform-win32/src/window.rs:707-746`, `app.rs:638-648` | Live dark/light switching never arrives on Windows; only the value at window creation (`:368`) |
| D2 | OS dark/light reaches only the CSS media environment; the widgets' `Theme` service is installed once as `Theme::default()` (dark) and never follows | `skelter-core/src/runner.rs:102-112`, `svelters-skelter/src/app.rs:47` | Built-in widget defaults (the `Ua` tables, tabs, table) stay dark in a light OS unless the host swaps the theme |
| D3 | macOS sends no initial `ThemeChanged` when a window is created | `skelter-platform-macos/src/app.rs:327-348` | The first frame uses the compile-time default scheme |
| D4 | `prefers-reduced-motion` and `prefers-contrast` are never set from the OS | `svelters-style/src/media.rs:66-77` | High contrast and Reduce Motion are ignored on both OSes (cadatomic SCOPE W11) |
| D5 | Text gamma/contrast is DirectWrite's 1.8/1.0 everywhere; `set_text_params` has no caller; the macOS dilation table is empty | `skelter-render/src/gamma.rs:16-19`, `skelter-text/src/raster/mod.rs:17-19` | macOS text draws lighter than native (architecture §9.2 says why) |
| D6 | `acceptsFirstMouse:` always returns true | `skelter-platform-macos/src/view.rs:65-68` | The first click on an inactive window acts on content; AppKit's default only activates the window |
| D7 | The wheel ignores `SPI_GETWHEELSCROLLLINES` | `skelter-platform-win32/src/pointer.rs:178-182`, `views/scroll.rs:644` | The user's scroll-lines setting is ignored |
| D8 | Tabs, table and pickers paint from Rust "Look" structs with hard-coded Light Modern fallbacks; pickers always use the built-in theme; `text-edit` hard-codes VS Code's dark selection colour | `tabs/mod.rs:76-97`, `table/mod.rs:121-142`, `pickers/kinds.rs:246-249`, `text/kind.rs:81-105` | These kinds ignore theme overrides and CSS, contrary to WIDGET-PARITY P9 |

## 4. What to add

### 4.1 Feel (behaviour): OS reads and policy measurements

| Item | Windows | macOS | Size |
|---|---|---|---|
| Theme changes (D1–D3) | Handle `WM_SETTINGCHANGE` (`ImmersiveColorSet`) in the top-level window procedure; also subscribe to `UISettings.ColorValuesChanged` | Send the initial appearance; already observes `viewDidChangeEffectiveAppearance` | S |
| Theme follows the OS (D2) | One host hook: on `ThemeChanged`, `host::set_theme(theme.with_scheme(..))`, unless the app pinned a scheme | same | S |
| High contrast, reduce motion, transparency (D4) | `SPI_GETHIGHCONTRAST` + `WM_SETTINGCHANGE`; `UISettings.AnimationsEnabled`, `AdvancedEffectsEnabled` | `NSWorkspace.accessibilityDisplayShould{IncreaseContrast,ReduceMotion,ReduceTransparency}` + `NSWorkspaceAccessibilityDisplayOptionsDidChangeNotification` | S |
| Policy timings from the OS, not constants | `GetDoubleClickTime`, `SM_CXDRAG/SM_CYDRAG`, `GetCaretBlinkTime`, `SPI_GETWHEELSCROLLLINES`, `SPI_GETMOUSEHOVERTIME` (D7) | `NSEvent.doubleClickInterval`, the `NSTextInsertionPointBlinkPeriodOn/Off` defaults | S |
| Measured policy rows | WG3w: pin the Windows row against WinUI/Win32 references | WG3m: pin the Mac row against AppKit references | M (needs machines) |
| Scrollbars | Fluent overlay bar: 12 px track, 6 px collapsed thumb (offset 2), expand after 0.40 s over 167 ms, contract after 0.5 s + 2 s; Windows "always show scrollbars" setting (`UISettings.AutoHideScrollBars`) | `NSScroller.preferredScrollerStyle` (changes with the pointing device, at runtime, `NSPreferredScrollerStyleDidChangeNotification`): overlay or legacy | S–M |
| Focus traversal | Unchanged (Tab everywhere) | Full Keyboard Access: `NSApp.isFullKeyboardAccessEnabled`. Off means Tab reaches only text fields and lists; Return = default button, Escape and Cmd-. = cancel | S |
| Click-through (D6) | — | `acceptsFirstMouse:` false by default; true only for parts marked click-through (toolbar buttons, as AppKit does) | S |
| Inactive window | Title bar content semi-transparent | Selection turns `unemphasizedSelectedContentBackgroundColor`; controls lose accent tint. `:backdrop` already exists; only tokens are missing | S |
| Menus | Context menus and the menu bar stay drawn (cadatomic's title bar); popups as OS windows (the API exists but nothing calls `create_popup`) with click-outside dismissal and flipping | Standard Window and Help menus (Help search) via muda; native context menus via muda are optional | M |
| macOS secure input | — | `EnableSecureEventInput` for password fields | S |
| Drag source | Landed | Missing (`start_drag` unsupported) | M |
| Notifications, taskbar | notify-rust; `ITaskbarList3` progress for long rebuilds | notify-rust; Dock badge | S each |

### 4.2 Frame (chrome, materials, OS colours)

**Windows 11**
- **Backdrop:** `DwmSetWindowAttribute(DWMWA_SYSTEMBACKDROP_TYPE, DWMSBT_MAINWINDOW)` (Mica), or
  `DWMSBT_TABBEDWINDOW` (Mica Alt, for tabbed shells like cadatomic's), `DWMSBT_TRANSIENTWINDOW`
  (Acrylic) for popups; build 22621+, attribute 1029 on 22000 (what `window-vibrancy` 0.8 does). It
  shows only where our surface is transparent: the DirectComposition root visual needs a
  premultiplied-alpha swap chain and the shell's title-bar and side-bar fills need alpha tokens.
  Mica must turn off when `AdvancedEffectsEnabled` is false or on battery saver.
- **Corners and shadow:** `DWMWA_WINDOW_CORNER_PREFERENCE` (round, round-small for popups) and the
  custom-frame shadow (`DwmExtendFrameIntoClientArea` with a 1 px margin), which SKELTER-win32 left
  for a hardware check.
- **Title bar:** 32 px (48 px with a search box, which is cadatomic's case; it has 36 today), icon 16
  px at 16 px from the edge, caption buttons with Segoe Fluent Icons E921/E922/E923/E8BB (46×32
  widely quoted, **[measure]** `SM_CXSIZE`), red close hover, `HTMINBUTTON`/`HTCLOSE` as well as
  `HTMAXBUTTON` so the system shows the right tooltips, right-click system menu, inactive dimming.
- **Accent:** `UISettings.GetColorValue(Accent, AccentLight1..3, AccentDark1..3)`. Fluent's accent
  button: rest = Accent (light theme Dark1, dark theme Light2 for contrast), hover = next step,
  pressed = the step after. This feeds `--sv-accent*`, `--sv-focus`, `--sv-list-sel` and
  `--sv-selection` only if the app opts in: cadatomic replaces the accent with its own
  (`cadatomic-ui/src/theme/mod.rs:171-197`), so the opt-in is per app (a theme flag
  `accent: Os | Fixed(color)`).
- **High contrast:** a token set built at runtime from `GetSysColor` (`COLOR_WINDOW`, `WINDOWTEXT`,
  `HIGHLIGHT`, `HIGHLIGHTTEXT`, `BTNFACE`, `BTNTEXT`, `GRAYTEXT`, `HOTLIGHT`), replacing the fixed
  VS Code HC palettes when Windows is in a contrast theme, as WinUI does.

**macOS (14–26)**
- **Title bar:** keep the native traffic lights (they get the OS's hover, full-screen menu and
  window-tiling menu for free) and position them with the title bar height
  (`standardWindowButton` frames, Zed's (9,9)/(12,12)); reserve their region in the shell's left
  inset. Never draw fake traffic lights.
- **Materials:** `NSVisualEffectView` (`.sidebar`, `.titlebar`, `.menu`, `.popover`,
  `.hudWindow`) behind the `CAMetalLayer`, showing where our layer is transparent, as on Windows.
  On macOS 26, `NSGlassEffectView` for toolbar groups and `NSBackgroundExtensionView` under side
  bars (look the classes up at runtime so the app still runs on 14/15). Liquid Glass's refraction
  and content-adaptive tint sample what is behind the view, so only the OS can draw it; glass
  inside our own content can only be approximated with a blur shader, which we don't have
  (`backdrop-filter`, §4.4).
- **Window corners:** Tahoe gives toolbar windows a larger radius (reported ~26 pt **[measure]**);
  our content near the corners must inset, or it is clipped. `layoutGuide(for: .safeArea(cornerAdaptation:))`
  gives the inset.
- **Semantic colours:** read `controlAccentColor`, `selectedContentBackgroundColor`,
  `unemphasizedSelectedContentBackgroundColor`, `selectedTextBackgroundColor` (the user's
  highlight colour, separate from the accent), `keyboardFocusIndicatorColor`, `labelColor` and
  its secondary to quaternary steps, `separatorColor`, `controlBackgroundColor`,
  `windowBackgroundColor`. Resolve them under the window's `effectiveAppearance`
  (`NSAppearance.performAsCurrentDrawingAppearance`), re-read on
  `NSSystemColorsDidChangeNotification` and appearance changes. Same opt-in rule as Windows.
- **Increase contrast:** AppKit's high-contrast appearances (`.accessibilityHighContrastAqua`,
  `…DarkAqua`) resolve the same semantic colours; map them onto the HC token sets.

### 4.3 Skin (per-OS control visuals), if the user wants it

This is the styling work the question asks about. skelter's three token layers already separate
colour roles (L2 `--sv-*`) from component tokens (L3 `--sv-<kind>-<part>`), so a skin is data plus
a few Rust shapes, chosen by a `Theme` variant or `@media (-sv-platform)`.

**Windows 11 / Fluent 2 skin** (published, from WinUI's XAML; cheap to get right)

| Token | Value | Notes |
|---|---|---|
| Font | Segoe UI Variable (system, `opsz` automatic, wght 400/600) | Needs `font-variation-settings` or automatic `opsz` in skelter-text |
| Type ramp | Caption 12/16, Body 14/20, Body Strong 14/20 600, Subtitle 20/28 600, Title 28/36 600 | VS Code uses 13 px body |
| Control radius / overlay radius | 4 px / 8 px; 0 where halves meet (split button) | Same as today's shape tokens |
| Control height | 32 px standard, 24 compact (`TextControlThemeMinHeight`), list rows 40/32, tree rows 32/24 | cadatomic's Compact density ≈ WinUI Compact |
| Elevation stroke | A 3 px vertical gradient border: dark `#18FFFFFF`→`#12FFFFFF` (brighter top), light `#29000000` at the bottom; HC solid `WindowText` | Needs a gradient on the border, or two borders (§4.4) |
| Fills | `ControlFillColorDefault` dark `#0FFFFFFF`, light `#B3FFFFFF`; Secondary/Tertiary for hover/pressed | Alpha fills over Mica |
| Focus | 2 px outer (dark `#FFFFFF`, light `#E4000000`) + 1 px inner (`#B3000000` / `#B3FFFFFF`), 1 px margin | Needs a double ring (§4.4) |
| Accent button | Accent / Light1 / Dark1 by state (see §4.2) | |
| Check box, radio, switch | 20 px box, radius 4; radio 20 px; switch 40×20 with a growing knob | Shapes in `controls/checkbox.rs`, `radio.rs`, `switch.rs` |
| Motion | 83 / 167 / 250 ms; enter `cubic-bezier(0,0,0,1)`, exit `cubic-bezier(1,0,1,1)` | today one 100/150 ms ease-out |
| Glyphs | Segoe Fluent Icons for chevrons, check, close (system font, not bundled) | Icons stay ours; only control glyphs change |

**macOS skin** (unpublished; expensive and version-dependent)

| Token | Value | Notes |
|---|---|---|
| Font | SF Pro from the system (`NSFont.systemFont`, `opsz` Text/Display automatic); body 13 pt, small 11, mini 9 | SF is licensed for Apple platforms only: load from the system, never bundle |
| Control sizes | mini / small / regular / large / extra large (26). Tahoe made mini–regular "slightly taller" than 15 and large/extra large capsules ([WWDC25-310](https://developer.apple.com/videos/play/wwdc2025/310/)) | Heights **[measure]** per release; `prefersCompactControlSizeMetrics` restores 15's metrics, a second set |
| Push button | Rounded rect (regular), capsule (large+); default button filled with the accent; bezel gradient pre-26 | One look per macOS major version |
| Pop-up, combo, stepper, segmented | Distinct shapes with double-arrow indicators | Each a new part set |
| Focus ring | `keyboardFocusIndicatorColor` halo outside the control, following its shape, ~3 pt **[measure]** | Needs a blurred outline or an outer spread shadow |
| Selection | Accent when the window is key, unemphasised grey when not | `:backdrop` + tokens |
| Lists, outline views | Inset rounded selection (macOS 11+ "inset" style), margins **[measure]** | Table and tree parts |
| Motion | Short ease-in-out, no published curves | **[measure]** |

**Cost.** Windows: about one lane (the ~55 role tokens, ~30 component-token groups, 4–6 Rust shape
changes, a Segoe UI Variable path, VIS1 references captured from WinUI 3 Gallery). macOS: two to
three lanes plus a re-measurement every macOS major release, references captured on a Mac for 15 and
26, and a decision whether to follow Tahoe or `prefersCompactControlSizeMetrics`. Both: the
determinism rule (DESIGN:91) holds only with the bundled fonts, so tests pin Inter and the native
skins are checked by VIS1 per OS, not by the shared goldens.

**Domain chrome stays ours.** Dock tabs (berth), the PCB and 3D viewports, property grids, the
command palette and the icon set have no native counterpart; a "native" skin only changes standard
controls. That limits how native the app can look: cadatomic would look like Fusion 360 or
SolidWorks with native buttons, not like a native app.

### 4.4 Engine and widget support

Needed for the frame (§4.2), and for the skin (§4.3) if wanted:

| Support | Where | For |
|---|---|---|
| Transparent window surfaces (premultiplied alpha through DComp and the root `CAMetalLayer`), alpha-aware clear | `skelter-render`, `skelter-compositor`, both platform crates | Mica, vibrancy, glass (frame) |
| OS colour provider: a platform API `system_colors() -> SystemColors` + change event, mapped onto `--sv-*` roles behind an app opt-in | `skelter-platform/src/api.rs`, `skelter-widgets/src/theme` | Accent, highlight, HC (frame) |
| `MediaEnv` fed from the OS (D4), plus `prefers-reduced-transparency` | `svelters-style/src/media.rs`, runner | Feel, frame |
| `system-ui` resolving to Segoe UI Variable / SF Pro, automatic `opsz`, `font-variation-settings` | `skelter-text/src/fonts.rs`, `svelters-style` tables | Skin |
| Per-OS text params (D5): read `IDWriteRenderingParams1` on Windows; contrast 0 + dilation table on macOS | `skelter-render/src/gamma.rs`, `skelter-text/src/raster` | Feel (text that matches the OS) |
| Per-side border colours (today one colour from `border-top-*`) or `border-image` gradients | `skelter-core/src/style/cascade.rs:404-414` | Fluent elevation stroke (skin) |
| Double focus ring (outline + `outline-offset` + inner ring, or `box-shadow` spread rings drawn over content) | `skelter-core` style, `paint/boxes.rs` | Fluent and macOS focus (skin) |
| `backdrop-filter: blur() saturate()` over the layer below | `skelter-render` (a blur pass per backdrop node) | In-content acrylic/glass approximations (optional) |
| `@keyframes`/`animation` | `svelters-style`, `skelter-anim` | Progress/indeterminate and spinner skins (optional) |
| Look structs to CSS (D8) | `tabs/`, `table/`, `pickers/`, `text/kind.rs` | Any theme, native or not |
| `Theme` variants beyond `Theme::vscode` (e.g. `Theme::fluent()`, `Theme::macos(version)`) and a host hook that follows the OS | `theme/mod.rs:548-596`, `host.rs` | Skin |
| Install the token sheet and `base.css` on the skelter path (today only svelters-egui uses them) | `svelters-skelter/src/app.rs` | Author CSS sees `var(--sv-*)` without each host adding it |

## 5. Suggested order

1. **Fix D1–D4 and D6–D7** (one small lane, both OSes): theme changes arrive, the widget theme
   follows them, contrast and motion come from the OS, macOS click-through and Windows wheel lines
   are right.
2. **OS colour provider and opt-in accent** (one lane): a platform API, a theme flag, the HC token
   set from system colours on Windows and AppKit's HC appearances on macOS.
3. **Frame lane per OS:** Windows Mica Alt + corners + shadow + full caption hit tests; macOS
   positioned traffic lights + `NSVisualEffectView` side bar and title bar materials. Needs
   transparent surfaces first. Hardware checks go to REVISIT.
4. **Feel measurements:** WG3w and WG3m pin the policy rows; OS timings replace constants; Full
   Keyboard Access; Window/Help menus; macOS drag source and secure input.
5. **D8 and the text params (D5)**, which every theme needs.
6. **Only if the user chooses a native skin (§6):** engine support from §4.4 (per-side borders,
   double rings, variable fonts), then a Fluent 2 theme set, then a macOS theme set.

## 6. Questions for the user

These are design choices or reverse a user decision, so the coordinator brings them to the user:

- **Q1. Native skin or one brand?** Recommendation: one brand (today's VS Code Modern + affordances)
  with native feel and frame. A Fluent 2 skin on Windows is cheap and close to today's look, so it
  could be an optional theme. A macOS skin is expensive, version-bound and lands in Slint's uncanny
  valley; do not build it unless macOS users are a priority.
- **Q2. Follow the OS accent?** cadatomic fixes its own accent (`#3474DA`/`#1F6FE5`). Recommendation:
  keep the brand accent for primary buttons and the CAD selection, take the OS highlight colour for
  text selection, and offer "use system accent" as a setting.
- **Q3. System UI font?** Inter is bundled for identical snapshots. Recommendation: offer `system-ui`
  (Segoe UI Variable / SF Pro) as a setting, default off for now; tests always pin Inter.
- **Q4. Mica/vibrancy behind panels?** Translucent shells cost contrast on a dense CAD UI.
  Recommendation: Mica Alt behind the title bar and tab strip only on Windows; vibrancy only under
  the macOS side bar; content panels stay opaque.
