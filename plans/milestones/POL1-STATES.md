# POL1-STATES: shared state looks for skelter's interactive kinds (polish-b1, lane `states`)

Branch `ci/pb1-states`, base `master` `9d97e7d` (the batch brief on `f77fad0`). Brief: `plans/cloud/polish-b1/BRIEF.md`,
`LANE-states.md`.

## Scope

The focus ring, disabled, hover and pressed looks of every interactive kind (GAL2 findings 1–3, GAL1's two styling
bugs, GAL3's two notes), button looks (`default`, `selected`), dialog-role emphasis, the radio/checkbox accent. Files:
`skelter-widgets/src/{paint,theme}/`, `controls/{button,checkbox,radio,switch,toggle_button}.rs`, the state rules in
`base.css`, plus two small causes outside them (skelter-core's forced-state paint path, `host.rs`'s paint state), noted
below.

## Two shared causes (why the first three findings showed nothing)

1. **Forced states never reached paint.** `Document::force_state` (the gallery's and the designer's way to show
   `:hover`, `:focus-visible`, …) promised "selector matching and painting", but (a) it set the node's `PAINT` bit
   directly instead of through `mark()`, so the ancestors had no `DESC_RECORD` and the frame's walk never re-recorded
   the node unless a CSS value also changed; and (b) kinds painted from `NodeView::state()`, the node's own bits only.
   So every UA state rule (the button's focus ring and `:selected` fill, the indicators' hover and pressed fills) was
   invisible in the gallery, while real keyboard focus did draw the ring. Fix: `force_state` marks through `mark()`
   (`skelter-core/src/style/inspect.rs`); new `NodeView::style_state()` = own | forced bits
   (`skelter-core/src/cx.rs`), which `skelter-widgets/src/host.rs` uses for every `Styles` it builds for measure,
   prepare and record. Events and accessibility still read `state()` (forced bits never reach them, as documented).
2. **A part's inherited value beat its UA table.** skelter-core computes a style for every paint part, inheriting
   from the host, so `color`, `font-size`, … always arrived as CSS values and won over the part's UA rules (the
   switch knob's `:checked` white, the icon part's 16px). Fix (`paint/mod.rs`, `StyleView::get`): for a part view, an
   inherited property whose value equals the host's (only inherited, not set by a `::part()` rule) yields to the
   part's own UA table. Edge: a `::part()` rule that sets exactly the host's value now loses to the UA value.

Plus: **`opacity` in a kind's UA table is never painted** (skelter-core applies opacity from the computed box only), so
R9's "opacity .4" lives in `base.css` now. The button UA's `opacity` entry stays for foreign painters' reference.

## Findings

