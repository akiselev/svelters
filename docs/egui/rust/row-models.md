---
title: Rust row models for tables, lists and trees
description: "Feed a `table` from a Rust model with no JS call per page: RowModel, RowView (sort, filter, tree rows), set_table_model, and RowSource for realised `virtual-list` rows."
status: pending   # LS2, lane ci/b8-tables (not merged yet)
verified_at: ~
sources:
  - crates/svelters-host/src/rows.rs           # RowModel, Cell, Column, RowView, RowFilter, VecModel
  - crates/svelters-egui/src/views/table.rs     # set_table_model, with_table_model, table_model
  - crates/svelters-egui/src/scene/virt.rs      # RowSource (virtual-list)
  - crates/svelters-egui/tests/ls2.rs           # the examples below, the T2 benches
  - plans/milestones/LS2-rust-row-models.md
---

# Rust row models for tables, lists and trees

When the rows live in Rust (nets, components, parameters, a property grid at 20k+ rows), don't copy
them into JS. Give the view a Rust model and it reads the visible cells itself: no `range` event, no
`set-rows`, no JS call per page. Svelte still declares the table, its columns and its props; Rust
supplies the rows.

| Your rows | Use | Where |
|---|---|---|
| cells of text and numbers, in columns, sortable, filterable, maybe a tree | `RowModel` on a `table` | `svelters_host::rows`, `SceneMut::set_table_model` |
| rows that are whole subtrees of widgets (editors, buttons) | `RowSource` on a `virtual-list` | `svelters_egui::scene::RowSource`, `SceneMut::set_row_source` |
| a few hundred rows that JS already holds | the JS window (`range` → `set-rows`) | [table](../reference/widgets/table.md) |

The trait and the view logic are in `svelters-host` and know nothing of egui, so the GTK and Qt
backends can adapt the same model later.

## The model

```rust
use svelters_host::rows::{Cell, CellKind, Column, RowModel};

/// cadatomic's nets list: name, class, pad count.
struct Nets {
    cols: Vec<Column>,
    nets: Vec<Net>,      // the app's own data
    version: u64,        // bump on every change
}

impl RowModel for Nets {
    fn len(&self) -> usize { self.nets.len() }
    fn key(&self, row: usize) -> u64 { self.nets[row].id }          // stable across changes
    fn columns(&self) -> &[Column] { &self.cols }
    fn cell(&self, row: usize, col: usize) -> Cell<'_> {
        let n = &self.nets[row];
        match col {
            0 => Cell::text(&n.name),                                  // borrowed: no allocation
            1 => Cell::text(n.class.as_str()),
            _ => Cell::Int(n.pads as i64),
        }
    }
    fn version(&self) -> u64 { self.version }
}

let cols = vec![
    Column::new("name", "Net").width(160.0),
    Column::new("class", "Class").width(100.0),
    Column::new("pads", "Pads").width(80.0).kind(CellKind::Number),   // right-aligned
];
```

- `key(row)` is the row's identity. Selection, the cursor, edits and expansion follow keys, so they
  survive sorting, filtering and model changes. Keys must be unique.
- `cell` returns a typed [`Cell`]: `Text`, `Int`, `Float`, `Bool` or `Empty`. Numbers sort
  numerically; text sorts naturally (case-insensitive, `N2` before `N10`). Override `sort_key` to sort
  by something other than what is shown.
- `version` tells the view the model changed. Bump it whenever you change rows or cells; the view
  then sorts and filters again and redraws.
- Optional: `label` (type-ahead text, default the first cell), `anchor`/`find_anchor` (paths for
  automation), `depth` (tree rows, below), `commit` (cell edits, below).
- `VecModel` is a ready model over owned rows, for tests and small tables.

## Showing it in a table

Svelte declares the table; Rust attaches the model to its node:

```svelte
<table bind:this={nets} sortable selection="multi" onrow-toggled={...} />
```

```rust
scene.write(provider, |m| m.set_table_model(nets_table, Nets::new(..), RowView::new()))?;
```

