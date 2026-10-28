# Lane overlays (POL3-OVERLAYS)

1. **A top layer** (POL2-REGISTER Q2): popups, menus, tooltips, toasts, combo and picker dropdowns are drawn inside
   their owner's subtree today, so a container's clip or a later sibling covers them. Give skelter-core a top layer
   (a per-window list of nodes drawn after everything, hit-tested first, positioned by `views/placement.rs`, removed
   with their owner), as the web's top layer and VS Code's context views do. Design note first; tests: a menu in a
   clipped scroll area draws fully and takes the click; Escape and outside clicks close in order; AccessKit keeps the
   owner relationship.
2. **Pickers** (Q4, Q5): an `open` prop on colour picker, date picker and combo (as `context-menu` has), and the
   pickers on the document clock (whatever the register lane found missing), so a story shows each open.
3. **Tab overflow** (Q3): more tabs than fit scroll (wheel and arrow buttons) with an overflow menu listing every tab,
   as VS Code's editor tabs and Qt's tab bar do; the active tab scrolls into view.
4. **Polish them:** stories for each open state; sheets in the four themes at zoom 1 and 2; fix what is off.
