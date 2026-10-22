# WG4a: the Tier 1 views of svelters-egui (table, tab-view, split-view, toast)

Design: `plans/research/EGUI.md` §5 (Tier 1; the catalog the user approved on 2026-10-22, §8) and
`plans/research/TRACKS.md` WIDGETS row WG4. The pickers, dialogs and drag and drop are WG4b's;
`tree-view` waits for LISTS. Builds on WG1's shared machinery (`origin/ci/wg1` 50e680f7, merged here):
container chrome through `leaf::container`, `walk::paint_children`, `Common::invalid`, the a11y name
fallbacks.

**Decided (coordinator, 2026-10-22):** SG4 made `position` a common `Widget` prop (CSS
`static`/`relative`/`absolute`), so `split-view`'s `position` is renamed `divider-position`
(`bind:divider-position`, `changed(divider-position)`, `P::DividerPosition`). Done in the INT1
integration (`ci/int-widgets`).

## What landed

| Element | Native | Props | Signals | Methods |
|---|---|---|---|---|
| `table` | `egui_table::Table` 0.11.0 (virtual rows and columns) | `row-count`, `row-height`, `sticky-columns`, `striped`, `overscan`, `selected` (bind) | `range(first, last)`, `changed(selected)`, `activated(row)`, `header-clicked(column)` | `set-rows(first, rows)`, `clear-rows()`, `scroll-to-row(row)` |
| `table-column` | `egui_table::Column` (data only; base `Widget`) | `title`, `width` (Widget's; null = 100), `resizable`, `align`, `sort` | | |
| `tab-view` | a bar of selectable tabs + the selected page | `selected` (bind) | `changed(selected)` | |
| `tab` | a flex box (base `Box`) | `title`, `closable` | `close-requested` | |
| `split-view` | two panes + a draggable divider | `orientation`, `divider-position` (bind; null = half), `min-size`, `resizable` | `changed(divider-position)`, `drag-started`, `drag-stopped` | |
| `toast-host` | a foreground `egui::Area` of popup frames | `corner`, `duration` | `closed(id, reason)`, `clicked(id)` | `show(text, level?, duration?)` → id, `dismiss(id)`, `clear()` |

Rust API: `svelters_egui::toast::show(ctx, text, Level, duration)` queues on the egui context; the first
`toast-host` drawn in that viewport takes the queue (`toast::queued(ctx)` counts what is waiting).

| Piece | Where |
|---|---|
| Kind tables (`TABLE`, `TABLE_COLUMN`, `TAB_VIEW`, `TAB`, `SPLIT_VIEW`, `TOAST_HOST`), enum lists | `crates/svelters-egui/src/kinds/views.rs`; six lines in `kinds/builtin.rs` `KINDS`, one `P` block in `kinds/mod.rs` |
| State, layout, paint, methods | `crates/svelters-egui/src/views/{mod,table,tabs,split,toast}.rs` |
| Scene payloads | `W::{Table, TableColumn, TabView, Tab, SplitView, ToastHost}` (`scene/node.rs`); defaults in `scene/tree.rs`; prop arms in `scene/props.rs` (table and toast-host delegate to their state's `set`/`get`) |
| Dispatch arms | `layout.rs` (`compute_child_layout` → `views::layout`; `Metrics::intrinsic`), `leaf.rs` (`container` and `leaf`), `ops.rs` (`call` → `views::call`, the views' methods take arguments) |
| Typings | `table` is an HTML tag: its props merge into `HTMLAttributes` like `label`/`button` (`spec.rs` postlude) |
| Gallery | `fixtures/gallery-egui` (new): one embed entry per `src/pages/*.svelte` (globbed, so lanes add pages by adding files); pages `TableView`, `TabView`, `SplitView`, `Toasts` |
| Tests | `crates/svelters-egui/tests/views.rs` (Rust-built nodes, no bundle), `tests/gallery_views.rs` (the gallery pages, JS side) |

## Design notes

- **Layout.** `table`, `tab-view` and `split-view` lay out their children themselves
  (`views::layout`): their own size is a leaf's (props, flex, or the natural size of their content),
  then `place_child` lays out each shown child at a definite size and `compute_hidden_layout` gives
  the rest the zero layout. A table's columns are always hidden; a tab-view lays out only the selected
  page under the bar; a split-view its first two children. `tab` is an ordinary flex box.
- **Widget writes** go through WG1's `leaf::widget_changed` (stamp, journal, `prop_changed` when
  observed, then `changed`), so `bind:selected` and `bind:divider-position` lower onto the generated
  `bindings` like the WG1 inputs. A tab click restyles the tab-view (`invalidate_style`), a divider
  drag invalidates the split-view's content: the panes follow at the next frame's layout (one frame
  behind the pointer, the same as egui's own resize handles).
- **Table data: the data half of the virtual-list protocol.** The table works out the rows it shows
  (egui_table's `prepare`) plus `overscan` on each side and emits `range(first, last)` once per change;
  JS answers with `set-rows(first, rows)`, which replaces the data window. Rows outside the window draw
  empty until their data arrives; with the default overscan (20 rows) a scroll of up to a page shows
  data already sent. A small table sends everything once (`set-rows(0, all)`). The `TableView` page
  shows the JS side: `onrange` stores the range, an `$effect` sends the slice (and sends it again when
  the array is sorted).
- **Not shared with LISTS.** Rich cells (a component per cell) need row slots recycled as the table
  scrolls, which is LISTS' `RowPool`/`VirtualChildren` (ARCH-SCENE §8.2). This lane does not touch
  `scene/virt.rs`. When `RowPool` lands, rich rows can take over the request side unchanged: the
  `range` signal is the same request a `RowPool` serves.
- **Column widths.** `width` is written into egui_table's state when it changes (per column, by the
  column node's egui id), so a user resize stays until the prop changes; the state is stored before
  the first `show`, so egui_table never runs its first-frame auto-size (and discard) pass. A
  double-click on a resize handle still fits the column to its content.
- **Toasts** live in the host's payload (`views::toast::Toasts`). Durations run on egui's clock from
  the first frame a toast is drawn; hovering holds a toast open; `request_repaint_after` wakes the
  frame that expires it (no repaint loop while idle). Closes by a method are emitted the next time the
  host is drawn.
- **a11y.** Tabs are `Role::Tab` with `selected`; each closable tab has a "Close <title>" button. The
  divider is `Role::Splitter` named by the split-view's `accessible-name`, with the position as its
  numeric value; it takes focus and the arrow keys (10 pt). The table is `Role::Table` named by its
  `accessible-name`, header cells are `Role::ColumnHeader`, cells are labels. A toast is
  `Role::Alert` with its text; its close button is "Dismiss".

## Frame time (10k rows)

`tests/views.rs` `table_with_10k_rows_scrolls_and_frame_time`: a 500x400 slot, a table of 10 000 rows
x 3 columns, all rows sent once. Measured locally on the WSL PC (debug build, headless kittest, so
egui's tessellation and kittest's AccessKit tree are in every frame, no GPU), one run:

| What | Time |
|---|---|
| `set-rows(0, 10k x 3)` (30 000 ABI values converted to strings) | 28 ms, once |
| a frame with nothing changing | 16.6 ms |
| 40 jumps (`scroll-to-row` every 245 rows), each to rest | 120 frames, 39 ms per frame |
| 20 small steps upward (5 rows a frame) | 31 ms per frame |

The cost follows the visible rows (about 18 here), not the row count: egui_table draws only visible
cells, and the data window is indexed by `row - first`. A scrolled frame costs about twice a still one
because each newly visible cell lays out its text again. A release build was not measured (it would
need a release build of the test dependencies). Expect egui's usual 10-20x gap between debug and
release, which puts a scrolled frame well under a millisecond. The JS side has its own cost: sorting
2 000 parts in the gallery page fits the embed's logic budget in a debug QuickJS only with `$state.raw`
and precomputed keys (a `$state` proxy per part, or a state read per comparison, was interrupted).

## Merges and fixes on the way

- `origin/ci/wg1` 50e680f7 (coordinator's instruction): its diff touched only widget files, scene payload
  variants and taffy's `grid` feature.
- `origin/ci/wg1` again at 66fcd9df (its milestone, TaskPanel fixture, `widget_changed` made public).
- `origin/master` three times. The first time (e07313e8), master did not compile svelters-egui after
  EV2b: `BehaviorBox` lacked SG3's `ErasedWidget::hit_test`, and `walk.rs` `paint_own` did not return
  its response. I reported it to the coordinator, who fixed it on master at edefc916, and this branch
  took master's version. The last merge brought in CS5e, which collides with WG1 (so WG1's own merge
  meets the same three spots):
  - `walk.rs`: CS5e's container state, box and opacity now wrap WG1's `leaf::container` chrome.
  - `layout.rs`: WG1's per-kind match is kept, plus CS5e's `display: grid` for generic containers.
  - Both lanes added the same `LayoutGridContainer` impl, and one copy is kept.
  - taffy features: `grid` and `calc`.
- CI's first run (linux-gtk) failed `observe` and `widgets`, which walk every non-toplevel kind with
  the Widget props. So `table-column` now takes the Widget base, and its width is Widget's `width`
  (null means 100). It has no text content (that test sets `text` on kinds that declare one).

## Not done here

- `rfd` dialogs, date and colour pickers, drag and drop: WG4b. `tree-view`: after LISTS.
- Rich table cells (see LISTS above); keyboard row navigation in the table; column reordering;
  bindable column widths.
- A typed JS helper for toasts (`host.call('show', …)` is the API today; the method names and
  arguments are in `egui.json`, not yet in `egui.d.ts`, which types props and events only).

## Verification (this lane, local)

- `tools/dev/cargo-slot cargo check -p svelters-egui --all-targets`: clean.
- After the last merge and fix: `tools/dev/cargo-slot cargo nextest run -p svelters-egui --test views
  --test gallery_views --test spec --test observe --test widgets --test style`: 28/28 (the gallery and
  embed bundles built with `pnpm --filter … build` after `cargo build -p svelters-compiler --bin
  svelters-compile`).
- `SVELTERS_UPDATE=1 … --test spec` regenerated `metadata/semantics/egui.json` and
  `packages/native-runtime/types/egui.d.ts` (they also pick up WG1's kinds, which its WIP had not
  regenerated); `python3 tools/metadata/style/gen_props.py` regenerated `props.generated.js`
  (`--check` clean); `python3 -m pytest -q tools/metadata`: 155 passed.
- `pnpm install` added the new fixture to `pnpm-lock.yaml`.
- Not run locally: the rest of svelters-egui's tests, the workspace, conformance, the other backends,
  `pnpm test`. CI runs them.
