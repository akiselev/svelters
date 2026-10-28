# Lane rows (POL4-ROWS): make the 24 px trial reach lists and trees

Branch `ci/pb4-rows`; record `plans/milestones/POL4-ROWS.md`. Read `BRIEF.md` (this batch) and `POL4-SMALL.md` ("Addition:
default-density rows"). The user's trial (2026-10-28, "lets try the WCAG target"): at the default density, rows are at
least 24 px; compact stays the opt-in below. POL4-SMALL moved `density.json` and the table, but the coordinator's
before/after shows tree and list rows unchanged, for two reasons:

1. **Kinds with their own constant:** `tree-view` defaults to `TREE_ROW_HEIGHT` 24 (views/tree.rs) and `virtual-list`
   to a `row-height` prop default of 32. Make an unset `row-height` follow the theme's `row-h` for both (with SV83's
   growth for a larger text size, as `auto-row-height` and the table do), so compact gives 20, default 24, comfortable
   28; an explicit `row-height` still wins. Check every other kind with its own row constant (the tab overflow menu
   rows, the select's no-theme fallback) and either route it through `row-h` or record why not. Tests at each density.
2. **Stories that pin 22:** `gallery/stories/**` set `row-height={22}` (tree-view: every variant) and possibly other
   fixed row heights. Remove the ones that only restate the old default, so stories show the theme's density; keep a
   fixed height only where the variant is about a fixed height (and say so in the story.toml notes).

Then render tree-view, virtual-list, table and combo-box open at the three densities with `--diff` against master and
look. Re-bless svelters snapshots this moves after looking. Files: `views/{tree,virtual_list}.rs`, `tabs/mod.rs` and
`views/select.rs` only for row constants, `gallery/stories/`. All six workflows green; then message the coordinator
(`session_01HBbGRgGENFKUjsqyGryCSr`) the sha.
