# Lane list-a11y: list and menu content reachable by AccessKit and automation (AX1)

Branch `ci/b10-list-a11y`; record `plans/milestones/AX1-list-a11y.md`. cadatomic's tours found (batch 9, gap
G-shell-7; tours-board's DRC waive): the content of a `scroll-area`, of `ListView`/virtual-list rows, and the items
of a context menu do not reach the AccessKit tree, or reach it as "present, not visible", so automation can neither
find nor click them. Examples: cadatomic's Jobs rows and their Cancel buttons, the Output list's anchor, Problems
rows' context menu (`waive`, `unwaive`).

1. Reproduce each case in a svelters fixture (a scroll-area with buttons, a virtual list of 1,000 rows with anchors,
   a context menu on a row) and record what AccessKit and `Session::query` see.
2. Fix it in `svelters-egui` so visible content is in the tree with correct bounds and visibility, clipped rows are
   present only while realised (virtual lists), scrolling a row into view through automation (`scroll_into_view`)
   works, and menu items are visible and actionable while the menu is open. Anchors inside list rows keep SV53's
   keyed ids.
3. Tests for each case, in process (kittest) and over the AU2 transport; a docs note in `docs/egui` where the
   behaviour is documented.
