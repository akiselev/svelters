# DESIGN: a VS Code base look for svelters-egui and cadatomic, with affordances restored

Status: research (DS0), 2026-10-22. This is not a plan of record. Branch `research/design`. Nothing was built or run.

The request (user): "research how we should do proper UI design across our app. We want it to look like VScode as a
start, but then add in the visual affordances that have been lost to the flat UI craze like shadows in order to
indicate what is actually clickable, interactable, etc".

Inputs:
- svelters: `plans/research/CSS.md`, `EGUI.md` §5, `ASSETS.md`, `ARCH-CRATES.md` §1 (theming comes from the host);
  the milestones `CS0`, `CS5e`, `FX1`, `TK1`, plus `CS5f` (`origin/ci/cs5f`) and `CS5g` (`origin/ci/cs5g`);
  `packages/style/src/tokens.js`; the egui 0.37.0 source (`style.rs`).
- cadatomic, read only: `docs/research/2026-10-22-ide-shell-and-ux.md`, `…-component-framework.md` §2.3/§3.3/§9,
  `…-widget-survey-and-native-acceptance.md` §1.10, `docs/best-practices/agent-driven-testing.md`,
  `crates/cadatomic-app/src/theme/mod.rs`, `docs/plan/00-decisions.md`.
- **VS Code 1.133.0 as installed on this PC** (`/mnt/c/Program Files/Microsoft VS Code/a5b5009513/resources/app`):
  - the theme JSONs (`extensions/theme-defaults/themes/*.json`);
  - the workbench CSS and JS bundles (`out/vs/workbench/workbench.desktop.main.{css,js}`). These hold the colour
    registry defaults, the size registry and the metrics.
- Web: NN/g, WCAG 2.2, Microsoft Learn (Windows 11 layering), Fluent UI tokens, the Blender API reference, the
  JetBrains guidelines.

Claims taken from the installed VS Code are marked **[1.133]**. Claims from memory that were not checked are marked
**[unverified]**.

## 0. Summary

1. **VS Code is the base, taken as data.** The base is the Dark Modern / Light Modern / High Contrast colours and
   VS Code's own size registry. A token importer reads VS Code theme JSON, so the base, any VS Code theme and the
   user's theme all go through one path. **What VS Code itself is doing changes the starting point:**
   - 1.133's default themes are **Dark 2026 / Light 2026**, not Modern **[1.133]**.
   - It ships an experimental `workbench.experimental.modernUI` style override that adds the following
     **[1.133]**:
     - **elevation tokens** `--vscode-shadow-{sm,md,lg,xl}`;
     - rounded corners, "sashHandles", "scrollShadows", a "keyboardFocusOnly" focus mode and a `no-shadows` switch;
     - shadows on menus, hovers, find, quick input and dialogs.

   So "VS Code plus shadows" is where VS Code is heading, and we can take our elevation scale from it.
2. **Affordances are a separate layer over the exact base:** a few component tokens and one theme-origin sheet
   (`@layer sv.affordance`). There are three rules of thumb:
   - controls are raised (a fill, a 1px lip, a boundary of at least 3:1);
   - fields are sunken (a field fill, an inset top shadow, a boundary of at least 3:1);
   - floating surfaces cast real shadows (tone, stroke and shadow).

   Lists, tree rows and editor text stay flat. Turning the layer off gives pixel-faithful VS Code colours for
   VIS1.
3. **The flat VS Code look fails WCAG 1.4.11 on control boundaries.** Computed from the theme files:

   | Theme | Boundary | Contrast |
   |---|---|---|
   | Dark Modern | `input.border` `#3C3C3C` on the side bar `#181818` | 1.61:1 |
   | Dark Modern | the field fill on the side bar | 1.36:1 |
   | Light Modern | `#CECECE` on `#F8F8F8` | 1.48:1 |
   | Dark 2026 | the input border | 1.41:1 |
   | Light 2026 | the input border | 1.15:1 |

   That is measurable evidence for the user's complaint, and it is what the affordance layer fixes.
4. **One token map feeds everything.** svelters-egui's new `theme` module resolves a token map (the VS Code base,
   then cadatomic's overrides, then the user's theme) into three outputs:
   - the `:root` sheet for the cascade (Svelte panels);
   - the egui `Style` (stock widgets and egui-native Rust code);
   - typed accessors that cadatomic uses to fill berth's `DockStyle` and quire's `EditorTheme`.

   The app then looks like one thing.
5. **What's missing in the engine:** `outline`/`outline-offset`, `cursor`, per-side borders, colour and shadow
   `transition`, `text-transform`, disabled reasons, scroll shadows, and scrollbar and density tokens. CS5g's
   `box-shadow` already covers elevation paint, including per state on stock widgets.
6. **VIS1** captures VS Code references on this PC: Chrome DevTools Protocol (CDP) element crops with forced
   pseudo-states, plus `getComputedStyle` metrics and every resolved `--vscode-*` variable. It renders every gallery
   kind×state×theme through TK1's lavapipe snapshots and gates on **metrics and colours**, not pixels. Agents review
   side-by-side contact sheets.

## 1. The VS Code baseline

### 1.1 Which VS Code

| Theme | Status in 1.133 [1.133] | Character |
|---|---|---|
| Dark Modern / Light Modern | Built in, default 1.78 to 1.12x [unverified range] | Grey chrome (`#181818`/`#F8F8F8`), blue accent `#0078D4`/`#005FB8`, the look most users picture |
| Dark 2026 / Light 2026 | **Current default** (`COLOR_THEME_DARK="Dark 2026"`), 297 colour keys, `include: dark_modern.json` | Teal accent `#297AA0`, flatter still: `widget.shadow` is `#00000000` in light |
| Default High Contrast (dark/light) | Built in | Black or white, `contrastBorder` `#6FC3DF`/`#0F4A85`, focus `#F38518`/`#006BBD`, no shadows |
| `workbench.experimental.modernUI` | Experimental overlay on any theme | Shadow tokens, radii, sash handles, scroll shadows, keyboard-only focus, pane headers 28px, tabs 32/28px |

**Recommendation:** take the **Modern** colours as the base and **modernUI's** elevation and radius scale as the
shape language. Ship 2026 as an alternative through the importer (§4.4). Q1 asks the user to confirm.

### 1.2 Typography

