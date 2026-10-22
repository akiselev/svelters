# TR2b: focus does not reach exiting nodes

Follow-up of TR2 (`TR2-native-animator.md`, D6). An exiting node is inert (`SVL_STYLE_INERT`); TR2
already dropped its events, hid it from hit tests and from the accessibility tree. Keyboard focus
still reached it.

**Change** (`crates/svelters-egui/src/walk.rs`, `paint_visual`): the inert subtree is painted in a
`Ui` that is `disable()`d, so its widgets sense nothing. Tab and Shift-Tab skip them, and a focused
widget that becomes inert loses focus (egui clears focus of a widget that stops being focusable).
`disabled_alpha` is set to 1 in that `Ui` first, so egui's usual dimming of disabled widgets does not
add to the outro's own fade. One additive hunk, no scene change.

**Tests** (`tests/animator.rs`, kittest, virtual clock): `tab_skips_exiting_nodes` (Tab and Shift-Tab
over a list while the outro runs never end on the exiting button; before the outro they do) and
`a_focused_node_that_starts_exiting_gives_up_focus`.

**Known:** disabled widgets use egui's noninteractive style (text colour, no hover/press), which
is visible for the length of the outro. Only widgets inside the inert subtree are affected.
