# svelters cloud batch polish-b2: every widget in the gallery, and the layout gaps (common brief)

The user asked on 2026-10-27 to polish every component in svelters and cadatomic. gallery-b1 built the gallery
(GAL1–GAL3) and polish-b1 polished what it could show (POL1-*, svelters master `e01ce9b`). On 2026-10-28 the user said
"Plan the next round". This round makes the rest of the widgets visible and closes the layout gaps polish-b1 recorded.
The coordinator is cloud session `session_01HBbGRgGENFKUjsqyGryCSr`; MAL's coordinator
(`session_016CTvCHJyR4hx83LMP4XD5y`) runs cadatomic and has no lane in these crates.

The rules, setup, method and record format are polish-b1's (`plans/cloud/polish-b1/BRIEF.md`: "Setup", "How to work
(the gallery loop)", contact sheets are never goldens, re-bless only snapshots your own change moved, after looking),
with these differences:
- **Branch** `ci/pb2-<lane>`, base `master` `e01ce9b` or later. **Record** `plans/milestones/POL2-<LANE>.md`.
- **Read first:** the polish-b1 brief, then `POL1-VIEWS.md` (Q1–Q4), `POL1-TEXT.md` (Q1–Q2), `POL1-STATES.md` and
  `POL1-GALLERY.md` (open items), `GAL2-stories.md` "Requests", `docs/skelter/gallery.md`, then your lane file.
- **The gallery now has** real fonts (Inter, JetBrains Mono, Arabic/CJK/symbols), icons and `asset://` SVGs,
  `--diff before.png`, `Renderer::reload`, host hooks. Use `--diff` for every before/after.

| Lane | Branch | Record | Scope | Merge |
|---|---|---|---|---|
| register | `ci/pb2-register` | `POL2-REGISTER.md` | make every skelter-widgets kind usable from Svelte, stories for each, open states for menus and tooltips, then polish them | 1 |
| layout | `ci/pb2-layout` | `POL2-LAYOUT.md` | skelter-core and svelters-skelter layout gaps: scroll bars on a fixed foreground layer, overflow states, chrome eliding, inherited disabled, the window's chrome | 2 |

Shared files: `base.css` (each lane edits only its kinds' blocks; keep both sides at a merge), `Cargo.lock`. register
owns `crates/svelters-skelter/src/{kinds.rs,surface/}`, `metadata/semantics/skelter.json` and its generators, and
`gallery/stories/` for new stories; layout owns `crates/skelter-core/`, `crates/skelter-widgets/src/views/{scroll.rs,
virtual_list.rs,tree.rs,common.rs}` and `src/text/` only where a scroll bar or elide needs it, and
`crates/svelters-skelter/src/` outside `kinds.rs`/`surface/`. A change the other lane needs is a request in your
record; work around it.
