# POL3-FINISH: the leftovers, the layout gaps and the full review (batch polish-b3, lane finish)

**Branch:** `ci/pb3-finish` (base `master` `242d688`).
**Brief:** `plans/cloud/polish-b3/BRIEF.md`, `LANE-finish.md`, plus two requests from cadatomic's gal2 lane
(`docs/notes/gal2-requests/components.md`, "To svelters": S1, S2), given to this lane by the coordinator.
**Status:** done; see CI.

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
- `r/*-b.png` → `r/*-a.png` (`*-a.diff.png`) for the review fixes below (befores rendered by the base binary with
  master's `base.css`, `--base-css`; the gallery reads `base.css` from the tree, so a CSS before needs that flag).

## The quire pin (the coordinator's request, 2026-10-28)

- `ed74967` (alone): skelter_quire, quire-core (root manifest) and skelter-widgets' quire-text at quire `388670c`;
  `Cargo.lock` has one quire-text; the `[patch."https://github.com/akiselev/svelters"]` lines kept.
  `plans/research/tc-eval/` keeps its own pin (a separate bench workspace).
- `ffe5a1f`: the gallery app registers its source pane with `CodeEditorKind::register_as(reg, "gallery-source")` and
  mounts it under that tag (`ui.rs` `GALLERY_SOURCE`, its `buffer` prop id under the new tag); `SHADOWED` and its
  fallback are gone; `tests/hooks.rs` `g2_a_hosts_code_editor_and_the_source_pane_work_in_one_app` (a host's
  `code-editor` keeps its tag and mounts, the pane is `gallery-source` and shows the opened variant's source);
  `docs/skelter/gallery.md` updated.

## The full review (item 3)

`skelter-gallery sheet --all` at zoom 1 and 2, four themes, every story's states (94 sheets, 47 stories), looked at
by four read-only reviewers (one per family group) and by me for every fix. Fixed here (each seen before and after):

| # | Story, state, theme | Finding | Fix (file) |
|---|---|---|---|
| F-1 | `context-menu/open`, `combo-box/open`, dark and light | the menu outline and separator were `widget.border` (`#313131` on `#202020`, about 1.05:1) | a role `border-menu` from VS Code's `menu.border` (`#454545` dark, `#CECECE` light; a theme without it takes `widget.border`); `menu-border-color` follows it (`theme/mod.rs`). Popups and pickers' drops share the token |
| F-2 | `combo-box/open`, all | option text 3 pt right of the field's text | option `padding-left` 5 (list padding 2 + 5 = field border 1 + padding 6) (`views/select.rs`) |
| F-3 | `modal/*`, all | heading at 12 pt, body at 24 pt (the stories padded content the modal pads already) | the stories' content boxes lose `padding={12}` (`gallery/stories/dialogs/modal/`) |
| F-4 | `dialog-buttons/order`, all | no row showed a primary button | OK is `default` in the story |
| F-5 | `sides/crowded`, all | the end button clipped off the box: a wrapping label's min-content width was its whole line | a wrapping label (`wrap-mode="wrap"`, no `elide`) measures min-content at width 0 (its longest word), so a row shrinks and wraps it (`controls/label.rs`) |
| F-6 | `toggle-button/*` checked, active | a pressed on toggle showed the off look | a `CHECKED|PRESSED` rule keeps `toggle-on-bg` (`controls/toggle_button.rs`; its test updated) |
| F-7 | `slider/*` disabled | value text dimmed twice (`disabled-fg` and the host's .4), 1.6:1 | the value part's disabled colour dropped (`controls/slider.rs`) |
| F-8 | `icon/glyph`, `icon/registry` disabled; `icon/button` selected | a disabled icon did not dim; a selected icon button looked unselected | `icon:disabled` in R9's opacity list; `icon:selected` with `button:selected` (`base.css`) |
| F-9 | `table/*` disabled; `scrollbar` disabled | no dimming | `table:disabled`, `scrollbar:disabled` in R9's list (`base.css`) |
| F-10 | `tree-view/*` disabled | twisties kept full colour | `tree-view:disabled::part(twisty)` dims (`base.css`) |

Open, with a recommendation each (`skelter-gallery shot …` shows it; run from the checkout, `WGPU_BACKEND=vulkan`):

| # | Story | Finding | Recommendation | Shot |
|---|---|---|---|---|
| O-1 | `dialog/confirm` | Cancel (first tab stop) takes the default look: auto-default moves `:default` to the focused button (core `input/focus.rs` `initial_in`, `input/scope.rs` `effective_default`) | core (overlays lane's crate): a dialog scope's initial focus prefers its usable `default` button | `shot dialog/confirm --zoom 2` |
| O-2 | rtl variants of `dialog`, `modal`, `menu-bar`, `combo-box`, `checkbox`, `radio-button`, `switch`, `collapsing-header`, `frame`, `progress-bar`, `rich-text`, `tree-view`, `table`, `tab-view` | nothing mirrors: skelter has no inline direction (R-18, POL1-VIEWS Q2) | architecture (user): an inherited `direction` that mirrors flex rows, start/end, part order and fill direction; Q1 | `shot checkbox/rtl --zoom 2` |
| O-3 | `menu-bar/titles` focus-visible; hover in HC; forced `:disabled` | no focus ring on the current title; HC hover is a fill, not VS Code's dashed outline; a host's disabled state does not reach the titles | overlays lane (`views/menu.rs` `TITLE_UA`): request | `shot menu-bar/titles --state focus-visible --theme hc-dark --zoom 2` |
| O-4 | `dialog-buttons/order` | Help sits in the right-aligned group; Qt puts a stretch after Help/Reset (Mac, GNOME, KDE) and Help last on Windows | needs the grid's columns to know its children when it builds its style (`Kind::layout_style` sees none): a spacer column after the help/reset group | `shot dialog-buttons/order --theme light-modern --zoom 2` |
| O-5 | all four pickers in HC and every density | the pickers paint Dark/Light Modern colours and default metrics (`pickers/kinds.rs` `look(dark)`); also: today and the cursor are near-identical rings, the drop buttons have no hover/pressed look, the colour editor's disabled parts stay full strength, white-only handles vanish on light colours, an empty date picker shows today | overlays lane (`src/pickers/`): request | `shot calendar/value --state focus-visible --theme hc-dark --zoom 2` |
| O-6 | `progress-bar/*` disabled in HC | the label over the fill is 1.4–1.6:1 (the host's .4 fades the label too) | dim the track and fill parts only, the label in `disabled-fg` (a small design call on R9's "dim the whole leaf") | `sheet progress-bar --variants percentage --states disabled --themes hc-dark,hc-light --zooms 2` |
| O-7 | `hyperlink/long-text` | the link runs past its box, focus ring cut | elide like `label` (`controls/hyperlink.rs`) | `shot hyperlink/long-text --state focus-visible --zoom 2` |
| O-8 | `checkbox/rtl`, `radio-button/rtl`, `text-edit/rtl`, `tooltip` Arabic | Arabic text makes controls taller (32 vs 22; a field 38 vs 26): the line box takes the fallback font's metrics | skelter-text: line boxes from the primary font's metrics or `line-height` | `inspect radio-button/rtl '#subject'` |
| O-9 | `radio-button/*` vs `checkbox/*` | a radio is 18 tall, a checkbox 22: mixed forms leave the 22 pt row grid | both at least `row-h` | `sheet radio-button --variants group --zooms 2` |
| O-10 | `collapsing-header` HC hover; `end-slot` | HC hover is a fill (the kind paints no outline, so a CSS outline rule is not enough); the end slot's button covers the header's ring | the kind paints `outline` (as buttons do); inset the end slot 2 pt | `shot collapsing-header/end-slot --state focus-visible --zoom 2` |
| O-11 | `table`, `tab-view`, `tree-view`, `virtual-list` focus-visible | the forced state shows nothing (table and tabs read focus from events; lists style `:focus-within`), so their active selection and rings are not reviewed | gallery: a forced `focus-visible` also sets `:focus`/`:focus-within`; the kinds read the node's focus bits when recording | `shot table/sorted-selected --state focus-visible --zoom 2` |
| O-12 | `virtual-list`, `tree-view` | rows' content top-aligned (3–4 pt above centre), list rows have no inline padding, tree twistie 2 pt low | centre the realized row child; 8 pt row padding; centre the chevron | `shot virtual-list/plain --theme hc-light --zoom 2` |
| O-13 | `table/*` | rows 20 pt (not `row-h` 22), no scroll bars, `10x10mm` drawn `10×10mm` (Inter's `calt`) | `row-height` default from `row-h`; the lists' bars; `calt` off for data text (needs `font-feature-settings` in skelter-text) | `shot table/overflow --zoom 2` |
| O-14 | `text-edit/invalid` focus; `text-edit/long-text`; `password/value` | an invalid field shows no focus; long text runs to the border (clip at the padding box); `●` masks touch and sit high | a focus outline on `:invalid:focus-visible` (design R4); clip single-line fields to the content box; mask `•` | `shot password/value --args 'text=correct horse' --zoom 2` |
| O-15 | `tooltip` long text | wider than a 320 pt window, clipped | overlays lane: wrap at `min(MAX_WIDTH, bounds.w - 2·margin)` | `shot tooltip/element --zoom 1` |
| O-16 | `split-view`, `panel`, `color-editor` hover/focus-visible | identical to normal: the sash and the editor's rings come from the kind's own state, not the forced bits | gallery or kind: honour the forced bits | `sheet split-view --states hover,focus-visible --zooms 2` |
| O-17 | `switch` hc-dark | on and off share a black track | fill the on track with `contrastActiveBorder` | `sheet switch --themes hc-dark --zooms 2` |

### Requests to the overlays lane

O-1 (core scope focus), O-3 (`menu.rs` titles), O-5 (pickers' theme, density and states), O-15 (tooltip width), and
the tab strip's 27 pt height against `tab-h` 35 and a disabled tab-view's full-accent marker (`tabs/`). F-1 changed
the token their surfaces share (`menu-border-color`); nothing in their files changed.

### Story table

| Story | Look |
|---|---|
| button | good |
| checkbox | open O-2, O-8 |
| collapsing-header | open O-10 |
| frame | good (RTL O-2) |
| hyperlink | open O-7 |
| icon | fixed here (F-8) |
| image | good |
| label | good |
| progress-bar | open O-6 (RTL O-2) |
| radio-button | open O-9 (RTL O-2, O-8) |
| rich-text | good (RTL O-2) |
| separator | good |
| slider | fixed here (F-7) |
| spinner | good |
| switch | open O-17 |
| toggle-button | fixed here (F-6) |
| table | fixed here (F-9); open O-11, O-13 |
| tree-view | fixed here (F-10); open O-11, O-12 (RTL O-2) |
| virtual-list | open O-11, O-12 |
| text-edit | open O-14 (RTL O-8) |
| multiline | fixed here (Q3 thumb drag); good (RTL O-8) |
| password | open O-14 |
| scroll-area | good |
| scrollbar | fixed here (R4, F-9) |
| status-bar | good (R-17 seen) |
| tooltip | open O-15 |
| tab-view | fixed here (R-14); open O-11, overlays (overflow, RTL) |
| dialog | open O-1 (RTL O-2) |
| dialog-buttons | fixed here (F-4); open O-4 |
| inner-window | good |
| modal | fixed here (F-3) (RTL O-2) |
| popup | good (placement: overlays) |
| toast | good |
| combo-box | fixed here (F-1, F-2) (RTL O-2) |
| context-menu | fixed here (F-1) |
| menu-bar | open O-3 (RTL O-2) |
| box | good |
| grid | good |
| panel | good at rest; open O-16 |
| sides | fixed here (F-5) |
| split-view | good at rest; open O-16 |
| stack | good |
| window | good |
| calendar | open O-5 |
| color-editor | open O-5, O-16 |
| color-picker | open O-5 |
| date-picker | open O-5 |

### R-13..R-20

R-13 fixed (R4). R-14 fixed. R-17 seen fixed (earlier work). R-18 open (O-2, architecture). R-15 (tab overflow),
R-16 (pickers' clock), R-19 (popup placement), R-20 (picker `open`) are the overlays lane's scope this batch.

## Questions (for the coordinator)

- **Q1 (architecture, user): inline direction.** Every rtl variant shows LTR layout with right-aligned text (O-2).
  Recommend an inherited `direction` property (CSS `direction`, set by `dir` on any element) read by flex rows
  (`row` mirrors), `start`/`end` alignment, the kinds' part order (indicators, twisties, chevrons) and fills.
- **Q2 (design): R9 for leaves with text over a fill** (O-6): dim parts, not the whole leaf. Recommend yes.

## Tests (filtered, on the code of the last code commit)

- `cargo test -p skelter-core --lib -- tests::hooks tests::style::flex_start` (36 + 1: S1, Q2).
- `cargo test -p skelter-widgets --lib -- views::scroll::tests::a_hosted_scrollbar views::scroll::tests::scrollbar_maps text::kind controls::label controls::slider controls::toggle_button views::select theme:: controls::icon` (all pass).
- `cargo test -p skelter-text` 42 pass (R-14's test fails without the fix).
- `cargo test -p svelters-style --lib` 28 pass.
- `cargo test -p svelters-skelter --test widgets_all --test spec --test tier1` (5, 6, 14 pass); `SVELTERS_UPDATE=1 … --test spec` regenerated `skelter.json`; `gen_props.py`, `gen_utilities.py`, `gen_semantics.py`, `gen_observe_outputs.py`, `coverage.py` `--check` clean.
- `SKELTER_GPU_TESTS=1 cargo test -p skelter-gallery-app --test hooks` (3 pass).
- Not run locally: workspace tests, pixel snapshot tests, conformance (CI is the gate).

## CI

All six workflows green on `140ad0b` (the last code sha; `origin/master` `242d688` is its base, nothing newer to
merge): js 40632409604, linux-gtk 40632409611, linux-qt 40632409557, macos-appkit 40632409550, skelter 40632409637,
windows-winui 40632409527. No pixel snapshot test moved, so none was re-blessed. Earlier pushes' runs were cancelled
by later pushes (the workflows cancel in progress). Commits after `140ad0b` change this record only (CI not rerun for
them).

## Handoff

Done. Open: Q1, Q2 and O-1..O-17 above (the overlays lane's are listed under its requests).
