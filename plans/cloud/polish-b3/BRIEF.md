# svelters cloud batch polish-b3: overlays and the finish (common brief)

The user asked on 2026-10-27 to polish every component in svelters and cadatomic; polish-b2 made every skelter-widgets
kind usable from Svelte and closed the layout gaps (POL2-REGISTER, POL2-LAYOUT; master `674d7de` or later). On
2026-10-28 the user approved round 3 ("Go ahead and start"). The coordinator is cloud session
`session_01HBbGRgGENFKUjsqyGryCSr`; MAL's coordinator (`session_016CTvCHJyR4hx83LMP4XD5y`) also merges svelters lanes.

Rules, setup, method and record format are polish-b1's (`plans/cloud/polish-b1/BRIEF.md`: "Setup", "How to work",
contact sheets never goldens, re-bless only snapshots your own change moved, after looking), plus polish-b2's
(`plans/cloud/polish-b2/BRIEF.md`: core changes start with a design note in the record; an architecture change stops
at the note), with:
- **Branch** `ci/pb3-<lane>`, base `master` `674d7de` or later. **Record** `plans/milestones/POL3-<LANE>.md`.
- **Read first:** both earlier briefs, `POL2-REGISTER.md` (R4, R-13..R-20, Q1–Q5, `tests/widgets_all.rs` KNOWN),
  `POL2-LAYOUT.md` (Q1–Q3), `docs/skelter/gallery.md`, then your lane file.
- Every before/after with `--diff`. Near 400k tokens of context, stop at a clean commit and write the handoff into
  your record; the coordinator starts a fresh session for the rest.

| Lane | Branch | Record | Scope | Merge |
|---|---|---|---|---|
| overlays | `ci/pb3-overlays` | `POL3-OVERLAYS.md` | a top layer in core for popups, menus, tooltips, toasts and picker dropdowns; pickers' open props and clock; tab overflow | 1 |
| finish | `ci/pb3-finish` | `POL3-FINISH.md` | R4, R-13..R-20, the KNOWN surface gaps, `flex-start`, multi-line thumb drag, and a full review of every story | 2 |

Files: overlays owns `crates/skelter-core/` (the top layer), `skelter-widgets/src/views/{popup,tooltip,toast,menu,
placement,overlay,modal}.rs`, `src/pickers/`, `src/tabs/`. finish owns `skelter-widgets/src/views/scroll.rs` and the
rest of `src/`, core's layout code only for `flex-start` (tell overlays before touching core), `crates/svelters-skelter/
src/surface/` and `kinds.rs`, and `gallery/stories/` (both lanes may add stories for their own kinds). `base.css` is
shared by blocks. A change the other lane owns is a request in your record.
