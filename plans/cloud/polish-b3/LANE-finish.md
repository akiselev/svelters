# Lane finish (POL3-FINISH)

1. **The register lane's leftovers:** R4 (a standalone `scrollbar` that measures and prepares itself), R-13..R-20
   (POL2-REGISTER.md), and the surface gaps pinned in `crates/svelters-skelter/tests/widgets_all.rs` KNOWN: close
   each (remove it from KNOWN with a test) or record why it stays.
2. **Layout:** `align-self`/`align-items: flex-start` (and `flex-end`) behave as `start`/`end` in skelter-core
   (POL2-LAYOUT Q2), with tests; the multi-line field's scroll thumb drags and its track pages (Q3).
3. **The full review:** render every story (`sheet --all`) in the four themes at zoom 1 and 2, and look at every cell
   against the VS Code reference (DESIGN.md §6): alignment, spacing on the density grid, text baselines, focus rings,
   disabled looks, HC outlines, RTL. Fix what is small and in your files; record the rest with `shot` commands. End the
   record with a table: story, state of its look (good, fixed here, open with row).
