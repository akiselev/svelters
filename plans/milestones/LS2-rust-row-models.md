# LS2: Rust row models for `table` and tree rows (SV41, SV44, SV47, SV52)

Lane `ci/b8-tables` (cloud batch 8, for cadatomic), from `master` c3034451 (b6efd3cf + the batch-8 briefs).
Brief: `plans/cloud/batch8/LANE-tables.md`. Sources: TRACKS.md LISTS (LS2), `LS1-SG5-virtual-lists.md`,
`OW5b-table.md`, `WG5a-list-table-interaction.md`. LS3 (row records, templates) stays out.

Status: **done on the lane, waiting for CI** (sha and run ids below).

## Scope
1. SV41: an engine-neutral Rust row-model trait, read by the egui `table` with no JS call per page; the JS
   window (`range` → `set-rows`) stays as the second source.
2. SV44/SV52: typed columns, sort and filter on the Rust side, tree rows with expand/collapse by key; key
   selection (SV46) across sort and filter.
3. SV47: T2 benches (bind per row, page rebind, fling; 20k and 100k rows; `virtual-list` and `table`), release
   in CI, generous bounds, numbers reported.
4. Docs: a `docs/egui` row-model page, the table reference's stale `egui_table` front matter, a nets-list example.

## Decisions (lane, within the agreed design)
- **The trait lives in `svelters-host`** (`svelters_host::rows`), as CADATOMIC.md §2.7 SV41 says: no egui types,
  so GTK's `GListModel` and Qt's `QAbstractItemModel` can adapt it later. The sort, filter and tree logic
  (`RowView`) is there too, so every backend gets the same order.
- **`RowModel` is separate from LS1's `RowSource`.** `RowSource` builds and binds realised node rows for
  `virtual-list` (widgets per row); a table paints cells and needs typed cells per column, not nodes. Both stay;
  the docs page says which to use.
- **Trees are pre-order rows with a depth** (`RowModel::depth`), not a parent/children API: a flat `Vec` with
  depths is how cadatomic's nets/components/hierarchy data already iterates, and it needs no storage per
  row in the view beyond the subtree ends (`Vec<u32>`, trees only).
- **Sort follows the table's own `sort-column`/`sort-order`** (FM1's QW4 props; header clicks set them), mapped
  to the model column by the column's `name` (= `Column::key`), else index. No new prop.
- **The filter is Rust API only** (`RowView::set_filter`): `RowFilter::text` or a predicate. No JS-facing
  `filter` prop yet (cadatomic's filter box is a Svelte field whose value can reach Rust through its own
  store); add one if asked.
- **Model columns become Rust-owned `table-column` children** when the table has none (adopted under a
  runtime-owned table the way list rows are), so the header, resizing, the column menu and `header-state`
  work unchanged. Declared Svelte columns take precedence.
- **Tree expansion: chevron in the first shown column**, 16 pt per level, `::part(expander)` (`:checked`
  expanded). Right/Left on the first column expand/collapse (Left on a collapsed child goes to the parent),
  as Qt's QTreeView and VS Code's tree. New signal `row-toggled(id, expanded)` (appended to the table's
  signals; spec, typings and the reference page regenerated).
- **Edits** reach the model through `RowModel::commit(row, col, text) -> Result<(), String>` after JS's
  `commit` listener (if any) accepts; an `Err` shows on the cell like a JS rejection.
