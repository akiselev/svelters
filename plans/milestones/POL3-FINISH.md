# POL3-FINISH: the leftovers, the layout gaps and the full review (batch polish-b3, lane finish)

**Branch:** `ci/pb3-finish` (base `master` `242d688`).
**Brief:** `plans/cloud/polish-b3/BRIEF.md`, `LANE-finish.md`, plus two requests from cadatomic's gal2 lane
(`docs/notes/gal2-requests/components.md`, "To svelters": S1, S2), given to this lane by the coordinator.
**Status:** in progress.

## Scope and what changed

| Item | What | Fix (files) | Test |
|---|---|---|---|
| R4 / R-13 | the standalone `scrollbar` drew nothing | `Scrollbar::measure` (`BAR_WIDTH` across; along: the CSS length or stretch, else the room, else two thumbs) and `prepare` (lays the thumb out at its size) (`views/scroll.rs`) | `views::scroll::tests::a_hosted_scrollbar_measures_and_prepares_itself` |
| POL2-LAYOUT Q2 | `align-self`/`align-items: flex-start` (and `flex-end`) did nothing | the cause was not core: skelter's style table lists only `start`/`end` for the align props, so svelters-style's enum parser refused `flex-start` and the declaration was dropped. The parser now takes `flex-start`/`flex-end` as `start`/`end` for `align-*`/`justify-*` props whose table lists those (`svelters-style/src/values.rs`). No core change | `skelter-core` `tests::style::flex_start_and_flex_end_align_as_start_and_end` |
| POL2-LAYOUT Q3 | the multi-line field's scroll thumb took no press | `CoreBars::pointer`/`repeat` (an adapter for kinds on skelter-core's `Kind`, returning a `BarPointer`: taken, capture, new offset, repaint, wake); `TextEdit::event` routes presses, moves, releases, cancel, leave and the repeat timer to the bars before the text (`views/scroll.rs`, `text/kind.rs`) | `text::kind::tests::a_multi_line_fields_thumb_drags_and_its_track_pages` (thumb drag scrolls and leaves the caret; a track press pages) |
| KNOWN (POL2-REGISTER Q1) | 24 pre-existing members without a surface name | rows from each kind's own core spec and doc (`surface/tier0.rs`, `tier1.rs`; `skelter.json` regenerated): `hyperlink`/`label` `open-url`, `label` `clickable`/`link-clicked`, the runs' `href` (`span`, `b`, `i`, `code`), `scroll-area` `overscroll-behavior`/`scrollend`, `virtual-list` `pool-limit`/`current`, `tree-view` `selected-ids`/`expanded-ids` (native state)/`pool-limit`/`select`/`selectionchange`/`current`, `context-menu` `activate`/`toggled`/`hover`/`open`/`close`, `combo-box` `toggle`. **Stays:** `<label> prop runs`: the surface has no list-of-records type, and Svelte gives a label its runs as inline children (the `runs` slot) | `widgets_all` KNOWN is one entry |
| R-14 | a long tab title elided right after a hyphen | skelter-text wrapped the text at the width first and put `…` after line 1; a one-line ellipsis (`max_lines == 1`) now breaks without a width and is cut where the width runs out, still aligned in the width (`skelter-text/src/system.rs`) | `skelter-text` `tests::layout::a_one_line_ellipsis_cuts_at_the_width_not_at_a_break` (fails without the fix) |
| R-17 | status-bar overflow overlapped | already fixed on master (POL2-LAYOUT eliding): the sections keep their text, the message elides between them. Seen (`status-bar/overflow`, zoom 2) | — |
| gal2 S1 | `PropCx::set_state(bits, on)` | `PropCx` collects state changes and the document applies them after the kind returns, as `PrepareCx::set_state` does; dropped when the kind rejects the write; also in `listen_changed` and the missed-props replay (`skelter-core/src/cx.rs`, `document.rs`) | `skelter-core` `tests::hooks::a_prop_write_sets_state_from_prop_changed` |
| gal2 S2 | JetBrains Mono garbled `<`, `>`, `=`, `}` in code | the glyphs were intact; the subset kept the font's code ligatures (`calt`): `<=` drew as ≤, `!=` as ≠, `</` as a ligature. `subset.py` drops `calt`, `liga`, `dlig` for JetBrains Mono; the subset rebuilt from upstream `JetBrainsMono-Regular.ttf` 2.304 (the same revision) with fonttools 4.55.0 (`skelter-gallery/fonts/`) | looked at (below) |

Core edits (skelter-core is the overlays lane's): only S1's `PropCx` (`cx.rs`, three `PropCx` sites in
`document.rs`) and a test each in `tests/hooks.rs` and `tests/style.rs`; no layout code changed (Q2's cause was in
svelters-style).

### Before/after (this container, not committed)

Under `/tmp/claude-0/-home-user-svelters/5665c853-7f1c-5470-9bbb-1a9a890c0530/scratchpad/img/`:
- `sb-before.png` → `sb-after.png` (`sb-after.diff.png`): `scrollbar` empty → a 10 pt thumb that follows `value`, both
  orientations.
- `mono-before.png` → `mono-after.png`: `<= => != == === -> </ />` drew as ligatures → as typed.
- `tabs-closable.png` → `tabs-closable-after.png`: `usb-c-power-delivery-…` → `usb-c-power-delivery-controll…`.
