# Lane tables: LS2 core, Rust row models for tables and trees

Branch `ci/b8-tables`; record `plans/milestones/LS2-rust-row-models.md`. Sources: TRACKS.md's
LISTS track (LS2: "SV41: a Rust row-model trait in `svelters-host`, engine-neutral … SV46 …
SV44: columns and tree rows … T2 benches"), `LS1-SG5-virtual-lists.md`, `OW5b-table.md` (its
OW5b2/OW5a2 handoff), `WG5a-list-table-interaction.md`.

Today `RowSource` (`crates/svelters-egui/src/scene/virt.rs:456`) serves only `virtual-list`; the own
`table` is a JS data window (`views/table.rs`: `range` → `set-rows`), sorting and filtering run in
JS, and nothing measures a table at scale. cadatomic needs nets, components and property grids at
20k+ rows.

1. **Row models in Rust (SV41):** the `RowSource` contract extended to `table` (row count, cell
   values per column, stable row keys), so a Rust model feeds the table with zero JS calls per
   page. Keep the JS window as a second source.
2. **Columns and tree rows (SV44/SV52):** typed columns (key, header, width, sort key, cell kind),
   sort and filter evaluated on the Rust side for a Rust model, tree rows (depth, expanded, has
   children) with expand/collapse by key; key-based selection (SV46 already delivered) works
   across sort and filter.
3. **Benches (SV47):** bind cost per row, a page rebind, and a fling, at 20k and 100k rows, for
   `virtual-list` and `table`, release, asserted in CI against generous bounds and reported as
   numbers (cloud, not a gate for the numbers themselves).
4. **Docs:** `docs/egui` pages for the row model and the table's Rust source (replace `table.md`'s
   stale `egui_table` front matter), with a cadatomic-shaped example (a nets list: name, class,
   pad count, sortable, 20k rows).
5. LS3 (row records, templates) stays out.
