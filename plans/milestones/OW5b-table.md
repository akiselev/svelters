# OW5b: own `table` and header (slice OW5b1)

Lane `ci/ow5b` (thinkpad), from `origin/master` 943b4c67 (OW5a and OW5a1b merged). Design:
ARCH-WIDGETS §6.1 (`table`, `table-column` rows) and §7 (OW5). Acceptance rows: WIDGET-PARITY §2.18
(OW5b). FM1's QW4 header state comes from `origin/ci/fm1` (its handoff in `FM1-form-widgets.md`).

**Decided (coordinator, 2026-10-22):** the split is accepted. OW5b1 is this lane; OW5b2 goes to a
fresh agent (handoff below). OW5a2's multi-column tree builds on this lane's `table-column` and header.
The table.rs egui cells are OW5b's, not OW1a4's.

**Split (proposed to the coordinator at the start).** The OW5b parity rows add up to about 5.15 d, on
top of replacing egui_table. This lane is **OW5b1**: the own table, its header, FM1's QW4, and the small
rows. The rows marked "OW5b2" below are left for a fresh agent.

## What landed

`crates/svelters-egui/src/views/table.rs` no longer uses `egui_table`, and the dependency is gone from
`Cargo.toml` (with its `clippy.toml` entry). `W::Table` stays a payload, as lists stay `W::VirtualList`.
The table now draws its own rows, cells, header and parts from CSS and UA tables:

- **Virtual on both axes.** Only rows in `first..end` and columns that intersect the view are painted
  and get AccessKit nodes. `range(first, last)` asks for the visible rows plus `overscan`, as before.
- **Scrolling** uses OW2's `Scrollable` on both axes:
  - wheel and trackpad, kinetic scrolling, overlay bars with the policy fade (`::part(scrollbar-x|y)`,
    `::part(thumb-x|y)`);
  - AccessKit scroll actions on the Table node, and a `ScrollBar` node per bar;
  - R15 edge shadows (`table:state(sv.overflow-top|left)`, base.css).
- **The header row is frozen**; the first `sticky-columns` columns stay fixed horizontally (`Geom`).
- **Column widths** come from each column's `width`, clamped by `min-width`/`max-width` (default
  minimum 16). `stretch-last` (default true) widens the last shown column to fill the view.
