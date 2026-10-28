# svelters cloud batch polish-b4: direction (RTL) and the leftovers (common brief)

The user decided on 2026-10-28: "lets add direction support", and "finish up the remaining small items: popups,
disabled progress bar, invalid quantity field, and the jetbrains mono code editor with optional ligatures". POL3-FINISH
Q1 found that skelter has no inline direction: every `rtl` story lays out left to right (Arabic text shapes right, but
rows, toolbars, twisties, chevrons and menus don't mirror). The coordinator is cloud session
`session_01HBbGRgGENFKUjsqyGryCSr`; MAL's coordinator (`session_016CTvCHJyR4hx83LMP4XD5y`) also merges svelters lanes.

Rules, setup, method and record format are polish-b1's, b2's and b3's briefs (`plans/cloud/polish-b{1,2,3}/BRIEF.md`:
setup, the gallery loop with `--diff`, sheets never goldens, re-bless only snapshots your change moved after looking,
a core change starts with a design note in the record), with:
- **Branch** `ci/pb4-<lane>`, base `master` `9f0272bb` or later. **Record** `plans/milestones/POL4-<LANE>.md`.
- **Read first:** the earlier briefs, `POL3-FINISH.md` (Q1, the RTL review), `POL3-OVERLAYS.md` (Q1, Q2),
  `plans/research/DESIGN.md`, `docs/skelter/gallery.md`, then your lane file.
- Near 400k tokens of context, stop at a clean commit and write the handoff; the coordinator starts a fresh session.

## The direction model (decided by the coordinator inside the user's decision; dir-core implements it, dir-widgets uses it)

As the web does it: CSS `direction: ltr | rtl` is an **inherited** property (initial `ltr`); a `dir` attribute on any
element (`ltr | rtl | auto`; `auto` from the first strong character of its text, as HTML) sets it; `:dir(ltr|rtl)`
matches the resolved direction. A window's root takes the document's direction (a host setting; default `ltr`). In an
`rtl` box: a `row` flex container lays out from the right (`row` = inline start → end), `justify-content: start/end`,
`align-self: start/end` on the inline axis, `text-align: start/end` (the default `start`), logical properties
(`margin-inline-start/end`, `padding-inline-*`, `inset-inline-*`, `border-inline-*`, and the `ms-/me-/ps-/pe-` utilities
if the utility table has them) all resolve against the box's direction; physical `left/right` stay physical. Grid
columns run from the right. A scroll container's horizontal origin is the inline start (the vertical scrollbar moves to
the left, as browsers do). Kinds read their resolved direction (`NodeView::direction()`), never the text.

| Lane | Branch | Record | Scope | Merge |
|---|---|---|---|---|
| dir-core | `ci/pb4-dir-core` | `POL4-DIR-CORE.md` | the model above in svelters-style and skelter-core (cascade, inheritance, `dir`, `:dir()`, layout, logical properties, scroll origin, text alignment, AccessKit), the surface prop, docs | 1 |
| dir-widgets | `ci/pb4-dir-widgets` | `POL4-DIR-WIDGETS.md` | every skelter-widgets kind mirrors in `rtl`; the RTL stories; the RTL sheet review | 2 |
| small | `ci/pb4-small` | `POL4-SMALL.md` | popup `anchor-to`, a disabled progress bar dims its parts, table rows follow density (FQ-146) | 3 |

**Order:** dir-core pushes the cascade/inheritance/`NodeView::direction()` part first (about the first hour), then
layout. dir-widgets starts on the kinds' own mirroring against that accessor (merge `origin/ci/pb4-dir-core` as it
lands: a merge, never a rebase) and on stories, and never edits core or svelters-style. small is independent.

Files: dir-core owns `crates/svelters-style/`, `crates/skelter-core/`, `crates/skelter-base/` if a type is needed,
`crates/svelters-skelter/src/{surface,kinds.rs,style.rs}` for the `dir` prop, `metadata/semantics/skelter.json` and its
generators, `docs/skelter/`. dir-widgets owns `crates/skelter-widgets/src/` except the files small names, and
`gallery/stories/` RTL variants. small owns `skelter-widgets/src/views/{popup,placement}.rs`,
`controls/progress.rs`, `table/{kinds,state,mod}.rs` (row height only). `base.css` is shared by blocks.
