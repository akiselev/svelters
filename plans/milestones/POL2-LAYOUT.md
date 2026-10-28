# POL2-LAYOUT: skelter-core and svelters-skelter layout gaps (polish-b2, lane layout)

Branch `ci/pb2-layout`, base `master` `993bd63` (the batch brief on `e01ce9b`). Brief: `plans/cloud/polish-b2/BRIEF.md`,
`LANE-layout.md`. Status: done (1–5 built, tested and seen; CI below).

## 1. Scroll bars on a fixed foreground layer

**Finding.** virtual-list and tree-view drew no scroll bar and the multi-line text field none (POL1-VIEWS Q1, POL1-TEXT
Q2): they scroll themselves (`self_scroll`), and their only fixed part (`RecordCx::fixed`) paints *below* the scroll
layer. A scroll-area's bars were in its own list, also below its scroll layer: the content painted over them.

**Design note.** Core gains a per-node **foreground**: `Kind::record_foreground(&self, cx)` records into its own list
(key `fore_key` = the node id with bits 62 and 63). On a scroll container (CSS `overflow` or `self_scroll`) the rebuild
walk places it in a layer of its own right above the node's scroll layer, in the node's border-box space (outside the
scroll transform), and that layer takes what paints after the node in its parent's layer, exactly as a content layer's
continuation does (A26), so later siblings stay above it. On any other node it is one more item after the node's list.
A scroll offset change on a node with a foreground records **the foreground alone** (`Document::fore_dirty`; no
`prepare`, no `record`), so a self-scrolling field still never re-records its text on a scroll;
`EventCx::request_foreground` does the same for a fade. Why no existing mechanism: `fixed` is below the scroll layer by
design, `overlay` is a popup surface (the window's overlay stack, unclipped by ancestors, takes input first), and the
app/tip overlays are per window. Rejected: core drawing bars for every scroll node from tokens (a look in core, and the
table/text kinds need their own thumb drag and fade anyway). Not a new phase and no commit-format change (one more list
key, one more `Content` layer per scrolled node with a foreground). Per-frame cost: none for nodes without a
foreground; a scroll of one with a foreground sends one small list instead of none; one extra layer per such node
(measurement on a real GPU: "cloud, not a gate", not done).

Widgets: the shim `Kind` and the host bridge gain `record_foreground(cx, scroll_geometry)` (the prepare's dry recording
includes it, so its tokens are tracked). `views::scroll::CoreBars` is a `Scroller` synced from core's geometry with the
scroll area's look (`THUMB_UA`, `scrollbarSlider.*`), overlay fade (DESIGN's hold and fade, no fade with reduced motion:
`bar_policy`), hover, thumb drag and track press (offset back through `cx.scroll_to`). Users: `scroll-area` (bars moved
from `record` to the foreground), `virtual-list` and `tree-view` (own `CoreBars`; events reach the bars first, so a press
on a thumb selects no row), multi-line `text-edit` (`CoreBars::note`/`record_list`; bars only when `multiline`, as VS
Code's input box has none). `table` already drew its bars last in its own list (it scrolls by re-recording and folds its
children), so it needed no foreground.

**Seen** (`--diff`, four themes): `virtual-list` (`target/gallery/after/virtual-list.png`): thumbs on `plain` and
`rich-rows`; `tree-view/overflow` (new variant, 25 rows: `target/gallery/after/tree-overflow.png`); `multiline/overflow`
(`target/gallery/after/multiline.png`); `scroll-area` (`target/gallery/after/scroll-area.diff.png`): the thumbs now over
the text. Trees that fit show no bar (correct).

