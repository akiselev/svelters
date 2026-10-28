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
inside `padding` with `gap`, as written, CSS `padding`/`gap` winning as on `box`. An unwritten `padding` is 0, not the
surface's documented default 8 (egui's): applying 8 moved every existing window's content (the gallery's harness window,
any app root such as cadatomic's) by 8 pt; the first build did that and the sheets showed it. Core gains
`NodeView::prop_written` (the written value, no default) for this. See Q1. Alternative rejected:
a full `window` core kind with its own spec (it would have to restate the surface's signals and methods, which the
synthesized spec already carries).

**Files.** `skelter-core/src/{window,document}.rs`; `svelters-skelter/src/window.rs` (new), `lib.rs`, `Cargo.toml`
(`taffy`); `svelters-skelter/src/kinds.rs`: two one-line hunks (register lane's file) — the placeholder registration
takes `crate::window::factory(t).unwrap_or(placeholder)`. At a merge keep both sides. The story's notes drop the known bug.

**Seen.** `target/gallery/before/window.png` → `target/gallery/after/window.png` (`--diff`: `window.diff.png`), all four
themes: no stale text under the toolbar; `padding=12` and `padding=0` rows now differ, children 8 pt apart.

**Tests.** `skelter-core` `tests::commit::a_new_root_removes_what_the_old_root_drew`; `svelters-skelter`
`tests/window.rs` `the_window_lays_its_children_out_inside_padding_and_gap_apart` (padding 12, unwritten = 0, 0, CSS
winning, gap).

## 4. Inherited disabled

**Finding.** A disabled frame left its content enabled (POL1-STATES request); only an unchecked checkable frame
disabled it (through `Kind::children_inert`).

**Design note.** Core already had the path: `refresh_inert` makes a node inert (`:disabled`, no focus, no input, and so
AccessKit's disabled flag, which follows the state) under a parent that is inert or whose kind makes its children inert.
It gains one source: a parent whose own `disabled` prop is set; the `disabled` write refreshes the children
(`skelter-core/src/document.rs`). So any disabled container disables its content (HTML's disabled fieldset, Qt, GTK),
and a disabled frame and an unchecked checkable frame share the path. The look follows: the content's own `:disabled`
rules (R9 opacity on interactive kinds, `disabled-fg` on labels) apply; nothing compounds because containers take no
opacity. Alternative rejected: a `children_inert` on each container kind reading its `disabled` (every kind, and the
placeholders, would need it). Not a new phase or commit change. Per frame cost: none (a write-time walk of the subtree,
as `children_inert` already does).

**Seen.** `skelter-gallery sheet frame --states disabled --themes dark-modern,light-modern`, before
`target/gallery/before/frame-dis2.png` → after `target/gallery/after/frame-dis.png` (`.diff.png`): every disabled frame's
checkboxes, radios and fields dim; layout unchanged; the checkable frames' content dims the same in both.

**Request (base.css frame block, not mine):** a disabled frame's own caption (`frame/caption`, "Export") stays at full
colour; recommend `frame:disabled::part(caption) { color: var(--sv-disabled-fg) }` as `label:disabled`.

**Tests.** `skelter-core` `tests::hooks::a_disabled_container_disables_its_content` (state, focus lost, AccessKit
disabled on a descendant, a late child, re-enabling, a child's own `disabled` holding); the existing
`inert_children_are_disabled` still passes.