| Item | VS Code | Ours |
|---|---|---|
| UI font | system: `Segoe WPC, Segoe UI` (Windows), `-apple-system, BlinkMacSystemFont` (macOS), `system-ui, Ubuntu, Droid Sans` (Linux) [1.133 CSS] | **Inter, bundled** (cadatomic already ships it, as does JetBrains' New UI). Snapshots stay identical on every OS. egui can't enumerate system fonts without a new dependency. |
| Base size | `.monaco-workbench{font-size:13px;line-height:1.4em}`; registry `bodyFontSize` 13, `.small` 12, `.xSmall` 11 [1.133] | `--sv-font-size: 13px`, `--sv-font-size-sm: 12px`, `--sv-font-size-xs: 11px` |
| Font ramp | `fontSize.heading1/2/3` 26/18/13, `label1/2/3` 12/11/10 [1.133] | `--sv-font-size-h1..h3`, `--sv-font-size-label1..3` |
| Pane and section headers | 11px, bold, uppercase (`.pane-header{font-size:11px;font-weight:700}`) [1.133] | `text-transform` needed (§5) |
| Status bar | 12px [1.133] | `--sv-font-size-sm` |
| Editor | Consolas / Menlo / Droid Sans Mono, 14px (12 on macOS) [unverified] | `--sv-font-mono`: bundle a monospace (JetBrains Mono, OFL) for quire; 13px in UI code spans |

### 1.3 Metrics (`normal` density)

| Element | VS Code value | Source |
|---|---|---|
| List and tree row | 22px | [unverified: `ListView` item height]; pane header `Hqt=22` [1.133] |
| Tree indent per level / twistie | `workbench.tree.indent` 8px [unverified default] / twistie 16px wide | [1.133 CSS `.monaco-tl-twistie{width:16px}`] |
| Pane (view) header | 22px (modernUI 28) | [1.133 JS] |
| Editor tab | 35px (compact 22, modernUI 32, modernUI compact 28) | [1.133 `EDITOR_TAB_HEIGHT`] |
| Title bar | 35px | [unverified] |
| Status bar | 22px | [1.133 CSS] |
| Activity bar | 48px wide, 48px actions (compact 36 wide / 28 tall) | [1.133 JS] |
| Text button | `padding:4px 8px; line-height:16px; border:1px; radius 4px` → 26px tall | [1.133 CSS `.monaco-text-button`] |
| Input box | 26px (24px input plus border), radius 4px (2px before modernUI) | [1.133 CSS; variants 23–25 exist] |
| Checkbox / toggle | 18×18 radius 3 / 20×20 radius 3 | [1.133 CSS] |
| Select | `min-height:24px; padding:2px 23px 2px 8px` | [1.133 CSS] |
| Action (toolbar) item | 16px codicon, 22px hit box [unverified box], `padding:2px` | [1.133 CSS] |
| Icons | `codiconFontSize` 16px (compact 12) | [1.133 size registry] |
| Sash | 4px (8px on touch), hover 4px after a 300ms delay [unverified delay] | [1.133 CSS `--vscode-sash-size`] |
| Scrollbar | lists 10px, editor 14px, overlay, fades out [unverified sizes] | — |
| Radii | `cornerRadius.xSmall` 2, `small` 4, `medium` 6, `large` 8, `xLarge` 12, `circle` 9999 | [1.133 size registry] |
| Spacing | `spacing.size20…400` = 2, 4, 6, 8, 10, 12, 16, 20, 24, 28, 32, 36, 40 | [1.133 size registry] |
| Stroke | `strokeThickness` 1px | [1.133] |
| Quick input | 600px wide, `cornerRadius-xLarge`, `shadow-xl` | [1.133 CSS] |
| Dialog | min 480px wide, padding 8, radius xLarge, `shadow-xl` | [1.133 CSS] |

### 1.4 Focus, hover, selection, motion

- **Focus.** A 1px solid `focusBorder` outline at `outline-offset:-1px` on inputs, list rows and buttons
  [1.133 CSS]. modernUI adds "keyboardFocusOnly" (`:focus-visible` semantics). FX1 already draws focus only after
  keyboard focus on egui.
- **Hover:**
  - lists: `list.hoverBackground`;
  - toolbars: `toolbar.hoverBackground`;
  - tabs: `tab.hoverBackground`, plus modernUI `shadow-sm` on `.tab:hover:not(.active)`;
  - High Contrast: a dashed `contrastActiveBorder` outline [unverified].
- **Selection** has four states:
  - hover;
  - active selection (the list has focus): `list.activeSelectionBackground`;
  - inactive selection: `list.inactiveSelectionBackground`;
  - focused row: a `list.focusOutline` outline.

  These are VS Code's strongest affordance, and we keep them.
- **Cursor.** `cursor:pointer` on text buttons, list rows, tabs and selects. `default` elsewhere [1.133 CSS].
- **Motion.** Almost none in the classic UI. modernUI animates menus and quick input with 150ms
  `cubic-bezier(.22,1,.36,1)` under `.monaco-enable-motion` [1.133]. The scrollbar fades.

### 1.5 Token table: `--sv-*` against VS Code keys

Values come from Dark Modern / Light Modern. Where a theme doesn't set a key, the fallback is `dark_vs`/`light_vs`,
then the colour registry default **[1.133 JS `oe("key",{dark,light,hcDark,hcLight})`]**.

The **bold** names are the 16 existing tokens (`packages/style/src/tokens.js`). They are kept, with their meanings
pinned as shown. "aff." marks a value the affordance layer (§2) overrides, and the override value is given in
parentheses.

**Surfaces and lines**

| `--sv-*` | VS Code key | Dark | Light | HC dark / HC light |
|---|---|---|---|---|
| **`bg`** (chrome: side bar, panel, title, status, activity) | `sideBar.background` | `#181818` | `#F8F8F8` | `#000000` / `#FFFFFF` |
| **`surface`** (content: editor, document, task panel body) | `editor.background` | `#1F1F1F` | `#FFFFFF` | `#000000` / `#FFFFFF` |
| `surface-raised` (menus, popovers, hovers, dialogs) | `editorWidget.background` | `#202020` | `#F8F8F8` | `#0C141F` / `#FFFFFF` |
| **`border`** (hairline between regions) | `sideBar.border` = `panel.border` | `#2B2B2B` | `#E5E5E5` | `contrastBorder` |
| `border-widget` (popover and menu outline) | `widget.border` / `menu.border` | `#313131` | `#E5E5E5` (menus `#CECECE`) | `contrastBorder` |
| `shadow-color` | `widget.shadow` | `#0000005C` | `#00000029` | none |
| `scroll-shadow` | `scrollbar.shadow` | `#000000` | `#DDDDDD` | none |

**Text and icons**

| `--sv-*` | VS Code key | Dark | Light | HC dark / HC light |
|---|---|---|---|---|
| **`fg`** | `foreground` | `#CCCCCC` | `#3B3B3B` | `#FFFFFF` / `#292929` [unverified] |
| `fg-strong` (headers, active tab) | `settings.headerForeground` | `#FFFFFF` | `#1F1F1F` | as fg |
| **`muted`** | `descriptionForeground` | `#9D9D9D` | `#3B3B3B` in Light Modern (same as fg, so we use `#616161` = `activityBar.inactiveForeground`) | as fg |
| `disabled-fg` | `disabledForeground` | `#CCCCCC80` | `#61616180` | `#A5A5A5` / `#7F7F7F` |
| `placeholder` | `input.placeholderForeground` | `#989898` | `#767676` | — |
| `icon` | `icon.foreground` | `#CCCCCC` | `#3B3B3B` | — |
| `link` | `textLink.foreground` | `#4DAAFC` | `#005FB8` | — |

**Accent and focus**

| `--sv-*` | VS Code key | Dark | Light | HC dark / HC light |
|---|---|---|---|---|
| **`accent`** (primary button, active markers) | `button.background` | `#0078D4` | `#005FB8` | — / — |
| **`accent-hover`** | `button.hoverBackground` | `#026EC1` | `#0258A8` | — |
| **`accent-fg`** | `button.foreground` | `#FFFFFF` | `#FFFFFF` | — |
| `focus` | `focusBorder` | `#0078D4` (aff. `#2488DB`) | `#005FB8` | `#F38518` / `#006BBD` |

**Controls**

| `--sv-*` | VS Code key | Dark | Light | HC dark / HC light |
|---|---|---|---|---|
| `button-border` | `button.border` | `#FFFFFF1A` | `#0000001A` | `contrastBorder` |
| `button2-bg` / `-hover` / `-fg` | `button.secondary{Background,HoverBackground,Foreground}` | `#00000000` / `#2B2B2B` / `#CCCCCC` (aff. bg `#313131`) | `#E5E5E5` / `#CCCCCC` / `#3B3B3B` | — |
| `control-bg` (field fill) | `input.background` | `#313131` | `#FFFFFF` | `#000000` / `#FFFFFF` |
| `control-border` | `input.border` (`checkbox.border`, `dropdown.border`) | `#3C3C3C` (aff. `#6B6B6B`) | `#CECECE` (aff. `#8A8A8A`) | `contrastBorder` |
| `control-fg` | `input.foreground` | `#CCCCCC` | `#3B3B3B` | — |
| `toggle-on-bg` / `-border` | `inputOption.activeBackground` / `activeBorder` | `#2489DB82` / `#2488DB` | `#BED6ED` / `#005FB8` | — |
| `toolbar-hover` / `-active` | `toolbar.hoverBackground` / `activeBackground` | `#5A5D5E50` / derived (lighten 10%) | `#B8B8B850` / derived (darken 10%) | — |
| `invalid-border` / `-bg` | `inputValidation.error{Border,Background}` | `#BE1100` / `#5A1D1D` | `#BE1100` / `#F2DEDE` | `contrastBorder` |

**Lists, tabs, menus, scrollbars and status**

| `--sv-*` | VS Code key | Dark | Light | HC dark / HC light |
|---|---|---|---|---|
| `list-hover` | `list.hoverBackground` | `#2A2D2E` | `#F2F2F2` | `#FFFFFF1A` / `#0F4A851A` |
| `list-sel` / `-fg` | `list.activeSelection{Background,Foreground}` | `#04395E` / `#FFFFFF` | `#E8E8E8` / `#000000` | — / `#0F4A851A` |
| `list-sel-inactive` | `list.inactiveSelectionBackground` | `#37373D` | `#E4E6F1` | — |
| `list-focus-outline` | `list.focusOutline` / `list.focusAndSelectionOutline` | `#0078D4` | `#005FB8` | `#F38518` |
| `list-drop` | `list.dropBackground` | `#062F4A` | `#D6EBFF` | — |
| `indent-guide` | `tree.indentGuidesStroke` | `#585858` | `#A9A9A9` | `#A9A9A9` / `#A5A5A5` |
| `selection` (text) | `editor.selectionBackground` | `#264F78` | `#ADD6FF` | `#F3F518` / `#0F4A85` |
| `caret` | `editorCursor.foreground` | `#AEAFAD` | `#000000` | `#FFFFFF` / `#0F4A85` |
| `tab-bg` / `tab-active-bg` | `tab.inactiveBackground` / `tab.activeBackground` | `#181818` / `#1F1F1F` | `#F8F8F8` / `#FFFFFF` | — |
| `tab-fg` / `tab-active-fg` | `tab.inactiveForeground` / `tab.activeForeground` | `#9D9D9D` / `#FFFFFF` | `#868686` / `#3B3B3B` | — |
| `tab-active-marker` | `tab.activeBorderTop` | `#0078D4` | `#005FB8` | — |
| `menu-sel` / `-fg` | `menu.selection{Background,Foreground}` | `#0078D4` / `#FFFFFF` | `#005FB8` / `#FFFFFF` | — |
| `scrollbar` / `-hover` / `-active` | `scrollbarSlider.*` | `#79797966` / `#646464B3` / `#BFBFBF66` | `#64646466` / `#646464B3` / `#00000099` | `contrastBorder` at 60%/80%/100% |
| `badge` / `-fg` | `badge.{background,foreground}` | `#616161` / `#F8F8F8` | `#CCCCCC` / `#3B3B3B` | — |
| `progress` | `progressBar.background` | `#0078D4` | `#005FB8` | — |
| **`danger`** | `errorForeground` | `#F85149` | `#F85149` | `#F48771` / `#B5200D` |
| `error` / `warning` / `info` (problems) | `editor{Error,Warning,Info}.foreground` | `#F14C4C` / `#CCA700` / `#59A4F9` | `#E51400` / `#BF8803` / `#0063D3` | `#F48771` `#FFD370` `#59A4F9` / `#B5200D` `#895503` `#0063D3` |
| `success` | `testing.iconPassed` | `#73C991` | `#73C991` | `#73C991` / `#007100` |

**Shape, type, metrics and motion**

These are independent of the scheme, except for density (§2.6).

| `--sv-*` | VS Code source | Value |
|---|---|---|
| **`font-family`** / `font-mono` | §1.2 | `Inter, system-ui, sans-serif` / `"JetBrains Mono", monospace` |
| **`font-size`**, `font-size-sm`, `-xs`, `-h1..h3` | `bodyFontSize*`, `fontSize.heading*` | 13px, 12px, 11px, 26/18/13px |
| **`radius`** (control) | `.monaco-text-button` / `cornerRadius.small` | 4px |
| `radius-xs`, `-md`, `-lg`, `-xl` | `cornerRadius.*` | 2, 6, 8, 12px |
| **`space-1..4`** (kept), `space-0-5`, `space-1-5`, `space-5..8` | `spacing.size*` | 4, 8, 12, 16 (kept); 2, 6; 20, 24, 32, 40 |
| `stroke` | `strokeThickness` | 1px |
| `row-h`, `control-h`, `tab-h`, `header-h`, `statusbar-h`, `titlebar-h`, `activitybar-w` | §1.3 | 22, 26, 35, 22, 22, 35, 48px |
| `indent`, `twistie-w`, `icon-size` | §1.3 | 8, 16, 16px |
| `sash-w`, `scrollbar-w` | §1.3 | 4, 10px |
| `elev-0..4` | §2.5 | see §2.5 |
| `motion-fast`, `motion`, `ease-out` | modernUI | 100ms, 150ms, `cubic-bezier(.22,1,.36,1)` |

The importer (§4.4) also emits every resolved VS Code key as `--vscode-<key with . → ->` (the names VS Code gives
webviews). A Svelte panel ported from a VS Code webview can then keep `var(--vscode-list-hoverBackground)`.

### 1.6 Mapping to egui `Style` (egui 0.37 fields, `style.rs`)

The tokens map onto egui's `Style`. Svelte nodes get the cascade on top, through CS5e. egui-native code
(cadatomic's shell, berth and quire through their style structs) reads the same `Style` and the theme accessors.

**`Visuals`**

| egui field | Token | Note |
|---|---|---|
| `dark_mode` | scheme | — |
| `panel_fill` | `bg` | side and bottom panels, status bar |
| `window_fill` | `surface-raised` | windows, menus, popups |
| `window_stroke` | 1px `border-widget` | — |
| `window_corner_radius` / `menu_corner_radius` | `radius-lg` (8) / `radius-md` (6) | modernUI radii |
| `window_shadow` / `popup_shadow` | `elev-4` / `elev-3` | `Shadow{offset,blur,spread,color}` holds the CSS values (i8/u8 limits are enough) |
| `extreme_bg_color`, `text_edit_bg_color` | `control-bg` | egui uses `extreme_bg` for text edits, the scroll track and progress bars |
| `faint_bg_color` | `list-hover` | striped rows (`striped` stays false) |
| `code_bg_color` | `button2-hover` | inline code |
| `hyperlink_color` | `link` | — |
| `warn_fg_color` / `error_fg_color` | `warning` / `error` | — |
| `selection.bg_fill` / `.stroke` | `selection` / 1px `fg-strong` | text selection; list rows get CSS (`list-sel`) |
| `text_cursor.stroke` | 2px `caret` | — |
| `widgets.noninteractive` | bg `bg`, `bg_stroke` 1px `border` (egui draws separators with it), fg `fg` | — |
| `widgets.inactive` | `bg_fill` = `weak_bg_fill` = `button2-bg`, stroke 1px `control-border`, fg `fg`, radius `radius`, expansion 0 | egui's default `expansion` grows widgets on hover. We set 0, as VS Code doesn't grow. |
| `widgets.hovered` | bg `button2-hover`, stroke `control-border`, expansion 0 | — |
| `widgets.active` | bg `toolbar-active`, stroke `control-border` | egui also draws focus with `active`. FX1 already separates focus-visible. |
| `widgets.open` | as hovered | open combo or menu |
| `interact_cursor` | `Some(CursorIcon::PointingHand)` | VS Code's `cursor:pointer` |
| `disabled_alpha` | 0.4 | VS Code dims disabled buttons to 40% [unverified selector] |
| `weak_text_color` | `muted` | — |
| `indent_has_left_vline` | true, colour `indent-guide` | tree indent guides |
| `button_frame` / `collapsing_header_frame` / `slider_trailing_fill` | true / false / true | — |

**`Spacing`**

| egui field | Value | Note |
|---|---|---|
| `button_padding` | (8, 4) | 26px buttons with a 16px line |
| `interact_size` | (40, `control-h` 26) | — |
| `item_spacing` | (8, 4) | — |
| `indent` | `indent` 8 | — |
| `icon_width` / `icon_width_inner` / `icon_spacing` | 18 / 10 / 8 | checkbox |
| `menu_margin` / `window_margin` | 4 / 8 | — |
| `scroll` | `ScrollStyle::floating()`, `bar_width` 10, dormant opacity 0 | VS Code fades bars |
| `combo_height` / `tooltip_width` / `menu_width` | 200 / 400 / 220 | — |

**Text styles and interaction**

| egui field | Value |
|---|---|
| text styles | Body and Button 13 Inter, Small 11, Heading 13 semibold, Monospace 13, `caption` 11 semibold (cadatomic's) |
| `interaction.tooltip_delay` | 0.35s (cadatomic's) |

## 2. Affordances

### 2.1 Evidence

| Source | Finding | What we take |
|---|---|---|
| NN/g, Meyer 2017, "Flat UI elements attract less attention and cause uncertainty" ([link](https://www.nngroup.com/articles/flat-ui-less-attention-cause-uncertainty/)) | 71 users and 9 page pairs on an eye tracker. Weak signifiers cost **22% more time** and **25% more fixations** (both significant). In one pair, 9 of 24 users stopped at a non-clickable heading. "Avoid ultraflat … on sites with dense content." | A CAD IDE is the dense case, so signifiers stay |
| NN/g, Moran 2015, "Flat design … why Flat 2.0 is better" ([link](https://www.nngroup.com/articles/flat-design/)) | "Almost flat": faint shadows, light highlights and layers, but no decorative effects | Subtle depth, not skeuomorphism |
| WCAG 2.2 SC 1.4.11 Non-text Contrast, AA ([link](https://www.w3.org/WAI/WCAG22/Understanding/non-text-contrast.html)) | Visual information that identifies a component, and its states, needs 3:1 against adjacent colours. A border that is the only cue for a field must meet 3:1. Focus indicators need 3:1. Author hover styling is exempt. | VS Code Modern's control borders (1.5–1.6:1) fail when the border is the cue (§0) |
| Windows 11 layering ([link](https://learn.microsoft.com/en-us/windows/apps/design/signature-experiences/layering)) | Elevation values: control 2, card 8, tooltip 16, flyout 32, dialog and window 128, **all with a 1px stroke**. A button at rest and on hover is 2, **pressed is 1**. | Pressed lowers. Elevation is shadow plus stroke. |
| Fluent 2 shadow ramp ([tokens source](https://github.com/microsoft/fluentui/blob/master/packages/tokens/src/utils/shadows.ts), [elevation](https://fluent2.microsoft.design/elevation)) | `shadowN` = ambient `0 0 2px` (8px for 28 and 64) plus key `0 N/2 N`. Dark themes about double the key alpha (14% → 28%). "Windows uses strokes instead of key shadows to outline an object." | Our elevation ramp (§2.5). In dark, shadows are weak, so tone and stroke carry elevation. |
| Material 3 (Compose `Surface` docs and others; m3.material.io did not render) | M3 moved from M2's shadow elevation to **tonal** surfaces, with shadows "only when needed", for example over a busy background | Elevation = lighter tone + stroke + shadow, so it reads in dark mode |
| VS Code 1.133 modernUI [1.133] | Its own `shadow-sm/md/lg/xl`: `0 0 4px .08`, `0 0 6px .08`, `0 0 12px .14`, `0 0 20px .15`. `lg` on menus, hovers, find, select dropdowns and notifications. `xl` on quick input and dialogs. `sm` on hovered inactive tabs. Also `sashHandles`, `scrollShadows`, `keyboardFocusOnly`. | Our scale stays compatible with VS Code |
| Blender `ThemeWidgetColors` ([API](https://docs.blender.org/api/4.5/bpy.types.ThemeWidgetColors.html)) | Per widget: `inner`, `outline`, `show_shaded` with `shadetop`/`shadedown` (−100..100: a vertical gradient that is raised for buttons and inverted for fields), `roundness`; `widget_emboss` (a 1px line under widgets) [unverified name] | The 1px lip under raised controls. Fields are inverted (sunken). |
| JetBrains New UI ([help](https://www.jetbrains.com/help/idea/new-ui.html)) | Inter, larger controls, higher contrast; buttons keep a visible border or fill; popups have shadows [unverified detail] | Same choices: Inter, bordered controls, shadowed popups |
| macOS AppKit [unverified, from memory] | Push buttons have a bezel with a light gradient and shadow. Text fields are inset (sunken). Borderless buttons are for toolbars only. | Raised means "press me", sunken means "type here" |
| Figma UI3, Fusion 360, KiCad [unverified] | Figma: floating panels with shadows; fields show a border only on hover (poor for discovery). Fusion: dropdown chevrons on tool icons. KiCad: native wx controls keep native bevels; the canvas stays flat. | Chevrons on anything that opens a menu. Fields always look like fields. |

### 2.2 The rules

Each rule gives the treatment, a one-line rationale and its source. The **top ten** are R1–R10.

| # | Rule | Rationale | Source |
|---|---|---|---|
| **R1** | **Push buttons, toggle buttons, segmented and split buttons are raised.** At rest they have the `button2-bg` fill (never transparent), a boundary of at least 3:1 to the surface (`control-border`, or the accent fill for primary buttons), radius 4, and a **1px lip** (`elev-1`: `0 1px 0 rgba(0,0,0,.35)` dark, `0 1px 1px rgba(0,0,0,.10)` light). | Raised means "press me". The lip carries the depth that a gradient border carries on Windows 11, at nearly no cost. | NN/g 2017; Fluent control elevation 2; Blender emboss |
| **R2** | **Pressed sinks.** `:active` removes the lip, darkens the fill one step (`toolbar-active`) and adds `inset 0 1px 2px rgba(0,0,0,.25)`. It takes effect at once, with no transition. | Direct feedback that the press registered | Windows 11: pressed elevation 1 |
| **R3** | **Hover lights up in the same frame.** The fill moves one step (`button2-hover`, `list-hover`, `toolbar-hover`). On standalone controls the lip stays, and an optional 100ms colour ease applies under `prefers-reduced-motion: no-preference`. | Confirms the target before the click. CS5e already restyles in the same frame. | VS Code; WCAG (hover is exempt, so it can stay subtle) |
| **R4** | **Editable fields are sunken and always visibly fields.** They have the `control-bg` fill, a boundary of at least 3:1 (`control-border`), radius 4, `inset 0 1px 1px rgba(0,0,0,.25)` (dark) or `.08` (light), and the I-beam cursor. On focus the border becomes `focus`. Validation errors use `invalid-border`. | "Type here" has to be visible before hovering. Figma's hover-only border hides that. | AppKit inset fields; Blender inverted shading; WCAG 1.4.11 |
| **R5** | **Read-only values look like text, not fields.** No fill, no border and no inset, with the `default` cursor (or I-beam if the value is selectable). A computed or locked value also shows a small lock or link codicon and gives its reason on hover. A **disabled** field keeps the field shape at 40%. | Separates "can't edit now" (disabled) from "is never edited here" (read-only) | VS Code settings: read-only rows are plain text [unverified]; cadatomic §2 rule 4 ("mixed values" shown) |
| **R6** | **Lists, tree rows, table cells and editor text stay flat** (`elev-0`). They show four distinct states: hover fill, active selection, inactive selection, and the focus outline (`list-focus-outline`, 1px inside). No shadows and no lips. | Dense repeated items: depth would be noise, and the selection colours are the signifier | VS Code, JetBrains, NN/g (flat works at low density only; rows are a convention) |
| **R7** | **Floating surfaces cast real shadows, with tone and stroke too.** That covers menus, context menus, dropdown lists, tooltips and hovers, popovers, the palette, toasts, dialogs and torn-off or floating panels. They get the `surface-raised` tone, a 1px `border-widget` stroke and `elev-3` (popovers, menus, tooltips) or `elev-4` (dialogs, quick pick, floating windows). In High Contrast, no shadow and a 1px `contrastBorder`. | Shows what is on top and what dismisses. The stroke keeps it readable in dark mode, where shadows vanish. | Windows 11 (flyout 32, dialog 128, stroke 1); VS Code modernUI `lg`/`xl`; Material 3 tonal elevation |
| **R8** | **Focus is visible, keyboard-only, and at least 3:1.** Rows and fields: 1px `focus` at offset −1 (VS Code parity). Standalone controls (buttons, toggles, tabs, chips): 2px `focus` at offset 1, so the ring is outside the fill and contrasts with the surface. Dark `focus` is `#2488DB` (VS Code's own `inputOption.activeBorder`): 3.2:1 or better on every dark surface, including selection. Stock `#0078D4` is 2.87:1 on `#313131`. | Keyboard users must always find focus. Pointer users don't get stray rings (FX1). | WCAG 1.4.11 and 2.4.7; VS Code modernUI keyboardFocusOnly |
| **R9** | **Disabled shows a reason.** Disabled controls keep their shape at `opacity .4` and stay hoverable, with a tooltip that gives the failed clause and a fix command, and a `not-allowed` cursor while hovered. Keyboard focus reaches them in menus and the palette, where the reason is read out. They are never hidden. | Disabled without a reason gives the uncertainty NN/g measured | cadatomic shell §2 rule 3, §6 "why disabled" |
| **R10** | **Cursors tell the verb.** `pointer` on buttons, tabs, links, rows and chips. `text` on editable text. `grab`/`grabbing` on drag handles. `col-resize`/`row-resize` on sashes. `crosshair` and tool cursors in the viewport (cadatomic's). `not-allowed` on disabled-with-reason. | A second channel for "what happens if I press", with no paint cost | VS Code CSS [1.133]; egui `CursorIcon` |
| R11 | **Sashes and splitters.** At rest, a 1px `border` hairline with a 4px hit area. On hover: the resize cursor at once, and after 300ms a 4px `focus`-coloured bar with a **grip** (three dots, or a 2×16 pill) at the midpoint. | Shows that the hairline is a handle without cluttering it at rest | VS Code sash and modernUI `sashHandles` [1.133] |
| R12 | **Drag handles are visible where dragging is the main gesture.** Reorderable rows, the layer list and the command strip show a 6-dot gripper (codicon `gripper`) on hover. Dragged items lift (`elev-3`, 0.9 opacity). Drop targets get `list-drop` and a 2px accent insertion line. | Without a grip, drag is undiscoverable. The lift shows what moves. | NN/g (signifiers for discovery); VS Code `list.dropBackground` |
| R13 | **Tabs.** Inactive tabs are recessed (`tab-bg` = chrome). The active tab joins the content (`tab-active-bg` = `surface`) with a 2px `tab-active-marker` on top. Hovering an inactive tab gives `elev-2` (modernUI `shadow-sm`). Close, dirty and pinned glyphs are always visible on the active tab. | VS Code's own affordance, kept as is | VS Code [1.133] |
| R14 | **Anything that opens something shows a chevron**: dropdowns, split buttons, menu buttons, collapsible headers (twistie). Overflow uses `…`. | Predicts the result of a click | VS Code; Fusion dropdown tools [unverified] |
| R15 | **Scrolled containers show edge shadows** (`inset 0 6px 6px -6px scroll-shadow` at the top when scrolled). Scrollbars fade in on hover or scroll. | "There is more above" | VS Code `scrollbar.shadow`, modernUI `scrollShadows` |
| R16 | **Cards and sections** (property panel groups, settings groups) use a 1px `border` or a tone step, with no shadow. They don't float. | Shadows mean "on top", not "grouped" | Fluent card elevation 8 is for free-floating cards only |
| R17 | **Toggles show state by fill, not colour alone.** On: `toggle-on-bg` plus a 1px `toggle-on-border`. A checkbox has a check glyph. A switch moves its knob. Each state reaches 3:1. | WCAG 1.4.11 states; colour-blind users | WCAG 1.4.11 |
| R18 | **Links and clickable text** (breadcrumbs, cross-probe references) get the `link` colour and an underline on hover. Non-clickable headings never use `link` or the accent. | The heading users mistook for a link in NN/g's study | NN/g 2017 |

### 2.3 What stays flat (so the layer never becomes skeuomorphic)

Region backgrounds, panel bodies, list and tree rows, table cells, editor text, the status bar, section headers,
separators and the viewport chrome all stay flat. Shadows are never more than `elev-4`. There are no gradients
beyond the optional 2-stop lip, no textures and no glow. In High Contrast every shadow becomes a stroke.

### 2.4 Editable against read-only (summary of R4/R5/R9)

| State | Fill | Border | Inset | Cursor | Extra |
|---|---|---|---|---|---|
| Editable | `control-bg` | `control-border` (≥ 3:1) | yes | I-beam | focus: `focus` border |
| Editable, invalid | `control-bg` | `invalid-border` | yes | I-beam | message below, codicon `error` |
| Pending (cadatomic `Op`) | `control-bg` | `control-border` | yes | I-beam | entered value plus a pending marker (cadatomic §3.3) |
| Read-only (by nature) | none | none | no | default (I-beam if selectable) | — |
| Locked or computed | none | none | no | default | lock or link codicon; reason tooltip |
| Disabled (temporarily) | `control-bg` at 40% | at 40% | no | `not-allowed` | reason tooltip (R9) |
| Mixed (multi-select) | `control-bg` | `control-border` | yes | I-beam | placeholder "Mixed", italic `muted` |

### 2.5 Elevation scale

| Token | Use | Dark | Light | HC |
|---|---|---|---|---|
| `elev-0` | rows, regions, text | `none` | `none` | `none` |
| `elev-1` | raised controls (lip) | `0 1px 0 rgba(0,0,0,.35)` | `0 1px 1px rgba(0,0,0,.10)` | `none` (1px `contrastBorder`) |
| `elev-2` | hovered tab, lifted chip | `0 0 4px rgba(0,0,0,.30)` | `0 0 4px rgba(0,0,0,.08)` (VS Code `shadow-sm`) | `none` |
| `elev-3` | menus, tooltips, popovers, dropdowns, drag ghost | `0 2px 8px #0000005C` (VS Code `0 2px 8px widget.shadow`) | `0 0 12px rgba(0,0,0,.14)` (VS Code `shadow-lg`) | `none` |
| `elev-4` | dialogs, quick pick, floating windows | `0 8px 28px rgba(0,0,0,.50)` | `0 0 20px rgba(0,0,0,.15)` (VS Code `shadow-xl`) | `none` |

The dark values roughly double the light alphas, as Fluent's ramp does. Dark values are a proposal for VIS1 to
tune. The light values are VS Code's.

### 2.6 Density

| Mode | `row-h` | `control-h` | `tab-h` | `header-h` | `icon-size` | padding (button) | Use |
|---|---|---|---|---|---|---|---|
| `compact` | 20 | 22 | 28 | 22 | 16 | 2×6 | data-heavy tables (BOM, constraint manager), per panel |
| `default` (VS Code) | 22 | 26 | 35 | 22 | 16 | 4×8 | the app |
| `comfortable` | 28 | 32 | 40 | 28 | 20 | 6×12 | pen and touch, presentations |

VS Code's `default` is already denser than most toolkits, and it is what cadatomic's framework calls "compact as the
CAD default". So `default` is the app default, and `compact` is a per-panel opt-in. Density is a token set (Rust
selects it, §4.2), not CSS math. `compact` rows (20px) fall below WCAG 2.5.8's 24px target in places. That is
accepted for opt-in tables and noted in VIS1's report.

## 3. Prior state: what CS5e, CS5f and CS5g give, and what's missing

| Capability | Status | Where |
|---|---|---|
| Colours, background, uniform border (colour, width, radius), opacity, `var()`, `@media prefers-color-scheme / prefers-contrast / prefers-reduced-motion`, `@layer`, `:state()`, `:hover/:active/:focus/:focus-visible/:disabled/:checked/:selected` | **have** | CS0, CS5e (master) |
| Same-frame hover and press per state on stock widgets (`WidgetVisuals` triple) | **have** | CS5e |
| `set_tokens(scheme, map)`, `set_color_scheme` | **have**, but only as a `:root` sheet. **Tokens don't set egui `Visuals`/`Spacing`**: unstyled widgets paint with egui's stock look. | CS5e |
| font family, weight and size in paint and measure; leaf padding; 1–4 value margin and padding | **in flight** | CS5f (`ci/cs5f`) |
| `box-shadow` (outer and inset lists, spread, blur; per state on stock widgets; culling) | **in flight** | CS5g (`ci/cs5g`) |
| keyboard-only focus drawing on button and checkbox | **have** (two kinds) | FX1 |
| `outline`, `outline-offset` (focus rings on every kind; egui `StrokeKind::Outside`) | **missing** | DS2 |
| `cursor` (per node, on hover; egui `ctx.set_cursor_icon`) | **missing** | DS2 |
| per-side borders (`border-top: 2px solid` for the tab marker and region edges) | **missing** (egui `RectShape` strokes are uniform; paint edge rects) | DS2 |
| `transition` for colour, opacity and box-shadow (R3; D7 says engine tweens) | **missing** (CS6 subset) | DS2 |
| `text-transform`, `letter-spacing`, `text-overflow: ellipsis` | **missing** | DS2 |
| `::placeholder` colour | **missing** (no pseudo-elements in CS0). Use the `placeholder` token read by the leaf. | DS1 |
| disabled-with-reason (`disabled-reason` prop → `on_disabled_hover_text`, a11y description) | **missing** | DS2 |
| scroll edge shadows (`:state(scrolled-top)` or a scroll-area prop) and scrollbar style from tokens | **missing** | DS2 / DS1 |
| gradients (2-stop vertical for an optional bevel) | **missing** (egui mesh). Low priority: the lip uses a shadow. | later |
| elevation, density and radius tokens; HC token sets; a default sheet per kind | **missing** (data plus a theme-origin sheet) | DS1, DS3 |
| CSS `url()` icons, SVG icons | planned | AS1, AS2 |

## 4. Architecture

### 4.1 Token layers

| Layer | Content | Names | Who writes it |
|---|---|---|---|
| L1 primitive | resolved VS Code colour keys (or a palette scale), plus the size registry | `--vscode-*` (alias layer), or DTCG palette keys | the importer (§4.4), from a theme file |
| L2 semantic | the roles of §1.5 | `--sv-<role>` | the base map (`vscode-*` sets) plus overrides |
| L3 component | per kind and state, only where a kind departs from the roles | `--sv-<kind>-<part>[-<state>]` (`--sv-button-bg-hover`, `--sv-tab-marker`) | the default sheet (§4.3) reads L2. cadatomic overrides L2 or L3. |

This is cadatomic's own three-layer plan (component framework §2.3: palette, roles, component visuals, after
rerun's `re_ui`), so the two repos agree. Components read only L2 and L3, and a hard-coded colour is a lint error,
as cadatomic's plan already says.

### 4.2 One token map, three outputs

```
 VS Code base set ─┐
 (vscode-dark/light/hc-dark/hc-light, generated data)
 affordance overlay (sv.affordance, on by default) ─┤
 density set (compact/default/comfortable) ─┤
 cadatomic theme (roles, its tokens) ─┤        svelters_egui::theme::Theme
 user theme (VS Code JSON or DTCG) ─┘──resolve──►  ├─► :root sheet (theme origin) → Svelte cascade (CS5e)
                                                   ├─► egui::Style (Visuals, Spacing, text) → ctx.set_style_of(Dark|Light)
                                                   └─► typed accessors theme.color(Role) / .metric(…) / .shadow(…)
                                                          → cadatomic fills berth DockStyle, quire EditorTheme, its shell
```

- **svelters-egui gets a `theme` module** (DS1):
  - `Theme { scheme, contrast, density, tokens: TokenMap }`;
  - `Theme::vscode(scheme, contrast)`;
  - `Theme::with(overrides)`;
  - `Theme::egui_style() -> egui::Style`, following §1.6;
  - typed getters.

  `EguiBackend::set_theme(&Theme)` does `set_tokens` for every scheme, `set_color_scheme`, `set_media(contrast)`
  and `ctx.set_style_of(Theme::Dark|Light, …)`, and it bumps a theme generation, so shape caches (cadatomic
  framework §8) invalidate.
- **berth and quire never depend on svelters.** Their `DockStyle`/`EditorTheme` stay plain structs (ARCH-CRATES
  §1). cadatomic fills them from `Theme` accessors in one adapter.
- **The viewport** keeps its own canvas sets, kept apart from chrome (cadatomic §2.3). The theme supplies the
  selection, hover and pick colours (`list-sel`, `focus`, `accent`), so a selection in the viewport and in the tree
  reads as the same thing.
- **The JS runtime** stops sending its Adwaita `DEFAULT_TOKENS` on egui. The backend owns the defaults, and
  GTK/Qt/AppKit keep theirs. (Otherwise two theme-origin `:root` sheets fight on source order.)

### 4.3 Where the defaults live

| What | Where | Why |
|---|---|---|
| Engine (no look) | `svelters-style` | Generic |
| VS Code token sets (4) and density sets (3) | `svelters-egui/src/theme/data/*.json` (a generated, MIT-derived token map) | The built-in look of the egui backend |
| Default look of every kind: `button { background-color: var(--sv-button2-bg); box-shadow: var(--sv-elev-1); … }`, `button:hover`, `text-edit`, `checkbox`, `tab`, `menu-item`, `tooltip`, … | `svelters-egui/src/theme/base.css`, a theme-origin sheet in `@layer sv.base` | One readable file. It is restyled like any sheet, CSS-only hot reload (H2) works, and VIS1 iterates on it without Rust rebuilds. |
| Affordance rules (§2.2) | `theme/affordance.css`, `@layer sv.affordance`, on by default, off with `Theme::affordances(false)` | Keeps the exact VS Code base reproducible for VIS1 |
| cadatomic's look | cadatomic: its `theme` turns into token overrides (its `Tokens::DARK/LIGHT` map onto `--sv-*` roles) and, if needed, a theme-origin sheet in `@layer cad` after `sv.*`. Svelte sources stay in cadatomic. | cadatomic owns its brand. svelters owns the mechanism and a good default. |
| Per-component styling | author `<style>` in each component | Normal CSS. It beats theme origin. |

CS0 has no user-agent origin, so the base sheet sits at theme origin in the lowest layer. Author rules beat it, and
a `@layer cad` theme sheet beats `sv.*`. This needs no engine change.

### 4.4 Modes and user theme import

- **Modes.** `prefers-color-scheme` (dark/light, from cadatomic's `ui.theme` = dark/light/system) and
  `prefers-contrast: more` select the HC sets. The backend sets `prefers-contrast` from the app (cadatomic settings,
  or the OS high-contrast flag: Windows `SPI_GETHIGHCONTRAST`, macOS `accessibilityDisplayShouldIncreaseContrast`
  [unverified API names]). That gives four sets, matching cadatomic §2.3.
- **Import VS Code theme JSON directly: yes, recommended.** There are thousands of themes, users know them, and the
  format is simple:
  - JSONC with `include` chains;
  - `colors` (about 700 keys);
  - `tokenColors` (TextMate scopes) and `semanticTokenColors`, which quire's `EditorTheme` takes for syntax.

  The importer:
  1. resolves `include`;
  2. fills unset keys from a **defaults table captured from VS Code** for the theme's `type`/`uiTheme`
     (`vs`/`vs-dark`/`hc-black`/`hc-light`): VIS1 step 0 reads every resolved `--vscode-*` variable over CDP;
  3. maps keys to `--sv-*` (§1.5);
  4. emits the `--vscode-*` alias layer;
  5. reports keys it ignored.

  VS Code's own themes and colour defaults are MIT (Code - OSS). The importer lives in `svelters-egui::theme`
  (no egui types in its parser, so it can move to a small crate later).
- **DTCG JSON** is cadatomic's open question C6 (DTCG or RON). Recommendation:
  - DTCG JSON is the token **file** format (tool exchange, Figma Tokens and Style Dictionary).
  - The importer reads both DTCG and VS Code JSON into the same `TokenMap`. RON adds nothing on top.
  - The resolved form at run time is the `:root` sheet.

### 4.5 Icons

| Option | For | Against |
|---|---|---|
| **Codicons** (VS Code's set: a 16px grid, an icon font, about 500+ glyphs [unverified count], CC-BY-4.0 for the icons [unverified licence detail]) | Exactly VS Code's look and metaphors (`gripper`, `chevron-*`, `lock`, `error`, `pin`, `split-*`). Already a font, so WG1's `icon` kind draws it today without AS1. | Attribution needed. No CAD or EDA glyphs. One weight. |
| **Phosphor** (cadatomic today, vendored 2.1.0 regular, MIT) | Large set (about 1,500 icons in 6 weights [unverified]), already wired (`theme/icons.rs`, WG1 `register_icons`) | Rounder style, 24px grid, not VS Code's look |
| SVG through AS1 (resvg, tinted monochrome) | Custom CAD and EDA icons, extension icons, multi-colour where needed | Needs AS1. Rasterises per size. |

**Recommendation:**
- **Codicons for chrome** (shell, panels, lists, toolbars, dialogs), as a font through WG1's `icon` with a name
  table generated from codicons' mapping file.
- **Semantic icon ids**: components write `icon="close"`, and the theme maps ids to (family, glyph). This is the
  same idea as VS Code's product icon themes, so a Phosphor or user icon theme is a mapping swap.
- Phosphor stays as the fallback set for ids codicons lacks.
- CAD-specific tools get an SVG set through AS1 later.
- Icons take `color` from `icon` / `fg` / `disabled-fg`.

Changing the chrome set is cadatomic's choice (Q3).

### 4.6 Making Rust panels and Svelte panels one look

- **Same tokens:** every egui-native widget reads `ui.style()` (from `Theme::egui_style`) or the `Theme` getters.
  Every Svelte node reads `var(--sv-*)`. There are no third colours.
- **Same metrics:**
  - berth tab height = `tab-h`, header = `header-h`, sash = `sash-w`;
  - quire gutter, line and scrollbar from `font-mono`, `scrollbar-w` and `selection`;
  - cadatomic rows = `row-h`.
- **Same states:** R6's four list states and R8's focus come from shared helpers. In Rust that's
  `theme::paint_row_state(ui, rect, RowState)` and `theme::focus_ring(ui, rect, Kind)`. In CSS it's the base
  sheet's rules for the same tokens.
- **Same icons:** both go through the icon id table.
- **Check:** VIS1's composite shot (cadatomic's MCP `screenshot`) compares a berth tab strip, a Rust tree and a
  Svelte property panel side by side for token drift.

## 5. Lanes

| Lane | Content | Size | Needs |
|---|---|---|---|
| **DS1** theme module | `svelters_egui::theme`: `Theme`, `TokenMap`, 4 VS Code sets plus 3 density sets as data, `egui_style()` (§1.6), `set_theme`, typed getters, `base.css` (`@layer sv.base`, every Tier 0 kind and state), `placeholder`/scrollbar from tokens. The runtime stops sending `DEFAULT_TOKENS` on egui. `--sv-*` names in `packages/style` gain the new roles. | 1 lane | CS5f, CS5g merged; token data from §1.5 (refined by VIS1 step 0) |
| **DS2** engine gaps | `outline`/`outline-offset`, `cursor`, per-side `border-*`, `transition` for colour/opacity/box-shadow (egui `animate_value_with_time`, `request_repaint`), `text-transform`, `letter-spacing`, `text-overflow`, the `disabled-reason` prop (tooltip and a11y description), scroll edge shadows | 1–1.5 lanes (STYLE writer) | CS5e; parallel with DS1 |
| **DS3** affordance layer | `affordance.css` (R1–R18), elevation tokens, HC rules (shadows to strokes), focus ring helpers in Rust (`focus_ring`, `paint_row_state`) | 0.5 lane | DS1, DS2 (outline, cursor) |
| **DS4** theme import | VS Code JSONC plus `include` plus the defaults table → `TokenMap`, the `--vscode-*` alias layer, DTCG reader, the `tokenColors` hand-off for quire, a diagnostics report | 0.5 lane | DS1; VIS1 step 0 for the defaults table |
| **DS5** icons | Codicon font (vendored, licence notice), generated name table, semantic icon ids with a per-theme mapping, Phosphor fallback | 0.5 lane | WG1 (`icon`) |
| **VIS1** visual conformance | §6: capture scripts, the metrics schema, gallery `vis` pages, contact sheets, checks, the iteration loop on `base.css`/`affordance.css` | 1 lane (capture is scripts; the iteration loop overlaps DS1/DS3) | TK1 snapshots; DS1 for the first comparison |
| cadatomic (request, through SV) | Map `theme::Tokens` onto `--sv-*` roles, or replace them with `Theme`; fill `DockStyle`/`EditorTheme` from it; adopt codicons if Q3 says yes; composite screenshots through its MCP | cadatomic's | DS1 |

Order: DS1 ∥ DS2 ∥ VIS1 step 0 → DS3 ∥ DS4 ∥ DS5 → the VIS1 loop. Total about 4–5 lanes.

## 6. VIS1: the visual-conformance process

### 6.1 References: what to collect

VS Code **1.133.0 is installed system-wide** on the WSL PC: `/mnt/c/Program Files/Microsoft VS Code/Code.exe`
(not under `AppData/Local/Programs`). The theme JSONs are under `…/a5b5009513/resources/app/extensions/theme-defaults/`.

| Set | Themes | Content |
|---|---|---|
| Whole workbench | Dark Modern, Light Modern, HC Dark, HC Light; plus Dark 2026 and Dark Modern with `modernUI: true` (the elevation reference) | Explorer tree expanded (indent guides), editor group with 3 tabs (active, dirty, preview), panel open, status bar, a context menu open, quick pick open |
| Element × state crops | the 4 base themes | primary, secondary and icon (toolbar) buttons; input-option toggle; checkbox off/on; select closed/open; input (empty with placeholder, filled, focus, validation error); list row (rest, hover, selected, selected and focused, inactive selection, focused only); tree twistie; tab (active, inactive, hover, dirty, focused); pane header; sash hover; scrollbar (rest, hover, drag); menu (item hover, separator, submenu, keybinding label); hover widget; notification toast; dialog; settings rows (checkbox, number, dropdown, description); progress bar; badge |
| Metrics | the same elements | box size, padding, border, radius, font (family, size, weight, line height), colours, `box-shadow`, cursor |
| Variables | each theme | every resolved `--vscode-*` on `.monaco-workbench` (feeds DS4's defaults table) |

### 6.2 How an agent captures them (from WSL)

1. **Clean profile.** Write `C:\vis1\ud\User\settings.json`:
   - `workbench.colorTheme`;
   - `window.zoomLevel: 0`;
   - `window.titleBarStyle: "custom"`;
   - `workbench.experimental.modernUI`;
   - `workbench.tree.indent: 8`;
   - `editor.fontSize: 14`.
2. **Launch.** Run `"/mnt/c/Program Files/Microsoft VS Code/Code.exe" --user-data-dir 'C:\vis1\ud' --extensions-dir
   'C:\vis1\ext' --disable-extensions --remote-debugging-port=9229 'C:\vis1\workspace'`. Call `Code.exe`, not
   `bin/code`: the shell script would route to the WSL remote.
3. **CDP is the main tool.**
   - From WSL, reach `http://localhost:9229/json`. That needs WSL mirrored networking; otherwise run a Windows-side
     `node` script through `cmd.exe /c` [unverified for this PC].
   - `CSS.forcePseudoState` (`hover`, `active`, `focus`, `focus-visible`) renders each state deterministically.
   - `Emulation.setDeviceMetricsOverride` with `deviceScaleFactor` 1 and 2 fixes the DPR.
   - `Page.captureScreenshot` with `clip` gives element crops.
   - `Runtime.evaluate` reads `getComputedStyle` and the `--vscode-*` variables.

   Playwright's `_electron` (as VS Code's smoke tests use) is the fallback [unverified that `--remote-debugging-port`
   passes through `Code.exe`; check in the lane's first hour].
4. **Whole window.** `powershell.exe -NoProfile -File capture.ps1`: user32 `PrintWindow(hwnd, hdc,
   PW_RENDERFULLCONTENT=2)` works when the window is occluded. `Graphics.CopyFromScreen` on the window rect is the
   fallback. Save to `C:\vis1\out` and copy through `/mnt/c`.
5. **Store.** `fixtures/vis-reference/vscode-1.133/<theme>/<element>-<state>@{1,2}x.png`, plus `metrics.json`,
   `vars.json` and the capture script and settings. Crop to widgets (no logos). The whole-workbench shots go to the
   CI artifact if they're too large.
6. **The real app** (composite, not a gate). A cadatomic session runs `cadatomic mcp` → `screenshot`, or the RPC
   `screenshot.window`, at `pixels_per_point` 1 with the same theme. svelters agents ask main-cadatomic for the
   shots; they don't edit cadatomic.

### 6.3 Our renders

- A gallery `vis` page set in `fixtures/gallery-egui` (one page per kind family; each kind in each state in a row).
- States are reached with kittest input: hover by pointer move; pressed by pointer down plus a step; focus-visible
  by Tab; disabled and checked by props.
- Rendered through TK1's path: `svelters-testkit --features snapshots`, `WGPU_BACKEND=vulkan`, lavapipe, Inter
  bundled, `pixels_per_point` 1 and 2, 4 themes, with and without affordances.
- Metrics come from the scene (`computed_style(h)`, layout rects, theme tokens) in `metrics.json` with the **same
  schema** as the VS Code capture, so one script diffs them.

### 6.4 Comparing and iterating

1. **Numeric diff (the gate).** Per mapped element: sizes, padding and radius within ±1px; colours ΔE2000 ≤ 2 on
   the exact base (affordance off); font size exact.
2. **Contact sheets for agents.** One PNG per theme and kind family: VS Code crop | ours | diff heat map, with a
   label per row. The agent Reads the PNGs, edits `base.css`/`affordance.css`/token data (no Rust rebuild, through
   H2 or a rerun), and reruns only the changed families.
3. **Pixels are not a gate.** DirectWrite/Skia text and epaint text differ. SSIM on grayscale crops is logged as a
   trend (target ≥ 0.85).
4. **Affordance checks (automatic, affordance on):**
   - per interactive kind, rest ≠ hover ≠ pressed ≠ focus-visible ≠ disabled (mean absolute pixel difference above
     a threshold);
   - control boundary or fill ≥ 3:1 against the parent surface;
   - text ≥ 4.5:1 (≥ 3:1 for 18px+ or bold 14px+);
   - focus ring ≥ 3:1 against adjacent colours;
   - every floating kind has a non-`none` `box-shadow` (except HC), and every row kind has none;
   - every disabled node in the gallery has a reason (a11y description non-empty).

### 6.5 Acceptance

- Every Tier 0 kind, and the Tier 1 kinds on `ci/int-widgets`, × {rest, hover, pressed, focus-visible, disabled,
  plus checked or selected or open where they apply} × 4 themes × {exact, affordance} has a committed snapshot,
  matched in CI's `egui-snapshots` job.
- On the exact base, at least 90% of the mapped numeric checks pass. Each failure is listed with its reason (egui
  limit, missing DS2 feature, or a deliberate deviation).
- With affordances on, every §6.4 check passes in all 4 themes.
- The contact sheets are attached to the milestone record, and the coordinator has viewed them.
- One composite cadatomic screenshot (Rust dock, Rust tree, Svelte panel) shows no token drift. This is reviewed,
  not gated.

## 7. Open questions for the user

1. **Which VS Code is the base: Modern (Dark/Light Modern) or 2026 (Dark/Light 2026, the current default in
   1.133)?** **Recommended: Modern.** It is what "looks like VS Code" means to most users, and it is less flat than
   2026, whose input borders are 1.41:1 (dark) and 1.15:1 (light). 2026 comes free through the importer, as a theme
   choice.
2. **Affordances on by default, with an "exact VS Code" switch?** **Recommended: on by default**
   (`ui.affordances = true`; off gives pixel-faithful VS Code colours). That is what the request asks for, and the
   exact base stays testable.
3. **Codicons for cadatomic's chrome, replacing Phosphor there?** **Recommended: yes**, with semantic icon ids so
   the set is a theme mapping. Phosphor stays for ids codicons lacks, and CAD tools get SVGs through AS1. It needs
   CC-BY-4.0 attribution in cadatomic's licences file.
