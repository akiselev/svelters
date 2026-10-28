# POL2-REGISTER: every skelter-widgets kind usable from Svelte (batch polish-b2, lane register)

**Branch:** `ci/pb2-register` (base `master` `e01ce9b`; `origin/master` merged at `a7c920c` (`ff4dfad0`) and again after
the layout lane landed (`f8e2669`)).
**Brief:** `plans/cloud/polish-b2/BRIEF.md`, `LANE-register.md`.
**Status:** done. Every skelter-widgets kind mounts from Svelte and has a story; the open states render; findings
fixed and seen fixed, or recorded below. CI: see "Tests and CI".

## Scope

1. **Registered** from `svelters_skelter::kinds::register_hosted`: `views::register_all` (it replaces the single view
   registrations), `tabs::kinds::register`, `pickers::kinds::register`. Nineteen surface kinds added to
   `surface/tier1.rs`: `split-view`, `panel`, `popup`, `tooltip`, `toast-host`, `status-bar`, `modal`, `inner-window`,
   `dialog-buttons`, `menu-bar`, `menu`, `menu-separator`, `scrollbar`, `tab-view`, `tab`, `calendar`, `date-picker`,
   `color-editor`, `color-picker`. `skelter.json` and the style tables (`gen_props.py`) regenerated.
2. **Open states:** `context-menu` `open`; `toast-host` `show`/`dismiss`/`clear` and `status-bar`
   `show-message`/`clear-message`; the gallery's `hover` state rests the pointer on the subject past the platform's
   tooltip delay, so a `tooltip` shows; a `tooltip` element draws the surface itself.
3. **Stories:** 15 new (`layout/{split-view,panel}`, `dialogs/{popup,modal,inner-window,dialog-buttons}`,
   `views/{status-bar,scrollbar}`, `menus/menu-bar`, `tabs/tab-view`, `pickers/{calendar,date-picker,color-editor,
   color-picker}`, `feedback/toast`) and new variants (`context-menu/open`, `tooltip/element`). `skelter-gallery list`:
   `coverage.missing` is empty; `check`: 47 stories, 170 variants, no problems.
4. **Polish:** each new family in the four themes (zoom 1; HC themes also at zoom 2), below.

## The surface rows: how they were made (decisions)

The rows come from each kind's **own core spec** (members, types, defaults, change classes, native state; signal
argument names from the kinds' `SignalSpec`s, which the host bridge drops), under svelters-egui's **tag, kind name,
base chain and default slot name** (`panes`, `pages`, `items`). A prop's doc is the kind's own comment, else egui's
doc for the same prop. Method params are left to the core spec (as the existing rows do). Nothing was invented: where
egui's member differs, the kind's is used and the difference is in `docs/skelter/surface.md` ("The views, tabs and
pickers"). The transcription script lived in the lane's scratchpad (a dump of the registry's core specs plus
`egui.json`); it is not committed.

