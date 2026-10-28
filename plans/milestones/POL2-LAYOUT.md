# POL2-LAYOUT: skelter-core and svelters-skelter layout gaps (polish-b2, lane layout)

Branch `ci/pb2-layout`, base `master` `993bd63` (the batch brief on `e01ce9b`). Brief: `plans/cloud/polish-b2/BRIEF.md`,
`LANE-layout.md`. Status: in progress.

## 5. The window's chrome

**Finding.** In `window/app` the toolbar drew over the text "cadatomic" (POL1-STATES, POL1-GALLERY), and the window's
`padding`/`gap` had no effect (the two `content` rows looked the same).

**Causes (two).**
1. The "title" under the toolbar was not the window's title: it was the previous cell's text-edit (`content`'s
   `cadatomic` field) still on screen. `Document::attach_window` on a window that already had a root replaced the
   window's `WindowState` after retiring the old tree into the *old* state's pending removals, so the compositor never
   got them: the old tree's layers (the text-edit's scroll layer with its glyphs) stayed drawn. `shot window/app`
   alone was clean; only a sheet (content, then app) showed it.
2. `<window>` was a `Placeholder` (svelters-skelter synthesizes a kind that draws and lays out nothing for every surface
   kind without a widget), so its `padding` and `gap` props were stored and never read.

**Design note.** (1) is a core bug, no new mechanism: `WindowState::take_over(old)` moves the old state's unsent
removals into the new one and adds every layer, root/drag/overlay property node and app/tip run the old state held
(`skelter-core/src/window.rs`, used by `attach_window` when the root changes; removals go out first in the frame, so a
re-used id's `Add` follows its `Remove`). (2) the window keeps its synthesized spec (props, signals, methods unchanged,
wire ids unchanged); only its node value changes: `svelters-skelter/src/window.rs`'s `Window` lays out a flex column
inside `padding` (the surface default, 8) with `gap`, CSS `padding`/`gap` winning as on `box`. Alternative rejected:
a full `window` core kind with its own spec (it would have to restate the surface's signals and methods, which the
synthesized spec already carries).

**Files.** `skelter-core/src/{window,document}.rs`; `svelters-skelter/src/window.rs` (new), `lib.rs`, `Cargo.toml`
(`taffy`); `svelters-skelter/src/kinds.rs`: two one-line hunks (register lane's file) — the placeholder registration
takes `crate::window::factory(t).unwrap_or(placeholder)`. At a merge keep both sides. The story: `app.svelte` sets
`padding={0}` (an app window's bars reach its edges) and the notes drop the known bug.

**Seen.** `target/gallery/before/window.png` → `target/gallery/after/window.png` (`--diff`: `window.diff.png`), all four
themes: no stale text under the toolbar; `padding=12` and `padding=0` rows now differ, children 8 pt apart.

**Tests.** `skelter-core` `tests::commit::a_new_root_removes_what_the_old_root_drew`; `svelters-skelter`
`tests/window.rs` `the_window_lays_its_children_out_inside_padding_and_gap_apart` (padding 12, the default 8, 0, CSS
winning, gap).