**Tests.** skelter-core `tests::hooks::a_foreground_paints_above_the_scrolled_content_and_follows_the_offset_alone`
(layer above the scroll layer, node space, later siblings above it, a scroll sends only the foreground, removal),
`a_foreground_off_a_scroll_container_paints_after_its_list_below_its_children`; skelter-widgets
`views::virtual_list::tests::draws_its_bars_in_the_foreground_and_drags_the_thumb`,
`text::kind::tests::a_multi_line_field_draws_its_bar_over_the_text_and_moves_it_alone`,
`views::scroll::tests::records_the_thumb_and_fades_overlay_bars` (updated: the bars are the foreground).

Not built: thumb drag on the multi-line field (its bars draw and fade; a press there goes to the text) — follow-up.

## 2. Overflow states

**Design note.** `:state(sv.overflow-top|bottom|left|right)` resolve on every skelter document: core's `StateNames`
resolver answers svelters' four names itself (`skelter_core::OVERFLOW_*`, the top four provider bits 60–63; a host
allocates its own from bit 32 to 59) and asks the host's provider for the rest, so base.css's existing rules now parse
(before, `:state()` without a host provider was a parse error). Core sets the bits on every scroll container from its
geometry when its property nodes refresh (`refresh_overflow_states`, after the offset or extent changes) and clears them
when it stops being one; they are pointer-only for AccessKit (no accessibility churn on scroll). A kind that scrolls
without core sets them itself: `PrepareCx::set_state` (new, applied after the prepare as its native writes are) — the
table does so from its body offset. The edge shadows (base.css's inset `box-shadow`) paint in the foreground, over the
content: `scroll-area`, `virtual-list`, `tree-view` (whose rows' box now leaves out its inset shadows,
`paint_box_layers(.., false)`), and the table over its body under its bars. Rejected: per-kind state setting in
skelter-widgets for every scroller (core already knows the geometry). Also fixed on the way: svelters-skelter's
`SkelterDocument::needs_frame` asked only the backend revision, which the frame itself stamps, so a state set in a
frame's paint walk never got its restyle frame; it now also asks `Document::needs_frame` (`app.rs`).

**Seen.** `skelter-gallery shot scroll-area/scrolled --zoom 2 --theme light-modern`
(`target/gallery/after/scrolled-x2.png`): the top edge shadow over the rows. No story scrolls a list or a table, so their
shadows are covered by tests only.

**Tests.** skelter-core `tests::style::a_scroll_container_has_the_overflow_states_of_its_offset` (bits per offset, a host
provider beside them, the rule matching, cleared when it stops scrolling); skelter-widgets
`table::kinds::tests::the_body_offset_sets_the_overflow_states`.

## 3. Chrome eliding

**Design note.** `chrome_min_size` became the node's CSS `min-width`, so a header was never narrower than its caption.
Core gains `Kind::chrome_widths(cx) -> (min-content, max-content)`: computed at style time (`LayoutBox::chrome_w`, a
change relayouts), and the layout tree widens an auto-width container's result to `max-content` when asked for it, to
the available width at most otherwise, never below `min-content` (a `PerformLayout` that widens re-runs with the width
known); a node with chrome widths lays out as a container even without children. No CSS minimum is set. The collapsing
header reports `[padding + twistie, padding + twistie + gap + caption]` and its `chrome_min_size` is now the twistie and
padding only; its paint already elides. Frame captions do not use this path (their caption row is `chrome_padding`
height only), so nothing changed there. Rejected: TEXT Q1's "chrome content size" as padding (padding cannot shrink).
Cost: one `Option` check per container layout.

**Seen.** `collapsing-header/long-text`, before `target/gallery/before/ch-long.png` (the caption ran out of its 300 pt
box) → after `target/gallery/after/ch-long.png` (ends in "…" inside it), four themes; the other variants unchanged.

**Tests.** skelter-core `tests::hooks::a_containers_chrome_widens_it_where_there_is_room_and_lets_it_shrink` (room,
narrower parent, below min, a row's flex shrink); skelter-widgets
`controls::collapsing::tests::the_caption_widens_the_header_without_setting_its_minimum`.

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

## R2 (coordinator, from register): document timers in svelters' app

**Finding.** `SkelterDocument` never ran skelter-core's document timers: no `run_timers`, no `next_wakeup`, and
`needs_frame` ignored the document; `SveltersApp::sync` also replaced the timer the App armed with the JS host's
deadline alone. Kind timers (`title` tips, toast timeouts, status messages, overlay bar fades) never fired in a svelters
app, cadatomic included.

**Fix** (`svelters-skelter/src/app.rs`, commit `2053fde`, on its own): `frame` runs `run_timers(target)` first and queues
their JS events; `next_wakeup` is `Document::next_timer`; `needs_frame` also asks `Document::needs_frame` (item 2) and
is true when a timer is due on the runner's clock (`SkelterDocument::with_clock`/`set_clock`; `SveltersApp::init` gives it
the platform's); `sync` arms the earlier of the host's deadline and the document's next timer.

**Test.** svelters-skelter `tests/app_timers.rs`: a kind's `wake_at` makes `next_wakeup` report it, no frame before it is
due, a frame when it is, and the frame runs it (virtual clock). Not a toast test: skelter's `toast-host` has no Svelte
surface or method on this base (its Rust `ToastQueue` needs a first tick), so register adds one with its `show`.

## Everything else unchanged

`skelter-gallery sheet --all --states normal` with the master binary and with this branch, compared per sheet: only
collapsing-header (long-text), multiline (overflow), scroll-area, tree-view (the new variant), virtual-list, window and
combo-box changed. combo-box's change is item 5's stale-layer bug: in the master run, the window story's "cadatomic"
text stayed on screen in the combo-box cells after it. No pixel snapshot test was moved or re-blessed.

## Questions (recommendations; for the coordinator)

- **Q1 (`<window>` default padding).** The surface documents `padding` default 8 (egui's). Applying it moved every
  existing window's content (the gallery's harness window, any app root) by 8 pt, so an unwritten `padding` is 0 here.
  Recommend changing the surface default to 0 (register lane's `surface/tier0.rs`, then its generators), or keeping 8
  and having cadatomic and the gallery write `padding={0}`; I'd take 0 (app windows are edge to edge).
- **Q2 (`align-items: flex-start`).** skelter-core's cascade maps `flex-start`/`flex-end` (`align`), but the value
  never reaches it in a test sheet: `align-self: flex-start` left the node stretched while `start` worked (seen while
  writing the item 3 test). Likely svelters-style's keyword table lacks `flex-start` for the align properties; recommend
  a check there (not this lane's crate).
- **Q3 (multi-line thumb drag).** The field's bars draw and fade but take no press; recommend routing a press on a bar
  to `CoreBars` in `TextEdit::event` before the field (the core `Event::Pointer*` variants need a small adapter).

## Requests to the register lane

- `svelters-skelter/src/kinds.rs`: two one-line hunks (the placeholder registration takes
  `crate::window::factory(t).unwrap_or(placeholder)`); keep both sides at a merge.
- base.css frame block: `frame:disabled::part(caption)` dims (item 4).

## Tests run (filtered; on the merged head)

- `cargo test -p skelter-core`: 186 passed.
- `cargo test -p skelter-widgets --lib -- views::virtual_list views::scroll views::tree text::kind table:: controls::collapsing`: 95 passed.
- `cargo test -p svelters-skelter --test window`: 1 passed; `--test app_timers`: 1 passed.
- `cargo check --all-targets` of skelter-core, skelter-widgets, svelters-skelter, skelter, skelter-test,
  skelter-gallery, skelter-gallery-app: clean.
- `skelter-gallery check`: 32 stories, 138 variants, 0 problems.
- Not run locally: the workspace tests, other crates, pixel snapshot tests, conformance (CI is the gate).

## Handoff

Done. If a snapshot test moves in CI it is from item 1 (scroll-area bars now over the content; lists, trees and
multi-line fields gain a thumb when they overflow) or item 5 (stale layers no longer drawn): re-bless only after looking.