The choices the brief asked to record:
- **popup anchoring:** none built. Hosted on skelter-core a popup is a node in its parent's flow (no top layer for
  node children), and its own `anchor` record is shadowed by the common `anchor` (the host leaves common names out of
  a hosted kind's spec). Smallest shape: its own `open`/`placement`/... with no anchor prop; stories place it with
  layout props. See Q2.
- **modal ownership:** a modal covers its **parent** (absolute, inset 0) and centres its children in the dialog box;
  `open` closes it (no `close()` method: egui's `close(result)` needs a result channel the kind lacks).
- **tabs' pages:** `tab-view`'s children are its pages (egui's `pages` slot); a `tab` names its caption.
- **toasts:** `toast-host` takes no `toast` children (skelter has no toast node); egui's `show`/`dismiss`/`clear`
  methods over `ToastHost`'s Rust API.
- **tooltip:** skelter's own element (egui has only the prop).
- `menu-bar` keeps its own `enabled` (the Widget `enabled` maps onto it: its core spec declares one).

## Findings

Each rendered before and after (`skelter-gallery sheet <story> --states normal`, dark-modern and light-modern, then
hc-dark and hc-light at zoom 2) and looked at.

| # | Finding (story) | Cause | Fix (file) | Seen fixed |
|---|---|---|---|---|
| R-1 | (found) `modal` and `toast-host` mounted with "unknown Prop 'anchor'" | their kind ids (FNV of the tag) fall in the host's app-export pack (bits 27..22 all set), so the host sent every lookup to the app schema | `kinds::kind_id_wire` moves a kind wire id out of pack 63 (bit 22 cleared; behaviours too); spec test `no_kind_id_is_in_the_app_pack` (`svelters-skelter`) | yes |
| R-2 | `context-menu/open`: `Paste` (`enabled={false}`) not greyed | Svelte's `enabled` writes the common `disabled`; the folded element records a menu reads carried only the item's own `enabled` | `host::element_records` folds the common `disabled`/`hidden` into `enabled`/`visible` (`skelter-widgets/src/host.rs`) | yes |
| R-3 | `context-menu/open`: the menu runs off the window's right edge (shortcuts, the submenu chevron cut) | nobody gave menus (or the pickers' drops) the window's room: `Menus::bounds` was unbounded | shim `Kind::room` (the window's area in local points, before each prepare; through the layout boxes on a node's first prepare, R3) used by `context-menu`, `menu-bar`, `date-picker`, `color-picker`, `inner-window` | yes (`target/gallery/pb2/cm-open3.png`) |
| R-4 | `tooltip/button` hover: no tooltip | the gallery forced `:hover` without a pointer; and no svelters runner ran the kinds' timers (R2, layout lane, now on master) | the renderer moves the pointer onto the subject and steps the virtual clock past the policy's delay; the app runner (`SkelterDocument`, given the harness's clock) fires the timer (`skelter-gallery/src/render.rs`) | yes |
| R-5 | `modal/*`: dialog box drawn tiny in the middle, content at the node's origin, heading over the content, backdrop only over the content's height | the hosted kind sized the box from a `content` size nothing set and placed no children | `modal` covers its parent, centres its content with taffy margins, draws the box around the content's laid-out box (`Kind::children_laid`, R3) and cuts a long heading (`views/modal.rs`) | yes (`a-modal.png`, `a-modal-long.png`) |
| R-6 | `dialog-buttons/*`: a full-width column, no platform order | the hosted kind had no layout (the egui host did it) | a one-row grid, end-aligned, 8 pt gaps; each button in its column by the build OS's order (`ChildCx::children` gives the children's `dialog-role`, default, cancel) (`views/dialog_buttons.rs`, `host.rs`, `shim.rs`) | yes (`a-dialog-buttons.png`) |
| R-7 | `status-bar/*`: sections stacked vertically, the message over them | as R-6 | `base.css` row at `--sv-statusbar-h`; temporary sections hide while a message shows, the first end section is pushed to the end, the message runs between the start and end sections (`views/status_bar.rs`, `theme/base.css`) | yes (`a-status-bar.png`) |
| R-8 | `inner-window/*`: `x`/`y` ignored, content offset by the frame twice | the node was never laid out at its frame; `child_layout` used window-space coordinates | `layout_style` places the node at its frame (absolute; `x`/`y` now layout-class); content just under the title bar (`views/inner_window.rs`; its test asserted the old offset) | yes (`a-inner-window.png`) |
| R-9 | `color-editor/alpha`: `#2e86c180` shows `#2e86c1`, the alpha handle at the end | `value` was normalised under the alpha mode at write time; Svelte writes `value` before `alpha` | the editor's and the picker's `sync` re-derive the value from the prop (`pickers/kinds.rs`; test) | yes |
| R-10 | `color-editor/*`: channel boxes crowd label into value (`R192`) | a 200 pt body leaves ~39 pt per box | body 232 pt (`pickers/color_editor.rs` `SQUARE`) | yes (`a-color-editor.png`) |
| R-11 | (found) `tab-view` hc-dark: the selected tab has no marker (reads inverted) | the indicator used `accent` (`button.background`, black in HC Dark) | `tab-marker` (`tab.activeBorderTop`, HC Dark's contrast border) (`tabs/kinds.rs`) | yes (`a-tab-view.png`) |
| R-12 | `panel/*`: the panel and the content area the same colour | the story, not the kind: `panel-bg` is chrome (`bg`), as the cell is | the story's content area is `var(--sv-surface)` | yes |
| R-13 | `scrollbar/*`: draws nothing | the standalone `Scrollbar` has no `measure` (zero size) and no `prepare` laying its thumb out at its size | not fixed: `views/scroll.rs` is the layout lane's. Request R4 | — |
| R-14 | `tab-view/closable`: a long title elided right after a hyphen, far shorter than its tab | the elide seems to cut at the last line-break opportunity, not the last grapheme that fits | not fixed (text code, layout lane). `skelter-gallery shot tab-view/closable` | — |
| R-15 | `tab-view/closable`: overflowing tabs are cut at the strip's edge with no sign | the strip scrolls (`TabView::scroll`) but shows no arrows, fade or scroll bar | not fixed: needs a design choice (VS Code: a thin horizontal bar on hover). Q3 | — |
| R-16 | `calendar/*`, `date-picker/empty`: "today" is the real clock (`Date::today_utc`): shots change daily; an empty date picker shows today's date | no clock or "today" input on the pickers | not fixed. Q4 | — |
| R-17 | `status-bar/overflow`: long sections in a narrow bar overlap | labels in the row do not shrink or elide | not fixed (layout lane: overflow and eliding) | — |
| R-18 | `modal/rtl`, `tab-view/rtl`: the heading and captions stay left-aligned | no RTL layout (POL1-VIEWS Q2) | not fixed | — |
| R-19 | `popup/*`: `placement` changes nothing | see Q2 | not fixed | — |
| R-20 | `date-picker`, `color-picker`: the drop cannot be shown open in a shot | the kinds have no `open` prop (their drop state is internal) | not fixed. Q5 | — |

Before/after images (this container, not committed): `target/gallery/pb2/b-modal.png` → `a-modal.png` (R-5),
`b-dialog-buttons.png` → `a-dialog-buttons.png` (R-6), `cm-open.png` → `cm-open3.png` (R-2, R-3),
`b-color-editor.png` → `a-color-editor.png` (R-9, R-10).

## Cross-lane edits (by the coordinator's decision, 2026-10-28)

- **R1 (skelter-core `frame.rs`):** a prepare's `PrepareCx::request_paint` was dropped (`let _ = request;`); the
  frame now re-marks the node after its dirty walk, so the next frame prepares it again. Test:
  `skelter-core` `tests/prepare.rs`.
- **R3 (skelter-core `cx.rs`):** `NodeView::layout_rect`, the node's box in its parent from the last layout (same
  test). Used by `host.rs` for `Kind::children_laid` and the first-prepare window room.
- R1 makes a prepare's request take effect: the text field's one-shot repaint flag now brings one more frame after its
  first. The layout lane's `text::kind::tests::a_multi_line_field_draws_its_bar_over_the_text_and_moves_it_alone`
  scrolled on that frame and saw the whole node re-record; the test now settles the requested frame before it scrolls
  (CI on `9dc01f1` was red on it in the skelter workflow, all three platforms).
- Small edits outside the lane's listed files, all for hosting the new kinds: `skelter-widgets/src/{host.rs,shim.rs}`
  (`Kind::room`, `Kind::children_laid`, `ChildCx::children`, the folded `disabled`/`hidden`),
  `skelter-gallery/src/render.rs` (hover pointer, the runner's virtual clock), and the `ChildCx`
  literal in `controls/{stack,collapsing}.rs` tests.

## Requests

- **R2 (layout lane)** landed with `ci/pb2-layout` on master: the svelters runner runs the document's timers and frames
  what they change. The gallery-side workaround is gone; the renderer gives `SkelterDocument` the harness's virtual
  clock (`set_clock`). Test: `skelter-gallery` `open_states::a_toast_times_out_under_the_app_runner` (a toast shown with
  `show(..., 0.5)` closes after 0.8 s of virtual time).
- **R4 (layout lane, `views/scroll.rs`):** the standalone `scrollbar` needs a `measure` (`BAR_WIDTH` across, stretch
  along) and a `prepare` that calls `Scrollbar::layout(cx.size, ...)`; the story `views/scrollbar` shows it once it does.

From the layout lane (POL2-LAYOUT "Requests to the register lane"), done: the `window` factory in `kinds.rs` (arrived
with the merge, both sides kept), `frame:disabled::part(caption)` dims in base.css (seen: `target/gallery/pb2/frame-dis.png`),
and `<window>`'s `padding` surface default is 0 (the coordinator's decision: 8 moved every existing window).

## Questions (recommendations, for the coordinator)

- **Q1 (pre-existing surface gaps):** members of kinds registered before this lane have no surface name:
  `hyperlink`/`label` `open-url`, `label` `runs`/`clickable`/`link-clicked`, the runs' `href`, `scroll-area`
  `overscroll-behavior`/`scrollend`, `virtual-list` `pool-limit`/`current`, `tree-view`
  `selected-ids`/`expanded-ids`/`pool-limit`/`select`/`selectionchange`/`current`, `context-menu`'s
  `activate`/`toggled`/`hover`/`open`/`close`, `combo-box` `toggle`. `tests/widgets_all.rs` pins them as KNOWN so a
  new gap fails. Recommend adding them (each is the kind's own member) in a small follow-up.
- **Q2 (architecture, user): floating nodes.** `popup` (and a modal that should cover the window, not its parent)
  needs a top layer for node children: skelter-core's overlay stack takes display lists only. Recommend a
  `Kind::top_layer` flag that lays the node out against the window and records it in the overlay stack, with its own
  `anchor` record renamed (`anchor-to`) so it is not shadowed by the common `anchor`.
- **Q3 (design): tab strip overflow.** Recommend VS Code's look: a thin horizontal scroll bar under the captions on
  hover, and the selected tab revealed (it already scrolls into view on a pick).
- **Q4 (pickers' "today"):** recommend the pickers read today from a document service (the virtual clock in tests and
  the gallery, the OS date in an app), and an empty `date-picker` show its placeholder, not today.
- **Q5 (picker drops):** recommend an `open` prop on `date-picker` and `color-picker`, as `combo-box` and
  `context-menu` have, so a story shows the drop.

## Tests and CI

Added or changed (all pass locally, filtered runs on the code of the last commit):
- `cargo test -p svelters-skelter --test widgets_all` (5: every skelter-widgets kind hosted with its members,
  `context-menu` `open`, toast `show`/`dismiss`/`clear`, `show-message`, modal/toast-host resolve) and `--test spec`
  (6, one new: `no_kind_id_is_in_the_app_pack`); `--test tier1` 14 pass.
- `cargo test -p skelter-widgets --lib -- views:: tabs:: pickers:: host::` 202 pass (new: folded `disabled`/`hidden`,
  the window room, a value written before alpha; `inner_window` content test updated).
- `cargo test -p skelter-core` 187 pass (new: `tests::prepare`).
- `SKELTER_GPU_TESTS=1 cargo test -p skelter-gallery --test open_states --test render --test check --test host` (3 new:
  hover tooltip, context menu open and four toasts, a toast timing out under the app runner; `render`'s hover test now
  expects the pointer note).
- `skelter-gallery check`: ok, 47 stories, 170 variants, no problems, no warnings. `gen_props.py --check` ok.
- Not run locally: the workspace tests, other crates, pixel snapshot tests, conformance.

CI on `deec09a` (the last code sha; master `f8e2669` merged), all six workflows green: js 40617494256, linux-gtk
40617494189, linux-qt 40617494197, macos-appkit 40617494230, skelter 40617494206, windows-winui 40617494298. No pixel
snapshot test moved, so none was re-blessed. Edits to this record after that sha are docs only (CI not rerun for them).

## Handoff

Open: Q1–Q5, R4, R-13..R-20. If a pixel
snapshot test moves in CI, it is from R-2 (disabled menu items), R-6/R-7/R-8 (hosted dialog buttons, status bar,
inner window layout), R-10 (colour editor width) or R-11 (tab marker); re-bless only after looking at its diff.
