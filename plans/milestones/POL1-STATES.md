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
2. **An inherited value beat the kind's own UA declaration.** A computed style always holds the inherited
   properties (skelter-core computes every part from its host and every node from its parent), so `color`,
   `font-size`, … arrived as CSS values and won over the UA tables: the switch knob's `:checked` white, the icon part's
   16px, a hyperlink's link colour, a select option's and menu item's `:selected` colour (views and controls hit it
   too and worked around it). In CSS, a declaration on the element (the UA's included) beats inheritance. Fix
   (`paint/mod.rs`, `StyleView::get`): an inherited property whose value equals the parent's (so only inherited) yields
   to the view's own UA declaration. A part compares with its host view; a host compares with its parent node's
   computed style, which `Styles::inherited` now carries (`host.rs`, from the new `NodeView::parent`) and
   `StyleView::with_inherited` takes (`controls/common::host_view`, `checkbox::host_in` and the kinds that build their
   host view themselves). Test: `paint::tests::a_ua_declaration_beats_an_inherited_value`. Edge: a rule that sets
   exactly the parent's value now loses to the UA value.
   Removed as redundant, each sheet pixel-identical with and without, in every state and theme:
   `views::common::part_fg` (select options, menu items read `color` directly), and base.css's restated
   `hyperlink`, `hyperlink:disabled`, `spinner`, `image`, `slider::part(mark-label)` and `frame::part(caption)` colours.
   Kept: `slider::part(thumb)` (`progress`, not its UA `accent`: controls' High Contrast fix). Across every family's
   normal sheets in Dark and Light Modern, the only changes from master are the intended ones (the default and
   selected buttons, dialogs, the switch knob, a select's placeholder colour, an unchecked checkable frame's
   dimmed content).

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
| frame/checkable: an unchecked frame's content looked enabled (controls' finding, waiting on this lane) | the disabled look | the content's own `:disabled` opacity (R9 above) | yes, all four themes | none |
| progress-bar, spinner: `disabled` looked like normal | — | both in the R9 opacity list (leaf kinds: nothing compounds) | yes, all four themes | none |
| After the merge with POL1-TEXT: hover and focus would replace an invalid field's red border, hover a read-only field's hairline | my state rules come after the field looks | `text-edit:invalid:{hover,focus,focus-visible,focus-within}` keep `invalid-border` (`danger` in HC), `text-edit[read-only=true]:hover` keeps `border` | yes, all four themes | none |
| GAL2: radio's dot dark grey while the checkbox uses the accent (light-modern) | not reproduced: both draw the `fg` mark on the `toggle-on` fill with the accent edge | no change | yes, both at zoom 2 | none |

## Before/after images (container paths, not committed)

Re-rendered after merging `origin/ci/pb1-gallery` `cd478f3` (Inter, JetBrains Mono, symbol, Arabic and CJK fonts):
the "before" binary is `ci/pb1-gallery` itself (no fix of this lane) with its own `base.css` (`--base-css`), the
"after" is this branch. Each pair is before (left) | after (right):

- `target/gallery/after2/button-pair.png`: button/look, normal, focus-visible, disabled × four themes. Before: the
  focus-visible and disabled rows equal normal, and default and selected equal plain. After: rings in every theme
  (orange in hc-dark), the accent default, the toggle-filled selected, dimmed disabled.
- `target/gallery/after2/checkbox-pair.png`: checkbox/value, hover, focus-visible, disabled. After: hover edges,
  round 2px rings, the HC on-state, dimmed disabled.
- `target/gallery/after2/dialog-pair.png`: dialog/destructive. After: Keep in the accent, Delete in the error colour.
- Single "after" sheets: `target/gallery/after2/{switch,radio-button,toggle-button,text-edit,combo-box,label}.png`.

## Requests to other lanes (not built here)

- Done by controls (merged in `dca6740`): the hyperlink's link colour and the hc-dark slider fill; my two requests for
  them are dropped (the hyperlink rules are now redundant and removed, see cause 2).
- **controls / skelter-core:** a disabled frame (`frame/checkable`, `disabled` row) leaves its content enabled; only an
  unchecked checkable frame disables it. Recommendation: the frame's `disabled` reaches its content as the unchecked
  state does; the content then dims through R9.
- **gallery:** the `window` story's toolbar draws over the window title (`cadatomic` under the toolbar buttons), Dark
  and Light Modern (`skelter-gallery sheet window --states normal`).

## Questions (with recommendations)

- Q1: Text fields show `:focus` (pointer focus too) while buttons ring only on `:focus-visible`. That is VS Code's
  behaviour (inputs: `focusBorder` on focus) and DESIGN R4; recommend keeping it.
- Q2: `outline-style: dashed` paints solid (seen on the HC hover outline at zoom 2), probably the renderer's quad
  border. Recommend a renderer check; the solid edge still reads as VS Code's `contrastActiveBorder`.
- Q3: Core doesn't pass a collapsing header's disabled state to its body. If it should (Qt and GTK disable the
  content), recommend skelter-core's inherited disabled covering the body, and then the header can take opacity too.

## Tests

- On the merged head (master `dca6740` and `ci/pb1-gallery` `37ce91a`): `SKELTER_GPU_TESTS=1 WGPU_BACKEND=vulkan
  cargo test --no-fail-fast -p skelter-widgets -p skelter-gallery`: 588 passed (skelter-widgets 562 with the new
  test; 4 ring-rect assertions updated earlier). Before the gallery merge, the same with `-p skelter-core
  -p skelter-test`: 875 passed except skelter-gallery's `media` reload test, intermittent there (0 blue pixels after a
  restart), which the gallery lane's `81fd1c4` fixed; after merging it, `media` passed 3 runs of 3.
- No pixel snapshot moved; none re-blessed.
- `cargo check --all-targets` of every crate that depends on skelter-widgets (skelter, skelter-test, skelter-gallery,
  skelter-gallery-app, svelters-skelter): clean.
- `skelter-gallery check`: 32 stories, 137 variants, 0 problems (after the gallery merge).
- Not run locally: the workspace tests, svelters-skelter, svelters-egui (it has its own theme and base.css), conformance.

## CI

(filled in below)

## Handoff

Done: every finding in scope fixed and seen on its sheet, or recorded. Next: the two requests above; Q2's dashed
outlines.
