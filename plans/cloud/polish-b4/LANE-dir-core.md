# Lane dir-core (POL4-DIR-CORE)

1. **Cascade and inheritance** (first push): `direction` as an inherited CSS property in svelters-style (the current
   `direction` row is a box-layout setter for another backend: keep that one working, add the CSS property for
   skelter), the `dir` attribute on every element (surface prop `dir`: `ltr | rtl | auto`, `auto` from the first strong
   character of the node's text or its first text descendant), `:dir()` in the selector matcher, the window root from a
   document/host setting (`Document::set_direction`), and `NodeView::direction()` for kinds. Tests: inheritance,
   `dir` beats inherited, `auto`, `:dir()` matching, a change restyles descendants (the SV65/SV69 dependency path).
2. **Layout:** flex `row` from the inline start, `start`/`end` alignment on the inline axis, grid columns from the
   right, `text-align: start/end` with `start` the default, logical properties for margin/padding/inset/border, a scroll
   container's horizontal origin at the inline start and its vertical bar on the inline-end side... (browsers put it on
   the left in RTL: follow Chrome). Tests for each, with an `rtl` and an `ltr` case.
3. **AccessKit and text:** text runs in an `rtl` box get the paragraph direction (bidi base) from the box, not only from
   the first strong character; AccessKit nodes carry the text direction.
4. **Docs:** a `docs/skelter/direction.md` page (the model, `dir`, `:dir()`, logical properties, how a kind mirrors).

Design note first in the record (where direction lives in the cascade, the layout engine's support, what changes per
frame). Measure the cost of the inherited property on the style pass (cloud, not a gate).
