# POL4-DIR-WIDGETS: every skelter-widgets kind mirrors in `rtl`

Lane `dir-widgets` of batch polish-b4 (`plans/cloud/polish-b4/BRIEF.md`, `LANE-dir-widgets.md`), branch
`ci/pb4-dir-widgets` from `master` `9f0272b` (+ the batch briefs `8bcc50d`), with dir-core merged as it landed
(`origin/ci/pb4-dir-core` `8ddfa09`: `dir`, the inherited CSS `direction`, `:dir()`, `NodeView::direction`;
`c508a53`: layout, logical properties, the text base direction, the rtl scroll origin). The user decided on 2026-10-28: "lets add direction support"; the direction model is the brief's.

## Design note

Kinds read their resolved direction, never their text:

- **Where a kind reads it.** `StyleView::is_rtl()` (the computed `direction` keyword; `direction` joins the view's
  inherited properties, so a part without a `::part()` style reads its host's), `Styles::is_rtl()` and
  `StyleCx::is_rtl()` in the shim, and `NodeView::is_rtl()` (dir-core) for the kinds that implement skelter-core's
  `Kind` directly (`tab-view`, `table`, `text-edit`). A kind that needs its direction in events or in `child_layout`
  keeps it from its last `layout_style` (a `Cell`, as `room`/`row` already are) or `prepare`.
- **Shared paint.** P3 part rows (`paint::layout_parts`: the indicator kinds, button, toggle button) run from the right
  in `rtl`, as a flex row of nodes does in core. `paint::Align` is logical (`Start` is the right in `rtl`;
  `Align::of` maps physical `left`/`right` to the logical side; `Align::physical(rtl)`), and `paint_text` resolves it.
- **Mirroring a left-to-right model** (`crate::dir`). Kinds whose geometry is a model laid out left to right in one
  box (table, tab strip, calendar, date picker) keep that model and mirror it at the boundary: the pointer goes in
  with `x → w − x`, Left/Right swap, AccessKit bounds are mirrored, and the recording comes out mirrored —
  `record_mirrored` draws text through `MirrorText` (each string in its mirrored rect, start-aligned to the right,
  its glyphs in reading order) and mirrors every other primitive (`mirror_items`: rects, lines, clips, radii, borders,
  shadows). Shapes keep their orientation (a check mark, a digit, an icon move, never flip); chevrons that point along
  the inline axis are lines, so they flip with their place (a calendar's previous-month button at the right points
  right).
- **A right-to-left line laid out at a width** sits at that width's right edge (skelter-text aligns a paragraph to
  its own start), while `size().w` is the text's width. The painters placed it by its layout box, which pushed an
  elided Arabic cell past its box in rtl (the table drew no text). They now place a single line by its runs
  (`paint::ink_x`, the table's text painter, `common::paint_text_in`); `label` places its ink in `rtl`. Multi-line
  paragraphs keep their lines' alignment.
- **Text base direction.** The shim's `RichText::rtl` (`in_dir`) carries the box's direction to skelter-text
  (`RichText::direction`, dir-core's): `paint::text_layout`, `label` and the text field's layouter
  (`SystemLayouter::rtl`) set it in `rtl`, so `start` and mixed runs follow the box; an `ltr` box keeps its first
  strong character's, as before.
- **Scroll offsets stay left-based** (dir-core: an rtl container starts at its range). `Scroller::rtl` only moves the
  bars (the vertical bar and the corner to the left); the thumbs map as in `ltr`. A scroll area's JS write of one
  axis (`scroll-y`) keeps the other where core has it, so an rtl area still opens at its inline start.
- **Physical stays physical**: `panel side="left"`, `inner-window x`, `toast-host corner` are physical names; the
  toast stack is mirrored in `rtl` (its corner names the left-to-right corner; documented on the prop).

## What mirrors, per kind

| Kind | Mirrored | How (file) |
|---|---|---|
| box, stack, grid, sides, frame, window, modal | yes | core's flex/grid direction (dir-core); frame caption and modal heading via `Align` |
| label, hyperlink, rich text runs (`span`, `b`, `i`, `code`, `a`) | yes | `label.rs` places the ink at the right in `rtl` (Latin text too); runs follow the paragraph |
| button, toggle-button | yes | P3 part row (icon after the label's start) |
| checkbox, radio-button | yes | P3 part row: indicator at the right, caption to its left; the check's shape stays |
| switch | yes | P3 row; the on knob at the left (`switch.rs`) |
| collapsing-header | yes | chevron at the right, `<` when closed, caption to its left, body indent and end slot mirrored (`collapsing.rs`) |
| slider | yes | horizontal: value grows leftward, fill from the right, value box and caption at the left, Left steps up (`slider.rs`) |
| progress-bar | yes | horizontal fill from the right (`progress.rs`, one line: small's file) |
| spinner, image, icon, separator | not applicable | non-directional |
| scroll-area, virtual-list, tree-view, text-edit (multi-line) bars | yes | `Scroller::rtl`: vertical bar at the left; content starts at the right (core); a one-axis JS write keeps the other axis (`scroll.rs`) |
| scrollbar | not applicable | its value is left-based, as core's offsets |
| tree-view | yes | twistie column at the right, `<` collapsed, levels indent leftward, Left expands (`tree.rs`) |
| table, table-column | yes | columns from the right, sort arrow at a header's left, `align="end"` at the left, bars and cell editor mirrored, Left/Right and the horizontal wheel swapped (`table/kinds.rs`, `dir.rs`) |
| tab-view, tab | yes | captions from the right, close buttons at their left, scroll and overflow buttons at the left (swapped), the overflow menu under them, arrows swapped (`tabs/`) |
| menu-bar, menu, menu-item, menu-separator, context-menu | yes | titles from the right, a menu opens `bottom-end`, submenus to the left, check column at the right, shortcut and `<` chevron at the left, arrows swapped (`menu.rs`) |
| select (combo-box) | yes | drop chevron at the left, list aligned to the right edge, check column at the left (`select.rs`) |
| calendar | yes | weekdays from the right, week numbers at the right, previous month at the right, arrows swapped (`pickers/kinds.rs`) |
| date-picker | yes | drop arrow at the left, popup aligned right and mirrored (`pickers/`) |
| color-picker | yes | button mirrored, popup aligned right |
| color-editor | not applicable | the gradients keep their axes (as the platforms' pickers) |
| dialog-buttons | yes | core's grid from the right: the platform order mirrored, the group at the left |
| split-view | yes | first pane at the right (core), sashes, pointer and Left/Right mirrored (`split.rs`) |
| panel | not applicable | `side` is physical |
| status-bar | yes | start sections at the right, end sections at the left (`margin-right: auto`), message between (`status_bar.rs`) |
| toast-host | yes | stack at the mirrored corner, accent bar and text at the right, close at the left (`toast.rs`) |
| tooltip | yes | `-start`/`-end` placements logical (`tooltip.rs`) |
| inner-window | yes | title-bar buttons at the left, heading at the right (`inner_window.rs`) |
| text-edit | yes | a line starts (and an empty field's caret sits) at the right in `rtl`, the layout's base direction is the field's, AccessKit runs `RightToLeft` (`text/field.rs`, `geometry.rs`) |
| popup | open (small's file) | `views/popup.rs` places by `placement`; a logical `-start`/`-end` there is small's to add (request R1) |

## Stories

Every existing `rtl` variant sets `dir="rtl"` on its root (21 files). New variants: `context-menu/rtl-open`,
`combo-box/rtl-open`, `tab-view/rtl-overflow`, `slider/rtl`, `calendar/rtl`, `date-picker/rtl-open`,
`color-picker/rtl`, `split-view/rtl`, `status-bar/rtl`, `toast/rtl`, `scroll-area/rtl`, `table/rtl-sorted`,
`inner-window/rtl`, `dialog-buttons/rtl`, `tree-view/rtl-collapsed`. `skelter-gallery check`: 47 stories, 189
variants, 0 problems.

## Review

`sheet <story> --variants <rtl…> --states normal --zooms 1,2` over all four themes for every rtl variant (36
sheets), and the ltr sheets of the families whose shared paint changed (calendar, checkbox, combo box, context
menu, slider, status bar, tab view, table, toast) looked at. Found and fixed in the loop: the rtl table drew no text
(the elide-width placement above), a Latin label in an rtl box started at the left, the combo box's chevron.

Before/after (this container, not committed), under
`/tmp/claude-0/-home-user-svelters/e73aa1bf-7684-510a-9e44-74b84ed336e6/scratchpad/img/`:
- `cb-before.png` → `cb-after.png` (`cb-after.diff.png`): `checkbox/rtl` at zoom 2, the box from the left to the right.
- `table-nodir.png` → `table-rtl.png`: the rtl table, columns from the right, its text back.
- `rtl1.png` (first pass) → `rtl2.png`, `rtl3.png`: every rtl variant; `rev0.png`..`rev5.png`: the four themes.
- `ltr.png`: ltr variants of the changed families.

## Tests (filtered, on the last code commit's code)

- `cargo test -p skelter-widgets --lib -- rtl dir:: paint::parts tabs:: views:: controls:: table:: pickers:: text::`
  (the families touched): 294 pass, then `views::scroll controls::label` 38 pass after the last two fixes.
- New: `dir::tests` (2), `paint::parts::tests::an_rtl_row_runs_from_the_right`,
  `tabs::tests::rtl_mirrors_the_strip_its_buttons_and_the_arrows`,
  `views::tree::tests::rtl_puts_the_twisties_at_the_right_and_swaps_the_arrows`,
  `views::scroll::tests::rtl_bars_mirror_and_drag_from_the_right`.
- Not run locally: the workspace tests, pixel snapshot tests, conformance (CI is the gate).

## Requests and questions

- **R1 (small):** `views/popup.rs` placement `-start`/`-end` logical in `rtl` (as `tooltip.rs` now does). The one
  edit in small's files is `controls/progress.rs` (the `inverted` line in `record`, a direction check).
- **Q1, Q2 (dir-core): answered by `c508a53`** — skelter-text takes a base direction (passed now, above) and
  core's offsets stay left-based with an rtl container starting at its range (the bars adjusted, above).
- The table's own text painter (`table/text_painter.rs`) does not pass the direction (its cells are single lines,
  placed by their runs); mixed Latin/Arabic cells order by their first strong character.
- A context menu opened without a pointer point (the `open` prop, Shift+F10) opens at the owner's top-left in rtl
  too; with a point it extends leftward from it. Recommend the owner's top-right in rtl (needs the owner's width in
  `Menus`, small).

## CI

(filled in below)

## Handoff

(filled in below)
