# POL4-DIR-CORE: inline direction in svelters-style and skelter-core

Lane `dir-core` of batch polish-b4 (`plans/cloud/polish-b4/BRIEF.md`, `LANE-dir-core.md`), branch `ci/pb4-dir-core`
from `master` `9f0272b` (+ the batch briefs `8bcc50d`). The user decided on 2026-10-28: "lets add direction support";
the coordinator's direction model is the brief's.

## Design note (written before the code)

**Where direction lives in the cascade.** Two values, as on the web:

- **Directionality** (HTML's; what `:dir()` matches): a node's `dir` prop (`ltr`, `rtl`; `auto` from the first strong
  character of its text, then its descendants' text in tree order, skipping descendants with a `dir` of their own; `ltr`
  when none), else its parent's, else (a window root) the document's (`Document::set_direction`, default `ltr`). Kept
  per node (`NodeStyle::dir`), set top-down by the restyle walk as it reaches each node, *before* its rules match, so
  `:dir()` on the node and on its ancestors reads current values. It is independent of CSS (as in browsers, where
  `direction: rtl` in a rule does not make `:dir(rtl)` match): no cycle between matching and the computed value.
- **CSS `direction`** (`ltr | rtl`, inherited, initial `ltr`): a new row in skelter-core's style table (`EXTRA`) and in
  skelter.json's engine properties. A node with a `dir` (and a window root) inherits its directionality instead of its
  parent's `direction`: the cascade gets a copy of the parent's computed style with `direction` set
  (`ComputedStyle::set`, new in svelters-style). The node's own author rules still win, as the UA rule
  `[dir=rtl] { direction: rtl }` lets them in a browser; only `direction: inherit` on such a node differs (it gets the
  `dir`'s value). The copy is made only where the hint differs from what the node would inherit (rare: a `dir` that
  changes direction, or an `rtl` root).
- `ComputedBox::direction` carries the computed value; `NodeView::direction()` / `is_rtl()` give it to kinds, and
  `Document::node_direction(node)` to hosts and tests. The Qt `direction` row (a `box-layout` setter,
  `tables.generated.rs` QT) is another table and is untouched.
- `:dir(ltr|rtl)` is a new `PseudoClass::Dir` in svelters-style, matched through a new `Element::is_rtl()` (default
  `false`: other backends have no direction, every element is `:dir(ltr)`). It reads no state bit; the invalidation
  map ignores it because every direction change restyles the subtree it reaches.

**What restyles.** A `dir` write restyles the node's subtree (`STYLE_SUBTREE`); `Document::set_direction` restyles every
window. A text write (the kind's `text_prop`) or a child inserted or removed looks for the nearest `dir` at or above it
and, when it is `auto` and resolves differently now, restyles that subtree. The walk up runs only once some node has
had `dir=auto` (`DocStyle::any_dir_auto`). A change of the computed `direction` counts as inherited (children restyle),
text (re-measure: alignment and the bidi base), paint and layout (`box_diff`, `inherited_differs`).

**The layout engine's support.** taffy 0.15 has `Style::direction` with RTL in flexbox (a `row` from the right,
`start`/`end` and `flex-start`/`flex-end` on the inline axis, `row-reverse` from the left), grid (columns from the
right) and block, and puts a scroll container's gutter on the left. skelter-core sets `st.direction` from the computed
value, after the kind's `layout_style` too (a kind that builds its own style keeps the node's direction). Physical
`left`/`right`, margins and paddings stay physical in taffy; logical properties (`margin-inline-start`, …) are resolved
to physical sides in the cascade against the node's own direction.

