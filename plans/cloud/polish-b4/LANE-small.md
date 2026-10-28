# Lane small (POL4-SMALL)

1. **Popup `anchor-to`** (POL3-OVERLAYS Q1, recommended there): a surface prop naming the element a popup positions
   against (an anchor id, as `anchor` names nodes), with `placement` (the existing placement sides and alignment, plus
   flipping when it doesn't fit, as `views/placement.rs` does for menus). Tests: a popup anchored to a button in a
   scrolled container follows it; it flips at the window edge; the anchor's removal closes it. Stories.
2. **Disabled progress bar** (POL3-FINISH Q2, accepted): dim its parts (track, fill, text) by the disabled roles, not
   the whole leaf's opacity; the indeterminate one too. Sheets in four themes.
3. **Table rows follow density** (cadatomic FQ-146): an unset `row-height` follows the theme's `row-h` (22 default, 20
   compact, 28 comfortable, with SV83's growth for a larger text size, as virtual-list does) and the header follows
   `header-h`; an explicit value still wins. Tests at each density.
