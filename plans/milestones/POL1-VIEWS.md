# POL1-VIEWS: polishing skelter's view kinds (batch polish-b1, lane views)

**Branch:** `ci/pb1-views` (base `master` `9d97e7d`). **Brief:** `plans/cloud/polish-b1/BRIEF.md`, `LANE-views.md`.
**Status:** done. Every finding in scope is fixed and seen fixed, or recorded below with a reason and a
recommendation. CI: see "Tests and CI".

## Scope

The views rows of GAL2's "Polish findings" (scroll area, virtual list and table bars, table elide and selection and
cursor, tree `root-decorated` and RTL twistie, combo popup highlight), plus what the family's sheets showed in the four
themes against the VS Code look (DESIGN.md §6). Files: `skelter-widgets/src/{views,table}/`, plus one small hook in
`shim.rs` and `host.rs` (below). No `base.css` rule changed.

## Findings

Each was rendered before and after (`skelter-gallery sheet <story> --states normal`, all four themes; zoom 2 for
tree-view, scroll-area and combo-box/open) and looked at.

| # | Finding (story) | Cause | Fix (file) | Seen fixed | Snapshots moved |
|---|---|---|---|---|---|
| V1 | `scroll-area/scrolled`: `scroll-y={200}` moves the thumb, the content stays at Row 1 | A JS write of `scroll-x`/`scroll-y` set the kind's own `Scroller` but never reached core's scroll node, which translates the content | The kind flags a JS offset; after its prepare the host hands it to core (`PrepareCx::scroll_to`, clamped to the content just laid out). The two props are now `Paint` (a write re-records the bars). New shim hook `Kind::take_scroll_request` (`shim.rs`, `host.rs`), used by `views/scroll.rs` | yes: Row 12 at the top, thumb in the middle, all themes, x1 and x2 | none known (CI) |
| V2 | `tree-view/expanded decorated=false` still draws level-0 twisties | skelter's `tree-view` kind had no `root-decorated` prop (the Svelte surface declares it; the backend kept it as a side value) | `root-decorated` added to `TreeProps` (`Layout`; no new API: the surface already declares it). Off: level 0 has no disclosure column, every level moves one indent left (Qt `rootIsDecorated`); keys, AccessKit and double click still expand. Unit test `root_decorated_off_drops_the_level_0_twisties_and_their_column` (`views/tree.rs`) | yes | none |
| V3 | (found) hc-dark: a selected list/tree row is invisible; hc-light only a faint tint | VS Code's HC themes have no selection fill; they outline the row in `contrastActiveBorder` (`listSelectionOutline`) | `ROW_UA` `:selected` draws a 1 pt dotted outline in `--vscode-contrastActiveBorder` (absent, so nothing, in the Modern themes); the cursor rule now names its colour, so a focused selected row keeps `list-focus-outline` (`views/virtual_list.rs`) | yes: dotted orange (hc-dark), dotted blue (hc-light), Modern unchanged | none known |
| V4 | `table/sorted-selected`: selection invisible in hc-dark | as V3 | `TableLook::selected_outline` from `--vscode-contrastActiveBorder`, a 1 pt frame on selected rows (`table/mod.rs`, `table/kinds.rs`) | yes | none known |
| V5 | `table/overflow`: `elide="middle"` elides at the end | the table's `ELIDES` held only `end`/`none`, so `middle` was refused and the column kept `end` | `ELIDES` gains `middle`, `start` (appended: indices kept); `draw_elided` cuts them with `paint::elide_with` over the painter's measure (`table/column.rs`, `table/mod.rs`) | yes: "Microcontrol…ion number 1" | none |
| V6 | `table/overflow`: the fourth column is hidden with no horizontal bar | The bar existed but was outside the cell: the table measured header + ten rows (224 pt) and pushed its parent past the cell, so the bottom row and the bar were cut | The table's `layout_style` clips (`overflow: hidden` unless CSS sets overflow: a scroll container's automatic minimum is zero) and, when it grows, takes a zero flex basis, as lists do (`scrolling_layout`) (`table/kinds.rs`) | yes: the table fits, the horizontal thumb shows | none known |
| V7 | `combo-box/open` light-modern: dark text on the blue highlight | The option's CSS colour, inherited from the root's `fg`, beat the option's UA `:selected` colour (`menu-sel-fg`): `StyleView` reads CSS before the UA table | `views/common.rs::part_fg`: a row's colour is its UA colour when its CSS colour is only inherited from the parent part; `paint_text_in` paints in it. Used by select options and menu items (label, check, chevron, mnemonic underline) | yes: white on blue in light-modern; dark-modern unchanged | none known |
| V8 | (found) `combo-box/open` hc-dark/hc-light: the highlighted option is barely visible | HC has no `menu-sel` fill; VS Code outlines it (`menu.selectionBorder` = `contrastActiveBorder`) | `OPTION_UA`/`ITEM_UA` `:selected` add a 1 pt solid outline in `--vscode-contrastActiveBorder`, painted with `paint_outline` (`views/select.rs`, `views/menu.rs`) | yes | none known |
| V9 | `scroll-area/direction`: bars overlay the text with no track or gutter | By design: VS Code's scrollbars are overlay sliders with no track (DESIGN §1.3 "overlay, fades out", §6 `scrollbarSlider.*`); classic bars (`Policy::scrollbars == Classic`) do draw tracks | none | looked at, x1 and x2 | — |
| V10 | `virtual-list/plain` (and every tree): no scroll bar | Lists and trees scroll themselves (`self_scroll`); their only fixed layer paints *below* the moving content, so there is nowhere to draw a bar that stays put. No bar code exists | not built (needs core): see Q1 | — | — |
| V11 | `tree-view/rtl`: the twistie stays left | Nothing in skelter has RTL layout (`direction` is not a CSS property here; svelters-style's `rtl` utility is "unsupported: no right-to-left layout"); the rows only look right-aligned because the label's bidi text aligns right | not built (architecture): see Q2 | — | — |
| V12 | (found) `scroll-area/scrolled`: no top edge shadow (R15) | base.css's `scroll-area:state(sv.overflow-top)` rules were egui's; on skelter no state names `sv.overflow-*` are registered and no kind sets them | not built: see Q3 | — | — |
| V13 | cursor row of an unfocused table (`current={2}`) is not shown | By design: VS Code shows the focus outline only while the list has focus; the table draws its cursor-cell ring while focused from the keyboard | none | looked at | — |

Tooltips, toasts, split, panel, status bar, menu bar and tabs: no story can show them (the kinds are not registered for
Svelte, `skelter-gallery list` lists none of them; the tooltip story's forced `:hover` does not start the hover timer),
and the context menu is not open in its story (GAL2 request 2). The menu-item fixes (V7, V8) share the select's code
path and are covered by its cells, not seen on a menu. See Q4.

## Before and after images (in the lane's container, not committed)

- `target/gallery/before/tree-view.png` → `target/gallery/after/tree-view.png`: `decorated=false` without level-0
  twisties; hc themes' selection outline. Zoom 2: `target/gallery/after/tree-x2.png`.
- `target/gallery/before/table.png` → `target/gallery/after/table.png`: middle elide, the overflow table fits with its
  horizontal bar, HC selection outlines.
- `target/gallery/before/combo-box.png` → `target/gallery/after/combo-open.png` (x2): light-modern white highlight
  text, HC outlines.
- `target/gallery/before/scroll-area.png` (taken after V1 built: the binary already had it) and
  `target/gallery/after/scroll-x2.png`: Row 12 at the top.

## Questions (recommendations; for the coordinator)

- **Q1 (skelter-core): bars for self-scrolling kinds.** Recommend a fixed *foreground* part for `self_scroll` kinds
  (`RecordCx::fixed_over`, painted above the content layer, outside the scroll transform), then virtual-list and tree
  draw a `Scroller`'s bars into it as table does (bar drag and track press through the same `Scroller::pointer`).
  Alternative: core draws overlay bars for every scroll node from tokens. Either is a core change, so not here.
- **Q2 (architecture, user): RTL layout.** Mirroring a tree's twistie, a table's columns and a menu's check column needs
  a `direction` property in svelters-style and taffy row-reverse placement in core. Recommend deferring until cadatomic
  needs RTL UI; the gallery's Arabic samples only test text.
- **Q3 (states lane or skelter-core): `:state(sv.overflow-*)`.** Register the four `sv.overflow-{top,bottom,left,right}`
  state names on skelter documents and have `scroll-area`, `table`, `virtual-list` and `tree-view` set them from their
  `Scroller` (`away_from_start`/`away_from_end`); base.css's rules then apply unchanged.
- **Q4 (svelters-skelter, gallery):** register the view kinds that have no Svelte name yet (tooltip, toast, split, panel,
  status-bar, menu-bar, tab-view) and give the gallery a way to open a context menu and show a tooltip (a story prop or a
  timer step); then a later polish pass can look at them.
- **Request to states (root cause of V7):** `paint::StyleView::get` lets an *inherited* CSS value beat the part's own UA
  declaration; in CSS the UA rule wins. Recommend `get` prefer the UA value for inherited properties when the part's CSS
  value equals its parent's (or when svelters-style can say the value was inherited). `views::common::part_fg` then
  becomes unnecessary.

## Tests and CI

- `cargo test -p skelter-widgets --lib -- views:: table::` (on the code of `37aa82e`): 171 passed, 0 failed (one new
  test, V2).
- `skelter-gallery check` (debug build, after the fixes): ok, 31 stories, 135 variants, 0 problems, no warnings.
- `cargo clippy -p skelter-widgets --all-targets`: nothing new in the lane's files (the skelter workflow's clippy is
  advisory).
- Not run locally: the workspace tests, other crates' tests, pixel snapshot tests, conformance (CI is the gate).

`origin/ci/pb1-gallery` `cd478f3` (fonts, icons) merged at the coordinator's request; the sheets were rendered
again after the merge (`target/gallery/after2/{tree-view,table,virtual-list,combo-box,scroll-area}.png`): every fix
holds with Inter and real Arabic; the RTL rows now show Arabic and the twistie still sits left (V11).

CI on `c2f4f90` (the merge `76ccf4c` plus this record; the last code sha), all six workflows green: js 40586311442,
linux-gtk 40586311749, linux-qt 40586311464, macos-appkit 40586311642, skelter 40586311593, windows-winui
40586311443. No pixel snapshot test moved, so none was re-blessed. Later edits to this record are docs only (CI not
rerun for them).

## Handoff

Nothing open in the lane beyond Q1–Q4 and the request to states. If a snapshot test moves in CI, it is from V3/V4/V7/V8
(row outlines and option text colour) or V6 (table height); re-bless only after looking at its diff.
