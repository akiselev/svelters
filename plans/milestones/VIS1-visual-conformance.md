# VIS1: visual conformance against VS Code (the default widget look)

Track: DESIGN (`plans/research/DESIGN.md` §6). Request (user): "a UI visual conformance agent. It should take
screenshots and look at them, and develop the default widget look to be consistent with VS Code's, so that we have a
proper base to start from." Lane VIS1, branch `ci/vis1`.

## 1. Result

| | Checks passing (both themes) |
|---|---|
| Before (egui's stock look, `VIS1_STOCK=1`) | 65 of 286 (23%) |
| After (VIS1, `svelters_egui::vscode_look`) | 263 of 286 (92%) |
| After VIS1b (`svelters_egui::theme`, `Theme::vscode(..).with_affordances(false)`; tabs and progress fill) | 278 of 286 (97%) |
| VIS1b with 22 more checks (`ta` text alignment, list-row `h`, focus-ring `oc`) | **292 of 308 (95%)**; the floor is 90% (DESIGN §6.5) |

Contact sheets (VS Code 1.133 left, svelters-egui right, failed checks named per row):

| Theme | Before | After |
|---|---|---|
| Dark Modern | `conformance/visual/sheets/dark-modern-before.png` | `conformance/visual/sheets/dark-modern-after.png` |
| Light Modern | `conformance/visual/sheets/light-modern-before.png` | `conformance/visual/sheets/light-modern-after.png` |

The full per-check table is `conformance/visual/report.md` (CI writes `target/vis1/report.md`).

## 2. References: VS Code 1.133.0, captured on this PC

`conformance/visual/capture/` (`setup.sh`, `settings.json`, `cdp.js`, `capture.js`, `winnode`):

- `setup.sh --launch` writes a clean profile (`C:\vis1\ud`, extensions disabled, custom title bar, dialogs and menus) and
  a small workspace, and starts `Code.exe --remote-debugging-port=9229`.
- **Reaching CDP from WSL**: the WSL network here is NAT, so the Windows loopback port is not reachable from Linux.
  The capture script runs **on Windows inside `Code.exe` as Node** (`ELECTRON_RUN_AS_NODE=1`, Node 24 with `fetch`
  and `WebSocket`), launched from WSL by `winnode`. No Windows Node install and no `cmd.exe`/PowerShell needed.
- **1x**: the display is at 125%. `Emulation.setDeviceMetricsOverride` changes `devicePixelRatio` but not Chromium's
  border snapping (borders stayed 0.8 px), so the profile sets `window.zoomLevel` to log(1/1.25)/log(1.2) and the
  page runs at a real DPR of 1. Crops are clipped in DIPs (`cdp.js` measures the factor).
- Every element is a **real workbench widget**, not a DOM copy (VS Code paints button hovers and list selections
  from JS): the notification toast and its primary button (the "extensions are disabled" toast after a reload), the
  explorer tree (rows rest/hover/selected/focused/inactive, twistie, pane header), editor tabs (active, inactive,
  hover, dirty), the editor scrollbar, the find widget (input box rest/focus/filled/invalid, the Match Case toggle
  rest/hover/focus/checked), the settings editor (checkbox, select box and its drop-down, number input, a setting
  row), a context menu (item rest/hover, keybinding, separator), a hover on an activity bar item, the save dialog
  (primary and secondary buttons) and the sash. States come from real mouse moves and clicks over CDP, plus
  `CSS.forcePseudoState` for CSS states; disabled is VS Code's own `disabled` class on the real widget.
- Stored per theme in `conformance/visual/vscode/{dark-modern,light-modern}/`: one crop per element and state
  (`<element>-<state>.png`, 1x, the element rect plus 4 px) and `metrics.json` (`meta`: VS Code version, theme,
  scale; `items`: `getComputedStyle` sizes, padding, border, radius, colours, font, outline, box-shadow, opacity;
  `vars`: every resolved `--vscode-*`, the defaults table DS4 needs).
- Not in VS Code, so no reference: radio button, switch, slider. Radios are compared with the checkbox; the switch
  and slider are in the page for review only. A running progress bar was not reachable; its colour comes from
  `--vscode-progressBar-background`. The settings text box and a disabled menu item were not reachable in this
  layout (the number box stands in for the former).

To refresh: `conformance/visual/capture/setup.sh --launch` (background), then
`VIS_THEME=dark-modern conformance/visual/capture/winnode 'C:\vis1\capture.js'` (and `light-modern`), and copy
`/mnt/c/vis1/out/<theme>/` to `conformance/visual/vscode/<theme>/`.

## 3. Our side

- **Page**: `fixtures/gallery-egui/src/pages/VisControls.svelte` (an embed export): every kind in its static states,
  each named `<element>:<variant>` (accessible name).
- **Gate**: `crates/svelters-testkit/tests/vis1.rs`. Mounts the page with the look applied, per theme; drives
  hover (pointer move), pressed (pointer down), focus (AccessKit focus); measures from what egui paints (the frame's
  `RectShape`, the checkbox box, the first `TextShape`) into `target/vis1/<theme>/metrics.json` in the capture's
  schema; compares 286 mapped checks: height, radius and border width within 1 px, fill, border, text and focus-ring
  colour within ΔE2000 2 (both composited over the theme's `--sv-bg`, VS Code's `opacity` applied), font size exact.
  Pixels are never compared. The gate is a ratchet (`conformance/visual/baseline.json`: every check that passes
  must keep passing; `SVELTERS_UPDATE=1` records) plus the 90% floor. It runs in the main GTK/egui job.
- **Crops**: with `--features snapshots` each probe is rendered (wgpu, lavapipe) and cropped; CI's `egui-snapshots`
  job runs it and uploads `target/vis1/`. `conformance/visual/tools/sheet.py <theme> target/vis1/<theme> out.png`
  makes the contact sheet; `montage.py` grids a capture; `summary.py` prints a metrics file.

```sh
pnpm --filter ./fixtures/gallery-egui build
WGPU_BACKEND=vulkan tools/dev/cargo-slot cargo nextest run -p svelters-testkit --features snapshots -E 'binary(vis1)'
VIS1_STOCK=1 ...   # egui's stock look into target/vis1-stock (the "before"), no gate
python3 conformance/visual/tools/sheet.py dark-modern target/vis1/dark-modern /tmp/dark.png
```

## 4. The default look (what changed)

Since VIS1b the look is DS1's `svelters_egui::theme` (VIS1's own `vscode_look` module is gone):

