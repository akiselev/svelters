# polish-b4 coordinator handoff (replace, don't append)

Written 2026-10-28 ~15:35 UTC, updated ~16:35: ROUND 4 IS DONE (all lanes merged, plus lane rows f611e13; cadatomic's single pin move is the MAL coordinator's next step) by the gallery/polish coordinator (cloud session `session_01HBbGRgGENFKUjsqyGryCSr`),
because the account may hit its usage limit. A fresh coordinator resumes from this file, the lane records and git.

## The work so far (user, 2026-10-27/28: polish every component in svelters and cadatomic)

- gallery-b1 (GAL1–GAL3), polish-b1 (POL1-*), polish-b2 (POL2-*), polish-b3 (POL3-*): all merged; rows in `STATUS.md`.
- quire: q3-editor landed on quire master `f220847` (register_as, reveal under the horizontal bar, TOML, ligature-safe
  fast path, `EditorTheme::font_features`). svelters and cadatomic both pin `f220847`.
- cadatomic: gal1–gal4 lanes merged by the MAL coordinator (`session_016CTvCHJyR4hx83LMP4XD5y`); gal4 is in
  `claude/nx3-consolidation` (reaches cadatomic master at nx3's gate).

## Round 4 (polish-b4), user decisions 2026-10-28

- "lets add direction support"; "finish up the remaining small items: popups, disabled progress bar, invalid quantity
  field, and the jetbrains mono code editor with optional ligatures"; and (relayed by MAL) "lets try the WCAG target":
  default-density rows at least 24 px as a trial, compact stays an opt-in below.
- Coordinator decision (inside the user's direction decision, as the web): a box's direction is the paragraph base for
  its text; `dir="auto"` is the opt-in for content-derived direction. Done in dir-widgets.

| Lane | Branch | Session | State at writing |
|---|---|---|---|
| dir-core | `ci/pb4-dir-core` | `session_01HBbGRd4ExpTviuYhWdHb7a` | merged to master `728647b` |
| dir-widgets | `ci/pb4-dir-widgets` | `session_01HBbGRfqwNB8mNYQfNagHSt` | merged to master `f8eb805` |
| small | `ci/pb4-small` | `session_01HBbGRc73MtF1Nmx6rcznv1` | merged to master `24f54bc`; was: popup anchor-to, disabled progress parts, table rows follow row-h, 24 px trial (density.json, DESIGN.md §2.6, snapshot re-bless), gallery host fonts registered before its bundled mono (gal4 S-L1), popup -start/-end logical in rtl and an rtl context menu opened without a pointer at the owner's top-right (dir-widgets follow-ups); record `POL4-SMALL.md` |
| cadatomic gal4 | `claude/gal4-editor` | `session_01HBbGRccX98XHVfWfkofZTf` | done `eb8d0998`, merged into nx3 by MAL |

## How to finish the round

1. For each running lane: when it reports done, check CI on its last code sha (all six workflows; a red that is only
   the post-job rust-cache save is not a failure: check the job's steps), merge `--no-ff` into master after fetching,
   run `cargo test -p svelters-style -p skelter-core -p skelter-widgets -p svelters-skelter -p skelter-gallery -p
   skelter-gallery-app` and `skelter-gallery check` on the merged tree, add a `STATUS.md` row (`POL4-*`), push.
2. Send each merge sha to the MAL coordinator; it moves cadatomic's svelters pin once at the end of the round and
   re-blesses cadatomic's snapshots (the 24 px rows and rtl text move them).
3. Render rtl vs ltr images (tree, menu with submenu, tabs, slider, dialog) and 24 px before/after for the user.
4. Open after round 4: cadatomic `dir="auto"` on user-content fields (in MAL's nx3 cleanup plan); the app's own
   direction setting (a cadatomic decision, not made); the 24 px trial's verdict (the user's).
