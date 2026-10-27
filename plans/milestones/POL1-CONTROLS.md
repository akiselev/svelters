# POL1-CONTROLS: polishing the control kinds (batch polish-b1, lane controls)

Branch `ci/pb1-controls`, base `master` `f77fad0`, with `origin/ci/pb1-gallery` `cd478f3` merged (its bundled
fonts, demo icons and `asset://` SVGs, so icon and image could be looked at). Brief: `plans/cloud/polish-b1/BRIEF.md`
and `LANE-controls.md`.

## Scope

GAL2's "Polish findings" rows for progress bar, slider marks, frame, collapsing-header end slot and the vertical
separator; then a look at every sheet of the family (progress-bar, slider, frame, collapsing-header, separator,
icon, image, spinner, hyperlink) in the four themes at zoom 1 and 2. Files: `skelter-widgets/src/controls/`
(`progress`, `slider`, `slider/rail`, `separator`, `frame`, `collapsing`, `image`, `icon`) and this lane's rule
blocks in `theme/base.css` (one block, marked `POL1-CONTROLS`). Pickers: not lookable yet (below).

## Findings and fixes

Every row was rendered before (a `master` build) and after, and both images were looked at.

| # | Finding | Cause | Fix (file) | Seen fixed |
|---|---|---|---|---|
| 1 | progress-bar/text: text wider than the bar overflows both ends, unclipped (GAL2) | `measure` widened the bar to the text; the label was drawn unclipped | The bar keeps its width; the label is elided (`…`) to the bar less 4px each end and clipped to it (`controls/progress.rs`, `paint_label`) | yes, 4 themes, x1 and x2 |
| 2 | progress-bar/percentage, text: text over the fill is dark on accent blue in light-modern (GAL2) | one text colour for the whole label | The label is drawn twice, clipped: off the fill in the host colour, over the fill in the `fill` part's `color`. base.css `progress-bar::part(fill) { color: accent-fg }`, and under `prefers-contrast: more` the window `bg` (black over hc-dark's cyan fill, white over hc-light's dark blue) | yes, 4 themes |
| 3 | progress-bar/fraction=0: the trough is invisible in light-modern (GAL2); also in both HC themes | trough was `control-bg` only: near-white on near-white, black on black | `track` part has the field look: `control-bg` inside a 1px `control-border` (`controls/progress.rs` `TRACK`) | yes, 4 themes |
| 4 | progress-bar/indeterminate: nothing but the trough (GAL2) | the recording is the sweep's frame at t=0, where the segment is off the track, and nothing in skelter runs the sweep | Records the sweep's middle frame (`BUSY_STILL`: a centred 30 % segment), the reduced-motion look. Running the sweep is not built (request 1) | yes, 4 themes |
| 5 | slider/marks: no marks or labels (GAL2) | marks drew only when `ticks` was set; `ticks` defaults to `none`, though the prop doc says marks draw | `ticks: none` with marks means `after` (`controls/slider.rs` `ticks()`) | yes, 4 themes, x1 and x2 |
| 6 | slider (own look): mark labels touch their ticks at zoom 2 | label placed at the tick's end | 2px `LABEL_GAP`, counted in the rail's extent (`slider/rail.rs`) | yes, x1 and x2 |
| 7 | slider (own look): the filled rail is invisible in hc-dark | fill used `accent`, black in High Contrast Dark | fill uses `progress` (equal to `accent` in the other three themes) (`slider/rail.rs` `FILL_UA`) | yes, 4 themes |
| 8 | slider (own look): the thumb's core is `fg` grey, not the accent its UA asks for | dead UA colour (shared cause A) | base.css `slider::part(thumb) { color: progress }`, `slider:disabled::part(thumb) { color: disabled-fg }`, `slider::part(mark-label) { color: muted }` | yes, normal and disabled |
| 9 | button/dense, icon/toolbar, separator/vertical: a vertical separator draws nothing (GAL2) | it measures no length; in a row with `align-items: center` its height was 0 | `layout_style` stretches it on the cross axis unless CSS sets `align-self` (`controls/separator.rs`) | yes |
| 10 | frame/style=canvas: smaller inner margin, the content almost touches the border (GAL2) | `CANVAS` had no padding | the group's 6px padding (`controls/frame.rs`) | yes, 4 themes |
| 11 | frame/checkable checked=false: content looks enabled (GAL2) | the frame is right: `inspect` shows both text-edits `state.own: [disabled]` and AccessKit `disabled: true`. They look enabled because text-edit has no disabled look | none here: the states lane's disabled finding (GAL2 row 2) fixes it | recorded, not fixed here |
| 12 | collapsing-header/end-slot: the button sits above the header's centre line (GAL2) | boxes are `position: static` unless CSS says otherwise, so the absolute end-slot box escaped to the outer box (it sat at y=0 of the cell, 4px past the header's right edge) | the header is `position: relative` unless CSS sets `position`; an end-slot child fills the header row's height (cached from `chrome_padding`, which runs before the children are placed), or is centred when its height is set (`controls/collapsing.rs`) | yes, 4 themes, open and hover |
| 13 | hyperlink (own look): links draw in `fg`, not `link`, in every theme | dead UA colour (shared cause A) | base.css `hyperlink { color: link }`, `hyperlink:disabled { color: disabled-fg }` | yes, normal and disabled |
| 14 | spinner (own look): the arc is `fg`, not `progress` | dead UA colour (A) | base.css `spinner { color: progress }` | yes |
| 15 | image/missing, icon/source (own look): the not-loaded placeholder is a loud `fg` frame; with its UA token (`border-widget`) it is nearly invisible on Dark and Light Modern | dead UA colour (A); `border-widget` too faint | `control-border` (the 3:1 boundary) in `PLACEHOLDER_UA` and base.css `image { color: control-border }` | yes, 4 themes |
| 16 | frame caption (own look): `fg`, not the `fg-strong` header colour its UA asks for | dead UA colour (A) | base.css `frame::part(caption) { color: fg-strong }` | yes |

