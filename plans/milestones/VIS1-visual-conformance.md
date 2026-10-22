# VIS1: visual conformance against VS Code (the default widget look)

Track: DESIGN (`plans/research/DESIGN.md` §6). Request (user): "a UI visual conformance agent. It should take
screenshots and look at them, and develop the default widget look to be consistent with VS Code's, so that we have a
proper base to start from." Lane VIS1, branch `ci/vis1`.

## 1. Result

| | Checks passing (both themes) |
|---|---|
| Before (egui's stock look, `VIS1_STOCK=1`) | 65 of 286 (23%) |
| After (`svelters_egui::vscode_look`) | **263 of 286 (92%)**; the floor is 90% (DESIGN §6.5) |

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

- `crates/svelters-egui/src/vscode_look.rs`: the token tables `DARK`/`LIGHT` (DESIGN §1.5 roles, values from the
  captured `vars`), `egui_style(scheme)` (DESIGN §1.6: control height 26, button padding 8x4, radius 4, checkbox 18,
  fonts 13/12/11, no hover expansion, pressed = hover, menu radius 8, dialog radius 12, VS Code's widget shadows,
  pointing-hand cursor, disabled alpha 0.4, floating 10 px scrollbars) and `apply(backend, ctx)`.
- `crates/svelters-egui/src/vscode_look/base.css`: the per-kind sheet, `@layer sv.base`, theme origin: secondary and
  `.primary` buttons, fields (`text-edit`, `combo-box`, `drag-value`, focus border), checkbox/radio/switch, the input
  option toggle (`toggle-button`), list rows (`toggle-button[frame="false"]`).
- Applied by default in the eframe runners (`run_desktop`, `svl_backend.run`); an embedder calls
  `vscode_look::apply` (or `apply_css` with its own egui style). Tests opt in (TK1's snapshots are unchanged).
- Engine changes the look needed (`style.rs`, `leaf.rs`, runtime `style.js`):
  - `toggle-button`, `combo-box`, `radio-button`, `switch`, `drag-value` take per-state CSS visuals like the other
    stock widgets; the two-state kinds match `:checked`.
  - A `:checked` fill and colour also set `Visuals::selection` (egui paints selected buttons from it), and a focused
    `text-edit`'s border colour sets `selection.stroke` (egui's focused frame).
  - The look's sheets (`@tokens:sv-look:*`) survive the runtime's "clear every sheet" at bundle start.
  - The runtime sent scheme `light` to egui unless the app chose one, forcing light CSS in a dark app; it now sends
    `auto` for the engine target and egui follows its context's dark mode.
  - A frameless `toggle-button` is a selectable row (`frame_when_inactive`: frame on hover and when checked, body
    text); button text uses `TextStyle::Button` explicitly (a plain string fell back to Body).
  - The base sheet reads L3 tokens (`--sv-button-bg`, `--sv-checkbox-fg`, ...) instead of `--sv-accent`/`--sv-fg`,
    because the runtime's `DEFAULT_TOKENS` still set those 16 names on egui (DS1 stops that).

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
| Pane header | **deviation** | 26 tall, 12 px; VS Code 22 px, 11 px bold uppercase (needs per-kind font size, CS5f, and `text-transform`, DS2) |
| Tabs (`tab-view`) | **deviation** | tabs.rs paints selectable buttons (26 tall, selection fill); VS Code 35 px, `tab.*` colours and a top marker: needs tab tokens in tabs.rs (DS1 accessors) |
| Progress fill | **deviation** | egui fills with `selection.bg_fill` (the list selection); VS Code `progressBar.background`: needs a fill colour from CSS |
| Scrollbar, sash, switch, slider | review only | sheets; floating 10 px bars (VS Code lists), editor bars are 14 |
| Menus, tooltip, toast, dialog | references captured | `window_fill`/`window_stroke`/radii/shadows set from them; not yet probed (needs an open popup in the page) |

Focus rings beyond the input box (buttons, checkbox, select: VS Code's 1px `focusBorder` outline) need `outline`
(DS2); FX1 draws focus only after keyboard focus.

## 6. Coordination

- DS1 (`svelters_egui::theme`) supersedes `vscode_look`: its `Theme::vscode()` takes `DARK`/`LIGHT` and `base.css`,
  `egui_style` becomes `Theme::egui_style`, and the runtime stops sending `DEFAULT_TOKENS` on egui (then the base
  sheet can read `--sv-accent` and `--sv-fg` again). `ci/ds1` was not on origin when this lane finished.
- DESIGN.md §1.5 corrections from the capture are listed there under "VIS1 measured".

## 7. Verification (this lane, local)

- `nextest -p svelters-testkit --features snapshots -E 'binary(vis1)'` (lavapipe): both tests pass, 263/286.
- `nextest -p svelters-egui` filtered to `style`, `gallery`, `gallery_views`, `wg4b` and `vscode_look`: 24 pass.
- `node --test packages/native-runtime/test/style.test.mjs`: 4 pass.
- Not run locally: the rest of the workspace suites, the other backends, conformance. CI runs them.