| Finding | Cause | Fix (file) | Before/after looked at | Snapshot tests moved |
|---|---|---|---|---|
| GAL2 #1 / GAL1 1: `focus-visible` draws no ring (every interactive kind, all four themes) | cause 1 | core + `host.rs` as above; the rings were already the kinds' UA outlines from `--sv-focus` (`focusBorder`: `#2488DB` aff., `#005FB8`, `#F38518`, `#006BBD`) | yes: button, checkbox, radio, switch, toggle-button, select, collapsing-header, dialog; zoom 1 and 2 | none |
| R8: checkbox, radio, switch, toggle-button rings were 1px | — | 2px at +1 like the button (`controls/{checkbox,switch,toggle_button}.rs`; radio uses checkbox's host UA) | yes | unit tests' ring rects updated (4 tests) |
| Indicator rings were square around a round radio or switch track | the ring took the host's radius (0) | `paint::paint_outline_radius`; `checkbox::paint_indicator` passes the indicator's radius (`paint/boxes.rs`, `controls/checkbox.rs`) | yes, all four themes | none |
| GAL2 #2 / GAL1 2: `disabled` changes nothing (button, checkbox, text-edit, …) | UA opacity is never painted | `base.css`: `opacity: .4; cursor: not-allowed` on button, toggle-button, checkbox, radio-button, switch, slider, select, text-edit (R9, VS Code's `.disabled { opacity }`); only interactive kinds, because a disabled container's state reaches its descendants and would compound | yes, all four themes, zoom 1 and 2 | none |
| GAL3: disabled label looks normal | no rule | `label:disabled` → `--sv-disabled-fg` (`#CCCCCC80`, `#61616180`, `#A5A5A5`, `#7F7F7F`; hc-dark's is subtle, VS Code's own value) | yes | none |
| Disabled collapsing header (seen on its sheet): opacity dimmed its body too | the header holds its body | colour dim (`disabled-fg`) instead of opacity; the twistie's UA dims already | yes | none |
| GAL2 #3: no hover or pressed on checkbox and text-edit; GAL3: checkbox hover hard to see in Dark Modern | cause 1 for the indicators; the fill step `#313131`→`#3B3B3B` alone is faint; text-edit had no rule | indicators (checkbox, radio, switch track) step the edge to `--sv-muted` with the fill (UA, plus `checkbox:hover` in base.css, whose host rule sets the border); `text-edit:hover`, `select:hover` step the border to `muted`; `text-edit:focus/:focus-visible/:focus-within`, `select:focus-visible/:open` take `--sv-input-border-color-focus` (R4) | yes, all four themes | none |
| GAL2 #4: button `default` and `selected` look like `plain` | base.css's `button { background-color }` beats the UA `:selected` rule; no `:default` rule | `button:default` = the accent (primary) button, `:hover` accent-hover; `button:selected` = the on toggle (`toggle-on-bg`, `toggle-on-border`), as `toggle-button:checked` | yes, all four themes, zoom 1 and 2 | none |
| GAL2 #5: `dialog-role="destructive"` unstyled; the default (Keep) not emphasised | no rules | `:default` above emphasises Keep; `button[dialog-role=destructive]` takes `--sv-error` on caption and edge (≈3.9:1 on the button fill in both Modern themes; `danger` is 2.7:1 in Light), filled with it when it is also the default | yes, dialog sheet, all four themes | none |
| High Contrast: checked toggle, checkbox, radio look unchecked (seen on toggle-button's sheet) | HC's `inputOption.activeBackground` is transparent and its `activeBorder` equals `contrastBorder` | HC token sets: `toggle-on-bg` `#FFFFFF33` / `#0F4A8533` (toolbar-active's tint), `toggle-on-border` `contrastActiveBorder` (`#F38518` / `#006BBD`) (`theme/data/vscode-hc-*.json`, with notes) | yes | none |
| hc-dark: a checked switch's track vanished | `accent` is HC's `#000000` button background | the checked track's edge is `toggle-on-border` (accent in Modern, `contrastActiveBorder` in HC) (`controls/switch.rs`) | yes | none |
| Switch's checked knob drew `fg`, not `accent-fg` | cause 2 | as above | yes | none |
| High Contrast hover is a fill step, VS Code's is a `contrastActiveBorder` outline | — | `@media (prefers-contrast: more)`: `button:hover`, `toggle-button:hover` outline 1px `--vscode-contrastActiveBorder` at −1, focus ring kept over it | yes (draws solid, see Q2) | none |
| GAL2: radio's dot dark grey while the checkbox uses the accent (light-modern) | not reproduced: both draw the `fg` mark on the `toggle-on` fill with the accent edge | no change | yes, both at zoom 2 | none |

## Before/after images (container paths, not committed)

- `target/gallery/before/button-look.png`: button/look, all states × four themes, before: focus-visible and disabled
  rows equal normal; default and selected equal plain.
- `target/gallery/s3/button.png` (and `button-z2.png` at zoom 2): after: rings in every theme (orange in hc-dark),
  accent default, toggle-filled selected, dimmed disabled, HC hover outlines.
- `target/gallery/s3/checkbox.png`, `sw-radio.png`: after: hover edges, round 2px rings, HC on-state, dimmed disabled.
- `target/gallery/s3/dialog.png`: after: Keep in the accent, Delete in the error colour, all four themes.

## Requests to other lanes (not built here)

- **controls (hyperlink):** a `hyperlink` draws in `fg`, not `--sv-link`: on a host view the inherited `color` beats
  the kind's UA (cause 2 at the host level; a host view has no parent view to compare with). Recommendation: a
  `hyperlink { color: var(--sv-link) }` rule in its base.css block (as `button` has), or the kind passing its parent's
  colour to `StyleView`. `hyperlink:disabled` then needs `color: var(--sv-disabled-fg)` (mine; add it with theirs).
- **controls (slider):** in hc-dark the filled part of the slider track is the accent `#000000` on black, invisible.
  Recommendation: the fill from `--sv-progress` or `--sv-toggle-on-border` (the switch's fix).

## Questions (with recommendations)

- Q1: Text fields show `:focus` (pointer focus too) while buttons ring only on `:focus-visible`. That is VS Code's
  behaviour (inputs: `focusBorder` on focus) and DESIGN R4; recommend keeping it.
- Q2: `outline-style: dashed` paints solid (seen on the HC hover outline at zoom 2), probably the renderer's quad
  border. Recommend a renderer check; the solid edge still reads as VS Code's `contrastActiveBorder`.
- Q3: Core doesn't pass a collapsing header's disabled state to its body. If it should (Qt and GTK disable the
  content), recommend skelter-core's inherited disabled covering the body, and then the header can take opacity too.

## Tests

- `cargo test -p skelter-widgets --lib`: 554 passed (4 ring-rect assertions updated); `--tests`: `text_pins` 1 passed.
- `SKELTER_GPU_TESTS=1 WGPU_BACKEND=vulkan cargo test -p skelter-core -p skelter-test -p skelter-gallery`: 303 passed,
  no pixel snapshot moved (none re-blessed).
- `skelter-gallery check`: 31 stories, 135 variants, 0 problems.
- Not run locally: the workspace tests, svelters-skelter, svelters-egui (it has its own theme and base.css), conformance.

## CI

(filled in below)

## Handoff

Done: every finding in scope fixed and seen on its sheet, or recorded. Next: the two requests above; Q2's dashed
outlines.