- **Benches** are tests (`tests/ls2.rs`, `bench_*`) with debug and release bounds, run in release by a new
  `egui-benches` job in `linux-gtk.yml` that writes the numbers to the job summary. "Bind per row" is the page
  frame minus the still frame, divided by the rows (the frame includes egui and kittest's AccessKit tree).

## What landed (public API)

`svelters-host` (`crates/svelters-host/src/rows.rs`, new `pub mod rows`):
- `trait RowModel: Any` — `len`, `is_empty`, `key`, `columns`, `cell`, `sort_key`, `depth`, `version`,
  `label`, `anchor`, `find_anchor`, `commit` (all but `len`/`key`/`cell` defaulted).
- `enum Cell<'a>` (`#[non_exhaustive]`: `Empty`, `Text(Cow)`, `Int`, `Float`, `Bool`) with `text`, `display`,
  `into_text`, `sort_cmp`; `fn natural_cmp`.
- `struct Column` (`#[non_exhaustive]`: `key`, `header`, `width`, `sortable`, `kind`) with `new`, `width`,
  `sortable`, `kind`; `enum CellKind` (`#[non_exhaustive]`: `Text`, `Number`, `Bool`).
- `enum SortOrder`; `enum RowFilter` (`#[non_exhaustive]`: `Text { needle, columns }`, `Custom(RowPredicate)`)
  with `text`, `custom`; `type RowPredicate`.
- `struct RowView` — `new`, `expanded_by_default`, `sync`, `invalidate`, `rebuilds`, `len`, `is_empty`, `row`,
  `pos_of`, `keys`, `sort`, `set_sort`, `filter`, `set_filter`, `is_expanded`, `set_expanded`,
  `set_all_expanded`, `is_tree`, `tree_row`, `parent_pos`; `struct TreeRow { depth, has_children, expanded }`.
- `struct VecModel` (a ready model: `new`, `push`, pub `columns`/`rows`/`version`).

`svelters-egui`:
- `SceneMut::set_table_model(table, model, view)`, `SceneMut::with_table_model::<T, R>(table, |model, view|)`,
  `SceneMut::clear_table_model(table)`, `SceneRef::table_model::<T, R>(table, |model, view|)`
  (`views/table.rs`).
- `TableState::model: ModelSlot`, `TableState::tree_row(pos)`; `TableState::cell` now returns
  `Option<Cow<str>>` (was `Option<&str>`).
- `table`: signal `row-toggled(id, expanded)`, part `expander` (`EXPANDER_UA`, `INDENT`); with a model no
  `range` fires.

## Tests

Run locally (debug), all pass:
- `svelters-host` `rows::tests` (4): natural order, sort/filter keep keys, tree expand/sort/filter, commit.
- `svelters-egui` `tests/ls2.rs` (8): `a_rust_model_feeds_the_table_with_columns_from_the_model` (20k rows,
  model columns, ≤ visible cells read per frame, `scroll-to-row`, a version bump redraws);
  `sorting_and_filtering_run_in_rust_and_keep_the_selection_by_key`;
  `tree_rows_expand_by_their_expander_and_the_keyboard` (a11y level/expanded, chevron click, Right/Left,
  sorted siblings); `edits_commit_into_the_rust_model_which_may_reject_them`; the four `bench_*`.
- `svelters-egui` `tests/spec.rs` (regenerated `egui.json` and `egui.d.ts` with `SVELTERS_UPDATE=1`);
  `tools/docs/egui/gen_reference.py` rerun (table.md events).
- `cargo clippy -p svelters-host -p svelters-egui --all-targets`: no errors (advisory warnings only).

- The existing table suites, unchanged and passing: `ow5b.rs` (3), `wg5a.rs` (5), `views.rs` table tests (2),
  `views::table` units (4).

Not run locally: the full workspace suite and the release benches: CI runs them.

### T2 bench numbers (SV47)

Local debug build, cloud container (4 cores), kittest harness (each frame includes egui and the AccessKit
tree). CI's release numbers: see the `egui-benches` job summary of the run below.

| | rows | still frame | page rebind / jump | bind per row (over still) | fling frame median / max | sort / filter |
|---|---|---|---|---|---|---|
| virtual-list | 20k | 4.16 ms | 14.22 ms (30 rows) | 0.335 ms | 27.7 / 45.0 ms | — |
| virtual-list | 100k | 4.22 ms | 15.40 ms (30 rows) | 0.373 ms | 27.8 / 45.9 ms | — |
| table | 20k | 4.48 ms | 14.07 ms (28 rows) | 0.343 ms | 20.1 / 33.2 ms | 24.1 / 27.7 ms |
| table | 100k | 6.87 ms | 15.31 ms (28 rows) | 0.301 ms | 19.8 / 27.7 ms | 90.7 / 113.2 ms |

Frame cost does not grow with the row count (virtual on both axes); only sort and filter are O(n) (O(n log n)).
Bounds asserted: frame ≤ 400 ms debug / 40 ms release; bind per row ≤ 20 ms / 1 ms; sort, filter ≤ 20 s / 1 s.

## CI

- sha: (filled in at the end)

## Handoff and open items
- **cadatomic request text not read:** `add_repo akiselev/cadatomic` was refused in this cloud session, so the
  SV41–SV53 wording comes from `CADATOMIC.md` §2.7, `HANDOFF.md` and the lane brief. Recommendation: the
  coordinator checks `eu-content`'s needs against the API list above at the merge.
- **A JS-facing `filter` prop** (text) on `table` for a Rust model: not built (decision above). Recommendation:
  add it if cadatomic's eu-content asks; it is a prop plus `RowFilter::text` in `sync_model`.
- **Per-column sort disable shows in the header:** a `.sortable(false)` model column ignores the sort but the
  header still shows the arrow after a click. Small follow-up if it matters.
- **Type-ahead with a model** copies all shown keys per key press (as the JS path does with `set-ids`); fine at
  100k, revisit at 1M.
- **GTK/Qt adapters** of `RowModel` (N3's `GListModel`/`QAbstractItemModel`): not in scope (egui first).
- `docs/egui/rust/row-models.md` is `status: pending`; flip to `available` with `verified_at` at the merge.
