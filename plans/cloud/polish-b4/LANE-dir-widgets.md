# Lane dir-widgets (POL4-DIR-WIDGETS)

Every skelter-widgets kind mirrors in `rtl`, reading `NodeView::direction()` (dir-core). Merge `origin/ci/pb4-dir-core`
as its parts land; start with the kinds' own paint and hit-testing, which need only the accessor.

1. **Mirror the kinds** (as VS Code and the platforms do in RTL): tree twisties on the right and pointing left when
   collapsed; chevrons and submenu arrows flip; menus open toward the inline start and submenus to the left; tabs run
   from the right, with the overflow buttons swapped; slider and progress fill from the right (a horizontal slider's
   value grows leftward); switch knob on-position left; checkbox/radio glyph before the caption on the right; table
   column order from the right and the sort arrow side; split view's first pane on the right; scroll bar positions per
   dir-core; text fields' caret and selection start at the right (single-line was fixed in POL1-TEXT); combo and
   pickers' drop button on the left; calendar columns from the right (weekday order mirrored); dialog buttons in the
   platform's RTL order; toast and tooltip placement mirrored; status bar sections swapped. What does NOT mirror (as in
   the platforms): media-like progress spinners, checkmarks' shape, numbers, and icons that aren't directional.
2. **Stories:** every story with text gets an `rtl` variant (most have one: make it set `dir="rtl"` on its root
   instead of relying on the text); add variants where a kind's mirroring needs to be seen (open menus, overflowing
   tabs, sliders at a value).
3. **Review:** `sheet --all` in rtl and ltr, the four themes, zoom 1 and 2; fix what is off; the record ends with a
   table: kind, mirrored (yes / not applicable / open with row).
