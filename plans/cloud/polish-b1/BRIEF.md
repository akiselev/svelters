# svelters cloud batch polish-b1: polishing skelter's widgets (common brief)

The user asked on 2026-10-27 to polish every component in svelters and cadatomic, and after the gallery batch
(gallery-b1: GAL1–GAL3, merged at `f77fad0`) chose "Polish + cadatomic-gallery". The coordinator is cloud session
`session_01HBbGRgGENFKUjsqyGryCSr`. A separate coordinator runs MAL (cadatomic's malleability batches); MAL4's svelters
work is host and isolate code (`svelters-host`, the embed), not widgets.

svelters' `CLAUDE.md` binds you, with these specifics:
- **Coordinator:** the session above; it merges your branch into `master` (`--no-ff`) once all six workflows are green.
  Lanes never ask the user; questions go in your record with a recommendation.
- **Your branch** is `ci/pb1-<lane>`; base `master` `f77fad0` or later. Push only there; no pull requests.
- **Your record** is `plans/milestones/POL1-<LANE>.md` (scope, what changed per finding, before/after, snapshots
  re-blessed, tests with counts and commands, CI run ids, questions, handoff). Don't edit `STATUS.md` or `MATRIX.md`.
- **Read first:** `CLAUDE.md`, `docs/skelter/gallery.md` (the loop you work in), `plans/milestones/GAL1-gallery-core.md`,
  `GAL2-stories.md` (its "Polish findings" table is your work list) and `GAL3-gallery-app.md`,
  `plans/research/DESIGN.md` §1.5 (roles), §4 (themes and layers), §6 (the VS Code reference look), then your lane file.
- **Setup:** as gallery-b1's brief (`plans/cloud/gallery-b1/BRIEF.md`, "Setup"): rustc 1.97.0, `corepack enable`,
  `pnpm install --frozen-lockfile`, `git config core.hooksPath .githooks`, the apt packages `skelter.yml` installs,
  cargo with `CARGO_PROFILE_DEV_DEBUG=0 CARGO_INCREMENTAL=0`, `WGPU_BACKEND=vulkan` for rendering.
- **Build little; CI is the gate;** wait with `tools/dev/ci-wait <sha> --jobs`. Push after every finding or two and at
  least every 30–45 minutes. Near 400k tokens of context, stop, commit and write the handoff into your record.

## How to work (the gallery loop)

1. Build the CLI once: `cargo build -p skelter-gallery` (a release build if sheets get slow).
2. For each finding: render the "before" (`target/debug/skelter-gallery sheet <story> -o target/gallery/before/…` or the
   finding's `shot` command), fix it, render the "after", and **look at both images**. A fix you haven't looked at is
   not done. `inspect` tells you which rule and token reached a node.
3. The fix is in the kind's look (`crates/skelter-widgets/src/theme/base.css`, `affordance.json`, the kind's UA paint
   and its token use), never in the story. A story changes only to show a case better or to add a variant the
   finding needs.
4. Fix the cause where several kinds share it (a shared state rule, a token, a paint helper), not one kind at a time.
5. Look past the list: while a family's sheets are open, compare each kind against the VS Code reference look in its
   four themes (DESIGN.md §6) and fix what else is off, adding a row to your record for each.
6. Contact sheets are not goldens (user, 2026-10-27). Existing pixel snapshot tests that your change moves: re-bless
   only those your own change moved, after looking at the diff, and list each in your record. A finding that needs an
   architecture change (a new layout feature, a new prop) is recorded with a recommendation, not built.

Put 2–4 before/after image paths from your container in the record, described; never commit images.

## Lanes and files

`base.css` is shared: each lane edits only the rule blocks for the kinds it owns (state rules for all kinds belong to
`states`); at a merge, keep every rule from both sides. Everything else is owned by one lane.

| Lane | Branch | Record | Kinds and files | Merge |
|---|---|---|---|---|
| states | `ci/pb1-states` | `POL1-STATES.md` | focus ring, disabled, hover and pressed for **every** interactive kind (the first three findings), button looks (`default`, `selected`), dialog-role emphasis, the radio/checkbox accent; files `skelter-widgets/src/{paint,interact,theme}/`, `controls/{button,checkbox,radio,switch,toggle_button}.rs`, `views/dialog_buttons.rs`, the state rules in `base.css` | 1 |
| text | `ci/pb1-text` | `POL1-TEXT.md` | text fields (padding, invalid, read-only, multi-line overflow), selection contrast, rich text `<code>`, label weights, caption wrapping and eliding (button, checkbox, switch, collapsing-header long text); files `skelter-widgets/src/text/`, `controls/{label,rich,run}.rs`, skelter-text if a weight needs it | 2 |
| views | `ci/pb1-views` | `POL1-VIEWS.md` | scroll area (offset bug, tracks and gutters), virtual list and table scroll bars, table elide and selection and cursor row, tree view (`root-decorated`, RTL twistie), combo popup highlight contrast, menus, tooltips, toasts, split, panel, status bar; files `skelter-widgets/src/{views,table,tabs}/` | 3 |
| controls | `ci/pb1-controls` | `POL1-CONTROLS.md` | progress bar (overflow, contrast over the fill, empty trough, indeterminate), slider marks, frame (checkable disables content, canvas margin), collapsing header end slot, vertical separator, icon, image, spinner, hyperlink, pickers; files `skelter-widgets/src/controls/` (the rest), `pickers/` | 4 |
| gallery | `ci/pb1-gallery` | `POL1-GALLERY.md` | the tool, so the other lanes see true images: fonts with symbol, Arabic and CJK coverage (`--system-fonts` or bundled fallback, GAL1 Q3), an icon registry and SVG assets in the gallery host (GAL2 requests 3–4), the sheet header escaping newlines, `Renderer::reload(paths)`, a `window` story, `--diff <before.png>` (a side-by-side or highlighted difference image, for looking only), and the GAL1/GAL3 requests: R1 rule layer in `MatchedRuleInfo`, R3 truncated module-load messages, R4 public `canonicalize` in svelters-modules, Q1 one `quire-text` (skelter-widgets' pin advanced to `ed0665b`); files `crates/skelter-gallery*`, `gallery/` (format and tooling only), and the small changes in `svelters-style`, `svelters-host`, `svelters-modules`, `skelter-widgets/Cargo.toml` those requests name | 0 (first) |

The gallery lane pushes its font and icon fixes early (first two items); the others merge `origin/ci/pb1-gallery`
when they need them (a merge, never a rebase). A cross-lane fix you need: put it in your record as a request to the
owning lane and work around it.
