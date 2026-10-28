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

(filled in as it lands)
