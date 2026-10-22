# OW2: scroll and containers (Scrollable, Resizable, `scroll-area`, `scrollbar`, `split-view`, `panel`, window chrome)

Lane `ci/ow2` (WSL PC), from `ci/ow0` (OW0a `7eef7362`, then OW0b `6f013c72`/`bb4bfa77`) with `origin/master`
merged. Design: `plans/research/ARCH-WIDGETS.md` §1.2 (Scrollable, Resizable), §3.3, §6.1 and the OW2 row of
§7.2; acceptance rows from `plans/research/WIDGET-PARITY.md` §2.11 and §2.17; look from DESIGN.md §1.3–1.5, R10,
R11 and R15; VIS1's scrollbar and sash references (`ci/vis1`, `conformance/visual/vscode/*/metrics.json`).

## What landed

### Units (`crates/svelters-egui/src/interact/`)

- **`Scrollable`** (`scroll.rs`). The host gives the geometry each frame (`set_geometry`, `bar_geometry`), then
  the unit runs on the host's response and its bar parts (`InputCx::parts`: `scrollbar-x/y`, `thumb-x/y`):
  - wheel and trackpad through egui's `smooth_scroll_delta` (egui already turns Shift+wheel horizontal); the
    delta it takes is subtracted from egui's input, so an outer scrollable gets only the rest. A scrollable
    runs **after its children** (post-order), so the innermost one under the pointer goes first and chains at
    its edge (`overscroll-behavior: contain|none` keeps the wheel);
  - momentum per `native::Policy::kinetic`: `Os` takes the OS's momentum events, `Own` decays the release
    velocity 0.95 per 16 ms and stops below 0.5 pt per frame, `None` stops;
  - thumb drag; a track press pages (repeating while held: 0.3 s, then every 0.05 s) or jumps
    (`ScrollPolicy::track_click`; Shift inverts it on GTK);
  - PageUp/PageDown/Home/End while focus is inside the area and the focused child did not claim them (the
    child's EV3 claims are taken out of the input before the area runs); arrows when the area itself is focused;
  - AccessKit: `scroll_x/y` with their ranges, ScrollUp/Down/Left/Right (100 pt, egui's) and SetScrollOffset;
  - overlay bars: shown while hovered, dragged or scrolled, then held 0.5 s and faded over 0.8 s
    (`ScrollPolicy::bar_hold`, `bar_fade`; VS Code's timings);
  - **R15 states** (coordinator, 2026-10-22): provider states `sv.overflow-top`, `-bottom`, `-left`, `-right`
    (`state_bits!`), set while content lies beyond that edge; the theme matches `:state(sv.overflow-top)`.
- **`Resizable`** (`resize.rs`), on a sash part's own response and state bits (`:hover`, `:active`,
  `:dragging`, `:focus`, `:focus-visible`): drag from the press origin (`Start`, live `Resize`, `Commit`),
  Escape cancels back to the size before the drag; arrows by `step` (Shift ×5), Home/End to the bounds, Enter
  collapses or restores; double click restores the size before a collapse or collapses (its own click count
  against `Policy::double_click_ms`, ARCH W2); `collapsible` below half the minimum goes to 0; R11's hover bar
  after `ScrollPolicy::sash_hover_delay` (0.3 s), at once while dragged or focused; AccessKit `Splitter` with
  value, bounds, step, orientation and Increment/Decrement/SetValue.
- **`native::ScrollPolicy`** (`Policy::scroll`): `track_click`, `shift_track_click_inverts`, `line` (40 pt),
  `bar_hold`, `bar_fade`, `sash_hover_delay`, beside OW0b's `kinetic` and `scrollbars`. Rows: GTK (jump,
  Shift inverts), Qt (page), Windows (page), Mac (page), egui/VS Code (jump).

### Widgets (`crates/svelters-egui/src/widgets/`)

| Tag | File | What | Notes |
|---|---|---|---|
| `scroll-area` | `scroll_area.rs` | box; Scrollable; parts `scrollbar-x/y`, `thumb-x/y`, `corner`, `shadow-top/left` | Children drawn moved by the offset in a child `Ui` clipped to the padding box (the walk culls against it, the hit list records it). Bars over the content (overlay, VS Code) or with a track (classic: Qt row, `scrollbar-gutter: stable`, `--sv-scrollbar-mode: classic`). |
| `scrollbar` (new kind) | `scrollbar.rs` | Scrollable over `value` from `min` to `max`, thumb sized by `page`; `step`, `orientation`; `changed(value)` | WIDGET-PARITY §2.11's `QScrollBar`. Always shown; always own (`OWN_ONLY`). AccessKit `ScrollBar` with Increment/Decrement. |
| `split-view` | `split_view.rs` | custom layout; Resizable per sash; parts `sash`, `grip`, `ghost` | N panes (every visible child), `sizes` (bind), `collapsible`, `restore-on-double-click`, `commit-mode` (`live`/`release` with a guide line), `commit(sizes)`; extra room by the panes' `flex-grow`; per-pane `min-*`/`max-*`. Sash = 1 pt `border` line, 4 pt hit (`sash-w`), R11 bar in `focus` with a three-dot grip. |
| `panel` | `panel.rs` | box (`bg`); Resizable on the inner edge; parts `separator`, `sash` | `size` via `Widget::taffy_style`; arrows on the focused sash. |
| `window` (inner chrome) | `windows.rs` | the window's background from its computed style over `WINDOW_UA` (`bg`) | Replaces `egui::CentralPanel`/`Frame::central_panel`. `window` stays the `W::Window` payload (a viewport, not a widget), so it has no `replaces=` struct. |

All three replacing kinds are **always own**: their egui wrappers are deleted (`leaf.rs` `scroll`,
`views/split.rs`, the panel chrome and edge of `elements/panes.rs`, and the `W::ScrollArea`, `W::SplitView`,
`W::Panel` payloads with their prop arms and layout code). `widgets::OWN_ONLY = [panel, scroll-area, scrollbar,
split-view]` makes the switch ignore them; `DEFAULT_OWN` is empty again. The `// OW: wrapper until OW2` lint
line in `leaf.rs` is gone; `views/tabs.rs`'s now reads OW5c (the tab strip's `ScrollArea` is OW5c's).

### Scroll requests (WG6 QA13, taken from `ci/wg6`)

`Scene::reveals`: one list per pass, answered by every scrollable around the target after its children drew,
innermost first (`widgets::scroll_area::{begin_pass, note, reveal_flagged, keep, scroll_into_view}`):
- **scroll on focus**: a leaf or own widget that gains focus (Tab, `request-focus`, a click) is revealed;
- **`scroll-into-view(align?)`**: a method on every node (`Widget`'s first method; `nearest` or `center`),
  keeping the area's `scroll-margin`;
- **AccessKit** ScrollIntoView and ScrollUp/Down/Left/Right aimed at a descendant (egui consumes them when the
  target is drawn, so the walk matches the target's response id against the pass's pending requests);
- the walk does not cull on a pass with Tab pressed, on the way to a `scroll-into-view`, or a node holding
  keyboard focus (egui drops the focus of a widget not drawn in a pass, so PageDown from a focused button would
  lose it).

### Shared changes

- `provider`: `Widget::taffy_style` (own widgets' part of their taffy style), `WidgetCx::{children_offset,
  layout_rect, children_extent, record_hit, run_units_with, relayout_self, relayout_children,
  record_part_focus, dark}`; `LayoutCx::{style, hide, child_common}`; a list payload spreads over a built-in
  signal of several args (`changed(scroll-x, scroll-y)`); writes of `direction`, `side`, `size`,
  `orientation`, `divider-position`, `min-size`, `sizes`, `collapsible` relayout an own widget.
- `layout.rs`: an own widget with `Caps::CONTAINER` lays out its children (OW0b keeps replaced leaves leaves);
  `provider_caps` sees own widgets' caps (`CUSTOM_LAYOUT` for split-view).
- `walk.rs`: own containers record their hit-list entry before their children (they are under them).
- `style.rs`: engine props `overflow-x`, `overflow-y` (also taffy's overflow; `visible`, the initial value,
  leaves the props' axes), `overscroll-behavior`, `scroll-margin`, `scrollbar-gutter` (`stable` reserves
  `scrollbar-w`); the box properties apply to own containers.
- `paint`: `paint_box_layers` and `paint_inset_shadows`, so the scroll area paints its inset shadows over its
  children.
- `theme/base.css`: `window, panel { background-color: var(--sv-bg) }`; R15's inset `box-shadow` on
  `scroll-area:state(sv.overflow-top)`/`(sv.overflow-left)` (VS Code shades top and left only). Without a theme
  the scroll area paints the same edges from `SHADOW_UA`.
- Kind table: `Ty::NumList`; split-view props `sizes`, `collapsible`, `restore-on-double-click`, `commit-mode`
  and signal `commit`; the `scrollbar` kind (`Scrollbar`, appended); `kinds/docs.rs` rows; egui.json, typings,
  style tables and the widget reference regenerated. (These went into the KINDS rows; OW0b's struct-side
  "extras" landed after they were written and would also do.)
- `focus::focused_node`; `NodeFlags::{WANT_SCROLL, SCROLL_BELOW, SCROLL_CENTER}` (bits 18, 19, 21; CX3 took 17);
  automation role `scrollbar`.

## DESIGN rules satisfied

- **R10** cursors: `col-resize`/`row-resize` on sashes at once on hover.
- **R11** sashes: 1 pt `border` line, 4 pt hit area, the resize cursor at once, after 300 ms a 4 pt `focus` bar
  with a three-dot grip at its middle (also while dragged or keyboard-focused).
- **R15** scroll edge shadows on the scrolled-away top and left (`scroll-shadow`), and overlay bars that show
  on hover or scroll and fade.
- **R8** (sashes): keyboard focus shows the hover bar; pointer focus is not `:focus-visible`.
- DESIGN §1.3/1.5 metrics: `scrollbar-w` 10, `sash-w` 4, `scrollbar*`, `scroll-shadow`, `bg`, `border`, `focus`.

## WIDGET-PARITY rows

§2.11 scroll area / scroll bar:

| Row | Status |
|---|---|
| `overflow-x`/`overflow-y` (AsNeeded/AlwaysOff/AlwaysOn) | done (CSS; `direction` stays the shorthand) |
| bar style overlay/classic + CSS hook | done (Policy `scrollbars`; `scrollbar-gutter: stable`, `--sv-scrollbar-mode`) |
| `ensureVisible`/`ensureWidgetVisible`, scroll on focus | done (`scroll-into-view`, focus reveal, AccessKit ScrollIntoView) |
| `scroll-margin` | done (uniform length) ; `scroll-padding` not done |
| kinetic per Policy | done (`Own` decay is unit-tested through its geometry only; no real touchpad here) |
| overscroll / chaining, `overscroll-behavior` | done |
| smooth wheel steps | done (egui's `smooth_scroll_delta`) |
| standalone `scrollbar` kind | done |
| `track-click` per Policy | done |
| `::part(thumb-x/y)`, `scrollbar-x/y`, `corner`, `shadow-*` names | done as paint parts with UA tables; **not yet styled by `::part()`** (OW-CSS's part styles reached master in this merge; wiring `cx.part_css` is the follow-up) |
| `track-before`/`track-after` parts | not done |
| scroll arrows `::part(inc/dec)` | not done (hidden in VS Code's look anyway) |
| corner slot (`slot="corner"`), viewport slots (`slot="top"/"left"`) | not done |
| fade via `transition: opacity` | done in the unit (Policy timings); not CSS-driven |

§2.17 splitter: N panes ✓; `sizes` ✓; stretch by `flex-grow` ✓; `::part(sash)` hit and line from tokens ✓;
`collapsible` ✓; restore on double click ✓; `commit-mode="release"` ✓ (not covered by a test);
`changed`/`drag-started`/`drag-stopped`/`commit` ✓; per-pane `min-*`/`max-*` ✓; keyboard ✓, **F6 to cycle
panes not done**; `::part(grip)` ✓ (painted); `[collapsed="true"]` on a pane **not done** (needs a write to the
child's props).

## VIS1 checks

The gate (`crates/svelters-testkit/tests/vis1.rs`) is still on `ci/vis1` (VIS1c merges it). Until then
`tests/ow2.rs::ow2_scrollbar_colours_match_vis1` checks the thumb's UA colours against VS Code 1.133's measured
`scrollbarSlider.*` (dark rest `rgba(121,121,121,.4)`, hover `rgba(100,100,100,.7)`, drag `rgba(191,191,191,.4)`;
light rest `rgba(100,100,100,.4)`, drag `rgba(0,0,0,.6)`) and `scrollbar.shadow` (dark `#000`, light `#DDD`),
within one step per channel. Known deviations: our bars are 10 pt (VS Code lists; its editor bars are 14); VS
Code's light hover keeps the rest colour while ours uses `scrollbar-hover`; the sash at rest is VS Code's
transparent 4 px with the border drawn by the pane, ours draws the 1 pt `border` line itself (R11).

## Pins

WG3s pin 9 (scroll chaining) and pin 10 (drag thresholds) are not captured yet; the chaining behaviour follows
ARCH §3.3 (innermost scrollable that can move takes the wheel) and is covered by
`ow2_wheel_chains_from_the_inner_area_at_its_edge`. No other pin applies.

## Tests

- `tests/ow2.rs` (11): own scroll area by default + wheel + `changed(x, y)` arity + `sv.overflow-*` states +
  AccessKit `ScrollView`; Shift+wheel horizontal; chaining inner → outer; PageDown/End/Home from a focused
  child; Tab to a button below the fold and `scroll-into-view`; thumb drag and track jump; VIS1 colours; split
  view with 3 panes (`sizes`, a sash drag moves its neighbours, `commit`, two Splitter nodes); collapse/restore
  by double click and the sash keys; the `scrollbar` kind (wheel, AccessKit); the window background.
- Unit tests: `interact::scroll` (scroll_by, thumbs, reveal, fade), `interact::resize` (collapse/restore),
  `widgets::split_view` (two-pane rule, flex-grow and bounds).
- Existing acceptance, passing on the own widgets with the wrappers deleted:
  `widgets.rs::scroll_area_scrolls_and_binds_its_offset` (unchanged), `tier0.rs::panel_resizes_by_its_edge`
  (unchanged), `tier0.rs::bad_input_falls_back_and_never_panics`, `views.rs::split_view_places_two_panes_and_the_divider_moves_them`
  (the divider is now R11's 1 pt line instead of 6 pt: two assertions changed, ARCH §6.2 step 4), the sweeps
  `observe.rs::every_prop_is_observable_and_host_writes_never_echo`, `widgets.rs::every_kind_has_a_tooltip_and_inputs_are_named`,
  `spec.rs` (regenerated).

## Run locally / not run

Run after the last code edit: `cargo check -p svelters-egui -p svelters-style --all-targets`; earlier in the
lane `cargo check --workspace --all-targets` and `cargo clippy -p svelters-egui --all-targets` (no warnings in
OW2 files); nextest `-p svelters-egui` `--test ow2 --test widgets` (25/25) after the last edit; before it, after
the master merge: `--test style --test tier0 --test views --test observe --test own_widgets` and the spec tests
(`SVELTERS_UPDATE=1`), `--lib`, `window`, `viewports`, `queries` (bundle-dependent ones excepted);
`tools/metadata/style/gen_props.py`, `tools/docs/egui/gen_reference.py` (and `--check`).

Not run locally: tests that need `fixtures/*/dist` or `pnpm install` (embed, `focus_keys` Svelte cases,
`own_widgets::ow0b_svelte_onclicked_reaches_js_through_dispatch`, `queries`, gallery pages: the worktree has no
`node_modules`), the rest of the workspace, the other backends, conformance, VIS1's gate. CI runs them.

OW2b (after merging `origin/master` at `befca905`: OW1a, EV1, AU1b, Q4b, LD2, SG6, RT2, LD3), after the last code
edit: `cargo check -p svelters-egui --all-targets`; nextest `-p svelters-egui` `--test ow2 --test theme --test views
--test spec --test events --test scene_memory --test animator --test tier0 --test widgets --test ow1a --test
rt2_rich` (85/85, with `pnpm install`); `SVELTERS_UPDATE=1 --test spec` (spec unchanged, typings regenerated);
`tools/docs/egui/gen_reference.py`; the TK1 egui snapshots `counter-three` and `transition-back` regenerated
(`UPDATE_SNAPSHOTS=1 WGPU_BACKEND=vulkan … -p svelters-testkit --features snapshots -E 'test(cases_on_egui)'`, llvmpipe)
and rerun without UPDATE. The only change in both PNGs is the window background, #1B1B1B (egui's `CentralPanel`)
to the theme's `bg` #181818, plus antialiased edges blended onto it. Not run locally: everything else (CI).

## Handoff

- **OW2b** (2026-10-22, WSL PC) took over from the first OW2 agent at `b4094d13`. It merged master twice. The
  conflicts were resolved as follows. `interact`, `widgets`, `provider::WidgetCx` (EV1's `ctx` beside OW2's
  `relayout`) and `Scene` (EV1's `filters` beside `reveals`) keep both sides. `windows.rs` keeps OW2's
  `window_chrome_ui` and calls EV1's `run_viewport_passes` after it. The kind table puts RT2's rows and `Href`
  first, then OW2's `Scrollbar` and props. SG6's walk culls through `walk::cull`, which now also asks
  `scroll_area::keep` (the one culling predicate, as SG6 asks). SG6's `layout::ink` treats the own scroll area
  (`W::Ext` replacing `scroll-area`) as clipping, in place of the deleted `W::ScrollArea`. CI on `b4094d13`:
  the js and linux-gtk failures were the stale reference pages and the snapshots (both fixed here). windows-winui
  failed in `event_content_dialog_closed` ("show-async did not open the dialog"), a WinUI ContentDialog test
  that OW2 does not touch.
- Not done: the OW2 own widgets' `changed`/`commit` dispatches report after the scroll or resize and ignore
  EV1's `prevented()` (a notification, not a default action). Revisit if a handler needs to veto a scroll.

## Follow-ups

- Style the parts through `::part()` (`thumb-x/y`, `sash`, `grip`, `separator`) now that OW-CSS's part styles
  are on master; until then the UA tables decide.
- Scroll arrows, `track-before`/`track-after`, corner and viewport slots, `scroll-padding`, F6 pane cycling,
  `[collapsed]` on panes, a focusable scroll area with no focusable children (Chrome's rule).
- OW5 moves `virtual-list`, `table` and the tab strip onto Scrollable; OW7's inner-window edges onto Resizable.