- **Resize edges** (`::part(resize)`) use OW2's `Resizable` on each header edge, with the R11 look
  (split view's `SASH_UA`) and a `col-resize` cursor:
  - a drag shows the new width live (`TableRt::live`);
  - on release the width is written into the column's `width` (announced, so `bind:width` works) and
    `column-resized(column, width)` fires;
  - a double click fits the column, and so does the `fit-columns(column?)` method.
- **Parts** use `Scene::part_at` positional styling, exactly as OW5a. Each item gets its own forced
  state and its logical position (P1):
  - `row` (R6's four states), `cell` (the R8 ring on the cursor cell, keyboard only), `error`;
  - `header`, `header-cell` (`:hover`, `:active`), `sort` (codicon `arrow-up`/`arrow-down`, or
    `content: icon(…)`; `:checked` while descending);
  - `menu`, `menu-item`, and the bars.

  `striped` without a CSS rule fills odd rows with `list-hover` at half strength. The selectors are
  documented in `docs/egui/css/own-widgets.md` ("Table rows, cells and header").
- **Selection and keys.** Selection keeps WG5a's `KeySelect` with `item_keys`. Left/Right move the
  cursor column through `RovingList::next_by` (lazy). Ctrl+C raises a new `copy(ids)` signal. R8
  keyboard modality: `kbd` is set by Tab-in or keys and cleared by a click.
- **The header (FM1's QW4, ported).** `sortable`, `sort-column`, `sort-order`, `column-menu`,
  `hidden-columns` and `header-state` (bindable), with `sort-changed`, `columns-changed` and
  `header-state-changed`. FM1's shown→data map is `TableState::col_map`/`data_col`; Tab skips hidden
  columns.
  - A right click raises `header-context(column)`. EV1 filters run first (`list_rows::allowed`); unless
    one prevents it, the own column menu opens: check items, the last shown column disabled. It
    replaces FM1's egui `context_menu` and `Checkbox`.
  - **Saving goes through DX1's `UiStateBackend`.** The app binds `header-state` to a `uiState()`
    field (`bind:header-state={ui.header}`). The table writes no files (ZU3).
- **Column props** are boxed in `W::TableColumn(Box<ColumnState>)`, so the per-node `W` stays small.
  New props: `name`, `hideable` (FM1), `header-align` (`auto` = `align`), `elide` (end, middle, start,
  none; for cells and the header). The Widget props `visible` and `tooltip` (on the header cell, OW3's
  shared tooltip) are now honoured.
- **New table signals:** `header-double-clicked`, `header-context`, `column-resized`, `copy`, plus FM1's
  three. **New method:** `fit-columns`.
- **A rejected commit's reason** is shown with OW3's tooltip (`tooltip::show` at the cell; one show per
  change, hidden when the error ends), and the cell gets `::part(error)`. The egui `Area`/`Frame` is
  gone.
- **Accessibility:**
  - Table: row and column counts, multiselectable, scroll values and actions;
  - Grid: the focus stop, with `active_descendant` on the cursor cell;
  - Row: row index and selected;
  - Cell: label (its text), row and column index, read-only unless editable;
  - ColumnHeader: label and sort direction;
  - ScrollBar, and the resize edges (Splitter, "Resize column …").

  egui fills AccessKit bounds only for focusable widgets, so hover-only rows and cells set theirs
  (`bounds`).
- **Seam (until OW4's own text field reaches master):** the built-in cell editor is still egui's
  `TextEdit`, in `paint_editor` only, under `// OW: wrapper until OW4`. It is the one egui widget left
  in the table. Column `editor` snippets are unchanged.
- **For OW5a2 (multi-column tree):** `ColumnState`, `columns()`, `Geom` and `header()` are `pub(crate)`
  and take no table-only state beyond `TableState`'s sort and `TableRt::edges`. A tree with
  `table-column` children can lay its rows on `Geom` and draw the header with `header()`. Expect to
  lift `TableRt`'s edges and live width into a small shared struct.

`table.rs`'s `// OW: wrapper until OW5b` and `// OW: wrapper until OW1a3` lines (header and cell
labels) are deleted. OW1a4 should drop its "table.rs egui cells" item; the coordinator has been told.

## DESIGN rules satisfied

- **R6:** rows and cells are flat, with four distinct states: `list-hover`, `list-sel` + `list-sel-fg`,
  `list-sel-inactive`, and the 1px focus outline on the cursor cell.
- **R8:** the ring is 1px `focus` at offset −1, keyboard focus only.
- **R9:** a disabled table keeps its header tooltips (OW3).
- **R10:** `pointer` on rows, `col-resize` on edges.
- **R11:** resize edges are a 1px `border` line with a `sash-w` hit area, and the `focus` hover bar after
  the policy delay or while dragging.
- **R15:** edge shadows and overlay bars.
- Tokens: `list-*`, `focus`, `bg`, `fg`, `icon`, `invalid-border`, `border`, `sash-w`, `stroke`,
  `scrollbar*`, `scroll-shadow`, menu tokens for the column menu.

## WIDGET-PARITY rows (§2.18, OW5b)

| Row | Status |
|---|---|
| `textElideMode`: `elide` on `table-column` | done (cells and header) |
| `hideRow`/`hideColumn`: `table-column visible` honoured | done |
| `resizeColumnsToContents`: `fit-columns()`, double-click on the edge | done |
| `stretchLastSection`: `stretch-last` | done |
| `minimumSectionSize`/`maximumSectionSize`: `min-width`/`max-width` | done |
| `sectionDoubleClicked`: `header-double-clicked` | done |
| `sectionResized`: `column-resized(column, width)`, `bind:width` | done |
| `defaultAlignment`: `header-align` | done |
| header context menu: `header-context` + `column-menu` (QW4) | done |
| header tooltip | done |
| Ctrl+A, copy: `copy` on table | done |
| `QHeaderView::section`: `::part(header-cell)` + P1 positions, `:hover`, `:active` | done; per-column `table-column::part(header)` (P6 style host): OW5b2 |
| `::up-arrow`/`::down-arrow`: `::part(sort)` | done |
| resize handle: `::part(resize)` | done |
| `saveState`/`restoreState` | done (`header-state`, persisted through DX1 `uiState()`) |
| AccessKit grid roles, `active_descendant` | done |
| `selection-unit` cell and column; `highlightSections` | OW5b2 |
| `edit-triggers`, `tab-navigation` | OW5b2 |
| `openPersistentEditor`, checkable cells, cell icons, cell part names (P10), cell tooltips from data | OW5b2 |
| `showGrid` (`::part(grid)`), corner button | OW5b2 |
| `sectionResizeMode` (`resize` modes), movable columns, `sort-priority`, header icons | OW5b2 |
| table DnD (`row-drag-type`/`row-drop-types`) | OW5b2 |

## VIS1 checks and pins

VIS1 has no `table` probe (VS Code has no table widget among the references). The table's rows use
the same tokens and R6 states as the list rows that VIS1 measures (`list-row-rest`, `-hover`,
`-focused`, `-selected-focused`, `-selected-inactive`). I did not run the VIS1 gate for this lane. No
native-behaviour pin covers tables.

## Tests

Run locally, all pass:
- `views.rs`: `table_asks_for_its_window_draws_it_and_selects_rows`, `table_with_10k_rows_scrolls_and_frame_time`;
- `wg5a.rs`: the 3 table tests (100k rows from the keyboard; F2, commit, reject, revert, Tab; column editor);
- `views::table` units: tab order, ids across `set-ids`, geometry (stretch, sticky, hit tests),
  `header-state` round trip and sorting;
- new `tests/ow5b.rs`: a sortable header and its a11y sort; the column menu, `header-state` restore and
  `visible`; edge drags with min-width, and `fit-columns`;
- `scene_memory` (≤640 B/node), `scene::mutations_allocate_nothing_without_observers`, `spec`.

Not run locally (they need the built fixtures; CI runs them): `gallery_views.rs`, `clipboard.rs`,
`embed.rs`. FM1's own QW4 tests (in `forms.rs` on `ci/fm1`) were not ported as they are; `ow5b.rs`
covers the same behaviour.

## OW5b2 handoff (for a fresh agent)

Start from master once `ci/ow5b` merges, and read this record and the module doc of `views/table.rs`
first. Keep the `Scene::part_at` design: every repeated part is computed per item, with its own state
and logical position. Rows (WIDGET-PARITY §2.18):

1. **`selection-unit="row" | "cell" | "column"`** (0.75). Cells are keyed by `(rowId, data column)`.
   This needs a cell set beside `KeySelect`; extend `views/selection.rs` with a cell mode rather than a
   second selector. Shift ranges are rectangles. Header cells get `:selected` when their column has
   selected cells (`highlightSections`).
2. **`edit-triggers`** (token list: `double-click f2 typing selected-click`; today's fixed set is the
   default) and **`tab-navigation`** (Tab moves the cursor while not editing). Both act in `paint`'s key
   block and in `start_edit`'s callers.
3. **Cell content from the data:**
   - cells as `{ value, tooltip, icon, part }` objects (`cell_text` only reads values today; keep plain
     values working);
   - `::part(cell-icon)`;
   - extra part names (P10, `table::part(cell negative)`, which needs OW-CSS's multi-name `part_at`);
   - tooltips through `tooltip::attach` on the cell's response.
4. **Checkable cells** (a boolean column paints `::part(cell-check)` and toggles on Space or click, with
   `commit`) and **persistent editors** (an always-on editor column). Both need a per-column `kind` or
   `editor-mode` prop on `ColumnState`. With OW4's own text field on master, also replace the seam in
   `paint_editor` (the one `egui::TextEdit`).
5. **Grid lines** (`::part(grid)`, `--sv-grid-lines: none | rows | cols | both`) and the **corner button**
   (`::part(corner)`, Ctrl+A on click; only with a row header, so it may wait for OW-P2's `row-header`).
6. **Header:**
   - `table-column resize="interactive" | "fixed" | "stretch" | "contents"` (`resizable` stays as
     shorthand; stretch columns share the slack that `stretch-last` gives the last column today, in
     `Geom::new`);
   - movable columns: `columns-movable` and `column-moved(from, to)`, a drag on a header cell with a
     drop line;
   - `sort-priority`: a badge in the `sort` part;
   - `table-column icon`;
   - a "Fit to contents" entry in the column menu (`column_menu`; `fit_width` exists);
   - P6: `table-column.numeric::part(header|cell)` (the style host is the column node; needs OW-CSS's
     style-host indirection in `part_at`).
7. **Table DnD:** `row-drag-type`/`row-drop-types`/`internal-move` as in OW5a1b's `RowDnd` (list).
8. **Column menu keyboard:** arrows, Enter and the Menu key (Shift+F10 on the focused table raising
   `header-context` at the cursor column). The menu is pointer-only today.

Tests to extend: `tests/ow5b.rs` (header), `tests/wg5a.rs` (keys, editing). Shared code OW5a2 also uses
(`columns`, `Geom`, `header`): change it in one place, and run `ow5b.rs` together with the tree tests.