**Shared cause A (dead UA colours).** `StyleView::get` reads the node's computed style before its UA table, and a
computed style always carries `color` (inherited from the root), so a UA table's `color` never reaches the paint in a
real document (unit tests, which have no computed style, still see it). Non-inherited properties are not affected. I
restated the colours of my kinds in base.css, the pattern `button` already uses. Other lanes' kinds with a UA
`color` have the same bug (request 2).

Looked at and fine: icon (glyph sizes, registry, tint, toolbar) and image (every `fit`, natural, missing) with the
gallery lane's fonts and assets; spinner sizes; hyperlink in a row.

Before/after images in this container (not committed), scratchpad
`/tmp/claude-0/-home-user-svelters/7fa2d99a-6c1c-5cb1-995d-8b330735211d/scratchpad/`:
- `b/progress-bar.png` → `a/pb5.png`: the progress sheet before (overflowing text, dark text on the fill, invisible
  empty and busy bars in light and HC) and after.
- `b/slider.png` → `a/slider3.png`: marks missing and hc-dark fill invisible before; marks, accent thumb and disabled
  grey after.
- `b/ch_end.png` → `a/ch_end.png`: the end-slot button above the header and past its edge, then on the caption line.
- `b/button_dense.png` → `a/button_dense.png`: the vertical separator missing, then drawn.

## Snapshot tests

None moved: the repository has no pixel snapshots of these kinds (`skelter-render/tests/reference` covers render
primitives only). Unit tests updated for intended changes: progress (the label is two clipped runs; the busy still
frame is centred), image and icon (placeholder token), plus new tests for elision, separator stretch and the
collapsing end slot.

## Tests

- `cargo test -p skelter-widgets --lib -- controls::progress controls::slider controls::separator
  controls::collapsing controls::frame controls::image controls::icon controls::spinner controls::hyperlink`: 83
  passed (before the slider gap edit); `-- controls::slider` after it: 19 passed; `-- theme`: 8 passed.
- `skelter-gallery check`: 31 stories, 135 variants, 0 problems (before the gallery merge).
- Not run locally: the workspace tests, other crates' tests, conformance (CLAUDE.md: CI is the gate).

## CI

On `3b841eb`: pending (filled in below when green).

## Requests to other lanes and the coordinator

1. **Running the busy sweep (MOTION-CSS or core).** Nothing animates `progress-bar`'s sweep or `spinner`'s
   rotation; both record one still frame. A kind cannot start a timer without an event (`wake_at` is on
   `EventCx`). *Recommendation:* a compositor animation for `:busy` nodes (rotate for spinner, translate a clipped
   segment for the bar), as the kinds' docs already plan, rather than per-frame re-recording.
2. **Dead UA colours (states lane, owner of `paint/`; svelters-style).** Any kind whose UA table sets `color` is
   overridden by the inherited computed colour (shared cause A). *Recommendation:* make UA tables a cascade origin
   below author rules (so a UA declaration beats inheritance, as in CSS), or have `ComputedStyle` record which
   properties were declared on the node so `StyleView::get` can prefer the UA value over an inherited one. Until
   then, each lane restates its kinds' colours in base.css. Seen elsewhere: the collapsing twistie and icon `icon`
   colour equal `fg` today, so they look right by chance.
3. **states:** frame/checkable unchecked depends on text-edit's disabled look (row 11); collapsing-header's header
   hover (`list-hover` in its `header` part UA) shows no change under the forced hover; slider thumb focus ring is
   not drawn at `focus-visible`. I added `hyperlink:disabled` and `slider:disabled::part(thumb)` in my block only
   because base.css now owns those kinds' colour; fold them into your state rules if you prefer.
4. **views:** `inner_window` places its content as an absolute box; check that the window is a positioned box
   (`position: relative`), or its content escapes to an ancestor as the collapsing end slot did (row 12).
5. **text:** collapsing-header/long-text caption overflow is in your list (caption eliding).

## Questions (with a recommendation)

- **Pickers.** The three pickers (colour, date, calendar) are not registered for Svelte (GAL2 request 1), so no
  story can mount them and I could not look at them. *Recommendation:* the coordinator schedules GAL2 request 1;
  a later controls pass looks at the picker stories it unblocks.
- **Collapsing header in RTL.** The twistie stays on the left in an RTL header (as tree-view's finding). Mirroring
  needs the header to read `direction`. *Recommendation:* solve it once with the views lane's tree twistie, through a
  shared helper.

## Handoff

Done: every GAL2 finding in scope fixed and looked at, or recorded (rows 4 partly, 11). Open: requests 1–5, the
pickers. Next steps: after the states lane merges, recheck frame/checkable unchecked and the slider focus ring.
