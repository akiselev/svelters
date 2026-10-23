# VIS1c: the visual gate for svelters' own widgets against VS Code

Lane `ci/vis1c` (WSL PC), from `origin/master` `d0542a3a` (OW0, OW1a, OW2, OW-CSS 1+2 merged). Design:
`plans/research/ARCH-WIDGETS.md` §3.7 and §6.2; references and capture from VIS1 (`ci/vis1` 88fe4f2a) and VIS1m
(`ci/vis1m` 300e9ce6); `plans/milestones/VIS1-visual-conformance.md` on `ci/vis1` for how the references were
captured. VIS1's mapping of VS Code tokens onto egui `Visuals` is not brought over: svelters paints every widget
itself, so the gate measures what the own widgets paint.

## 1. Result

Score (passing checks / checks) per reference element and theme, with `KindSet::All` and
`Theme::vscode(scheme, Normal).with_affordances(false)`, on master `e832e001` (OW1a2 and OW3 merged).
Windows references (what Linux CI checks):

| element | lane | dark-modern | light-modern |
|---|---|---|---|
| button-primary | OW0 | 25/30 | 27/30 |
| button-primary-dialog | OW0 | 10/13 | 11/13 |
| button-secondary | OW0 | 21/30 | 22/30 |
| checkbox (incl. radio, compared with the check box) | OW1a, OW1a2 | 21/27 | 21/27 |
| input | OW4 | 0/17 | 0/17 |
| pane-header | OW1a2 | 3/4 | 3/4 |
| progress | OW1a | 1/2 | 1/2 |
| sash | OW2 | 1/2 | 2/2 |
| scrollbar | OW2 | 2/4 | 1/4 |
| select | OW3 | 10/16 | 11/16 |
| setting-row (label) | OW1a | 2/2 | 2/2 |
| tab | OW5 | 0/12 | 0/12 |
| toggle | OW1a | 8/18 | 8/18 |
| **all** | | **104/177** | **109/177** |