- **Columns.** A table with no `table-column` children gets one per `RowModel::columns`, with `name`
  set to the column's key, its header and width, and `align="end"` for numbers. You can instead
  declare `<table-column name="pads" title="Pads" />` in Svelte (to set `editable`, `hideable`,
  `elide`, …); declared columns map to model columns by `name`, else by position.
- **Everything else is the table's** (see [table](../reference/widgets/table.md)): the frozen
  header, column resizing and the column menu, `header-state`, keyed selection, the keyboard,
  type-ahead, editing and a11y. `scroll-to-row` takes a shown position.
- **Changing the model:** `with_table_model::<Nets, _>(table, |model, view| ...)` gives you the
  model and its view. Change the data, bump `version`, and the table redraws.
- **Reading:** `scene.read(|s| s.table_model::<Nets, _>(table, |model, view| ...))`.
  `view.row(pos)` is the model row shown at a position, `view.pos_of(key)` the reverse, and
  `view.keys(model)` the shown keys in order.
- `clear_table_model(table)` goes back to the JS window.

## Sorting and filtering in Rust

- **Sorting** follows the table's `sort-column` and `sort-order`. A header click on a `sortable` table
  sets them (and fires `sort-changed`, which you no longer need to handle). The table sorts the model's
  rows in Rust, stably, so ties keep model order. A `Column` with `.sortable(false)` ignores it.
- **Filtering** is set on the view:

  ```rust
  m.with_table_model::<Nets, _>(table, |_, view| view.set_filter(Some(RowFilter::text("gnd"))));
  // or any predicate:
  view.set_filter(Some(RowFilter::custom(|m, row| m.cell(row, 1).display() == "power")));
  ```

  `RowFilter::text` matches case-insensitively in every model column; use
  `RowFilter::Text { needle, columns }` to limit it to some columns. A row that is filtered out keeps
  its selection (by key) and shows it again when the filter is cleared.

## Tree rows

Return rows in pre-order and give each a depth:

```rust
fn depth(&self, row: usize) -> u32 { self.items[row].depth }   // 0 = top level
```

A row's children are the rows after it that are deeper, up to the next row at its own depth or less.
The table then:

- indents the first shown column by 16 points per level and draws a chevron (`::part(expander)`,
  `:checked` while expanded) on rows that have children;
- expands or collapses a row on a click on its chevron, or with Right/Left on the first column. Left
  on a collapsed row moves to its parent. Each toggle fires `row-toggled(id, expanded)`;
- sorts siblings under their parent, and while a filter is on, shows the matches with their
  ancestors expanded;
- reports each row's `level` and `expanded` to AccessKit.

Rows start collapsed. Use `RowView::new().expanded_by_default(true)` to start them expanded,
`view.set_expanded(key, on)` and `view.set_all_expanded(on)` from Rust, and `view.is_expanded(key)` to
read the state.

## Edits

The table's editing (F2, typing, double-click; see [table](../reference/widgets/table.md)) works on
model rows too. After JS's `commit` listener accepts an edit (or when there is none), the table calls
`RowModel::commit(row, col, text)`. Store the value and bump `version`, or return `Err(reason)`: the
cell then shows the reason, like a JS rejection. The default `commit` accepts the edit and stores
nothing.

## Realised rows: `RowSource` on a `virtual-list`

When every row is a small widget tree rather than text cells, use a
`virtual-list` with a `RowSource` (`scene/virt.rs`). `build` makes a row's nodes once, and `bind`
writes a row's values. Rows are recycled by key, so a fling builds no rows. The property grid in
`tests/vlist.rs` (`Params`) is the pattern. Change the model through
`with_row_source::<T, _>(list, ..)` and bump its `version`; every realised row is then bound again
before the next layout.

## What it costs (SV47, T2 benches)

`tests/ls2.rs` (`bench_*`) measures, at 20k and 100k rows:
- for `virtual-list`: bind per row and a page rebind (every realised row bound again);
- for `table`: a page jump (only new rows), a fling (60 frames of 600 points), sorting all rows by
  name, and a text filter.

CI's `egui-benches` job (linux-gtk.yml) runs them in release and writes the numbers to the job
summary. The asserted bounds are generous: they gate gross regressions, not the numbers.
The latest numbers are in `plans/milestones/LS2-rust-row-models.md`.