**What changes per frame.** Nothing new per frame: direction is computed in the style pass only, for the nodes it
already restyles; layout reads one more field of the interned taffy style. The extra per-node style work is one
`dir` prop lookup (a hash probe in the node's prop store) and, for nodes with a `dir`, one parent-style copy.

## Work

| Part | What changed | Tests |
|---|---|---|
| 1. cascade (first push, `8ddfa09`) | svelters-style: `:dir(ltr|rtl)` (`PseudoClass::Dir`, `Element::is_rtl`), `ComputedStyle::set`. skelter-base: `text::Direction`, `first_strong`/`strong` (R/AL blocks without marks and Arabic digits). skelter-core: the common `dir` prop (string: `ltr`, `rtl`, `auto`; anything else is no `dir`), `direction` in the style table, `style/direction.rs` (directionality, `auto`, `Document::set_direction`/`direction`/`node_direction`, restyle on `dir`, text and child changes), `ComputedBox::direction`, `NodeView::direction()`/`is_rtl()`, `skelter_core::Direction`. svelters-skelter: the `dir` surface prop (tier0 `Widget`), `direction` in `ENGINE_PROPS`; skelter.json, `props.generated.js` regenerated | skelter-core `tests::direction` (inheritance, `dir` beats inherited, author CSS beats `dir`, document direction, `auto` incl. text and child changes, `:dir()`, subtree-only restyle count); svelters-style `selectors::dir_matches_the_nearest_dir` |
| 2. layout | taffy's `Style::direction` from the computed value (also over a kind's own `layout_style`): rows from the right, `start`/`end`, grid columns from the right. Logical longhands in the style table and skelter.json (`margin-`, `padding-`, `inset-inline-start/end`, `border-inline-start/end-{width,style,color}`) with svelters-style shorthands `margin-inline`, `padding-inline`, `inset-inline`, `border-inline{,-start,-end}`; resolved against the box's own direction (margins, paddings, insets in `layout_style`; borders in `computed_box`). `ComputedBox::physical_text_align` (`start` = right in `rtl`; the default stays `start`). Scroll origin: an `rtl` horizontal scroller starts at the right end and keeps its distance from it as content changes; offsets stay left-based everywhere (compositor, `ScrollGeometry`, scroll bars), the content space is shifted by the range (`scroll.rs` `rtl_scroll_range`/`scroll_shift`, `frame.rs`); a direction change refreshes the property trees | `tests::direction` `a_row_lays_out_from_the_inline_start`, `start_and_end_alignment_follow_the_direction`, `logical_sides_resolve_against_the_boxs_direction`, `an_rtl_scroller_starts_at_its_inline_start`, `text_align_start_is_the_inline_start`; svelters-style `shorthand::tests::logical_inline_shorthands` |
| 3. text and AccessKit | `TextSpec::direction` (skelter-base) and `RichText::direction` (skelter-text) set parley's base direction; core's measure spec carries an `rtl` box's direction (an `ltr` box keeps first-strong, see Q1); a node in `rtl` (or `ltr` under `rtl`) carries AccessKit `text_direction` unless its kind set one | skelter-text `tests::geometry::a_base_direction_overrides_the_first_strong_character`; skelter-core `tests::a11y::nodes_carry_their_text_direction` |
| 4. docs | `docs/skelter/direction.md`: the model, `dir`, `:dir()`, logical properties, scroll offsets, how a kind mirrors, cost | — |

Outside the lane's files, the one line a compile needs: `skelter-widgets/src/host.rs` sets `direction: None` in its
`skelter_text::RichText` literal (marked for dir-widgets, who replace it with the node's direction).

### Cost of the style pass (measured, not a gate)

`tests::direction::style_pass_cost` (ignored; `cargo test -p skelter-core --release --lib -- --ignored style_pass_cost
--nocapture`): a full restyle and layout of 5,000 boxes under three rules, best of 30, this container, the same test on
`master` `9f0272b` in a worktree:

| Build | Plain | `dir` on every tenth box |
|---|---|---|
| `master` | 50.8–51.8 ms | — |
| direction + logical rows, before the fix below | 54.6–55.0 ms | 55.1–55.5 ms |
| logical rows removed (an experiment) | 51.3 ms | 51.7–51.9 ms |
| final (with the fix) | 36.2–36.9 ms | 36.9–37.0 ms |

Direction itself costs under 1% (the `dir` lookup per node; the parent-style copy where a `dir` changes direction).
The 12 logical table rows cost ~6%: svelters-style's `compute` ran `Prop::is_inherited` (a by-name binary search) for
every table property on every element. It now looks at the parent's slot first (empty for most properties), which
makes a full restyle ~30% faster than `master` (`svelters-style/src/computed.rs` `inherit`, same results).

### For dir-widgets (what the kinds do with it)

- `NodeView::direction()` / `is_rtl()`; `ComputedBox::physical_text_align()`.
- Text: put the node's direction into the rich text (`host.rs` `LayoutRich`: `direction: Some(dir)`; the shim's
  `RichText` needs the field), so `start` alignment and run order follow the box. `paint/text.rs` reads `text-align`
  keywords itself: `start`/`end` there should resolve through the direction.
- Scroll bars: offsets are left-based, so thumbs need no change; the vertical bar goes to the left in `rtl` (Chrome).
- `text/field.rs` sets `LeftToRight` on its text runs: it should follow the run's direction.

## Questions

- **Q1 (design, recommendation: later, with the RTL stories).** An `ltr` box leaves its text's paragraph direction
  to the first strong character (`ComputedBox::text_spec` sets a base only in `rtl`), so Arabic text in an `ltr`
  document still reads from the right, as before. The web sets the base from the box always. Recommend switching to
  the box's direction everywhere once dir-widgets' RTL stories carry `dir="rtl"`, re-blessing what moves.
- **Q2 (procedure).** `skelter-widgets/src/host.rs` got one line (`direction: None` in the `skelter_text::RichText`
  literal) so the workspace compiles; dir-widgets owns the file and replaces it.
- Not done: the `ms-`/`me-`/`ps-`/`pe-` utilities (the utility table marks them unsupported; adding them is a
  `utilities.yaml` row each against egui.json, which has no logical properties); a logical property and its physical
  side set on one element take the logical one (CSS takes the later declaration; documented).