macOS references (what the macOS job checks): the same per element except light-modern button-primary 26/30
(there VS Code's light primary button shows its hover fill on focus; on Windows it keeps the rest fill, as ours
does) and light scrollbar 2/4 (the Windows capture missed the light thumb's hover colour); 213/354 overall in both.

Of the 141 failing checks (Windows refs), 58 are kinds that still draw through their egui wrapper and trace
nothing (`text-edit`, `tab-view`): they pass when OW4 and OW5 make them own. The other 83 are the differences in
§4. Before the merge of OW1a2 and OW3 (master `d0542a3a`) the score was 176/354.

## 2. What is here

| Path | What |
|---|---|
| `conformance/visual/vscode/{dark,light}-modern/` | Windows references (VS Code 1.133.0): `metrics.json` (`items`: computed sizes, padding, border, radius, colours, font, outline, opacity; `vars`: every resolved `--vscode-*`) and one 1x crop per element and state |
| `conformance/visual/vscode-macos/{dark,light}-modern/` | macOS references: `metrics.json` (the 2x capture; CSS px) and the 1x crops. The `@2x` crops and `metrics@1x.json` stay on `ci/vis1m` (identical style metrics) |
| `conformance/visual/capture/` | VIS1/VIS1m capture scripts (re-capture only, never run in CI): `setup.sh`/`winnode` (Windows over WSL), `setup-macos.sh`/`macnode`, `capture.js`, `cdp.js`, `settings.json` |
| `conformance/visual/tools/` | `sheet.py` (contact sheet: VS Code crop next to ours, failed checks per row), `montage.py`, `summary.py` |
| `conformance/visual/baseline-{windows,macos}.json` | the ratchet: check ids that pass |
| `crates/svelters-testkit/tests/vis1.rs` | the gate (`own_widgets_match_vscode`), ΔE2000 and outline-parser unit tests |
| `fixtures/gallery-egui/src/pages/VisControls.svelte` | the page (embed export): every kind with a reference, named `<element>:<variant>`; button captions are the references' captions |
| `crates/svelters-egui/src/paint/{trace,mod,boxes}.rs` | PaintTrace entries carry `radius`, `padding` (box) and the painter's `opacity` |
| `crates/svelters-egui/src/theme/base.css` | `button.primary` (VS Code's primary button: `accent`, `accent-hover`, `button-border`, `accent-fg`) |
| `.github/workflows/linux-gtk.yml`, `macos-appkit.yml` | the CI steps (§3.4) |

## 3. The gate

### 3.1 Rendering and finding parts

The test mounts `VisControls` in an embed slot (kittest, no GPU), once per theme, with `set_own_widgets(KindSet::All)`
and the exact VS Code theme (affordance overlay off, as VIS1: VS Code's own colours; DESIGN's affordances are
compared in §4 as deviations, not checked separately). `paint::trace::enable` turns PaintTrace on (debug builds;
a release test build skips the gate with a message).

Per probe: the node is found by its `accessible-name` prop in the scene; a node that is not `W::Ext` is an egui
wrapper and all its checks fail as `egui wrapper (<lane>)`. States: rest (`PointerGone`), hover (pointer over the
node, or over the part when the probe targets a part such as the scroll thumb, which an overlay bar shows only
after the pointer enters its area), pressed (pointer down; released outside afterwards), focus (AccessKit focus:
a program focus, so `:focus-visible` and the ring). 30 frames (0.5 s) settle hover eases and R11's 300 ms sash
delay. Then from the latest pass's trace:
- the box: the node's own `Box` entry, or for a part probe the largest `Box`/`Path` entry of that part (of the
  node, or anywhere inside the node's rect when a child node paints it);
- the text: the node's first `Text` (part `label` first); `fg` from it, or from a named part (`mark`, `dot`);
- the outline: the node's first `Outline` entry (the focus ring).

### 3.2 Checks and tolerances

All in CSS px against the reference item's `metrics` (or a `--vscode-*` variable / a constant where the capture
lacks the item: progress colour and its 2 px height, the sash hover colour):

| Check | Compares | Tolerance |
|---|---|---|
| `h`, `w`, `t` | height, width, thickness (smaller side) of the box | 1 px |
| `wt`, `tw` | a text-sized width, the text's own width (ref: `text_rect`, else box less padding and border) | 15% of the reference text width + 1 px: Segoe UI (Windows), SF (macOS) and svelters' bundled font differ |
| `r`, `b`, `p` | radius (top-left), border width, padding `[t, r, b, l]` | 0.5 px |
| `ow`, `oo` | outline width, outline offset (ring rect vs box) | 0.5 px |
| `bg`, `bc`, `fg`, `oc` | fill, border, text (or mark), outline colour | CIEDE2000 ΔE ≤ 2, both composited over the theme's `--sv-bg` |
| `op` | opacity (ours: the painter's opacity) | 0.05 |
| `fs` | font size | 0.01 |
| `ta` | text alignment in the box (left/centre/right) | exact |

Pixels are never compared. With `--features snapshots` each probe is rendered (wgpu) and cropped to
`target/vis1c/<refs>/<theme>/<node>-<state>.png` for review.

### 3.3 Ratchet and output

`conformance/visual/baseline-<refs>.json` lists the check ids that passed when recorded; the test fails when one of
them fails now, never on the absolute score. New passes are printed. Outputs in `target/vis1c/<refs>/`:
`score.md` (the table above; also appended to `$GITHUB_STEP_SUMMARY`), `report.md` (every check, failures first,
ours vs VS Code, lane), `<theme>/metrics.json` (our measurements in the references' schema).

```sh
pnpm --filter ./fixtures/gallery-egui build                       # needs target/debug/svelters-compile
tools/dev/cargo-slot cargo nextest run -p svelters-testkit -E 'binary(vis1)'
VIS_REFS=macos tools/dev/cargo-slot cargo nextest run -p svelters-testkit -E 'binary(vis1)'   # macOS refs on Linux
SVELTERS_UPDATE=1 ...                                             # record new passes (do it for both refs)
python3 conformance/visual/tools/sheet.py dark-modern target/vis1c/windows/dark-modern /tmp/dark.png windows
```

Our rendering does not depend on the OS (bundled fonts, kittest), so both baselines can be recorded on Linux
(`VIS_REFS=macos`); the macOS job checks the macOS one.

### 3.4 CI

- linux-gtk, main job: `svelters-testkit` is in the nextest list, so `vis1` runs against the Windows references
  (`pnpm build` builds the gallery). `target/vis1c/` is uploaded with `linux-gtk-artifacts`.
- linux-gtk, `egui-snapshots`: builds the gallery and runs `binary(vis1)` with `--features snapshots` (lavapipe):
  the review crops, uploaded with `egui-snapshots`.
- macos-appkit: a step `cargo test -p svelters-testkit --test vis1` against the macOS references; `target/vis1c/`
  in `macos-appkit-artifacts`.
- windows-winui: not added. That job builds no egui crate today, so the step would add the whole egui and kittest
  build to it, and the Windows references already run on Linux with the same (OS-independent) rendering.
  Recommendation to the coordinator: keep it off unless a Windows-only paint path appears.

### 3.5 How a lane moves its kind's checks to own

The probe table is `probes()` in `tests/vis1.rs` (node, state, part, reference item, checks, lane). When a lane
swaps a kind: its probes start measuring (no table change when the node and part names below hold), the lane looks
at `report.md`, fixes what it owns, and records the new passes with `SVELTERS_UPDATE=1` for both refs. Contracts
the probes assume:
- OW1a2 (merged, measured): `radio-button` paints its circle as `::part(indicator)` and the dot as `::part(dot)`;
  `collapsing-header` paints its header row as `::part(header)` and its caption as `::part(label)`.
- OW3 (merged, measured): `combo-box` paints its field as the node's box, the shown option as `::part(label)`,
  the ring as the node's outline.
- OW4: `text-edit` paints its field as the node's box and its text as a `Text`; `invalid` changes the border
  colour.
- OW5: each `<tab>` node (`accessible-name` set on it) paints its own box and title (or OW5 changes the probe to a
  `tab` part of the `tab-view`). List rows have no probe yet: OW5 adds a list to `VisControls` and probes for
  `list-row` rest/hover/selected-focused/focused/selected-inactive (references captured).
- OW3/OW7: menus, tooltip, toast and dialog have references but no probe (they need an open popup on the page);
  the lane that makes them own adds the probes.
- Kinds VS Code lacks (switch, slider, spinner, hyperlink, separator) have no VIS1c check; they are on the page
  for the crops. DESIGN-rule checks (Q4: ≥ 3:1 boundaries, lip, sunken fields) are not part of this gate yet.

## 4. Where the own widgets differ from VS Code (by owning lane)

"Intentional" marks a DESIGN rule the user chose over VS Code's look (affordances on by default); those stay as
failing checks unless the coordinator decides to drop them from the probes.

### OW0 / OW-CSS (button, the UA and theme mechanism)

1. **Disabled opacity is applied twice: 0.16 instead of 0.4.** egui's `disabled_alpha` (0.4, `theme/egui_style.rs`)
   fades the disabled ui and the button's UA `opacity: .4` multiplies on top (`widgets/button.rs`
   `ui.multiply_opacity`). The checkbox (no UA opacity) is at 0.4. Own nodes should set `disabled_alpha = 1` on
   their ui (as `walk.rs` does for inert nodes) and own their fade. Both themes, primary and secondary.
2. **UA tokens ignore the installed theme.** `UaValue::Token` resolves against static built-in themes
   (`paint/mod.rs` `ua_theme`: `Theme::dark_modern()`/`light_modern()`, affordances on), not the theme the backend
   installed. With the exact theme installed, every UA `focus` is the affordance overlay's `#2488DB` instead of
   `#0078D4` (dark `oc` fails on button, checkbox, toggle; the sash hover bar too). App token overrides, High
   Contrast and `with_affordances(false)` never reach UA values. Only base.css rules (`var(--sv-*)`) follow the
   installed theme.
3. **Secondary button border**: ours `control-border` (`#3C3C3C` dark, `#CECECE` light) vs VS Code
   `button.secondaryBorder` (`#CCCCCC26`, `#3B3B3B26`), ΔE 2.9/3.2. VIS1b added a `button2-border` role for it,
   with the affordance overlay keeping `control-border` (R4's 3:1); not on master. Theme data, not base.css.
4. **Focus ring 2 px at offset +1** vs VS Code 1 px at +2 (primary) / +1 (secondary). Intentional (DESIGN R8: 2 px
   keyboard-only ring, +1 for standalone controls).
5. **Focus fill**: VS Code shows the hover fill on keyboard focus (dark: both buttons; light: secondary and the
   dialog's primary); ours keeps the rest fill. Low priority.
6. Pressed: matches (VS Code has no pressed look; `button.primary` keeps the hover fill while pressed; the R2 inset
   shadow is not measured).

### OW1a / OW1a2 (checkbox, toggle-button, progress-bar, label; radio-button, collapsing-header)

1. **Checkbox radius 4, VS Code 3.** The base sheet's `checkbox { border-radius: var(--sv-button-radius) }` reaches
   the indicator (the part view inherits the host's box) and beats the UA's 3 px. Not changed here: the same rule
   styles the egui wrapper (TK1 snapshots); fix it with the part's own CSS once `::part(indicator)` rules are read
   (the OW1a part UA tables are still hard-coded, `TODO(OW-CSS ::part)`), or add a `checkbox-radius` token.
2. **Checkbox ring at offset +1, VS Code −1** (inside the box). Intentional (R8, standalone control).
3. **Checkbox hover fill** (light): ours `button-bg-hover` `#CCCCCC`, VS Code keeps `#F8F8F8` (ΔE 9.7; dark passes
   at ΔE 1.9). Intentional (R3 hover feedback), but the light step is large.
4. **Toggle (input option) at rest**: VS Code's Match Case toggle is a flat 20×20 icon toggle (transparent fill
   and border, radius 3); ours is a raised 25 px button (`button-bg`, border, radius 4). Intentional (R1: toggle
   buttons are raised); the flat look belongs to an icon toggle inside a field (OW4's field actions) rather than
   to `toggle-button`.
5. **Toggle checked text colour**: ours `fg` (`#CCCCCC`/`#3B3B3B`), VS Code `inputOption.activeForeground`
   (`#FFFFFF`/`#000000`). Needs a `toggle-on-fg` role (VIS1b had it); the UA's `:checked` colour then reads it.
6. **Toggle radius 4 vs 3** (VS Code's input option radius); hover fill light ΔE 5.6 (`button-bg-hover` vs
   `inputOption.hoverBackground`). Follows from 4.
7. **Progress bar 4 px tall, VS Code 2 px** (`.monaco-progress-container`; VIS1 gap 7). Colour matches.
8. Label: matches (13 px, foreground).
9. **Radio border**: the UA's `control-border` from the built-in theme (`#6B6B6B` dark, `#8A8A8A` light: the
   affordance overlay's 3:1 boundary) vs the check box's `#3C3C3C`/`#CECECE`, ΔE 16/20. The UA token issue
   (OW-CSS 2); the check box gets its border from the base sheet and matches.
10. **Radio checked**: `toggle-on-bg`/`toggle-on-border` on-fill vs the check box's unchanged fill (VS Code has no
   radio). Intentional (R17: the on state is filled). The dot colour matches the check mark's.
11. **Pane header font 13 px, VS Code 11 px** (bold, uppercase; VIS1 gap 2). Height 22, fill and colour match.

### OW2 (scroll-area, split-view)

1. **Overlay thumb hidden at rest**; VS Code's editor bar reference shows it (`rgba(121,121,121,.4)` dark). Ours
   follows VS Code's list behaviour (shown while hovered, scrolled or dragged). Thumb thickness is measured only
   at rest, so `t` fails; recommend measuring it in hover (both are VS Code behaviours; 14 px editor vs 10 px list).
2. **Thumb colours from the built-in theme** (`ua_only(THUMB_UA, .., dark)`): same issue as OW-CSS 2. Light
   active thumb `rgba(100,100,100,.4)` vs VS Code `rgba(0,0,0,.6)` (`scrollbarSlider.activeBackground`), ΔE 30.
3. **Sash hover bar `#2488DB` dark vs `sash.hoverBorder` `#0078D4`**: the UA `focus` token (OW-CSS 2). Width 4 px
   matches.

### OW3 (select)

1. **Border from the built-in theme**: `#6B6B6B`/`#8A8A8A` (UA `control-border`, affordance overlay) vs VS Code
   `#3C3C3C`/`#CECECE` (`dropdown.border`), ΔE 16/20, on rest, hover and disabled. OW-CSS 2, or a base-sheet
   `combo-box` rule like the check box's.
2. **Vertical padding 3 px, VS Code 2 px** (`.monaco-select-box` padding 2/6). Height 26 matches.
3. **Disabled at opacity 0.16** (VS Code's disabled select does not dim: opacity 1, same colours): the double
   fade of OW0 1, plus R9's dimming.
4. **Focus ring `#2488DB` dark**: the UA `focus` token (OW-CSS 2). Width 1 and offset −1 match.
5. Menus, tooltip and the open drop-down have references but no probe yet (§3.5).

### OW4, OW5 (not own yet)

`input` (OW4, 17 checks per theme) and `tab` (OW5, 12) fail as egui wrappers. VIS1's notes for them still hold:
disabled input must not dim (VS Code's settings box keeps full opacity, gap 3); tabs need the dirty dot,
focused-group colours and close-on-hover (gap 6).

## 5. Kinds whose egui wrapper can go

ARCH-WIDGETS: wrappers (`SVELTERS_OWN_WIDGETS=none`) stay until VIS1c. A kind qualifies when every one of its
probes is measured from the own widget and none of its gaps needs the wrapper (each is a UA, token or base-sheet
fix in the own widget, listed above):

| Kind | VIS1c | Wrapper |
|---|---|---|
| `button` | measured on every probe; gaps OW0 1–5 are own-widget fixes | **can go** |
| `checkbox` | measured on every probe; gaps OW1a 1–3 | **can go** |
| `radio-button` | measured on every probe; gaps OW1a 9–10 (10 intentional) | **can go** |
| `toggle-button` | measured on every probe; gaps OW1a 4–6 (4 intentional) | **can go** |
| `progress-bar` | measured; height (OW1a 7) | **can go** |
| `label` | passes | **can go** |
| `collapsing-header` | measured; font size (OW1a 11) | **can go** |
| `combo-box` (select) | measured on every probe; gaps OW3 1–4 | **can go** |
| `switch`, `hyperlink`, `separator`, `spinner`, `frame`, `icon`, `image` | no VS Code reference, no VIS1c check | VIS1c does not gate them; their lane's acceptance decides |
| menus, `popup`, tooltips (OW3) | references captured, no probe yet (needs an open popup on the page) | not gated by VIS1c yet; the coordinator decides whether OW3's own acceptance suffices |
| `scroll-area`, `scrollbar`, `split-view`, `panel` | measured | already deleted (OW2) |
| `slider`, `drag-value` (OW1b), `text-edit` (OW4), `tab-view` (OW5), OW6/OW7 kinds | not own yet | stays until its lane's probes pass |

Deleting wrappers is not done in this lane (coordinator's instruction). The OW1a2 and OW3 milestones expected
VIS1c to delete them; that is now a separate step for the coordinator to assign.

## 6. Verification

Local (WSL PC), after the last code edit:
- `nextest -p svelters-testkit -E 'binary(vis1)'`: 3 pass; baselines recorded with `SVELTERS_UPDATE=1` for
  `windows` and `VIS_REFS=macos` (213/354 each, after the merge of master `e832e001`);
- `cargo check -p svelters-egui -p svelters-testkit --all-targets`, and `-p svelters-testkit --features snapshots
  --tests` (the crop path compiles; not run: no local wgpu render);
- `nextest -p svelters-egui` filtered to the PaintTrace and theme users and the gallery: `binary(own_widgets) |
  binary(ow_css) | binary(ow1a) | binary(ow1a2) | binary(ow2) | binary(ow3) | binary(theme) | binary(gallery) |
  binary(gallery_views)` (69 pass, with the new page in the gallery), after the merge.

Not run locally: the crops (`--features snapshots`), the full workspace suites, other backends, conformance. CI
runs them.

## 7. Follow-ups (not in this lane)

- The fixes in §4, by lane; the coordinator routes them.
- OW5: list-row probes and a list on the page; OW3/OW7: popup probes.
- An affordance-on run (the default theme) reporting DESIGN-rule checks for kinds VS Code lacks (Q4).
- Windows CI step (§3.4), if wanted.
