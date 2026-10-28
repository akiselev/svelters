# Lane layout: the layout gaps polish-b1 recorded (POL2-LAYOUT)

Branch `ci/pb2-layout`; record `plans/milestones/POL2-LAYOUT.md`. These need skelter-core or svelters-skelter
changes, which polish-b1's lanes were told not to make. The coordinator approves them as implementation inside
skelter's agreed design (`plans/research/skelter/architecture.md`). For each: write a short design note in your record
first (what core gains, why no existing mechanism does it, the alternative rejected), then build it with tests, then
show the sheet. A change that would alter skelter's architecture (a new phase, a change to the commit format) stops at
the note: record it with a recommendation and move on.

1. **Scroll bars on a fixed foreground layer** (POL1-VIEWS Q1): virtual-list and tree-view show no scroll bar, and
   the multi-line text field (POL1-TEXT Q2) has none, because their content scrolls under any painted bar. Give core a
   per-node foreground that does not scroll with the content (or reuse an existing overlay mechanism if one fits), and
   draw the `Scroller`'s bars there for scroll-area, virtual-list, tree-view, table and multi-line text-edit. Bars
   follow the theme (`scrollbarSlider.*`), fade per DESIGN.md, and jump with reduced motion.
2. **Overflow states** (POL1-VIEWS Q3): set `:state(sv.overflow-start|end)` from each `Scroller`
   (`away_from_start`/`away_from_end`) on skelter documents, so base.css's existing rules (shadows at the edges) apply.
3. **Chrome eliding** (POL1-TEXT Q1): a collapsing-header caption can't elide while it is core's `chrome_min_size`;
   let a kind's chrome text shrink to an ellipsis when the parent is narrower, and use it for collapsing-header (and
   frame captions if they share the path).
4. **Inherited disabled** (POL1-STATES open item): a disabled frame (and any disabled container) disables its
   content: the state and the look, and AccessKit's disabled flag on each descendant control; an unchecked checkable
   frame already dims, make it use the same path.
5. **The window's chrome** (POL1-STATES, POL1-GALLERY): in the `window` story the toolbar overlaps the title (padding
   and gap in svelters-skelter's window); fix it and check the window story in all four themes.

Tests for each in the crate it changes; before/after with `--diff` for 1–5. Refuse rather than guess: a core change
whose cost to every frame you can't measure (record it; measurements are "cloud, not a gate").