- **Data** (`crates/svelters-egui/src/theme/data/`): the VS Code keys VIS1 measured that DS1's sets lacked
  (`button.secondaryBorder`, `checkbox.foreground`, `inputOption.hoverBackground`, `inputOption.activeForeground`,
  `tab.hoverBackground`), High Contrast role values for them, `checkbox-radius` 3px in `shape.json`, and `button2-border`
  in the affordance overlay (VS Code's `#CCCCCC26` is below R4's 3:1, so the overlay keeps DS1's control boundary).
- **Roles and components** (`theme/mod.rs`): new roles `button2-border`, `check-bg`, `check-fg`, `toggle-hover`,
  `toggle-on-fg`, `tab-hover-bg`; `button-border-color` now defaults to `button2-border`, `button-bg-active` to
  `button2-hover` (VS Code has no pressed look), `checkbox-bg` to `check-bg` (Light Modern's check box fill is
  `#F8F8F8`, not the input's), new `checkbox-fg`; `menu-radius` 8 and `window-radius` 12 (measured).
- **`Theme::egui_style`** (`theme/egui_style.rs`): button text 12 px (`.monaco-text-button`), `bg_fill` (check boxes,
  radios) kept under the pointer and while pressed, one stroke for every stock widget = `checkbox-border-color`,
  VS Code's list scrollbars (floating 10 px, no rest track). `Theme::apply_to` also leaves an `Applied` (colours,
  metrics) in the context's data for chrome painted outside the cascade.
- **`theme/base.css`** (`@layer sv.base`): VIS1's per-kind rules on the theme's tokens: secondary and `.primary`
  buttons (`--sv-accent*`, now that the runtime no longer sends `DEFAULT_TOKENS` to egui), fields (`text-edit`,
  `combo-box`, `drag-value`, focus border), checkbox/radio/switch, the input option toggle (`toggle-button`), list rows
  (`toggle-button[frame="false"]`).
- **Runners**: `run_desktop` and `svl_backend.run` call `backend.set_theme(&Theme::vscode(scheme, Normal))` (scheme
  from the context at start) unless the app installed a theme first. An embedder (cadatomic) calls `set_theme` itself.
  Tests opt in (TK1's snapshots are unchanged).
- **Chrome from the applied theme** (VIS1b): `tab-view` paints VS Code tabs (`tab-h` 35, `tab-*` colours, hover
  `tab-hover-bg`, a 1 px `tab-active-marker` on top, the `border` hairline between tabs, the close button inside the
  tab; egui's selectable buttons without a theme); progress bars fill with `progress`; the `invalid` outline uses
  `invalid-border` (else `error_fg_color`).
- Engine changes from VIS1 (`style.rs`, `leaf.rs`, runtime `style.js`):
  - `toggle-button`, `combo-box`, `radio-button`, `switch`, `drag-value` take per-state CSS visuals like the other
    stock widgets; the two-state kinds match `:checked`.
  - A `:checked` fill and colour also set `Visuals::selection` (egui paints selected buttons from it), and a focused
    `text-edit`'s border colour sets `selection.stroke` (egui's focused frame).
  - The runtime sent scheme `light` to egui unless the app chose one, forcing light CSS in a dark app; it now sends
    `auto` for the engine target and egui follows its context's dark mode.
  - A frameless `toggle-button` is a selectable row (`frame_when_inactive`: frame on hover and when checked, body
    text); button text uses `TextStyle::Button` explicitly (a plain string fell back to Body).

## 5. Match per element (exact base, both themes)

| Element | Result | Notes |
|---|---|---|
| Primary button (rest, hover, pressed, focus, disabled) | match | Light focus: VS Code keeps the rest fill, ours shows the hover fill (ΔE 2.9); dark VS Code shows hover on focus |
| Secondary button | match | |
| Checkbox (box) | match | 18x18, radius 3 |
| Radio (vs checkbox colours) | match | circle, no VS Code radio |
| Input option toggle | match | size 26 on our page (VS Code 20, a find-widget size) |
| Input box (rest, focus ring, filled, invalid) | match | 26 tall (VS Code 25 in find, 27 in settings) |
| Input disabled | **deviation** | egui dims a disabled widget to 40%; VS Code's settings box does not dim |
| Select box | match | |
| List row (rest, hover, selected) | match | text is centred (egui buttons); VS Code is left aligned |
| Pane header | **deviation (DS2)** | 26 tall, 12 px; VS Code 22 px, 11 px bold uppercase: needs a per-kind font size and `text-transform`, left to DS2 |
| Tabs (`tab-view`) | match (VIS1b) | 35 px, `tab.*` colours, hover, top marker; the dirty dot and the focused-group colours are not drawn |
| Progress fill | match (VIS1b) | the applied theme's `progress`; a per-node CSS colour (`accent-color`, not in egui's style table yet) waits for DS2 |
| Scrollbar, sash, switch, slider | review only | sheets; floating 10 px bars (VS Code lists), editor bars are 14 |
| Menus, tooltip, toast, dialog | references captured | `window_fill`/`window_stroke`/radii from them; not probed (needs an open popup in the page; VIS1b skipped it, see §8). DS1's dark `elev-3`/`elev-4` (`0 2px 8px #0000005C`, `0 8px 28px #00000080`) differ from the measured `0 0 12px rgba(0,0,0,.14)`/`0 0 20px rgba(0,0,0,.15)`; the light set matches |

Focus rings beyond the input box (buttons, checkbox, select: VS Code's 1px `focusBorder` outline) need `outline`
(DS2); FX1 draws focus only after keyboard focus.

## 6. Coordination

- DS1 (`svelters_egui::theme`) superseded `vscode_look`; VIS1b folded it in (§4). The gate checks VS Code's exact
  colours (`with_affordances(false)`); the runners install the affordance overlay (on by default, DESIGN Q2).
- Left to DS2: the pane header's 11 px uppercase (`font-size` per kind, `text-transform`), focus rings on buttons,
  check boxes and selects (`outline`), a per-node progress colour (`accent-color` in egui's style table).
- Left to DS3: the input box's disabled look (egui dims a disabled widget to 40%; VS Code's settings box does not).
- DESIGN.md §1.5 corrections from the capture are listed there under "VIS1 measured".

## 7. Verification

VIS1 (local): `binary(vis1)` 263/286; svelters-egui filtered (`style`, `theme`, `gallery`, `gallery_views`, `wg4b`,
`vscode_look`); runtime `style.test.mjs` + `style-engine.test.mjs`.

VIS1b (local, after its last edit and the merge of `origin/master` at `424ebcf4`):
- `nextest -p svelters-testkit -E 'binary(vis1)'`: both pass, **278/286**; baseline recorded with `SVELTERS_UPDATE=1`
  and `--features snapshots` (lavapipe), which also refreshed `report.md` and the "after" contact sheets.
- `nextest -p svelters-egui` filtered to `test(/theme/)`, `binary(theme)`, `binary(views)`, `binary(style)`,
  `binary(tier0)`: 31 pass (before the merge also `gallery_views`, `widgets`: pass; `widgets_panel` needs
  `fixtures/embed-egui/dist`, not built here).
- `cargo check -p svelters-egui -p svelters-testkit --all-targets`.
- Not run locally: the rest of the workspace suites, the other backends, conformance, the gallery fixture rebuild
  (the gate used the existing `fixtures/gallery-egui/dist`). CI runs them.

## 8. Handoff

**Stopped (user, 2026-10-22, relayed by svelters-agent):** svelters will draw all its widgets itself; egui stays only as
painter, input, text shaping, layers and AccessKit. Mapping the theme onto egui's shared `Visuals` is superseded.
`ci/vis1` is pushed as WIP (not waited on CI). Last CI seen: on `fd573ada` four green, macos-appkit failed in
`counter-mac` `request_frame_delivers_one_frame` (a CADisplayLink timing assertion this lane does not touch).

Done in VIS1b: the fold of `vscode_look` into `theme` (§4); VS Code tabs (`tab-h`, `tab.*` colours, top marker, the
close codicon drawn as a cross inside the tab); progress bars as VS Code's square 2 px line in `progress`; list rows
left aligned at `row-h` (themed); the `invalid` outline in `invalid-border`; gate checks `ta` (text alignment, list
rows and tabs), list-row `h`, and `oc` (focus ring) on button, check box and select focus. Gate 292/308, baseline
recorded (`SVELTERS_UPDATE=1`, `--features snapshots`), `report.md` and the "after" sheets refreshed. After the last
edits only `binary(vis1)` and `cargo check -p svelters-egui` ran; the egui filtered suites ran before them.

**Reuse for the own-widget lanes (keep intact; they are the acceptance tests):**
- the VS Code 1.133 references: `conformance/visual/vscode/<theme>/` (crops, `metrics.json` with `items` and every
  resolved `--vscode-*` in `vars`), and the capture scripts `conformance/visual/capture/` (§2);
- the gate `crates/svelters-testkit/tests/vis1.rs`: the probe table (node, state, part, reference, checks), the
  measurement from painted shapes (independent of egui widgets: it reads `RectShape`/`TextShape`), ΔE2000 colour
  checks, the ratchet `conformance/visual/baseline.json` and the 90% floor; the contact sheets (`tools/sheet.py`);
- the theme data and roles in `svelters_egui::theme` (DS1 plus the VIS1 keys of §4), which the new widgets should
  paint from directly instead of through `Visuals`.

**Visible gaps (the gate fails or does not measure them):**
1. Focus rings: VS Code draws a 1 px `focusBorder` outline (offset 1-2 px on buttons, -1 px on check box, select,
   list row); we draw none. The `oc` checks now fail on buttons, check box and select (8 checks); the input box's
   passes. The gate compares VS Code's outline with our border, so an own-drawn ring needs `measure` to pick it up.
2. Pane header: 26 px / 12 px vs 22 px / 11 px bold uppercase (`text-transform`).
3. Disabled input: egui dims to 40%; VS Code's settings box does not dim.
4. Light primary button on focus: hover fill (ΔE 2.9); VS Code keeps the rest fill in light.
5. List rows: no icon or twistie on our page, so the text sits at the padding (8 px), not VS Code's 38 px; the gate
   only checks left alignment. The crop margin (4 px) shows the neighbouring row, which reads as a bar on the edge.
6. Tabs: no dirty dot, no focused-group colours, close button always shown (VS Code: active or hovered tab only);
   the crops' 4 px margin shows the neighbouring tab's marker and the page's first line under the bar.
7. Progress: the reference capture has no progress crop (only `progressBar.background` from `vars`), so only the
   colour is checked; the 2 px height is not.
8. Select chevron: ours a filled triangle, VS Code's the `chevron-down` codicon (ICR1/DS5).
9. Menus, tooltip, toast, dialog: references captured, not probed; the dark popup and dialog shadows of DS1's data
   differ from the measured ones (§5).
