# egui ecosystem and the svelters-egui element catalog (M7 research)

Research slice for M7: where egui is as of 2026-10-21, which ecosystem crates exist and are maintained, which elements svelters-egui should ship and from what source, how our X6 styling maps onto egui, and how to live with egui's release cadence. Sibling slices cover the retained scene graph, JS custom widgets, Rust export macros and compiler/devex. This note does not cover them, but it marks where its choices depend on them.

Sources: crates.io API (queried 2026-10-21 with a throwaway script; numbers in §3), the egui 0.37.0 / eframe 0.37.0 / epaint 0.37.0 / egui_extras 0.37.0 sources in `~/.cargo/registry`, the [egui releases page](https://github.com/emilk/egui/releases), the GitHub API for repo activity and issues, and the sibling projects `~/projects/egui-workbench` (egui 0.35, egui_dock 0.20, egui_plot 0.36, egui_table 0.9, egui-phosphor 0.13), `~/projects/egui_tiles-fork` and `~/projects/egui_mcp-fork` (both ported to 0.37.0, patch series in their `FORK.md`).

## 1. Summary

- **egui is at 0.37.0 (2026-10-15)**, MSRV 1.95 (our toolchain is 1.97). It has had a breaking minor release every 2 to 3 months for two years (0.28 in 2024-07 to 0.37 in 2026-10: ten minors in 27 months). Every minor breaks something, and the ecosystem takes 0 to 3 months to follow.
- **The core has grown into a credible base.** It now has text shaping (harfrust), system font fallback with colour emoji and CJK, AccessKit trees good enough for agents to drive apps (`egui_inspection` + `egui_mcp`), `Modal`, `Sides`, `Popup`/`MenuBar` with keyboard navigation, `RangeSlider`, `TextEdit` completion popups, atoms (`WidgetAtom`/`ContainerAtom`), `Classes` on `Ui`, and an **experimental `StyleProvider` theme system keyed by classes and widget state**.
- **Still missing:** bidi/RTL text (PR [#8577](https://github.com/emilk/egui/pull/8577) is open, tracking issue [#1016](https://github.com/emilk/egui/issues/1016)), OS-level modal or transient windows, and native menus. Core also has no tabs, splitter, tree, table (that lives in egui_extras), switch, or stack widget.
- **Element catalog.**
  - Tier 0 is egui core only: about 25 elements, of which 5 are composites we build.
  - Tier 1 uses core plus emilk/rerun-maintained crates: `egui_extras` (table, date picker, image loaders), `egui_table`, `egui_tiles`, plus `rfd` for native file and message dialogs. We build tabs, the splitter and the tree ourselves.
  - Tier 2 is optional packs behind cargo features: `egui_plot`, `egui_dock`, `egui-snarl`, `egui_commonmark`, `egui_code_editor`.
- **Styling.** egui has no stylesheet, but svelters-egui retains the tree, so the backend can run the selector match itself. It applies the computed declarations as per-node `Style`/`Visuals` overrides, and later through egui's `StyleProvider`. `--sv-*` tokens map almost one to one onto `Visuals`.
- **Maintenance.**
  - Pin one egui minor per svelters-egui release line.
  - Re-export `egui` from svelters-egui so that every pack uses the same copy.
  - Enforce a single copy with a `links` key on our pack-API crate, plus `cargo tree -d` in `svelters doctor`.
  - First-party elements depend only on crates that release in the same week as egui.
  - Everything else is a feature-gated pack that may lag, using the same fork-and-patch workflow as `egui_tiles-fork`.

## 2. egui core state (0.37.0)

### 2.1 Releases and breakage

| Minor | Date | Highlights that matter to us | Notable breaks |
| --- | --- | --- | --- |
| 0.32 | 2025-07-10 | atoms (`Atom`, `IntoAtoms`, `AtomLayout`); `Popup` and menu rewrite | popup/menu API |
| 0.33 | 2025-10-09 | trait `Plugin` API, better kerning | |
| 0.34 | 2026-03-26 | `skrifa` + `vello_cpu` font rasterization (hinting, variable fonts); `Ui` is the entry point (`Context::run_ui`, `App::ui`/`App::logic`); unified `Panel`; per-widget styles | `App::update` gone, `SidePanel`/`TopBottomPanel` replaced, MSRV 1.92 |
| 0.35 | 2026-06-25 | `egui_inspection` + `egui_mcp`; `Classes` on `Ui`; IME composition underline; harfrust shaping (kerning, ligatures); nested atom layouts; `ViewportBuilder::with_monitor` | deprecated items removed, `impl Into<f32>` args removed |
| 0.36 | 2026-08-05 | iOS/Android IME in eframe web; OS window chrome follows app theme; `BoxedWidget` | `Modifiers` moved to an event, `clip_rect_margin` removed, MSRV 1.95 |
| 0.37 | 2026-10-15 | system font provider (any system glyph, colour emoji); `CompletionPopup`; `RangeSlider`; `Aligned`; experimental `StyleProvider` themes; `WidgetAtom`/`ContainerAtom`/`AtomLayoutStyle`; menu arrow-key navigation, explicit popup sizing passes; AccessKit: accessible names, `WidgetType` is now `accesskit::Role`, TextEdit set-value; kittest video | `default_fonts` not a default feature; `Id::new` deprecated (`Id::unique` / `IdSalt::new`); `Panel` constructors take `impl AsIdSalt`; screenshots by callback |

Patch releases (0.36.1, 0.36.2) are bug fixes and are safe to take. Breaking changes are widespread but shallow: renames, moved types, new builder signatures. The 0.34 `App::update` → `App::ui` change is the size of break to expect about once a year. Source: [releases](https://github.com/emilk/egui/releases), crates.io version history.

### 2.2 Backends and windows

- eframe 0.37 default features: `wgpu` (default renderer), `accesskit`, `clipboard`, `default_fonts`, `links`, `system_fonts`, `wayland`, `x11`, `web_screen_reader`. `glow` is opt-in, and the sibling projects use `glow` for CI and smaller binaries. Optional features: `persistence`, `inspection` (egui_inspection plugin) and `experimental` (exposes the theme system).
- Windowing goes through winit 0.30.13.
- **eframe owns the event loop** (`run_native`), so egui is the toolkit loop that P6 calls authoritative. Mapping our ABI onto it:
  - `schedule_wake` → `ctx.request_repaint_after` (already in `backend-abi.md`);
  - thread wake → `ctx.request_repaint()`, which is thread-safe;
  - rAF → one frame per `request_repaint`;
  - checkpoints run in `App::logic` before `App::ui`.
- For the SVR14 embedding mode (render into a caller's `&mut Ui`), `Context::run_pass`/`root_ui` (0.37) and `set_repaint_observer` are the hooks.
- **Multi-viewport.** Real OS windows come from `show_viewport_deferred` (each window has its own pass, and the choice is efficient) or `show_viewport_immediate` (repaints with the parent).
  - Where real windows are not available (web, some backends), egui falls back to embedded viewports drawn as an in-app `egui::Window`.
  - `ViewportBuilder` has title, size, decorations, window level, monitor, taskbar and similar options. It has **no parent/transient-for and no OS modality**. Our X7 `show="modal"` therefore becomes either an in-window `Modal` or a deferred viewport plus `Modal`-style input blocking in the parent. That is a documented degradation.
  - Tracking issue: [#3556](https://github.com/emilk/egui/issues/3556).

### 2.3 Text, fonts, IME, bidi

- **Shaping and fonts.** epaint 0.37 shapes with `harfrust` (a HarfBuzz port) over `skrifa` and supports variable font axes and hinting targets. A `FontProvider` chain resolves fonts: configured `FontDefinitions` come first, then discovered fonts. eframe's `system_fonts` feature (`egui_system_font_provider`) discovers system fonts for any missing glyph, so **CJK and colour emoji render with no bundled fonts** on native. `default_fonts` (Ubuntu-Light, Hack, Noto Emoji, the emoji icon font) is no longer on by default in `egui`. Turn it on for deterministic screenshot tests.
- **Bidi/RTL: not supported.** `Layout::right_to_left` exists for widget order, but text is not reordered. Arabic or Hebrew renders in logical order. The epaint source has `TODO(emilk): heed bidi characters`. [PR #8577](https://github.com/emilk/egui/pull/8577) (open since 2026-09-17) adds UAX #9 levels via `unicode-bidi`, RTL shaping and visual reordering. It is likely to land in 0.38, but we should not promise RTL for M7.
- **IME:**
  - desktop IME works through winit;
  - 0.35 underlines the composition text and shows the cursor;
  - open bugs: Linux X11 + Fcitx5 pre-edit not shown ([#7975](https://github.com/emilk/egui/issues/7975)), macOS Korean hanja candidates ([#7974](https://github.com/emilk/egui/issues/7974));
  - mobile IME works in eframe web (0.36).
- **Text selection.** Labels can be selected (`Interaction::selectable_labels`) and selection can span widgets. 0.37 adds selection by double/triple click and drag.

### 2.4 Accessibility and testing

- AccessKit 0.25 is a hard dependency of `egui`, and `accesskit` is on by default in eframe. It uses UIA on Windows, NSAccessibility on macOS and AT-SPI on Linux, through `accesskit_winit` 0.34.
- 0.37 cleaned up the trees for popups, menus, modals and panels. It added accessible names, hides invisible widgets, uses `Role` directly, and supports AccessKit set-value on TextEdit.
- Open gaps:
  - `egui_extras` tables expose no table/row/cell roles ([#8700](https://github.com/emilk/egui/issues/8700));
  - an occluded or minimized native window never answers AccessKit ([#8671](https://github.com/emilk/egui/issues/8671)).
- **Our retained graph has the semantics before the frame starts** (SVR14's rule), so svelters-egui sets names and roles from node props instead of guessing from `Response`.
- **`egui_kittest` 0.37** drives a headless egui through AccessKit queries (`get_by_label`, `get_by_role`). It takes wgpu snapshot images (`snapshot` feature) and can record video.
  - It covers our generated fact tests (set prop → run a frame → read the AccessKit node or pixels) and the gallery screenshot tests.
  - It needs no Xvfb, because the wgpu path can use a software adapter. The sibling projects run it with `glow`/`wgpu` features.
- **`egui_inspection` + `egui_mcp`** (0.37 / 0.3) expose a live app's AccessKit tree and accept input over MCP. That matches our UIA-driven `--selftest` on WinUI and gives agents an inspector for free.

### 2.5 Layout, sizing, popups, atoms

- **Layout is single pass.** A widget allocates as it is called, and the `Layout` of a `Ui` decides the direction (`left_to_right`, `top_down`, `right_to_left`, `bottom_up`), main/cross alignment, justify and wrapping. A parent's size comes from the previous frame. When a size is unknown, egui runs a **sizing pass** (`Ui::is_sizing_pass`), or a widget calls `ctx.request_discard()` and the frame is re-run (`Options::max_passes`, default 2). 0.37 gives popups explicit sizing passes.
- **Consequence for us:**
  - GTK-style `hexpand`/`vexpand`/`homogeneous`, or a CSS-flex `grow`, have no core equivalent beyond `main_justify`/`cross_justify` and `Sides`/`Aligned`.
  - Options:
    1. map onto `egui::Layout` and accept the differences;
    2. use `egui_flex` (lucasmerlin, already on 0.37);
    3. run `taffy` 0.15 over our retained tree with leaf sizes measured in the previous frame, which is what `egui_taffy` does (it lags at 0.36).
  - The scene-graph slice decides. From the catalog side, `box` needs `orientation`, `spacing`, `align`/`justify`, `wrap` and child `parent:grow`/`parent:align`, whichever engine sits behind them.
- **`Sides`** puts left and right groups with a dynamic gap; it is the header-bar/status-bar primitive. **`Aligned`** (0.37) aligns content of unknown size.
- **Atoms** are the content model of buttons, checkboxes, menu items, text-edit prefix/suffix and window titles. The atom kinds are text, image, an icon glyph, space/grow, custom `Paint` (0.37) and nested `ContainerAtom` layouts.
  - A `<button>` with children (an image and text) maps onto atoms, not onto a nested `Ui`.
  - egui does not support interactive widgets nested inside a button, so the compiler should report a diagnostic there.
- **Popups and menus (0.32 rewrite, 0.37 polish).**
  - `Popup` has anchors and close behaviour.
  - Menus come as `MenuBar`, `MenuButton`, `SubMenuButton`, `Response::context_menu`, with arrow-key navigation and `Button::shortcut_text` for accelerator labels.
  - Shortcuts are not bound by the menu. The app calls `input_mut(|i| i.consume_shortcut(..))`, so our `action shortcut=` handling lives in the backend's per-frame action table.
- **`Modal`** (since 0.31) is centred, has a backdrop, blocks input and stacks.
- **Tooltips** are per response (`on_hover_text`, `on_hover_ui`, `on_disabled_hover_text`). They are not elements.
- **`Scene`** is a pan/zoom container for arbitrary widgets. Node graphs, our canvas zoom and diagram views can use it.

### 2.6 Styling model

- `Style` holds the text styles, `Spacing` (item spacing, button padding, margins, widths), `Interaction`, `Visuals`, `animation_time`, `scroll_animation` and other settings.
- `Visuals` holds:
  - `dark_mode`, the panel and window fills, `extreme_bg_color` (text-edit background), `faint_bg_color`, `code_bg_color`;
  - `hyperlink_color`, `warn_fg_color`, `error_fg_color`, `weak_text_color`;
  - selection, the text cursor, window and menu corner radii, shadows, `disabled_alpha`;
  - `widgets` with five `WidgetVisuals` (`noninteractive`, `inactive`, `hovered`, `active`, `open`), each with `bg_fill`, `weak_bg_fill`, `bg_stroke`, `corner_radius`, `fg_stroke` and `expansion`.
- **Scoping.** Every `Ui` has its own `Arc<Style>`, so `ui.scope(|ui| ui.style_mut()...)` or `UiBuilder::style` gives a subtree its own style. Many builders also take overrides: `Button::fill/stroke/corner_radius/min_size`, `RichText::color/size/strong/italics/font`, `Frame::fill/stroke/inner_margin/corner_radius/shadow`, `TextEdit::text_color/background_color/margin`. `Ui::set_opacity`/`multiply_opacity` gives subtree opacity.
- **Classes and themes.**
  - Since 0.35, `Ui`s carry `Classes` (`UiBuilder::class`, `UiStack::has_class`).
  - In 0.37, `Button`, `Checkbox`, `DragValue`, `TextEdit`, `Separator`, `ComboBox` and `UiBuilder` implement `HasClasses`.
  - Behind the `experimental` feature, a `StyleProvider<S>` plugin computes a typed per-widget style (`ButtonStyle`, `TextEditStyle`, `CheckboxStyle`, `SeparatorStyle`, built on `AtomLayoutStyle`) from `StyleArgs { classes, state, stack, style }`. `state` is `Noninteractive`/`Inactive`/`Hovered`/`Active`, taken from the previous frame's response.
  - This is egui's own route towards selector-based theming. Our class-based X6 model maps onto it directly once it stabilizes.
- **Dark mode.** `ThemePreference::System` follows the OS. Since 0.36 the window chrome follows the app theme.
- **Animation.** `ctx.animate_bool_with_time` and `animate_value_with_time` are keyed by `Id`, and `style.animation_time` is the global duration (0 gives reduced motion). Scroll animation and rubber-band overscroll arrived in 0.37. `egui_animation` (hello_egui) adds easing and collapse/transition helpers. A Svelte `transition:`/`animate:` running on our rAF works, because every frame requests the next one.

## 3. Ecosystem inventory

Columns: latest version and date from crates.io on 2026-10-21; `egui` is the requirement of that version; "12 mo" counts releases in the last 12 months; DL is recent (90-day) downloads. Verdict key:

- **core-dep**: a first-party element may depend on it;
- **pack**: wrap as an optional first-party pack behind a cargo feature;
- **user**: leave to user packs;
- **skip**: superseded or dead.

| Crate | Version | Released | egui | License | 12 mo | DL (90 d) | Maintainer | Verdict |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| egui / eframe | 0.37.0 | 2026-10-15 | — | MIT/Apache | 11 | 7.0 M | emilk / rerun | base |
| egui_extras | 0.37.0 | 2026-10-15 | =egui | MIT/Apache | 11 | 2.5 M | emilk (same repo) | **core-dep**: `TableBuilder` (virtual rows), `DatePickerButton` (now `jiff`), image loaders (file/http/svg/gif/webp/image), `syntect` highlighting, `StripBuilder`, `SvgGlyph` |
| egui_kittest | 0.37.0 | 2026-10-15 | =egui | MIT/Apache | 11 | 0.8 M | emilk | **dev-dep**: fact tests, screenshots |
| egui_inspection / egui_mcp | 0.37.0 / 0.3.0 | 10-15 / 10-19 | ^0.37 | MIT/Apache | 5 / 3 | 0.16 M / 0.09 M | emilk / rerun | **dev feature**: inspector, agent driving |
| egui_plot | 0.38.0 | 2026-10-19 | ^0.37 | MIT/Apache | 5 | 1.1 M | emilk (own repo) | **pack** (`plot`): line/scatter/bar/box plots, linked axes, legends |
| egui_table | 0.11.0 | 2026-10-19 | ^0.37 | MIT/Apache | 6 | 0.40 M | rerun (`egui_crates` monorepo) | **core-dep** (feature on by default): virtual rows *and* columns, sticky columns, grouped headers, prefetch for lazy data |
| egui_tiles | 0.18.0 | 2026-10-19 | ^0.37 | MIT/Apache | 6 | 0.62 M | rerun (`egui_crates`); user fork exists | **pack** (`tiles`): tiling (tabs/linear/grid), drag-and-drop, used by Rerun |
| egui_dock | 0.21.1 | 2026-08-06 | ^0.36 | MIT | 7 | 0.30 M | moved to `anhosh/egui_dock`; used by egui-workbench | **pack** (`dock`): IDE-style docking, floating windows, serde. Lags one minor today |
| egui_commonmark | 0.26.0 | 2026-10-15 | ^0.37 | MIT/Apache | 4 | 0.58 M | lampsitter | **pack** (`markdown`): pulldown-cmark, optional syntect code blocks |
| egui_code_editor | 0.4.2 | 2026-10-07 | ^0.36 | MIT | 17 | 20 k | p4ymak | **pack** (`code-editor`): line numbers, simple highlighting, completion. Small; alternative: `TextEdit` + `egui_extras::syntax_highlighting` layouter |
| egui-snarl | 0.12.0 | 2026-08-26 | ^0.36 | MIT/Apache | 4 | 17 k | zakarumych | **pack** (`node-graph`): typed pins, serde, active |
| egui_node_graph / egui_node_graph2 | — / 0.7.0 | — / 2024-11 | ^0.29 | MIT | 0 | <1 k | dead | **skip**, use snarl |
| egui_node_editor, egui-graph-edit | 0.11 / 0.9.1 | 2026-08 / 07 | 0.36 / 0.35 | MIT | 2–3 | <1 k | small | user |
| egui_graphs | 0.32.0 | 2026-08-31 | ^0.36 | MIT | 4 | 0.15 M | blitzarx1 | user (petgraph visualization) |
| egui_taffy | 0.14.0 | 2026-08-27 | ^0.36 | MIT | 4 | 7 k | PPakalns | reference only; if we use taffy, use it directly on our tree |
| egui_flex | 0.9.0 | 2026-10-19 | ^0.37 | MIT | 4 | 7 k | lucasmerlin (hello_egui) | candidate `box` engine (scene-graph slice decides) |
| egui_virtual_list | 0.13.0 | 2026-10-19 | ^0.37 | MIT | 4 | 6 k | hello_egui | reference: variable-height virtualization; core `ScrollArea::show_rows` is enough for the uniform `virtuallist` |
| egui_dnd | 0.18.0 | 2026-10-19 | ^0.37 | MIT | 4 | 0.36 M | hello_egui | pack candidate for reorderable lists (`parent:draggable`) |
| egui_animation, egui_form, egui_router, egui_inbox, egui_suspense, egui_infinite_scroll | 0.10–0.14 | 2026-10-19 | ^0.37 | MIT | 4 each | varies | hello_egui | user; `egui_animation` is a possible helper for transitions |
| egui_ltreeview | 0.9.0 | 2026-08-23 | ^0.36 | MIT | 5 | 52 k | LennysLounge | **pack** (`tree`) or vendor. Selection, multi-select, DnD, keyboard nav. Lags one minor |
| egui-file-dialog | 0.15.0 | 2026-08-08 | ^0.36 | MIT | 4 | 0.16 M | jannistpl (pushed 10-16) | **pack** (`file-dialog-egui`): in-app dialog for platforms without portals/web |
| egui_file | 0.28.0 | 2026-08-17 | ^0.36 | MIT | 6 | 0.12 M | Barugon (codeberg) | skip (older, simpler) |
| rfd | 0.17.2 | 2026-01-12 | none | MIT | 4 | 14.6 M | PolyMeilex | **core-dep** (`native-dialogs`): native open/save/folder pickers + message dialogs, async; portal or GTK3 on Linux, IFileDialog, NSOpenPanel. No egui dependency, so it never lags |
| muda | 0.21.2 | 2026-10-08 | none | MIT/Apache | 10 | 17.9 M | tauri | **pack** (`native-menus`): NSMenu on macOS, HMENU on Windows. Linux needs GTK3, so it stays off there |
| tray-icon | 0.26.1 | 2026-10-07 | none | MIT/Apache | 13 | 16 M | tauri | user |
| egui-notify / egui-toast | 0.23.0 / 0.22.0 | 2026-09 / 08 | ^0.36 | MIT | 2 / 5 | 46 k / 25 k | small | user; a toast is easy to build on `Area` |
| egui-modal | 0.6.0 | 2024-12 | ^0.30 | MIT | 0 | 5 k | dead | **skip**, core `Modal` |
| egui-phosphor | 0.14.0 | 2026-09-10 | ^0.36 | MIT/Apache | 4 | 0.14 M | amPerl; used by egui-workbench | **pack** (`icons-phosphor`); the font itself is MIT, and the crate only registers it, so we could vendor the TTF |
| egui_material_icons | 0.9.0 | 2026-10-19 | ^0.37 | MIT (font Apache-2.0) | 4 | 44 k | hello_egui | **pack** (`icons-material`) |
| catppuccin-egui | 5.7.0 | 2025-11-15 | ≤0.33 (one feature per egui version) | MIT | 1 | 19 k | catppuccin org, stale | skip as a dep; port its palettes as `--sv-*` token sets |
| egui_colors | 0.14.0 | 2026-10-16 | ^0.37 | MIT | 4 | 2 k | small | reference: generates Radix-style 12-step scales from a seed, a model for token generation |
| egui-theme-switch | 0.10.0 | 2026-10-17 | ^0.37 | MIT/Apache | 6 | 1 k | codeberg | user |
| egui_styled | 0.8.0 | 2026-07-02 | ^0.35 | MIT/Apache | 16 | <1 k | small | reference: Tailwind-like utility styling with per-state styles and tokens |
| egui-aesthetix, egui-theme, egui_themes | 0.1–0.2 | 2022–24 | ≤0.29 | mixed | 0 | tiny | dead | skip |
| zeus-theme | 0.5.0 | 2026-08-22 | ^0.36 | MIT/Apache | 12 | <1 k | small | skip |
| transform-gizmo-egui | 0.11.0 | 2026-08-17 | ^0.36 | MIT/Apache | 4 | 22 k | urholaukkarinen | user (3D); MSRV 1.96 |
| egui-gizmo | 0.16.2 | 2024-03 | ^0.26 | non-standard | 0 | 10 k | superseded | skip |
| egui_json_tree | 0.17.0 | 2026-08-15 | ^0.36 | MIT/Apache | 3 | 58 k | dmackdev | user |
| egui-probe, egui_struct | 0.13 / 0.4.2 | 2026-08 / old | 0.36 / old | MIT(/Apache) | 3 / 0 | 24 k / — | — | user (struct → UI derive; not our model) |
| egui-dropdown, egui_autocomplete | 0.16 / 15.0 | 2026-09 | ^0.36 | MIT | 3 / 4 | 6 k / 5 k | small | skip: core `CompletionPopup` (0.37) covers it |
| egui_double_slider | 4.0.0 | 2026-08-19 | ^0.36 | Apache-2.0 | 5 | 2 k | small | skip: core `RangeSlider` (0.37) |
| egui_knob | 0.6.3 | 2026-09-01 | ^0.36 | MIT | 15 | <1 k | small | user |
| egui-data-table | 0.11.0 | 2026-08-01 | ^0.35 | non-standard | 3 | <1 k | small | skip (license) |
| egui_selectable_table | 0.9.0 | 2026-10-17 | ^0.37 | MIT | 4 | <1 k | small | user |
| egui_logger, egui_tracing | 0.13 / 0.3 | 2026-10 / 04 | 0.37 / 0.34 | MIT / Unlicense | 8 / 1 | — | small | user |
| egui-video, egui_term, egui_tabs, egui-datepicker, egui_nerdfonts, egui_cosmic_text, egui_twemoji | various | 2022–2025 | ≤0.33 | various | 0–1 | tiny | stale | skip |

**What the egui-version column shows.** On 0.37 already (≤4 days after the egui release) are egui_extras, egui_kittest, egui_inspection, egui_plot, egui_table, egui_tiles, egui_commonmark, every hello_egui crate (lucasmerlin releases them in lockstep), egui_material_icons, egui_colors and egui-theme-switch. Still on 0.36 are egui_dock, egui-snarl, egui_ltreeview, egui-file-dialog, egui_code_editor, egui-phosphor, egui-notify, egui-toast, egui_taffy and transform-gizmo. Their lag has historically been 1 to 8 weeks, and in that window the user ports them with a one-line requirement bump (the `egui_tiles-fork` patch had no source change). Anything still on 0.33 or older is dead for our purposes. All licenses we would ship are MIT or MIT/Apache-2.0, which is compatible with our MIT. Exclude egui-gizmo and egui-data-table, whose license files are non-standard.

## 4. Element catalog

### 4.1 Paradigm differences that shape every element

1. **Everything is painted by egui.** svelters-egui has no native widgets, so the app looks like egui on every OS. Accessibility comes from AccessKit, and how faithful it is depends on the role and name we supply.
2. **All props are `set_only`, and observation is a frame diff.** Interaction state lives in our retained node: the widget writes into `&mut value` that the node owns. After the frame, the backend compares the value with the last one reported and queues a `changed` batch, which is the R1 contract. `bind:` lowers to this for every bindable prop. The R0 observe facts are `exact` by construction (`mechanism: frame`), since the backend sees every write. REACTIVITY.md already says this.
3. **egui keeps some state in `Memory`, keyed by `Id`:** scroll offsets, collapsing state, the text cursor and selection, combo open state, popup open state, window positions. Ids must come from our stable node identity, through a salt per handle, so state survives HMR when identity survives. 0.37 deprecates `Id::new` in favour of `Id::unique`/`IdSalt`.
4. **Events come from `Response`, after the widget call.**
   - `clicked`, `secondary_clicked`, `double_clicked`, `changed`, `gained_focus`/`lost_focus`, `hovered`, `drag_started`/`dragged`/`drag_stopped`, `drag_delta`.
   - Keys and scroll come from `ctx.input`.
   - Nothing can call JS while a `Ui` borrow is live (SVR14's re-entrancy rule).
   - So **sync handlers (`preventDefault`, `e.result`) are only possible where the backend can defer the effect to the next frame.** Examples: veto a window close (`ViewportCommand::CancelClose` in `close_requested`), or reject a text edit by restoring the previous buffer. Each sync event needs its own design and a `sync: false` default elsewhere.
5. **Tooltips, context menus and cursors are per response.** `tooltip=` is a prop on every element, and `<tooltip>` is an optional rich-content child slot (`on_hover_ui`). `context-menu` attaches to its parent's response, the same model X4 uses. `cursor=` maps onto `Response::on_hover_cursor`.
6. **Menus are in-window.** There is no native macOS menubar unless we add `muda` (`native-menus` feature: NSMenu/HMENU, off on Linux). Shortcuts are a per-frame action table that the backend consumes.
7. **Dialogs have no OS modality.** `Modal` gives in-window modality. Secondary windows are deferred viewports with no transient-for. Native file and message dialogs come from `rfd` as Promises, matching Qt's `file-dialog` `open()` Promise (S2).
8. **Layout differs** (§2.5). Size props (`width-request`/`min-width`), alignment and expand must be defined by svelters-egui itself, because egui has no single prop model for them.

### 4.2 Naming

Follow N1: tag = kebab(native type name).

| egui type | tag |
| --- | --- |
| `Button` | `button` |
| `Checkbox` | `checkbox` |
| `RadioButton` | `radio-button` |
| `Slider` | `slider` |
| `RangeSlider` | `range-slider` |
| `DragValue` | `drag-value` |
| `TextEdit` | `text-edit` |
| `ProgressBar` | `progress-bar` |
| `ComboBox` | `combo-box` |
| `ScrollArea` | `scroll-area` |
| `CollapsingHeader` | `collapsing-header` |
| `Grid` | `grid` |
| `Frame` | `frame` |
| `Panel` | `panel` |
| `Sides` | `sides` |
| `Modal` | `modal` |
| `Hyperlink` | `hyperlink` |
| `Image` | `image` |
| `Separator` | `separator` |
| `Spinner` | `spinner` |
| `MenuBar` | `menu-bar` |
| `Scene` | `scene` |

That gives natural overlap with the other backends for portability mode:

- `button` (GTK/AppKit/WinUI), `label` (GTK/Qt), `slider` (Qt/AppKit/WinUI), `progress-bar` (GTK/Qt), `combo-box` (Qt), `radio-button` (Qt), `scroll-area` (Qt), `grid` (GTK/WinUI), `frame` (GTK/Qt), `image`, `separator`, `spinner` (GTK);
- the X4 menu kinds;
- `window`, `canvas`, `virtuallist`.

Exceptions, all of them synthetic kinds with no egui type:

- `window` is the OS toplevel (viewport), as on every backend. egui's in-app floating `Window` becomes `inner-window`.
- `box` (GTK's name) is the horizontal/vertical `Ui` layout, because egui has no container type.
- `label` is `Label`, with `RichText` props.
- The composites we build take GTK/Qt-familiar names: `stack`, `tab-view`, `split-view`, `switch`, `tree-view`, `toast`.

Portability mode then reports the rest as tag differences, as it does today.

### 4.3 Tier 0: egui core only

Generic props on every element:

- `visible`, `enabled` (→ `ui.add_enabled`), `tooltip`, `disabled-tooltip`, `cursor`, `id`/`class`/`style` (X6);
- `min-width`/`min-height`, `width`/`height` (fixed size, egui `add_sized`);
- `accessible-name`, `accessible-description`.

Generic events: `onclicked`, `onsecondary-clicked`, `ondouble-clicked`, `onhovered-changed`, `onfocus-changed`, and drag events on elements that sense drag.

| Element | egui source | Props | Events | `bind:` | GTK / Qt / AppKit / WinUI counterpart | Gaps and notes |
| --- | --- | --- | --- | --- | --- | --- |
| `window` | eframe root viewport / `show_viewport_deferred` | `title`, `default-width/height`, `min-*`, `resizable`, `decorations`, `maximized`, `fullscreen`, `always-on-top`, `icon`, `show` (X7) | `close-request` (sync veto via `CancelClose`), `resize` | `maximized`, `fullscreen` (from `ViewportInfo`) | window / main-window / window / window | no transient-for or OS modal (§2.2); web: embedded fallback |
| `inner-window` | `egui::Window` | `title`, `open`, `collapsible`, `resizable`, `movable`, `anchor`, `default-pos/size` | `close` | `open`, `collapsed` | ≈ MDI sub-window, GTK none | in-app only |
| `modal` | `egui::Modal` | `open`, `backdrop-color` | `close` (Esc/backdrop, `should_close`) | `open` | dialog `show=modal` | the X7 dialog default on egui |
| `panel` | `egui::Panel` (top/bottom/left/right/central) | `side`, `resizable`, `default-size`, `min/max-size`, `collapsible`, `frame` | `resize` | `size`, `expanded` | header-bar/status-bar/sidebar | `show_inside` for nested panels |
| `box` | `ui.with_layout(Layout)` / `horizontal`/`vertical`/`horizontal_wrapped` | `orientation`, `spacing`, `main-align`, `cross-align`, `main-justify`, `cross-justify`, `wrap`, `reverse` | — | — | box / box-layout / stack-view / stack-panel | flex grow depends on the scene-graph slice's engine (§2.5) |
| `grid` | `egui::Grid` | `num-columns`, `striped`, `spacing`, `min-col-width`, `min-row-height` | — | — | grid / grid-layout / – / grid | children fill row-major; `parent:row`/`column` need our own placement (pad empty cells); no spans in core (`StripBuilder` or taffy for spans) |
| `sides` | `egui::containers::Sides` | `spacing`, `shrink` (`extend`/`shrink-left`/`shrink-right`) | — | — | center-box / header-bar | child slots `start`/`end` |
| `frame` | `egui::Frame` | `inner-margin`, `outer-margin`, `fill`, `stroke`, `corner-radius`, `shadow`, preset `group`/`canvas`/`popup` | — | — | frame / frame, group-box / box | mostly a style host |
| `scroll-area` | `egui::ScrollArea` | `direction` (`vertical`/`horizontal`/`both`), `auto-shrink`, `max-height/width`, `stick-to-bottom`, `scroll-bar-visibility`, `animated` | `scroll` | `offset-x`, `offset-y` (`state.offset`) | scrolled-window / scroll-area | `scroll-to` method via `scroll_to_rect` on the next frame |
| `collapsing-header` | `egui::CollapsingHeader` | `label` (atoms), `default-open`, `icon` | `toggled` | `open` | expander / tool-box page | |
| `label` | `egui::Label` + `RichText` | `text`, `wrap` (`wrap`/`truncate`/`extend`), `selectable`, `strong`, `weak`, `italic`, `monospace`, `size`, `color`, `underline`, `strikethrough` | `clicked` (with sense) | — | label / label / text-field (label) / text-block | no bidi (§2.3); markup → `LayoutJob` from inline children (`<b>`, `<code>` lowering is a compiler question) |
| `hyperlink` | `egui::Hyperlink` / `Link` | `url`, `text`, `new-tab` | `clicked` | — | link-button | opens via `ctx.open_url` |
| `button` | `egui::Button` (atoms) | `label`, `icon`, `image`, `shortcut-text`, `small`, `frame`, `fill`, `selected`, `wrap`, `min-size`, `corner-radius` | `clicked` (+ generic) | — | button / push-button / button / button | children → atoms; nested interactive children rejected |
| `toggle-button` | `ui.toggle_value` / `Button::selectable` | `label`, `icon` | `toggled` | `active` | toggle-button / push-button checkable / button (toggle) / toggle-button | |
| `checkbox` | `egui::Checkbox` | `label`, `indeterminate` | `toggled` | `checked` | check-button / check-box / button (switch type) / check-box | |
| `radio-button` | `egui::RadioButton` / `ui.radio_value` | `label`, `group`, `value` | `toggled` | `checked`; `bind:group` on the group | check-button with group / radio-button | exclusivity is ours: same `group` within a parent, like X4 actions |
| `switch` | **build** (≈40 lines, the egui demo `toggle_switch` pattern) | `label` | `toggled` | `active` | switch / – / switch / toggle-switch | composite |
| `slider` | `egui::Slider` | `min`, `max`, `step`, `orientation`, `logarithmic`, `clamping`, `show-value`, `text`, `prefix`/`suffix`, `decimals`, `trailing-fill`, `handle-shape` | `changed`, `drag-started`/`drag-stopped` | `value` | scale / slider / slider / slider | f64 values; formatting through `custom_formatter`, which can't call JS → use a declarative `format` string or `decimals` |
| `range-slider` | `egui::RangeSlider` (0.37) | `min`, `max`, `min-gap`, … | `changed` | `low`, `high` | – | new in 0.37 |
| `drag-value` | `egui::DragValue` | `min`, `max`, `speed`, `prefix`, `suffix`, `decimals`, `hexadecimal` | `changed` | `value` | spin-button / spin-box, double-spin-box | no arrow buttons (compose two small buttons for a spin look) |
| `text-edit` | `egui::TextEdit::singleline` / `multiline` / `password` | `multiline`, `password`, `hint`, `char-limit`, `rows`, `desired-width`, `code` (monospace), `interactive`, `lock-focus`, `clip`, `align`, prefix/suffix atoms | `changed`, `submit` (Enter + lost focus), `focus-changed`; sync `before-change` = restore previous text | `text`; `cursor` (char range), `selection` | entry, text-view, password-entry, search-entry / line-edit, text-edit / text-field, secure-text-field / text-box | completion via a `<completion>` child or `suggestions` prop (`CompletionPopup`, 0.37); syntax highlighting via a `layouter` needs Rust (`egui_extras::syntax_highlighting`), exposed as `highlight="rust"` behind `syntect` |
| `image` | `egui::Image` + egui_extras loaders | `src` (`file://`, `bytes://`, `https://`, svg/gif/webp), `fit`, `max-width/height`, `tint`, `corner-radius`, `rotation`, `alt` | `clicked`, `loaded`/`error` | — | image, picture / label pixmap / image-view / image | loaders are an egui_extras feature; animated GIF/WebP supported |
| `icon` | glyph in a registered icon font (`icons-phosphor`/`icons-material`) | `name`, `size`, `color` | — | — | image `icon-name` | portable icon names need a table from freedesktop names to font glyphs; GTK/Qt use theme icons |
| `separator` | `egui::Separator` | `orientation`, `spacing`, `shrink` | — | — | separator / frame HLine | |
| `progress-bar` | `egui::ProgressBar` | `value` (0..1), `text`, `show-percentage`, `animate`, `fill`, `corner-radius` | — | — | progress-bar, level-bar / progress-bar / progress-indicator / progress-bar | indeterminate = `animate` without value |
| `spinner` | `egui::Spinner` | `size`, `color` | — | — | spinner / – / progress-indicator (spinning) / progress-ring | |
| `combo-box` | `egui::ComboBox` + `selectable_value` | `label`, `width`, `height`, `wrap`; items as `<combo-item value label>` children or `items` prop | `changed` | `selected` (value or index) | drop-down / combo-box / pop-up-button / combo-box | popup content is arbitrary children: a "rich combo" is free |
| `menu-bar`, `menu`, `action`, `menu-separator`, `context-menu`, `tool-bar` (X4) | `MenuBar`, `MenuButton`/`SubMenuButton`, `Button` + `shortcut_text`, `Separator`, `Response::context_menu`, horizontal `Ui` of buttons | X4 props: `label`, `icon`, `shortcut`, `tooltip`, `enabled`, `visible`, `checkable`, `checked`, `group` | `activate`, `toggled` | `checked` | same | shortcuts are consumed by the backend each frame (`consume_shortcut`); macOS native menubar only with `native-menus` (muda) |
| `menu-button` | `MenuButton` / `Popup::menu` | `label`, `icon` | — | `open` | menu-button / tool-button with menu | |
| `popup` | `egui::Popup` | `anchor`, `close-behavior`, `position`, `gap` | `close` | `open` | popover / – | |
| `canvas` | `ui.painter()` + epaint shapes, `Sense` for input | `ops` (X8 display list), `width`, `height`, `sense` | `resize`, pointer events | — | drawing-area / canvas | **best fit of any backend**: paths, beziers, rounded rects, text, images (textures) and meshes are native; `measureText` = `fonts.layout`; gradients via mesh vertex colours; `drawImage` via texture handles; optional `scene` parent for pan/zoom |
| `virtuallist` (M5) | `ScrollArea::show_rows` (uniform) | the M5 protocol: `splice`, `count`, `scroll-to`, `set-row`, `row-at`, `selected`, `selection`, `layout`, `row-height` | M5 `setup`/`bind`/`unbind`/`teardown`, `activate`, `selection-changed` | `selected` | same | rows exist only while visible, which suits the pool protocol; `bind` must complete before the frame paints, so JS bind runs between frames and the row shows from the next frame (or we run a discard pass); grid layout = row of N cells; variable heights → `egui_virtual_list` approach later |
| `stack` | **build**: render only the visible child | `visible-child-name`, `transition` (fade/slide via `animate_value`) | — | `visible-child-name` | stack / stacked-widget / tab-view (tabless) / frame | composite |

Tier 0 is about 25 elements from core plus 4 composites (`switch`, `stack`, `icon`, `toggle-button` sugar). It needs no third-party crate except, optionally, `egui_extras` for image loaders.

### 4.4 Tier 1: rich desktop widgets (core + emilk/rerun crates + rfd)

| Element | Source | Props / events / bind | Counterpart | Notes |
| --- | --- | --- | --- | --- |
| `table` | `egui_table` (default) or `egui_extras::TableBuilder` | columns as `<table-column id title width resizable sortable>`; `row-count`, `row-height`, `sticky-columns`, `striped`; cell content through a snippet bound like M5 rows (setup/bind per visible cell); events `header-clicked` (sort), `row-activate`; `bind:selected` | column-view / table-view / table-view / list-view | egui_table virtualizes rows *and* columns and has a prefetch hook; this element is "virtuallist with columns". No AccessKit table roles yet (#8700) |
| `list` | `ScrollArea` + `Button::selectable` rows (non-virtual) | `selection` (`single`/`multi`), items as children; `bind:selected` | list-box / list-widget | small lists; big ones use `virtuallist` |
| `tab-view` | **build**: a row of selectable buttons (or `Sides`) + `stack` | `<tab-page title icon closable>` children; `closable`, `reorderable`, `position` | notebook / tab-widget / tab-view / tab-view | closable/reorderable via `egui_dnd` later; `egui_dock`/`egui_tiles` give tabs only inside their layout |
| `split-view` | **build** on `Panel::show_inside` (resizable) or a drag-handle separator | `orientation`, `position`, `min-start`, `min-end`, `collapsible`; `bind:position` | paned / splitter / split-view / split-view | composite |
| `tree-view` | **build** on `CollapsingHeader` + selectable rows, or `egui_ltreeview` (pack, lags one minor) | nested `<tree-item id label icon>` children, or virtual rows via an M5-like protocol; `bind:selected`, `bind:expanded`; `activate`, `context-menu` per item | tree-view (list-view+TreeListModel) / tree-widget / outline-view / tree-view | first version as a composite; switch to ltreeview when it is on our egui minor |
| `date-picker` | `egui_extras::DatePickerButton` (`jiff`, `datepicker` feature) | `bind:date` (ISO string), `format`, `calendar-week`, `highlight-weekends`, `start-end-years` | calendar / calendar-widget, date-edit / date-picker / calendar-date-picker | button + popup only; an inline calendar means extracting the popup (it is crate-private) or building one |
| `color-picker` / `color-button` | core `egui::color_picker` (`color_edit_button_srgba`, `color_picker_color32`), `Alpha` modes | `bind:color` (CSS hex), `alpha` (`opaque`/`blend`/`additive`), inline vs button | color-dialog-button / color-dialog / color-well / color-picker | core, no crate needed |
| `file-dialog` | `rfd::AsyncFileDialog` (native); `egui-file-dialog` pack where no native dialog exists | `mode` (`open`/`open-multiple`/`save`/`folder`), `title`, `filters`, `directory`, `file-name`; `open()` returns a Promise, like Qt | file-dialog (Qt) / GtkFileDialog (GTK, method-based) | rfd runs on its own thread or in the portal; resolve through `request_repaint` + event queue |
| `message-box` | `rfd::AsyncMessageDialog` (native) or a `modal` composite | `level`, `title`, `text`, `buttons`; `open()` → Promise of the button | message-box / alert-dialog / NSAlert / content-dialog | native look via rfd; in-app via `modal` |
| `toast` | **build** on `Area` + `animate_value` (or `egui-notify` pack) | `<toast>` children of `window`, or a JS `toast()` helper; `duration`, `level` | adw toast, KDE notification | composite |
| `drop-zone` / drag-and-drop | core `ui.dnd_drag_source`, `dnd_drop_zone`, `DragAndDrop` payloads | `parent:draggable`, `ondrop`; file drops from `ctx.input().raw.dropped_files` | GtkDragSource/DropTarget | payloads are JS values carried by node id |

### 4.5 Tier 2: optional packs (cargo features, may lag egui)

| Pack / element | Crate | Notes |
| --- | --- | --- |
| `plot` (`<plot>` with `<line-series points>`, `<bar-series>`, `<scatter>`; `bind:bounds`, `onhover`) | egui_plot 0.38 | same-week releases; data as `Float64Array` values (S1 value model), which avoids per-point props |
| `dock` (`<dock-area>`, `<dock-tab title>`; `bind:layout` = serde JSON) | egui_dock 0.21 (0.36) | the workbench sibling uses it; tab content = retained subtrees routed by tab id |
| `tiles` (`<tiles>`, `<tile-tabs>`, `<tile-linear>`, `<tile-grid>`, `<tile-pane>`) | egui_tiles 0.18 | Rerun-grade; the user already maintains a fork |
| `node-graph` (`<node-graph>`, `<graph-node>`, pins as children; `onconnect`, `ondisconnect`) | egui-snarl 0.12 (0.36) | Snarl owns its node storage, so the adapter keeps a node-id ↔ handle map |
| `markdown` (`<markdown source>`) | egui_commonmark 0.26 | syntect behind a feature; links → event |
| `code-editor` (`<code-editor bind:text language theme line-numbers>`) | egui_code_editor 0.4 (0.36) or our own `TextEdit` + `egui_extras` highlighting | prefer our own on `TextEdit` to avoid a lagging dep |
| `icons-phosphor`, `icons-material` | egui-phosphor, egui_material_icons | fonts; feeds `icon` |
| `native-menus` | muda 0.21 | macOS/Windows only |
| `inspector` | egui_inspection + egui_mcp | dev builds |

Leave to user packs: node visualizers (egui_graphs), 3D gizmos, JSON trees, knobs, logging panes, routers, forms, theme switches, struct derives.

## 5. Styling mapping (X6 → egui)

**Strategy.** The X6 translator (`@svelters/style`) gains a third target, **`egui`**. Instead of a CSS dialect it emits a compact data sheet: rules = (selector AST, specificity, declarations with values already parsed into typed units and colours). Tokens stay symbolic.

- The backend owns the cascade, because it owns the retained tree. On a structural or class change it re-matches the affected subtree (type, class, `#id`, descendant/child, `:first-child`...). It caches the computed style per node and resolves `var(--sv-*)` against the active token set.
- At frame time, the per-state blocks (`:hover`, `:active`, `:focus`, `:checked`, `:disabled`) are picked from the node's previous-frame `Response` state, the same way egui's own `StyleProvider` does it.
- The ABI `style` op carries this unchanged: `TARGET` returns `egui`, and sheets, classes, inline declarations and the scheme all use the existing sub-ops.
- When egui's `StyleProvider` leaves `experimental`, the computed style can be delivered through it, and egui `Classes` mirror our classes, so third-party egui widgets inside packs pick up classes too. Until then we apply overrides with `ui.scope` + `style_mut()` and builder overrides.

| X6 / CSS | egui target | Coverage |
| --- | --- | --- |
| `color` | `RichText::color` for text; `WidgetVisuals::fg_stroke.color` (per state) for widget foreground; `Visuals::override_text_color` for a subtree | full |
| `background-color` | `Frame::fill`; `Button::fill`; `WidgetVisuals::bg_fill`/`weak_bg_fill` per state; `Visuals::extreme_bg_color`/`text_edit_bg_color` for text edits; `panel_fill`/`window_fill` | full (solid colours; gradients → own painting) |
| `font-family` | `FontFamily::Name` (must be registered in `FontDefinitions` or found by the system font provider) | full for registered or system families |
| `font-size` | `FontId::size` / `RichText::size`; `text_styles` for a subtree | full (pt → points: egui points = logical px) |
| `font-weight` | variable-font `wght` axis (0.35+) or a separate registered family ("…-bold"); `RichText::strong` is only a colour change | partial: needs bold faces or a variable font |
| `font-style: italic` | `RichText::italics` (synthetic skew) or an italic family | partial |
| `padding` | `Frame::inner_margin`; `Spacing::button_padding`; `AtomLayoutStyle::frame` (0.37 typed styles); `TextEdit::margin` | full on frame-like widgets |
| `margin` (X6 "prefer props") | `Frame::outer_margin`, or spacing in our layout | full |
| `border-width`, `border-color` | `Stroke` on `Frame`, `WidgetVisuals::bg_stroke` per state, `Button::stroke` | full |
| `border-radius` | `CornerRadius` (per corner, u8) on `Frame`/`WidgetVisuals`/`Button` | full |
| `opacity` | `Ui::multiply_opacity` on the node's scope | full |
| `min-width`, `min-height` | `Button::min_size`, `ui.set_min_width/height`, `AtomLayoutStyle::min_size` | full |
| `box-shadow` (not portable today) | `Frame::shadow`, `Visuals::window_shadow`/`popup_shadow` | egui extra |
| `:hover` `:active` `:focus` `:disabled` `:checked` | `WidgetVisuals` `hovered`/`active`/`noninteractive`; `:checked` → selected state (`Button::selected`, `selection.bg_fill`); `:disabled` → `ui.disable()` + `disabled_alpha` | full (one-frame lag, the same as egui's own theming) |
| named parts (`slider::trough`, `checkbox::indicator`) | `CheckboxStyle::checkbox_frame/check_stroke`, `Visuals::slider_trailing_fill`, `handle_shape`, `Spacing::slider_rail_height`, `text_cursor`, `selection` | per widget, small vocabulary |
| `transition` | `ctx.animate_value_with_time` on computed colours/opacity, keyed by node id | best effort (better than Qt, where X6 drops it) |
| `prefers-color-scheme` | `ThemePreference` / `ctx.theme()`; `system_theme` from eframe | full |
| `prefers-reduced-motion` | `style.animation_time = 0`, `scroll_animation` off | full (no OS query; set from token or env) |
| `prefers-contrast` | none in egui | none |
| layout props (`width`, `display`, `gap`...) | refused, as on GTK/Qt (layout stays props) | — |
| `@native egui { … }` | no stylesheet to pass through; instead an `@native egui` block of egui-specific properties (`item-spacing`, `button-padding`, `interact-size`, `window-shadow`...) mapped onto `Style` fields by name | design choice |

| Token | egui `Visuals` / `Style` field |
| --- | --- |
| `--sv-bg` | `panel_fill`, `window_fill` |
| `--sv-fg` | `widgets.noninteractive.fg_stroke.color` (body text), `override_text_color` if forced |
| `--sv-surface` | `widgets.inactive.bg_fill`/`weak_bg_fill`, `faint_bg_color`, `extreme_bg_color` (text edit; or a derived darker shade) |
| `--sv-border` | `widgets.*.bg_stroke.color`, `window_stroke` |
| `--sv-accent` | `selection.bg_fill`, `hyperlink_color`, `widgets.active.bg_fill`, `slider_trailing_fill` colour |
| `--sv-accent-hover` | `widgets.hovered.bg_fill` |
| `--sv-accent-fg` | `selection.stroke.color`, `widgets.active.fg_stroke.color` |
| `--sv-muted` | `weak_text_color` |
| `--sv-danger` | `error_fg_color` (`warn_fg_color` from a derived token) |
| `--sv-radius` | `widgets.*.corner_radius`, `window_corner_radius`, `menu_corner_radius` |
| `--sv-space-1..4` | `spacing.item_spacing`, `button_padding`, `window_margin`, `menu_margin`, `indent` |
| `--sv-font-size`, `--sv-font-family` | `text_styles[Body/Button]` `FontId`; `Small`/`Heading`/`Monospace` scaled from it |

A token change rebuilds one `Style` and calls `ctx.set_style_of(theme, style)`. It is cheap because egui restyles every frame anyway. The dark scheme is the second `Style` (`set_style_of(Theme::Dark, …)`), so `setColorScheme('system')` maps directly onto `ThemePreference::System`.

**CSS-for-egui prior art.** No maintained crate parses real CSS for egui.

- [`egui_styled`](https://docs.rs/egui_styled) (0.8, egui 0.35) is Tailwind-style builders with per-state styles and tokens.
- `egui-aesthetix` (a theme trait) and `zeus-theme` are dead or small.
- `catppuccin-egui` sets `Visuals` from a palette, which is our token path.
- `egui_colors` generates 12-step scales from a seed colour.
- egui's own answer is the 0.37 experimental `Classes` + `StyleProvider`.

We need our own matcher either way, and it is small: the X6 vocabulary is about 15 properties and 5 pseudo-classes. A Servo `selectors` dependency would be overkill. Pixel read-back tests can use `egui_kittest` wgpu snapshots, the way X6 tests read back computed colours on GTK/Qt.

## 6. Maintenance strategy

**Cadence.** Plan on an egui minor every 2 to 3 months, each with a dozen or so mechanical breaks. Treat each one as a small lane.

1. **One egui minor per svelters-egui line.** svelters-egui depends on `egui = "0.37"`, `eframe = "0.37"`, `egui_extras = "0.37"` (caret within the minor, so patch releases come in). Its own version encodes the egui minor in the docs and in `svelters doctor` output. The next egui minor gets a new svelters-egui minor; we do not support two egui minors at once.
2. **Re-export egui.** `pub use egui; pub use eframe; pub use egui_extras;` from `svelters-egui`, plus a `svelters-egui-pack` API crate that re-exports them for widget packs. Pack authors write `use svelters_egui_pack::egui`. They never list `egui` in their own `Cargo.toml` unless they also wrap a third-party egui crate, and then the versions must agree.
3. **Enforce one copy.** Cargo happily links egui 0.36 and 0.37 into one binary as unrelated crates. The failure is a type error at the seam (`expected egui::Ui, found egui::Ui`) and a doubled binary, not undefined behaviour. Defences:
   - `svelters-egui-pack` declares `links = "svelters_egui"`. Cargo refuses two versions of a crate with the same `links` key, so a pack built for svelters-egui 0.36 cannot be resolved into a 0.37 app. The resolver error names the pack.
   - `svelters doctor` and CI run `cargo tree -d -e normal -i egui` and fail on more than one egui.
   - Each pack manifest states its egui minor (`svelters-pack.toml` `[egui] minor = "0.37"`), and the CLI checks it before adding the pack.
4. **No dynamic egui packs.** Rust has no stable ABI, and `egui::Context`/`Ui` cannot cross a dylib built by another compiler or egui version. CUSTOM-WIDGETS' data packs (a reflective engine) have no egui equivalent. egui packs are always **code packs, statically linked through the app's leaf crate**, through the existing `svl_pack` vtable for id routing, with Rust traits for the actual drawing. JS-only packs (Svelte components over core elements and `canvas`) need no Rust and are the recommended route for most custom widgets (JS-custom-widgets slice).
5. **Feature flags.**
   - `svelters-egui` default: `wgpu`, `accesskit`, `system-fonts`, `image-loaders` (file, svg, png/jpeg via `image`), `table` (egui_table), `datepicker`, `native-dialogs` (rfd).
   - Opt-in: `glow`, `default-fonts` (deterministic tests), `syntect`, `http-images`, `gif`/`webp`, `plot`, `tiles`, `dock`, `markdown`, `node-graph`, `code-editor`, `tree-ltreeview`, `icons-phosphor`, `icons-material`, `native-menus`, `inspection`, `experimental-theme`.
   - The prebuilt runner (CLI §3) ships the default set plus `plot` and `icons-phosphor`. Anything else means a leaf-crate build.
6. **Dependency classes.**
   - **Same-week crates** (egui, eframe, egui_extras, egui_kittest, egui_inspection; in practice also egui_plot, egui_table, egui_tiles) may back first-party elements.
   - **Lagging crates** (egui_dock, snarl, ltreeview, file-dialog, code_editor, phosphor) are packs only. A svelters-egui release does not wait for them. A lagging pack is marked unavailable for that release, or we carry a fork in the `egui_tiles-fork` style: a `[patch.crates-io]` path, a `FORK.md` with base sha and patch series, usually a version-requirement bump only.
   - **No-egui crates** (rfd, muda, taffy, accesskit via egui) never lag.
7. **Upgrade lane checklist per egui minor:**
   - bump;
   - fix compile errors in the hand-written element impls;
   - run the generated fact tests (kittest) and gallery snapshots (expect pixel drift from style default changes such as 0.37's button padding, so re-bless with review);
   - re-run `cargo public-api` diff (or a rustdoc-JSON diff) of egui to catch removed builder methods our metadata names;
   - update the forks;
   - record it in `STATUS.md`.
8. **Metadata.** egui has no GIR/qdoc/winmd. The element specs (`metadata/egui/*.yaml` → `metadata/semantics/egui.json`) are hand-authored, like the AppKit/WinUI style tables:
   - props carry `observe: {class: exact, mechanism: frame}` for bindables and `set_only` otherwise, and each fact still gets a generated kittest test for the ledger;
   - a rustdoc-JSON ingest of the egui crates can cross-check that every builder method a prop maps to exists in the pinned version, which is the drift detector for step 7.

## 7. Risks

| Risk | Impact | Mitigation |
| --- | --- | --- |
| egui breaking churn (an upgrade lane every 2–3 months) | ongoing cost; packs fall behind | §6 (pin one minor, same-week deps only, fork pattern, rustdoc-diff detector) |
| egui is the loop owner (eframe `run_native`), and multi-viewport has no transient/modal | X7 semantics degrade on egui | document: `show=modal` → `Modal`; secondary windows → deferred viewports; window close veto through `CancelClose` |
| Immediate-mode re-entrancy: no JS during a `Ui` borrow | sync handlers and synchronous M5 `bind` are harder | events after the pass; bind between frames + `request_discard` for first paint; per-event sync designs only |
| One-frame lag for state-dependent styling and auto-sizing | flicker on first show | egui's own sizing pass / `request_discard`; precompute from the cached response |
| No RTL/bidi in 0.37 | no Arabic/Hebrew apps | track PR #8577 (likely 0.38); state the limitation in the docs |
| IME bugs (Fcitx5 X11, Korean hanja) | CJK input on some Linux setups | upstream issues; test on Wayland + IBus too |
| AccessKit gaps (tables, occluded windows) | screen readers, our UIA-style self tests | supply roles/names from the graph; upstream #8700 |
| Non-native look on every OS | users expecting a native feel | egui is the "same everywhere" backend by design; theming via tokens |
| Layout model mismatch (expand/homogeneous, grid spans) | portable layouts look different | the scene-graph slice's choice of engine (egui Layout vs egui_flex vs taffy on our tree) |
| Experimental `StyleProvider` may change or be dropped | styling rework | apply overrides ourselves first; adopt the provider only when it is stable |
| Duplicate egui versions through user packs | confusing compile errors, bloat | `links` key, re-exports, `cargo tree -d` in doctor |
| Lagging ecosystem crates hold back the prebuilt runner | release blocked | runner features use same-week crates only; packs are optional |
| Default font changes (`default_fonts` off since 0.37) and system font discovery | screenshots differ by machine | tests enable `default_fonts`, disable `system_fonts`, and pin a font set |

## 8. Open questions for the user

1. Layout engine for `box`/`grid` (egui `Layout` vs `egui_flex` vs taffy on our retained tree): decided with the scene-graph slice.
2. Should portable names win over egui-native names for the synthetic kinds (`box`, `stack`, `switch`, `tab-view`, `split-view`)? This note proposes GTK-familiar names for synthetic kinds and N1 names for real egui types.
3. Should `native-menus` (muda) be in the default feature set on macOS? It brings a native menubar, but a second menu code path.
4. Is `egui_table` the default table (virtual columns, rerun-maintained), or `egui_extras::TableBuilder` (no extra crate)? This note proposes egui_table.
5. Should we target egui 0.37 for M7, or wait for 0.38 (likely bidi and a stabilized theme system)? A 0.37 start is fine; the first upgrade lane will be 0.38.
